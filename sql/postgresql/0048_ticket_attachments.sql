-- Tickets carry contract attachments at creation and delivery documents at
-- delivery. Files reuse the existing infra_file / RustFS pipeline; this table
-- binds them to a ticket with a kind discriminator.
CREATE SEQUENCE IF NOT EXISTS public.infra_resource_ticket_attachment_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.infra_resource_ticket_attachment (
    id bigint DEFAULT nextval('public.infra_resource_ticket_attachment_seq'::regclass) NOT NULL,
    ticket_id bigint NOT NULL,
    file_id bigint NOT NULL REFERENCES public.infra_file(id),
    kind character varying(32) NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint
);

CREATE UNIQUE INDEX IF NOT EXISTS infra_resource_ticket_attachment_tenant_id_key
    ON public.infra_resource_ticket_attachment(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_ticket_attachment_ticket
    ON public.infra_resource_ticket_attachment(tenant_id, ticket_id, kind, id) WHERE deleted=0;
-- The composite attachment FK needs a unique (tenant_id, id) key on tickets,
-- which earlier tenant migrations never created for this table.
CREATE UNIQUE INDEX IF NOT EXISTS infra_resource_ticket_tenant_id_key
    ON public.infra_resource_ticket(tenant_id, id);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_resource_ticket_attachment'::regclass
          AND conname='infra_ticket_attachment_ticket_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_resource_ticket_attachment
            ADD CONSTRAINT infra_ticket_attachment_ticket_tenant_fk
            FOREIGN KEY (tenant_id, ticket_id)
            REFERENCES public.infra_resource_ticket(tenant_id, id);
    END IF;
END $$;

DROP TRIGGER IF EXISTS require_tenant ON public.infra_resource_ticket_attachment;
CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id
    ON public.infra_resource_ticket_attachment FOR EACH ROW
    EXECUTE FUNCTION public.require_resource_tenant();
