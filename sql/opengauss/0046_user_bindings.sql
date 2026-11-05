-- Under the single-operator model customers and ticket applicants are
-- system users. Resources and tickets bind the user id; display names stay
-- denormalized in customer_name / applicant_name for list continuity.
DO $$ BEGIN ALTER TABLE public.infra_cloud_resource ADD COLUMN customer_user_id bigint REFERENCES public.system_users(id); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_physical_resource ADD COLUMN customer_user_id bigint REFERENCES public.system_users(id); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_resource_ticket ADD COLUMN applicant_user_id bigint REFERENCES public.system_users(id); EXCEPTION WHEN duplicate_column THEN NULL; END $$;

CREATE INDEX IF NOT EXISTS idx_infra_cloud_resource_customer
    ON public.infra_cloud_resource(tenant_id, customer_user_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_physical_resource_customer
    ON public.infra_physical_resource(tenant_id, customer_user_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_resource_ticket_applicant
    ON public.infra_resource_ticket(tenant_id, applicant_user_id, id) WHERE deleted=0;
