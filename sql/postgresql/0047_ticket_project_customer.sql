-- Tickets carry the same structured fields as the resource ledgers so the
-- delivery writeback can populate them: the business project and the
-- customer (bound to a system user under the single-operator model).
ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS project_name character varying(128);

ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS customer_user_id bigint REFERENCES public.system_users(id);
