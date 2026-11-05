-- Preserve the Kairos ticket workflow fields used by the migrated Vben page.
-- The generic infra table adapter automatically accepts these columns once
-- present. ADD COLUMN IF NOT EXISTS keeps fresh and upgraded databases aligned.

DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN ticket_type varchar(32) NOT NULL DEFAULT 'create'; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN risk_level varchar(32) NOT NULL DEFAULT 'normal'; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN target_resource_id bigint; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN target_config text; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN maintenance_window varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN allow_interruption boolean NOT NULL DEFAULT false; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN backup_confirmed boolean NOT NULL DEFAULT false; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN rollback_plan text; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN retention_until varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN approval_stage integer NOT NULL DEFAULT 1; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN approval_total integer NOT NULL DEFAULT 1; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_resource_ticket ADD COLUMN current_approval_role varchar(64) DEFAULT '资源管理员'; EXCEPTION WHEN duplicate_column THEN NULL; END $$;

CREATE INDEX IF NOT EXISTS idx_resource_ticket_workflow
    ON infra_resource_ticket (ticket_type, ticket_status)
    WHERE deleted = 0;

-- Physical-resource placement fields retained by the selected Kairos page.
-- Cabinet inventory itself remains out of scope, so no parallel cabinet
-- tables or routes are introduced here.
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN machine_room_id bigint; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN deployment_type varchar(32); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN management_ip varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN business_ip varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN network_cidr varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN gateway varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN vlan_id varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN dns_servers varchar(256); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN mac_address varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN switch_name varchar(128); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE infra_business_resource ADD COLUMN switch_port varchar(64); EXCEPTION WHEN duplicate_column THEN NULL; END $$;

CREATE INDEX IF NOT EXISTS idx_business_resource_machine_room
    ON infra_business_resource (machine_room_id)
    WHERE deleted = 0;
