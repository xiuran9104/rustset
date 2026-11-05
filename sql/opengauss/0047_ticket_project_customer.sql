-- Tickets carry the same structured fields as the resource ledgers so the
-- delivery writeback can populate them: the business project and the
-- customer (bound to a system user under the single-operator model).
DO $$ BEGIN ALTER TABLE public.infra_resource_ticket ADD COLUMN project_name character varying(128); EXCEPTION WHEN duplicate_column THEN NULL; END $$;

DO $$ BEGIN ALTER TABLE public.infra_resource_ticket ADD COLUMN customer_user_id bigint REFERENCES public.system_users(id); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
