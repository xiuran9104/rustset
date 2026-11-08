-- OpenTofu adaptation layer (docs/cmdb-roadmap.md): track real provision
-- runs on resource tickets and store auto-approval rules.

ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS apply_status character varying(32) DEFAULT 'none',
    ADD COLUMN IF NOT EXISTS apply_log text,
    ADD COLUMN IF NOT EXISTS tf_outputs jsonb,
    ADD COLUMN IF NOT EXISTS tofu_workspace character varying(128);

COMMENT ON COLUMN public.infra_resource_ticket.apply_status IS
    'none | planned | applied | failed — OpenTofu run tracking';

CREATE SEQUENCE IF NOT EXISTS public.infra_approval_rule_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.infra_approval_rule (
    id bigint DEFAULT nextval('public.infra_approval_rule_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    resource_type character varying(64) DEFAULT ''::character varying NOT NULL,
    max_cpu_cores integer,
    max_memory_gb integer,
    max_resource_count integer,
    auto_provision boolean DEFAULT false NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    remarks text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT infra_approval_rule_pkey PRIMARY KEY (id)
);
