-- Database backstop for every instance writer, including imports and provisioning.
-- Fail on existing duplicates; never silently discard business data.
LOCK TABLE public.cmdb_model, public.cmdb_instance IN SHARE ROW EXCLUSIVE MODE;

CREATE TABLE IF NOT EXISTS public.cmdb_instance_unique_value (
    instance_id bigint PRIMARY KEY REFERENCES public.cmdb_instance(id) ON DELETE CASCADE,
    model_id bigint NOT NULL REFERENCES public.cmdb_model(id),
    value jsonb NOT NULL,
    CONSTRAINT cmdb_instance_unique_value_key UNIQUE (model_id, value)
);

-- Match the application's empty-value semantics (missing/null/blank/empty array).
CREATE OR REPLACE FUNCTION public.cmdb_unique_value(attributes jsonb, key text)
RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE
        WHEN key IS NULL OR key = '' OR attributes->key IS NULL
          OR attributes->key IN ('null'::jsonb, '[]'::jsonb)
          OR (jsonb_typeof(attributes->key) = 'string'
              AND btrim(attributes->>key, U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000') = '')
        THEN NULL ELSE attributes->key END
$$;

CREATE OR REPLACE FUNCTION public.cmdb_sync_instance_unique_value()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    model_key text;
    key_value jsonb;
BEGIN
    -- Serialize key-definition changes and all writers, also outside the API.
    IF TG_OP = 'DELETE' THEN
        PERFORM id FROM public.cmdb_model WHERE id = OLD.model_id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        PERFORM id FROM public.cmdb_model WHERE id IN (OLD.model_id, NEW.model_id)
            ORDER BY id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
    END IF;
    SELECT unique_key INTO model_key FROM public.cmdb_model
        WHERE id = NEW.model_id FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'instance model does not exist' USING ERRCODE = '23503';
    END IF;
    key_value := public.cmdb_unique_value(NEW.attributes, model_key);
    IF NEW.deleted = 0 AND key_value IS NOT NULL THEN
        INSERT INTO public.cmdb_instance_unique_value(instance_id, model_id, value)
            VALUES (NEW.id, NEW.model_id, key_value);
    END IF;
    RETURN NEW;
END
$$;

CREATE OR REPLACE FUNCTION public.cmdb_rebuild_model_unique_values()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.unique_key IS DISTINCT FROM NEW.unique_key THEN
        DELETE FROM public.cmdb_instance_unique_value WHERE model_id = NEW.id;
        INSERT INTO public.cmdb_instance_unique_value(instance_id, model_id, value)
            SELECT id, model_id, public.cmdb_unique_value(attributes, NEW.unique_key)
            FROM public.cmdb_instance
            WHERE model_id = NEW.id AND deleted = 0
              AND public.cmdb_unique_value(attributes, NEW.unique_key) IS NOT NULL;
    END IF;
    RETURN NEW;
END
$$;

DROP TRIGGER IF EXISTS cmdb_instance_unique_value_sync ON public.cmdb_instance;
CREATE TRIGGER cmdb_instance_unique_value_sync
AFTER INSERT OR UPDATE OF id, model_id, attributes, deleted OR DELETE ON public.cmdb_instance
FOR EACH ROW EXECUTE FUNCTION public.cmdb_sync_instance_unique_value();

DROP TRIGGER IF EXISTS cmdb_model_unique_value_sync ON public.cmdb_model;
CREATE TRIGGER cmdb_model_unique_value_sync
AFTER UPDATE OF unique_key ON public.cmdb_model
FOR EACH ROW EXECUTE FUNCTION public.cmdb_rebuild_model_unique_values();

DELETE FROM public.cmdb_instance_unique_value;
INSERT INTO public.cmdb_instance_unique_value(instance_id, model_id, value)
    SELECT i.id, i.model_id, public.cmdb_unique_value(i.attributes, m.unique_key)
    FROM public.cmdb_instance i JOIN public.cmdb_model m ON m.id = i.model_id
    WHERE i.deleted = 0 AND public.cmdb_unique_value(i.attributes, m.unique_key) IS NOT NULL;
