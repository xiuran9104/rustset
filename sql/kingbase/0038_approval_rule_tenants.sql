-- Approval rules govern tenant-owned tickets and therefore share their owner.
-- Existing rules remain inactive for tenant matching until ownership is set.
LOCK TABLE public.infra_approval_rule IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_approval_rule
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
CREATE INDEX IF NOT EXISTS idx_infra_approval_rule_tenant
    ON public.infra_approval_rule(tenant_id, id) WHERE deleted=0;

DROP TRIGGER IF EXISTS require_tenant ON public.infra_approval_rule;
CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id
    ON public.infra_approval_rule FOR EACH ROW
    EXECUTE FUNCTION public.require_resource_tenant();
