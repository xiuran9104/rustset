-- NULL is quarantine, not a default tenant. Historical ownership is assigned
-- only after an operator has explicitly verified it; no automatic backfill.
LOCK TABLE public.cmdb_model, public.cmdb_instance, public.cmdb_relation,
    public.infra_resource_ticket IN SHARE ROW EXCLUSIVE MODE;
ALTER TABLE public.cmdb_instance ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
ALTER TABLE public.cmdb_relation ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
-- Persist the origin tenant before asynchronous/automatic CMDB provisioning.
ALTER TABLE public.infra_resource_ticket ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
ALTER TABLE public.cmdb_instance_unique_value ADD COLUMN IF NOT EXISTS tenant_id bigint;

CREATE UNIQUE INDEX IF NOT EXISTS cmdb_instance_tenant_id_key ON public.cmdb_instance(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_cmdb_instance_tenant_model ON public.cmdb_instance(tenant_id, model_id, id) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_cmdb_relation_tenant ON public.cmdb_relation(tenant_id) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_infra_resource_ticket_tenant ON public.infra_resource_ticket(tenant_id, id) WHERE deleted = 0;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'public.cmdb_relation'::regclass AND conname = 'cmdb_relation_source_tenant_fk') THEN
        ALTER TABLE public.cmdb_relation ADD CONSTRAINT cmdb_relation_source_tenant_fk
            FOREIGN KEY (tenant_id, source_id) REFERENCES public.cmdb_instance(tenant_id, id);
        ALTER TABLE public.cmdb_relation ADD CONSTRAINT cmdb_relation_target_tenant_fk
            FOREIGN KEY (tenant_id, target_id) REFERENCES public.cmdb_instance(tenant_id, id);
    END IF;
END $$;

CREATE OR REPLACE FUNCTION public.require_resource_tenant()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' AND (NEW.tenant_id IS NULL OR NEW.tenant_id <= 0) THEN
        RAISE EXCEPTION 'tenant_id is required for new resources' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.tenant_id IS NOT NULL AND NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
        RAISE EXCEPTION 'resource tenant is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.tenant_id IS NOT NULL AND NEW.tenant_id <= 0 THEN
        RAISE EXCEPTION 'tenant_id must be positive' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS cmdb_instance_require_tenant ON public.cmdb_instance;
CREATE TRIGGER cmdb_instance_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.cmdb_instance
FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();
DROP TRIGGER IF EXISTS cmdb_relation_require_tenant ON public.cmdb_relation;
CREATE TRIGGER cmdb_relation_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.cmdb_relation
FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();
DROP TRIGGER IF EXISTS infra_ticket_require_tenant ON public.infra_resource_ticket;
CREATE TRIGGER infra_ticket_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_resource_ticket
FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();

ALTER TABLE public.cmdb_instance_unique_value DROP CONSTRAINT IF EXISTS cmdb_instance_unique_value_key;
UPDATE public.cmdb_instance_unique_value v SET tenant_id = i.tenant_id FROM public.cmdb_instance i WHERE i.id = v.instance_id;
ALTER TABLE public.cmdb_instance_unique_value ADD CONSTRAINT cmdb_instance_unique_value_key UNIQUE NULLS NOT DISTINCT (tenant_id, model_id, value);

CREATE OR REPLACE FUNCTION public.cmdb_sync_instance_unique_value()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE model_key text; key_value jsonb;
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM id FROM public.cmdb_model WHERE id = OLD.model_id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        PERFORM id FROM public.cmdb_model WHERE id IN (OLD.model_id, NEW.model_id) ORDER BY id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
    END IF;
    SELECT unique_key INTO model_key FROM public.cmdb_model WHERE id = NEW.model_id FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'instance model does not exist' USING ERRCODE = '23503';
    END IF;
    key_value := public.cmdb_unique_value(NEW.attributes, model_key);
    IF NEW.deleted = 0 AND key_value IS NOT NULL THEN
        INSERT INTO public.cmdb_instance_unique_value(instance_id, tenant_id, model_id, value)
            VALUES (NEW.id, NEW.tenant_id, NEW.model_id, key_value);
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION public.cmdb_rebuild_model_unique_values()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.unique_key IS DISTINCT FROM NEW.unique_key THEN
        DELETE FROM public.cmdb_instance_unique_value WHERE model_id = NEW.id;
        INSERT INTO public.cmdb_instance_unique_value(instance_id, tenant_id, model_id, value)
            SELECT id, tenant_id, model_id, public.cmdb_unique_value(attributes, NEW.unique_key)
            FROM public.cmdb_instance WHERE model_id = NEW.id AND deleted = 0
              AND public.cmdb_unique_value(attributes, NEW.unique_key) IS NOT NULL;
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS cmdb_instance_unique_value_sync ON public.cmdb_instance;
CREATE TRIGGER cmdb_instance_unique_value_sync
AFTER INSERT OR UPDATE OF id, tenant_id, model_id, attributes, deleted OR DELETE ON public.cmdb_instance
FOR EACH ROW EXECUTE FUNCTION public.cmdb_sync_instance_unique_value();
