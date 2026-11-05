-- Asset Management System Migration
-- Modules: service providers, machine rooms, cloud platforms, network zones,
--          security products, assets, business applications, business resources,
--          resource tickets, tasks, risks

CREATE SEQUENCE public.infra_service_provider_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_machine_room_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_cloud_zone_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_cloud_platform_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_cloud_provider_config_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_network_zone_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_security_product_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_asset_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_cloud_asset_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_business_application_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_application_endpoint_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_business_resource_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_resource_ticket_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_task_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE public.infra_risk_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE public.infra_service_provider (
    id bigint DEFAULT nextval('public.infra_service_provider_seq'::regclass) NOT NULL,
    provider_name character varying(128) NOT NULL, provider_code character varying(64) NOT NULL,
    short_name character varying(64) NOT NULL, logo_url character varying(512),
    contact_person character varying(64) NOT NULL, contact_phone character varying(32) NOT NULL,
    contact_email character varying(128) NOT NULL, headquarters character varying(256) NOT NULL,
    service_area character varying(256) NOT NULL, business_license character varying(128) NOT NULL,
    remarks text, status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_machine_room (
    id bigint DEFAULT nextval('public.infra_machine_room_seq'::regclass) NOT NULL,
    room_name character varying(128) NOT NULL, room_code character varying(64) NOT NULL,
    facility_type character varying(64) NOT NULL, address character varying(512) NOT NULL,
    provider_id bigint NOT NULL, room_type character varying(64) NOT NULL,
    contact_person character varying(64) NOT NULL, contact_phone character varying(32) NOT NULL,
    floor character varying(64), cabinet_count integer, area_size character varying(32),
    remarks text, status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_cloud_zone (
    id bigint DEFAULT nextval('public.infra_cloud_zone_seq'::regclass) NOT NULL,
    zone_name character varying(128) NOT NULL, zone_code character varying(64) NOT NULL,
    description text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_cloud_platform (
    id bigint DEFAULT nextval('public.infra_cloud_platform_seq'::regclass) NOT NULL,
    zone_id bigint NOT NULL, platform_name character varying(128) NOT NULL,
    platform_code character varying(64) NOT NULL, description text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_cloud_provider_config (
    id bigint DEFAULT nextval('public.infra_cloud_provider_config_seq'::regclass) NOT NULL,
    zone_id bigint, platform_id bigint, provider character varying(32) NOT NULL,
    region_id character varying(128) NOT NULL, region_name character varying(128) NOT NULL,
    available_zones text, account_name character varying(128) NOT NULL,
    access_key_id character varying(256) NOT NULL, access_key_secret character varying(512) NOT NULL,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    remarks text, last_test_time timestamp without time zone, last_test_result character varying(512),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_network_zone (
    id character varying(64) NOT NULL, name character varying(128) NOT NULL,
    cidr character varying(64) NOT NULL, priority integer DEFAULT 0 NOT NULL,
    cloud_platform_id bigint, cloud_platform_name character varying(128),
    machine_room_id bigint, machine_room_name character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_security_product (
    id bigint DEFAULT nextval('public.infra_security_product_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL, category character varying(64) NOT NULL,
    vendor character varying(128) NOT NULL, model character varying(128) NOT NULL,
    version character varying(64) NOT NULL, serial_number character varying(128),
    license_type character varying(64) NOT NULL, license_expiry character varying(32),
    management_ip character varying(64), deployment_mode character varying(64) NOT NULL,
    cloud_platform_id bigint, machine_room_id bigint, provider_id bigint,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    features text, throughput character varying(64),
    contact_person character varying(64) NOT NULL, contact_phone character varying(32) NOT NULL,
    remarks text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_asset (
    id bigint DEFAULT nextval('public.infra_asset_seq'::regclass) NOT NULL,
    name character varying(256) NOT NULL, ip character varying(64) NOT NULL,
    zone character varying(128) NOT NULL, ports text DEFAULT '[]'::text NOT NULL,
    last_scanned character varying(32), contact_person character varying(64),
    contact_phone character varying(32), created_by character varying(64),
    updated_by character varying(64), owner character varying(64),
    weight integer DEFAULT 0 NOT NULL, labels text DEFAULT '[]'::text,
    os character varying(128), device_type character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_cloud_asset (
    id bigint DEFAULT nextval('public.infra_cloud_asset_seq'::regclass) NOT NULL,
    cloud_provider_config_id bigint NOT NULL, provider_type character varying(32) NOT NULL,
    platform_name character varying(128) NOT NULL, region_id character varying(128) NOT NULL,
    instance_id character varying(128) NOT NULL, name character varying(256) NOT NULL,
    status character varying(32) NOT NULL, private_ip character varying(64),
    public_ip character varying(64), cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL, instance_type character varying(128) NOT NULL,
    os_name character varying(128), expire_time character varying(32), raw_payload text,
    synced_at character varying(32),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_business_application (
    id bigint DEFAULT nextval('public.infra_business_application_seq'::regclass) NOT NULL,
    name character varying(256) NOT NULL, description text, created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_application_endpoint (
    id bigint DEFAULT nextval('public.infra_application_endpoint_seq'::regclass) NOT NULL,
    business_application_id bigint NOT NULL, protocol character varying(16) NOT NULL,
    dest_ip character varying(64) NOT NULL, nat_ip character varying(64),
    dest_port character varying(16) NOT NULL, domain character varying(256),
    created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_business_resource (
    id bigint DEFAULT nextval('public.infra_business_resource_seq'::regclass) NOT NULL,
    resource_type character varying(32) NOT NULL, ecs_name character varying(256) NOT NULL,
    ecs_status character varying(64) NOT NULL, resource_id character varying(128) NOT NULL,
    cloud_region character varying(128) NOT NULL, cloud_category character varying(128) NOT NULL,
    cloud_provider_config_id bigint, zone_name character varying(128),
    platform_name character varying(128), county_city character varying(64),
    vdc_name character varying(128), customer_name character varying(128) NOT NULL,
    application_name character varying(128), contract_name character varying(128),
    instance_id character varying(128) NOT NULL, ecs_type character varying(128) NOT NULL,
    ecs_os character varying(128) NOT NULL, cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL, system_disk character varying(64) NOT NULL,
    system_disk_size_gb integer DEFAULT 0 NOT NULL, data_disk text,
    completion_time character varying(32), release_time character varying(32),
    has_security_product integer DEFAULT 0 NOT NULL, ip_address character varying(64) NOT NULL,
    ecs_login_method character varying(64), ecs_login_username character varying(128),
    ecs_initial_password character varying(256), bastion_address character varying(128),
    bastion_admin_account character varying(128), bastion_initial_password character varying(256),
    serial_number character varying(128), rack_location character varying(128),
    hardware_model character varying(128), warranty_expiry character varying(32),
    agent_status character varying(32), ipmi_address character varying(64), remarks text,
    application_status character varying(32), delivery_status character varying(32),
    delivery_confirmed_at character varying(32), delivery_confirmed_by character varying(64),
    applicant character varying(64), department character varying(128),
    approver character varying(64), approval_time character varying(32),
    approval_remarks text, rejection_reason text,
    bandwidth_mbps integer, bandwidth_type character varying(32),
    public_ip_count integer, network_type character varying(32),
    project_name character varying(128), project_code character varying(64),
    business_owner character varying(64), tech_owner character varying(64),
    contact_phone character varying(32), billing_method character varying(32),
    purchase_duration integer, cost_center character varying(64),
    security_level character varying(32), data_sensitivity character varying(32),
    purpose text, expected_delivery_time character varying(32),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_resource_ticket (
    id bigint DEFAULT nextval('public.infra_resource_ticket_seq'::regclass) NOT NULL,
    resource_type character varying(32) NOT NULL, ecs_name character varying(256) NOT NULL,
    ticket_status character varying(32) NOT NULL,
    provider_id bigint, provider_name character varying(128),
    cloud_platform_id bigint, cloud_platform_name character varying(128),
    machine_room_id bigint, machine_room_name character varying(128),
    cloud_region character varying(128), cloud_category character varying(128),
    zone_name character varying(128), zone_cabinet character varying(128),
    rack_units integer DEFAULT 0 NOT NULL, customer_name character varying(128),
    application_name character varying(128), application_endpoint_id bigint,
    application_domain character varying(256), contract_name character varying(128),
    ecs_type character varying(128), ecs_os character varying(128),
    resource_count integer DEFAULT 0 NOT NULL, cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL, system_disk character varying(64),
    system_disk_size_gb integer DEFAULT 0 NOT NULL, data_disk text,
    expire_at character varying(32), has_security_product integer DEFAULT 0 NOT NULL,
    security_products text, ip_address character varying(64),
    delivery_status character varying(32), remarks text,
    created_by character varying(64) NOT NULL, applicant_name character varying(64),
    organization_id bigint, organization_name character varying(128),
    department_id bigint, department_name character varying(128),
    approver character varying(64), approve_time character varying(32),
    approve_comment text, provisioner character varying(64),
    provision_time character varying(32), provision_details text,
    deliverer character varying(64), deliver_time character varying(32),
    deliver_comment text,
    fw_source_zone character varying(128), fw_source_address character varying(64),
    fw_source_port character varying(16), fw_dest_zone character varying(128),
    fw_dest_address character varying(64), fw_dest_port character varying(16),
    fw_protocol character varying(16), fw_port character varying(16),
    fw_direction character varying(16), fw_valid_until character varying(32),
    fw_firewall_name character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_task (
    id character varying(64) NOT NULL, name character varying(256) NOT NULL,
    target character varying(256) NOT NULL, status character varying(32) NOT NULL,
    start_time character varying(32), end_time character varying(32),
    found_assets integer DEFAULT 0 NOT NULL, found_risks integer DEFAULT 0 NOT NULL,
    port_policy character varying(32) NOT NULL, domain_brute integer DEFAULT 0 NOT NULL,
    service_detection integer DEFAULT 0 NOT NULL, os_detection integer DEFAULT 0 NOT NULL,
    site_identify integer DEFAULT 0 NOT NULL, created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

CREATE TABLE public.infra_risk (
    id character varying(64) NOT NULL, asset_ip character varying(64) NOT NULL,
    port integer NOT NULL, severity character varying(32) NOT NULL,
    description text NOT NULL, solution text, status character varying(32) NOT NULL,
    assigned_to character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);

-- Indexes
CREATE INDEX idx_service_provider_code ON public.infra_service_provider (provider_code) WHERE deleted = 0;
CREATE INDEX idx_machine_room_code ON public.infra_machine_room (room_code) WHERE deleted = 0;
CREATE INDEX idx_machine_room_provider ON public.infra_machine_room (provider_id) WHERE deleted = 0;
CREATE INDEX idx_cloud_zone_code ON public.infra_cloud_zone (zone_code) WHERE deleted = 0;
CREATE INDEX idx_cloud_platform_zone ON public.infra_cloud_platform (zone_id) WHERE deleted = 0;
CREATE INDEX idx_cloud_provider_config_zone ON public.infra_cloud_provider_config (zone_id) WHERE deleted = 0;
CREATE INDEX idx_cloud_provider_config_platform ON public.infra_cloud_provider_config (platform_id) WHERE deleted = 0;
CREATE INDEX idx_network_zone_name ON public.infra_network_zone (name) WHERE deleted = 0;
CREATE INDEX idx_security_product_category ON public.infra_security_product (category) WHERE deleted = 0;
CREATE INDEX idx_security_product_vendor ON public.infra_security_product (vendor) WHERE deleted = 0;
CREATE INDEX idx_asset_ip ON public.infra_asset (ip) WHERE deleted = 0;
CREATE UNIQUE INDEX idx_asset_ip_unique ON public.infra_asset (ip) WHERE deleted = 0;
CREATE INDEX idx_cloud_asset_provider ON public.infra_cloud_asset (cloud_provider_config_id) WHERE deleted = 0;
CREATE INDEX idx_cloud_asset_instance ON public.infra_cloud_asset (instance_id) WHERE deleted = 0;
CREATE INDEX idx_business_app_name ON public.infra_business_application (name) WHERE deleted = 0;
CREATE INDEX idx_app_endpoint_business ON public.infra_application_endpoint (business_application_id) WHERE deleted = 0;
CREATE INDEX idx_business_resource_type ON public.infra_business_resource (resource_type) WHERE deleted = 0;
CREATE INDEX idx_business_resource_category ON public.infra_business_resource (cloud_category) WHERE deleted = 0;
CREATE INDEX idx_business_resource_customer ON public.infra_business_resource (customer_name) WHERE deleted = 0;
CREATE INDEX idx_resource_ticket_type ON public.infra_resource_ticket (resource_type) WHERE deleted = 0;
CREATE INDEX idx_resource_ticket_status ON public.infra_resource_ticket (ticket_status) WHERE deleted = 0;
CREATE INDEX idx_resource_ticket_provider ON public.infra_resource_ticket (provider_id) WHERE deleted = 0;
CREATE INDEX idx_task_status ON public.infra_task (status) WHERE deleted = 0;
CREATE INDEX idx_risk_asset_ip ON public.infra_risk (asset_ip) WHERE deleted = 0;
CREATE INDEX idx_risk_severity ON public.infra_risk (severity) WHERE deleted = 0;
CREATE INDEX idx_risk_status ON public.infra_risk (status) WHERE deleted = 0;
