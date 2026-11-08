-- Implement the asset inventory and firewall policy ledgers defined by
-- docs/资产采集表.xlsx. Existing scan-oriented columns remain available so
-- migrated Kairos pages and scan results continue to work.

ALTER TABLE public.infra_asset
    ADD COLUMN IF NOT EXISTS city character varying(64),
    ADD COLUMN IF NOT EXISTS district character varying(64),
    ADD COLUMN IF NOT EXISTS organization_name character varying(256),
    ADD COLUMN IF NOT EXISTS business_department character varying(128),
    ADD COLUMN IF NOT EXISTS department_contact character varying(64),
    ADD COLUMN IF NOT EXISTS application_name character varying(256),
    ADD COLUMN IF NOT EXISTS server_name character varying(256),
    ADD COLUMN IF NOT EXISTS hardware_configuration character varying(512),
    ADD COLUMN IF NOT EXISTS operating_system character varying(128),
    ADD COLUMN IF NOT EXISTS database_type character varying(128),
    ADD COLUMN IF NOT EXISTS launch_date date,
    ADD COLUMN IF NOT EXISTS decommission_date date,
    ADD COLUMN IF NOT EXISTS application_type character varying(128),
    ADD COLUMN IF NOT EXISTS network_environment character varying(128),
    ADD COLUMN IF NOT EXISTS internet_ipv4 character varying(64),
    ADD COLUMN IF NOT EXISTS internet_ipv6 character varying(128),
    ADD COLUMN IF NOT EXISTS domain_address character varying(512),
    ADD COLUMN IF NOT EXISTS internal_network_ip character varying(64),
    ADD COLUMN IF NOT EXISTS government_extranet_ip character varying(64),
    ADD COLUMN IF NOT EXISTS open_ports text,
    ADD COLUMN IF NOT EXISTS publishing_endpoint character varying(128),
    ADD COLUMN IF NOT EXISTS publishes_other_endpoint boolean DEFAULT false NOT NULL,
    ADD COLUMN IF NOT EXISTS other_endpoint_name character varying(128),
    ADD COLUMN IF NOT EXISTS security_product_installation text,
    ADD COLUMN IF NOT EXISTS development_vendor character varying(256),
    ADD COLUMN IF NOT EXISTS development_vendor_contact character varying(128),
    ADD COLUMN IF NOT EXISTS security_vendor character varying(256),
    ADD COLUMN IF NOT EXISTS security_vendor_contact character varying(128),
    ADD COLUMN IF NOT EXISTS operations_vendor character varying(256),
    ADD COLUMN IF NOT EXISTS operations_vendor_contact character varying(128),
    ADD COLUMN IF NOT EXISTS classified_protection_level character varying(32),
    ADD COLUMN IF NOT EXISTS classified_protection_assessed boolean DEFAULT false NOT NULL,
    ADD COLUMN IF NOT EXISTS classified_protection_assessor character varying(256),
    ADD COLUMN IF NOT EXISTS classified_protection_assessment_date date,
    ADD COLUMN IF NOT EXISTS classified_protection_score numeric(5,2),
    ADD COLUMN IF NOT EXISTS classified_protection_filed boolean DEFAULT false NOT NULL,
    ADD COLUMN IF NOT EXISTS classified_protection_filing_date date,
    ADD COLUMN IF NOT EXISTS classified_protection_filing_number character varying(128),
    ADD COLUMN IF NOT EXISTS classified_protection_filing_authority character varying(256),
    ADD COLUMN IF NOT EXISTS cryptography_assessed boolean DEFAULT false NOT NULL,
    ADD COLUMN IF NOT EXISTS cryptography_assessment_level character varying(32),
    ADD COLUMN IF NOT EXISTS cryptography_assessment_date date,
    ADD COLUMN IF NOT EXISTS cryptography_assessment_number character varying(128);

-- Give existing scan assets useful inventory defaults without guessing
-- organizational or compliance data that was never collected.
UPDATE public.infra_asset
SET server_name = COALESCE(server_name, name),
    operating_system = COALESCE(operating_system, os),
    network_environment = COALESCE(network_environment, zone),
    department_contact = COALESCE(department_contact, contact_person),
    internet_ipv4 = COALESCE(internet_ipv4, CASE WHEN zone = 'Internet' THEN ip END),
    internal_network_ip = COALESCE(internal_network_ip, CASE WHEN zone <> 'Internet' THEN ip END)
WHERE deleted = 0;

CREATE INDEX IF NOT EXISTS idx_infra_asset_inventory_org
    ON public.infra_asset (organization_name, business_department)
    WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_infra_asset_inventory_application
    ON public.infra_asset (application_name, server_name)
    WHERE deleted = 0;

CREATE SEQUENCE IF NOT EXISTS public.infra_network_policy_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.infra_network_policy (
    id bigint DEFAULT nextval('public.infra_network_policy_seq'::regclass) NOT NULL,
    firewall_name character varying(128) NOT NULL,
    destination_organization character varying(256) NOT NULL,
    destination_project character varying(256) NOT NULL,
    source_organization character varying(256) NOT NULL,
    source_project character varying(256) NOT NULL,
    source_security_zone character varying(128) NOT NULL,
    source_ip character varying(128) NOT NULL,
    destination_security_zone character varying(128) NOT NULL,
    destination_ip character varying(128) NOT NULL,
    service_port character varying(128) NOT NULL,
    applicant character varying(64) NOT NULL,
    application_date date NOT NULL,
    traffic_direction character varying(64) NOT NULL,
    action character varying(64) NOT NULL,
    implementer character varying(64),
    implementation_date date,
    delivery_date date,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT infra_network_policy_pkey PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_infra_network_policy_endpoints
    ON public.infra_network_policy (source_ip, destination_ip)
    WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_infra_network_policy_application_date
    ON public.infra_network_policy (application_date DESC)
    WHERE deleted = 0;

SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

WITH root AS (
    SELECT id
    FROM system_menu
    WHERE deleted = 0 AND path = '/asset-ops' AND parent_id = 0
    ORDER BY id
    LIMIT 1
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), '网络策略', '', 2, 105, root.id,
    'network-policy', 'lucide:shield-check', 'asset-ops/network-policy/index',
    'AssetOpsNetworkPolicy', 0, true, true, true,
    'migration', 'migration', 0
FROM root
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND component = 'asset-ops/network-policy/index'
);

WITH permission_defs(action, label, sort) AS (
    VALUES
        ('query', '网络策略查询', 1),
        ('create', '网络策略创建', 2),
        ('update', '网络策略更新', 3),
        ('delete', '网络策略删除', 4)
), page AS (
    SELECT id
    FROM system_menu
    WHERE deleted = 0 AND component = 'asset-ops/network-policy/index'
    ORDER BY id
    LIMIT 1
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), permission.label,
    'infra:network-policy:' || permission.action,
    3, permission.sort, page.id, '', '', '', '',
    0, true, true, true, 'migration', 'migration', 0
FROM permission_defs permission
CROSS JOIN page
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0
      AND existing.permission = 'infra:network-policy:' || permission.action
);

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);

WITH policy_menus AS (
    SELECT page.id
    FROM system_menu page
    WHERE page.deleted = 0 AND page.component = 'asset-ops/network-policy/index'
    UNION
    SELECT action.id
    FROM system_menu action
    JOIN system_menu page ON page.id = action.parent_id
    WHERE page.deleted = 0
      AND page.component = 'asset-ops/network-policy/index'
      AND action.deleted = 0
), super_roles AS (
    SELECT id, tenant_id
    FROM system_role
    WHERE deleted = 0 AND status = 0 AND code = 'super_admin'
)
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT
    nextval('system_role_menu_seq'), role.id, menu.id,
    'migration', 'migration', 0, role.tenant_id
FROM super_roles role
CROSS JOIN policy_menus menu
WHERE NOT EXISTS (
    SELECT 1
    FROM system_role_menu existing
    WHERE existing.deleted = 0
      AND existing.role_id = role.id
      AND existing.menu_id = menu.id
);
