-- Client retries for ticket creation resolve to the original tenant-owned row.
ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS idempotency_key character varying(128);

CREATE UNIQUE INDEX IF NOT EXISTS idx_resource_ticket_tenant_idempotency
    ON public.infra_resource_ticket(tenant_id, idempotency_key)
    WHERE deleted=0 AND idempotency_key IS NOT NULL;
