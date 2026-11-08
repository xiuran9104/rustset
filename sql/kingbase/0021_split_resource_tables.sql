-- 0021: split the mixed 业务资源 ledger into two typed tables.
-- infra_cloud_resource and infra_physical_resource each carry their own
-- columns and constraints; the generic infra_business_resource table is
-- dropped after its rows (if any) are migrated. The ticket reference gains
-- a discriminator column target_resource_type ('cloud' | 'physical').

CREATE SEQUENCE IF NOT EXISTS infra_cloud_resource_seq;
CREATE SEQUENCE IF NOT EXISTS infra_physical_resource_seq;

CREATE TABLE IF NOT EXISTS infra_cloud_resource (
    id bigint DEFAULT nextval('public.infra_cloud_resource_seq'::regclass) NOT NULL,
    ecs_name character varying(256) NOT NULL,
    ecs_status character varying(64) NOT NULL,
    resource_id character varying(128) DEFAULT ''::character varying NOT NULL,
    cloud_region character varying(128) NOT NULL,
    cloud_category character varying(128) NOT NULL,
    cloud_provider_config_id bigint,
    zone_name character varying(128),
    platform_name character varying(128),
    county_city character varying(64),
    vdc_name character varying(128),
    customer_name character varying(128) NOT NULL,
    application_name character varying(128),
    contract_name character varying(128),
    instance_id character varying(128) DEFAULT ''::character varying NOT NULL,
    ecs_type character varying(128) DEFAULT ''::character varying NOT NULL,
    ecs_os character varying(128) DEFAULT ''::character varying NOT NULL,
    cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL,
    system_disk character varying(64) DEFAULT ''::character varying NOT NULL,
    system_disk_size_gb integer DEFAULT 0 NOT NULL,
    data_disk text,
    completion_time character varying(32),
    release_time character varying(32),
    has_security_product integer DEFAULT 0 NOT NULL,
    ip_address character varying(64) DEFAULT ''::character varying NOT NULL,
    ecs_login_method character varying(64),
    ecs_login_username character varying(128),
    ecs_initial_password character varying(256),
    bastion_address character varying(128),
    bastion_admin_account character varying(128),
    bastion_initial_password character varying(256),
    bandwidth_mbps integer,
    bandwidth_type character varying(32),
    public_ip_count integer,
    network_type character varying(32),
    billing_method character varying(32),
    purchase_duration integer,
    cost_center character varying(64),
    project_name character varying(128),
    project_code character varying(64),
    business_owner character varying(64),
    tech_owner character varying(64),
    contact_phone character varying(32),
    remarks text,
    application_status character varying(32),
    delivery_status character varying(32),
    delivery_confirmed_at character varying(32),
    delivery_confirmed_by character varying(64),
    applicant character varying(64),
    department character varying(128),
    approver character varying(64),
    approval_time character varying(32),
    approval_remarks text,
    rejection_reason text,
    security_level character varying(32),
    data_sensitivity character varying(32),
    purpose text,
    expected_delivery_time character varying(32),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE IF NOT EXISTS infra_physical_resource (
    id bigint DEFAULT nextval('public.infra_physical_resource_seq'::regclass) NOT NULL,
    ecs_name character varying(256) NOT NULL,
    ecs_status character varying(64) NOT NULL,
    cloud_region character varying(128) NOT NULL,
    cloud_category character varying(128) NOT NULL,
    customer_name character varying(128) NOT NULL,
    machine_room_id bigint,
    deployment_type character varying(32),
    management_ip character varying(64),
    business_ip character varying(64),
    network_cidr character varying(64),
    gateway character varying(64),
    vlan_id character varying(32),
    dns_servers character varying(256),
    mac_address character varying(128),
    serial_number character varying(128),
    hardware_model character varying(128),
    rack_location character varying(128),
    warranty_expiry character varying(32),
    agent_status character varying(32),
    ipmi_address character varying(64),
    cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL,
    has_security_product integer DEFAULT 0 NOT NULL,
    application_name character varying(128),
    contract_name character varying(128),
    completion_time character varying(32),
    release_time character varying(32),
    project_name character varying(128),
    project_code character varying(64),
    business_owner character varying(64),
    tech_owner character varying(64),
    contact_phone character varying(32),
    remarks text,
    application_status character varying(32),
    delivery_status character varying(32),
    delivery_confirmed_at character varying(32),
    delivery_confirmed_by character varying(64),
    applicant character varying(64),
    department character varying(128),
    approver character varying(64),
    approval_time character varying(32),
    approval_remarks text,
    rejection_reason text,
    security_level character varying(32),
    data_sensitivity character varying(32),
    purpose text,
    expected_delivery_time character varying(32),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_resource_customer
    ON infra_cloud_resource (customer_name) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_cloud_resource_region
    ON infra_cloud_resource (cloud_region) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_physical_resource_customer
    ON infra_physical_resource (customer_name) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_physical_resource_room
    ON infra_physical_resource (machine_room_id) WHERE deleted = 0;

-- Tickets first learn which ledger their target lives in (before the old
-- table goes away), then rows move into their typed table. The whole block is
-- guarded so re-runs after the drop stay idempotent.
ALTER TABLE infra_resource_ticket
    ADD COLUMN IF NOT EXISTS target_resource_type varchar(16);

DO $$
BEGIN
IF to_regclass('public.infra_business_resource') IS NOT NULL THEN

UPDATE infra_resource_ticket ticket
SET target_resource_type = buried.resource_type
FROM infra_business_resource buried
WHERE ticket.target_resource_id = buried.id
  AND ticket.target_resource_type IS NULL;

INSERT INTO infra_cloud_resource
    (id, ecs_name, ecs_status, resource_id, cloud_region, cloud_category,
     cloud_provider_config_id, zone_name, platform_name, county_city, vdc_name,
     customer_name, application_name, contract_name, instance_id, ecs_type, ecs_os,
     cpu_cores, memory_gb, system_disk, system_disk_size_gb, data_disk,
     completion_time, release_time, has_security_product, ip_address,
     ecs_login_method, ecs_login_username, ecs_initial_password,
     bastion_address, bastion_admin_account, bastion_initial_password,
     bandwidth_mbps, bandwidth_type, public_ip_count, network_type,
     billing_method, purchase_duration, cost_center,
     project_name, project_code, business_owner, tech_owner, contact_phone,
     remarks, application_status, delivery_status, delivery_confirmed_at,
     delivery_confirmed_by, applicant, department, approver, approval_time,
     approval_remarks, rejection_reason, security_level, data_sensitivity,
     purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted)
SELECT id, ecs_name, ecs_status, resource_id, cloud_region, cloud_category,
       cloud_provider_config_id, zone_name, platform_name, county_city, vdc_name,
       customer_name, application_name, contract_name, instance_id, ecs_type, ecs_os,
       cpu_cores, memory_gb, system_disk, system_disk_size_gb, data_disk,
       completion_time, release_time, has_security_product, ip_address,
       ecs_login_method, ecs_login_username, ecs_initial_password,
       bastion_address, bastion_admin_account, bastion_initial_password,
       bandwidth_mbps, bandwidth_type, public_ip_count, network_type,
       billing_method, purchase_duration, cost_center,
       project_name, project_code, business_owner, tech_owner, contact_phone,
       remarks, application_status, delivery_status, delivery_confirmed_at,
       delivery_confirmed_by, applicant, department, approver, approval_time,
       approval_remarks, rejection_reason, security_level, data_sensitivity,
       purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted
FROM infra_business_resource
WHERE resource_type = 'cloud'
  AND NOT EXISTS (SELECT 1 FROM infra_cloud_resource WHERE infra_cloud_resource.id = infra_business_resource.id);

INSERT INTO infra_physical_resource
    (id, ecs_name, ecs_status, cloud_region, cloud_category, customer_name,
     machine_room_id, deployment_type, management_ip, business_ip, network_cidr,
     gateway, vlan_id, dns_servers, mac_address, serial_number, hardware_model,
     rack_location, warranty_expiry, agent_status, ipmi_address,
     cpu_cores, memory_gb, has_security_product,
     application_name, contract_name, completion_time, release_time,
     project_name, project_code, business_owner, tech_owner, contact_phone,
     remarks, application_status, delivery_status, delivery_confirmed_at,
     delivery_confirmed_by, applicant, department, approver, approval_time,
     approval_remarks, rejection_reason, security_level, data_sensitivity,
     purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted)
SELECT id, ecs_name, ecs_status, cloud_region, cloud_category, customer_name,
       machine_room_id, deployment_type, management_ip, business_ip, network_cidr,
       gateway, vlan_id, dns_servers, mac_address, serial_number, hardware_model,
       rack_location, warranty_expiry, agent_status, ipmi_address,
       cpu_cores, memory_gb, has_security_product,
       application_name, contract_name, completion_time, release_time,
       project_name, project_code, business_owner, tech_owner, contact_phone,
       remarks, application_status, delivery_status, delivery_confirmed_at,
       delivery_confirmed_by, applicant, department, approver, approval_time,
       approval_remarks, rejection_reason, security_level, data_sensitivity,
       purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted
FROM infra_business_resource
WHERE resource_type = 'physical'
  AND NOT EXISTS (SELECT 1 FROM infra_physical_resource WHERE infra_physical_resource.id = infra_business_resource.id);

-- Retire the mixed table: set sequences past any migrated ids (PERFORM:
-- top-level SELECTs with result sets are rejected inside migrations).
PERFORM setval('infra_cloud_resource_seq',
    GREATEST((SELECT COALESCE(max(id), 0) + 1 FROM infra_cloud_resource),
             (SELECT last_value FROM infra_cloud_resource_seq)));
PERFORM setval('infra_physical_resource_seq',
    GREATEST((SELECT COALESCE(max(id), 0) + 1 FROM infra_physical_resource),
             (SELECT last_value FROM infra_physical_resource_seq)));

DROP TABLE IF EXISTS infra_business_resource;
DROP SEQUENCE IF EXISTS infra_business_resource_seq;

END IF;
END $$;

-- Permission codes follow the split: the cloud menu buttons become
-- infra:cloud-resource:*, and the physical menu gains its own button set.
UPDATE system_menu
SET name = REPLACE(name, '业务资源', '云资源'),
    permission = REPLACE(permission, 'infra:business-resource:', 'infra:cloud-resource:'),
    updater = 'migration', update_time = now()
WHERE parent_id = 30252 AND deleted = 0
  AND permission LIKE 'infra:business-resource:%';

INSERT INTO system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component,
     component_name, status, visible, keep_alive, always_show,
     creator, create_time, updater, update_time, deleted)
SELECT 30346 + step.idx, step.name, step.permission, 3, step.sort, 30345,
       '', '', '', NULL, 0, true, true, true,
       'migration', now(), 'migration', now(), 0
FROM (VALUES
    (0, '物理资源查询', 'infra:physical-resource:query', 1),
    (1, '物理资源创建', 'infra:physical-resource:create', 2),
    (2, '物理资源更新', 'infra:physical-resource:update', 3),
    (3, '物理资源删除', 'infra:physical-resource:delete', 4)
) AS step(idx, name, permission, sort)
WHERE NOT EXISTS (SELECT 1 FROM system_menu WHERE id = 30346 + step.idx);

-- Roles that held a cloud button keep the matching physical button.
INSERT INTO system_role_menu
    (id, role_id, menu_id, creator, create_time, updater, update_time, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), grant_.role_id, 30346 + step.idx,
       'migration', now(), 'migration', now(), 0, grant_.tenant_id
FROM system_role_menu grant_
CROSS JOIN (VALUES (0), (1), (2), (3)) AS step(idx)
WHERE grant_.menu_id = 30292 + step.idx
  AND grant_.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu
      WHERE role_id = grant_.role_id AND menu_id = 30346 + step.idx AND deleted = 0
  );
