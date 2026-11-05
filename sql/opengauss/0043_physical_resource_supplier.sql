-- Physical resources record their procurement source. Project and contract
-- columns already exist; the hardware supplier is free text because
-- procurement vendors are distinct from the service-provider catalog that
-- owns cloud zones and machine rooms.
DO $$ BEGIN ALTER TABLE public.infra_physical_resource ADD COLUMN supplier_name character varying(128); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
