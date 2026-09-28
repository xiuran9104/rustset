-- Private address space commonly overlaps between tenants. Keep uniqueness
-- inside the authenticated tenant and leave unassigned historical rows
-- quarantined until an operator confirms ownership.
DROP INDEX IF EXISTS public.idx_asset_ip_unique;
CREATE UNIQUE INDEX IF NOT EXISTS idx_asset_tenant_ip_unique
    ON public.infra_asset(tenant_id, ip)
    WHERE deleted = 0 AND tenant_id IS NOT NULL;
