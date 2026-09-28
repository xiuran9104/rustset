-- Asset/policy CSV and all dependent reads use the authenticated tenant.
-- Historical rows remain quarantined until ownership is explicitly verified.
DO $$ DECLARE tbl text; BEGIN
    FOREACH tbl IN ARRAY ARRAY['infra_asset','infra_network_policy','infra_task','infra_risk','infra_inspection_result','infra_inspection_baseline'] LOOP
        EXECUTE format('ALTER TABLE public.%I ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id)', tbl);
        EXECUTE format('CREATE INDEX IF NOT EXISTS %I ON public.%I(tenant_id)', 'idx_' || tbl || '_tenant', tbl);
        EXECUTE format('DROP TRIGGER IF EXISTS require_tenant ON public.%I', tbl);
        EXECUTE format('CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.%I FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant()', tbl);
    END LOOP;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS cmdb_net_zone_tenant_id_key ON public.cmdb_net_zone(tenant_id,id);
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_asset'::regclass AND conname='infra_asset_net_zone_tenant_fk') THEN
        ALTER TABLE public.infra_asset ADD CONSTRAINT infra_asset_net_zone_tenant_fk
            FOREIGN KEY(tenant_id,net_zone_id) REFERENCES public.cmdb_net_zone(tenant_id,id);
    END IF;
END $$;
-- IP addresses and risk keys may repeat between isolated networks.
ALTER TABLE public.infra_inspection_baseline DROP CONSTRAINT IF EXISTS infra_inspection_baseline_pkey;
CREATE UNIQUE INDEX IF NOT EXISTS idx_inspection_baseline_tenant_ip ON public.infra_inspection_baseline(tenant_id,ip) NULLS NOT DISTINCT;
DROP INDEX IF EXISTS public.idx_risk_inspection_key;
CREATE UNIQUE INDEX idx_risk_inspection_key ON public.infra_risk(tenant_id,inspection_key)
    WHERE deleted=0 AND inspection_key IS NOT NULL;
