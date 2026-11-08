-- Preserve the Kairos ticket workflow fields used by the migrated Vben page.
-- The generic infra table adapter automatically accepts these columns once
-- present. ADD COLUMN IF NOT EXISTS keeps fresh and upgraded databases aligned.

ALTER TABLE infra_resource_ticket
    ADD COLUMN IF NOT EXISTS ticket_type varchar(32) NOT NULL DEFAULT 'create',
    ADD COLUMN IF NOT EXISTS risk_level varchar(32) NOT NULL DEFAULT 'normal',
    ADD COLUMN IF NOT EXISTS target_resource_id bigint,
    ADD COLUMN IF NOT EXISTS target_config text,
    ADD COLUMN IF NOT EXISTS maintenance_window varchar(64),
    ADD COLUMN IF NOT EXISTS allow_interruption boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS backup_confirmed boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS rollback_plan text,
    ADD COLUMN IF NOT EXISTS retention_until varchar(64),
    ADD COLUMN IF NOT EXISTS approval_stage integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS approval_total integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS current_approval_role varchar(64) DEFAULT '资源管理员';

CREATE INDEX IF NOT EXISTS idx_resource_ticket_workflow
    ON infra_resource_ticket (ticket_type, ticket_status)
    WHERE deleted = 0;

-- Physical-resource placement fields retained by the selected Kairos page.
-- Cabinet inventory itself remains out of scope, so no parallel cabinet
-- tables or routes are introduced here.
ALTER TABLE infra_business_resource
    ADD COLUMN IF NOT EXISTS machine_room_id bigint,
    ADD COLUMN IF NOT EXISTS deployment_type varchar(32),
    ADD COLUMN IF NOT EXISTS management_ip varchar(64),
    ADD COLUMN IF NOT EXISTS business_ip varchar(64),
    ADD COLUMN IF NOT EXISTS network_cidr varchar(64),
    ADD COLUMN IF NOT EXISTS gateway varchar(64),
    ADD COLUMN IF NOT EXISTS vlan_id varchar(64),
    ADD COLUMN IF NOT EXISTS dns_servers varchar(256),
    ADD COLUMN IF NOT EXISTS mac_address varchar(64),
    ADD COLUMN IF NOT EXISTS switch_name varchar(128),
    ADD COLUMN IF NOT EXISTS switch_port varchar(64);

CREATE INDEX IF NOT EXISTS idx_business_resource_machine_room
    ON infra_business_resource (machine_room_id)
    WHERE deleted = 0;
