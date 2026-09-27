--
-- PostgreSQL database dump
--

\restrict O11kQ74k2KCb73stt2JlE8yvRaKwWt5cSHy1KaavgPkPrE4pMJF15fbbx63PsTZ

-- Dumped from database version 18.6 (Ubuntu 18.6-0ubuntu0.26.04.1)
-- Dumped by pg_dump version 18.6 (Ubuntu 18.6-0ubuntu0.26.04.1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: ai; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA ai;


--
-- Name: infra; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA infra;


--
-- Name: cmdb_rebuild_model_unique_values(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.cmdb_rebuild_model_unique_values() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.unique_key IS DISTINCT FROM NEW.unique_key THEN
        DELETE FROM public.cmdb_instance_unique_value WHERE model_id = NEW.id;
        INSERT INTO public.cmdb_instance_unique_value(instance_id, tenant_id, model_id, value)
            SELECT id, tenant_id, model_id, public.cmdb_unique_value(attributes, NEW.unique_key)
            FROM public.cmdb_instance WHERE model_id = NEW.id AND deleted = 0
              AND public.cmdb_unique_value(attributes, NEW.unique_key) IS NOT NULL;
    END IF;
    RETURN NEW;
END $$;


--
-- Name: cmdb_sync_instance_unique_value(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.cmdb_sync_instance_unique_value() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE model_key text; key_value jsonb;
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM id FROM public.cmdb_model WHERE id = OLD.model_id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        PERFORM id FROM public.cmdb_model WHERE id IN (OLD.model_id, NEW.model_id) ORDER BY id FOR UPDATE;
        DELETE FROM public.cmdb_instance_unique_value WHERE instance_id = OLD.id;
    END IF;
    SELECT unique_key INTO model_key FROM public.cmdb_model WHERE id = NEW.model_id FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'instance model does not exist' USING ERRCODE = '23503';
    END IF;
    key_value := public.cmdb_unique_value(NEW.attributes, model_key);
    IF NEW.deleted = 0 AND key_value IS NOT NULL THEN
        INSERT INTO public.cmdb_instance_unique_value(instance_id, tenant_id, model_id, value)
            VALUES (NEW.id, NEW.tenant_id, NEW.model_id, key_value);
    END IF;
    RETURN NEW;
END $$;


--
-- Name: cmdb_unique_value(jsonb, text); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.cmdb_unique_value(attributes jsonb, key text) RETURNS jsonb
    LANGUAGE sql IMMUTABLE
    AS $$
    SELECT CASE
        WHEN key IS NULL OR key = '' OR attributes->key IS NULL
          OR attributes->key IN ('null'::jsonb, '[]'::jsonb)
          OR (jsonb_typeof(attributes->key) = 'string'
              AND btrim(attributes->>key, U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000') = '')
        THEN NULL ELSE attributes->key END
$$;


--
-- Name: require_resource_tenant(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.require_resource_tenant() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'INSERT' AND (NEW.tenant_id IS NULL OR NEW.tenant_id <= 0) THEN
        RAISE EXCEPTION 'tenant_id is required for new resources' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.tenant_id IS NOT NULL AND NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
        RAISE EXCEPTION 'resource tenant is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.tenant_id IS NOT NULL AND NEW.tenant_id <= 0 THEN
        RAISE EXCEPTION 'tenant_id must be positive' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: chat_conversations; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.chat_conversations (
    id bigint NOT NULL,
    user_id text NOT NULL,
    title text NOT NULL,
    pinned boolean DEFAULT false NOT NULL,
    role_id bigint,
    model_id bigint NOT NULL,
    temperature double precision DEFAULT 0.7 NOT NULL,
    max_tokens integer DEFAULT 4096 NOT NULL,
    max_contexts integer DEFAULT 20 NOT NULL,
    system_message text,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL,
    tool_ids bigint[] DEFAULT '{}'::bigint[] NOT NULL,
    knowledge_ids bigint[] DEFAULT '{}'::bigint[] NOT NULL
);


--
-- Name: chat_messages; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.chat_messages (
    id bigint NOT NULL,
    conversation_id bigint NOT NULL,
    user_id text NOT NULL,
    type text NOT NULL,
    model_id bigint,
    content text DEFAULT ''::text NOT NULL,
    reasoning_content text,
    tokens integer DEFAULT 0 NOT NULL,
    segment_ids bigint[] DEFAULT '{}'::bigint[] NOT NULL,
    attachment_urls text[] DEFAULT '{}'::text[] NOT NULL,
    tool_calls jsonb DEFAULT '[]'::jsonb NOT NULL,
    create_time bigint NOT NULL,
    CONSTRAINT chat_messages_type_check CHECK ((type = ANY (ARRAY['system'::text, 'user'::text, 'assistant'::text, 'tool'::text])))
);


--
-- Name: chat_roles; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.chat_roles (
    id bigint NOT NULL,
    user_id text,
    model_id bigint NOT NULL,
    name text NOT NULL,
    avatar text DEFAULT ''::text NOT NULL,
    category text DEFAULT '通用'::text NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    system_message text DEFAULT ''::text NOT NULL,
    welcome_message text DEFAULT ''::text NOT NULL,
    public_status boolean DEFAULT false NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    knowledge_ids bigint[] DEFAULT '{}'::bigint[] NOT NULL,
    tool_ids bigint[] DEFAULT '{}'::bigint[] NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: images; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.images (
    id bigint NOT NULL,
    user_id text NOT NULL,
    model_id bigint NOT NULL,
    platform text NOT NULL,
    model text NOT NULL,
    prompt text NOT NULL,
    width integer NOT NULL,
    height integer NOT NULL,
    status integer DEFAULT 10 NOT NULL,
    public_status boolean DEFAULT false NOT NULL,
    pic_url text,
    error_message text,
    options jsonb DEFAULT '{}'::jsonb NOT NULL,
    task_id text,
    buttons jsonb DEFAULT '[]'::jsonb NOT NULL,
    create_time bigint NOT NULL,
    finish_time bigint,
    parent_id bigint,
    action_custom_id text,
    poll_count integer DEFAULT 0 NOT NULL,
    last_poll_time bigint
);


--
-- Name: knowledge_bases; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.knowledge_bases (
    id bigint NOT NULL,
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    embedding_model_id bigint NOT NULL,
    top_k integer DEFAULT 5 NOT NULL,
    similarity_threshold double precision DEFAULT 0.5 NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: knowledge_documents; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.knowledge_documents (
    id bigint NOT NULL,
    knowledge_id bigint NOT NULL,
    name text NOT NULL,
    url text DEFAULT ''::text NOT NULL,
    content text DEFAULT ''::text NOT NULL,
    content_length integer DEFAULT 0 NOT NULL,
    tokens integer DEFAULT 0 NOT NULL,
    segment_max_tokens integer DEFAULT 500 NOT NULL,
    retrieval_count integer DEFAULT 0 NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: knowledge_segments; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.knowledge_segments (
    id bigint NOT NULL,
    document_id bigint NOT NULL,
    knowledge_id bigint NOT NULL,
    vector_id text NOT NULL,
    content text NOT NULL,
    content_length integer NOT NULL,
    tokens integer NOT NULL,
    retrieval_count integer DEFAULT 0 NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    embedding real[],
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: model_catalog; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.model_catalog (
    platform text NOT NULL,
    model text NOT NULL,
    type text NOT NULL,
    source text DEFAULT 'preset'::text NOT NULL,
    source_url text DEFAULT ''::text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    synced_at bigint NOT NULL,
    missing_count integer DEFAULT 0 NOT NULL,
    verified_at bigint
);


--
-- Name: model_configs; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.model_configs (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    key character varying(255) NOT NULL,
    platform character varying(64) NOT NULL,
    type character varying(32) NOT NULL,
    model character varying(255) NOT NULL,
    api_key text DEFAULT ''::text NOT NULL,
    url text DEFAULT ''::text NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: model_platforms; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.model_platforms (
    platform text NOT NULL,
    label text NOT NULL,
    default_url text DEFAULT ''::text NOT NULL,
    supported_types text[] DEFAULT '{}'::text[] NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: music; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.music (
    id bigint NOT NULL,
    user_id text NOT NULL,
    model_id bigint NOT NULL,
    title text DEFAULT ''::text NOT NULL,
    lyric text DEFAULT ''::text NOT NULL,
    image_url text,
    audio_url text,
    video_url text,
    status integer DEFAULT 10 NOT NULL,
    gpt_description_prompt text,
    prompt text DEFAULT ''::text NOT NULL,
    platform text NOT NULL,
    model text NOT NULL,
    generate_mode integer DEFAULT 1 NOT NULL,
    tags text DEFAULT ''::text NOT NULL,
    duration double precision DEFAULT 0 NOT NULL,
    public_status boolean DEFAULT false NOT NULL,
    task_id text,
    error_message text,
    create_time bigint NOT NULL,
    finish_time bigint,
    poll_count integer DEFAULT 0 NOT NULL,
    last_poll_time bigint
);


--
-- Name: tools; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.tools (
    id bigint NOT NULL,
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    input_schema jsonb DEFAULT '{"type": "object", "properties": {}}'::jsonb NOT NULL,
    executor jsonb DEFAULT '{}'::jsonb NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL
);


--
-- Name: writes; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.writes (
    id bigint NOT NULL,
    user_id text NOT NULL,
    model_id bigint NOT NULL,
    type integer NOT NULL,
    prompt text NOT NULL,
    original_content text DEFAULT ''::text NOT NULL,
    length integer DEFAULT 0 NOT NULL,
    format integer DEFAULT 1 NOT NULL,
    tone integer DEFAULT 1 NOT NULL,
    language integer DEFAULT 1 NOT NULL,
    platform text NOT NULL,
    model text NOT NULL,
    generated_content text DEFAULT ''::text NOT NULL,
    error_message text,
    create_time bigint NOT NULL,
    finish_time bigint
);


--
-- Name: _sqlx_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public._sqlx_migrations (
    version bigint NOT NULL,
    description text NOT NULL,
    installed_on timestamp with time zone DEFAULT now() NOT NULL,
    success boolean NOT NULL,
    checksum bytea NOT NULL,
    execution_time bigint NOT NULL
);


--
-- Name: cmdb_attribute_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cmdb_attribute_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cmdb_attribute; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_attribute (
    id bigint DEFAULT nextval('public.cmdb_attribute_seq'::regclass) NOT NULL,
    model_id bigint NOT NULL,
    name character varying(64) NOT NULL,
    code character varying(64) NOT NULL,
    attr_type character varying(32) NOT NULL,
    required boolean DEFAULT false NOT NULL,
    choices jsonb,
    default_value jsonb,
    show_in_list boolean DEFAULT true NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: cmdb_instance_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cmdb_instance_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cmdb_instance; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_instance (
    id bigint DEFAULT nextval('public.cmdb_instance_seq'::regclass) NOT NULL,
    model_id bigint NOT NULL,
    attributes jsonb DEFAULT '{}'::jsonb NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint
);


--
-- Name: cmdb_instance_unique_value; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_instance_unique_value (
    instance_id bigint NOT NULL,
    model_id bigint NOT NULL,
    value jsonb NOT NULL,
    tenant_id bigint
);


--
-- Name: cmdb_model_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cmdb_model_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cmdb_model; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_model (
    id bigint DEFAULT nextval('public.cmdb_model_seq'::regclass) NOT NULL,
    name character varying(64) NOT NULL,
    code character varying(64) NOT NULL,
    description character varying(256),
    icon character varying(128),
    unique_key character varying(64),
    sort integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: cmdb_net_zone_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cmdb_net_zone_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cmdb_net_zone; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_net_zone (
    id bigint DEFAULT nextval('public.cmdb_net_zone_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    zone_type character varying(32) DEFAULT 'company'::character varying NOT NULL,
    cidr character varying(64),
    sort integer DEFAULT 0 NOT NULL,
    description character varying(256),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 1 NOT NULL
);


--
-- Name: cmdb_relation_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cmdb_relation_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cmdb_relation; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cmdb_relation (
    id bigint DEFAULT nextval('public.cmdb_relation_seq'::regclass) NOT NULL,
    source_id bigint NOT NULL,
    target_id bigint NOT NULL,
    relation character varying(64) DEFAULT 'relates_to'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint
);


--
-- Name: infra_api_access_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_api_access_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_api_access_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_api_access_log (
    id bigint DEFAULT nextval('public.infra_api_access_log_seq'::regclass) NOT NULL,
    trace_id character varying(64),
    user_id bigint,
    user_type smallint,
    application_name character varying(100),
    request_method character varying(16),
    request_url character varying(1024),
    request_params text,
    response_body text,
    user_ip character varying(64),
    user_agent character varying(512),
    operate_module character varying(100),
    operate_name character varying(100),
    operate_type smallint,
    begin_time timestamp without time zone,
    end_time timestamp without time zone,
    duration integer DEFAULT 0 NOT NULL,
    result_code integer,
    result_msg character varying(512),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_api_error_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_api_error_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_api_error_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_api_error_log (
    id bigint DEFAULT nextval('public.infra_api_error_log_seq'::regclass) NOT NULL,
    trace_id character varying(64),
    user_id bigint,
    user_type smallint,
    application_name character varying(100),
    request_method character varying(16),
    request_url character varying(1024),
    request_params text,
    user_ip character varying(64),
    user_agent character varying(512),
    exception_time timestamp without time zone,
    exception_name character varying(255),
    exception_message text,
    exception_root_cause_message text,
    exception_stack_trace text,
    exception_class_name character varying(255),
    exception_file_name character varying(255),
    exception_method_name character varying(255),
    exception_line_number integer,
    process_status smallint DEFAULT 0 NOT NULL,
    process_time timestamp without time zone,
    process_user_id bigint,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_application_endpoint_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_application_endpoint_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_application_endpoint; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_application_endpoint (
    id bigint DEFAULT nextval('public.infra_application_endpoint_seq'::regclass) NOT NULL,
    business_application_id bigint NOT NULL,
    protocol character varying(16) NOT NULL,
    dest_ip character varying(64) NOT NULL,
    nat_ip character varying(64),
    dest_port character varying(16) NOT NULL,
    domain character varying(256),
    created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_approval_rule_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_approval_rule_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_approval_rule; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_approval_rule (
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
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_asset_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_asset_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_asset; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_asset (
    id bigint DEFAULT nextval('public.infra_asset_seq'::regclass) NOT NULL,
    name character varying(256) NOT NULL,
    ip character varying(64) NOT NULL,
    zone character varying(128) NOT NULL,
    ports text DEFAULT '[]'::text NOT NULL,
    last_scanned character varying(32),
    contact_person character varying(64),
    contact_phone character varying(32),
    created_by character varying(64),
    updated_by character varying(64),
    owner character varying(64),
    weight integer DEFAULT 0 NOT NULL,
    labels text DEFAULT '[]'::text,
    os character varying(128),
    device_type character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    city character varying(64),
    district character varying(64),
    organization_name character varying(256),
    business_department character varying(128),
    department_contact character varying(64),
    application_name character varying(256),
    server_name character varying(256),
    hardware_configuration character varying(512),
    operating_system character varying(128),
    database_type character varying(128),
    launch_date date,
    decommission_date date,
    application_type character varying(128),
    network_environment character varying(128),
    internet_ipv4 character varying(64),
    internet_ipv6 character varying(128),
    domain_address character varying(512),
    internal_network_ip character varying(64),
    government_extranet_ip character varying(64),
    open_ports text,
    publishing_endpoint character varying(128),
    publishes_other_endpoint boolean DEFAULT false NOT NULL,
    other_endpoint_name character varying(128),
    security_product_installation text,
    development_vendor character varying(256),
    development_vendor_contact character varying(128),
    security_vendor character varying(256),
    security_vendor_contact character varying(128),
    operations_vendor character varying(256),
    operations_vendor_contact character varying(128),
    classified_protection_level character varying(32),
    classified_protection_assessed boolean DEFAULT false NOT NULL,
    classified_protection_assessor character varying(256),
    classified_protection_assessment_date date,
    classified_protection_score numeric(5,2),
    classified_protection_filed boolean DEFAULT false NOT NULL,
    classified_protection_filing_date date,
    classified_protection_filing_number character varying(128),
    classified_protection_filing_authority character varying(256),
    cryptography_assessed boolean DEFAULT false NOT NULL,
    cryptography_assessment_level character varying(32),
    cryptography_assessment_date date,
    cryptography_assessment_number character varying(128),
    net_zone_id bigint,
    ownership_source character varying(32) DEFAULT 'manual'::character varying,
    tenant_id bigint
);


--
-- Name: COLUMN infra_asset.ownership_source; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.infra_asset.ownership_source IS 'manual | segment — how organization_name was assigned';


--
-- Name: infra_business_application_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_business_application_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_business_application; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_business_application (
    id bigint DEFAULT nextval('public.infra_business_application_seq'::regclass) NOT NULL,
    name character varying(256) NOT NULL,
    description text,
    created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_cloud_asset_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_cloud_asset_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_cloud_asset; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_cloud_asset (
    id bigint DEFAULT nextval('public.infra_cloud_asset_seq'::regclass) NOT NULL,
    cloud_provider_config_id bigint NOT NULL,
    provider_type character varying(32) NOT NULL,
    platform_name character varying(128) NOT NULL,
    region_id character varying(128) NOT NULL,
    instance_id character varying(128) NOT NULL,
    name character varying(256) NOT NULL,
    status character varying(32) NOT NULL,
    private_ip character varying(64),
    public_ip character varying(64),
    cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL,
    instance_type character varying(128) NOT NULL,
    os_name character varying(128),
    expire_time character varying(32),
    raw_payload text,
    synced_at character varying(32),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_cloud_platform_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_cloud_platform_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_cloud_platform; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_cloud_platform (
    id bigint DEFAULT nextval('public.infra_cloud_platform_seq'::regclass) NOT NULL,
    zone_id bigint NOT NULL,
    platform_name character varying(128) NOT NULL,
    platform_code character varying(64) NOT NULL,
    description text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_cloud_provider_config_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_cloud_provider_config_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_cloud_provider_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_cloud_provider_config (
    id bigint DEFAULT nextval('public.infra_cloud_provider_config_seq'::regclass) NOT NULL,
    zone_id bigint,
    platform_id bigint,
    provider character varying(32) NOT NULL,
    region_id character varying(128) NOT NULL,
    region_name character varying(128) NOT NULL,
    available_zones text,
    account_name character varying(128) NOT NULL,
    access_key_id character varying(256) NOT NULL,
    access_key_secret character varying(512) NOT NULL,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    remarks text,
    last_test_time timestamp without time zone,
    last_test_result character varying(512),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_cloud_resource_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_cloud_resource_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_cloud_resource; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_cloud_resource (
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


--
-- Name: infra_cloud_zone_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_cloud_zone_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_cloud_zone; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_cloud_zone (
    id bigint DEFAULT nextval('public.infra_cloud_zone_seq'::regclass) NOT NULL,
    zone_name character varying(128) NOT NULL,
    zone_code character varying(64) NOT NULL,
    description text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_codegen_column_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_codegen_column_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_codegen_column; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_codegen_column (
    id bigint DEFAULT nextval('public.infra_codegen_column_seq'::regclass) NOT NULL,
    table_id bigint DEFAULT 0 NOT NULL,
    column_name character varying(200) DEFAULT ''::character varying NOT NULL,
    data_type character varying(100) DEFAULT ''::character varying NOT NULL,
    column_comment character varying(500) DEFAULT ''::character varying NOT NULL,
    nullable boolean DEFAULT true NOT NULL,
    primary_key boolean DEFAULT false NOT NULL,
    ordinal_position integer DEFAULT 0 NOT NULL,
    java_type character varying(100) DEFAULT ''::character varying NOT NULL,
    java_field character varying(100) DEFAULT ''::character varying NOT NULL,
    create_operation boolean DEFAULT true NOT NULL,
    update_operation boolean DEFAULT true NOT NULL,
    list_operation boolean DEFAULT true NOT NULL,
    list_operation_result boolean DEFAULT true NOT NULL,
    html_type character varying(100) DEFAULT 'input'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_codegen_table_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_codegen_table_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_codegen_table; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_codegen_table (
    id bigint DEFAULT nextval('public.infra_codegen_table_seq'::regclass) NOT NULL,
    data_source_config_id bigint DEFAULT 0 NOT NULL,
    scene smallint DEFAULT 1 NOT NULL,
    table_name character varying(200) DEFAULT ''::character varying NOT NULL,
    table_comment character varying(500) DEFAULT ''::character varying NOT NULL,
    module_name character varying(100) DEFAULT ''::character varying NOT NULL,
    business_name character varying(100) DEFAULT ''::character varying NOT NULL,
    class_name character varying(100) DEFAULT ''::character varying NOT NULL,
    class_comment character varying(500) DEFAULT ''::character varying NOT NULL,
    author character varying(100) DEFAULT ''::character varying NOT NULL,
    template_type smallint DEFAULT 1 NOT NULL,
    front_type smallint DEFAULT 20 NOT NULL,
    parent_menu_id bigint,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_config_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_config_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_config (
    id bigint DEFAULT nextval('public.infra_config_seq'::regclass) NOT NULL,
    category character varying(64) DEFAULT ''::character varying NOT NULL,
    type smallint DEFAULT 2 NOT NULL,
    name character varying(100) DEFAULT ''::character varying NOT NULL,
    config_key character varying(100) DEFAULT ''::character varying NOT NULL,
    value character varying(500) DEFAULT ''::character varying NOT NULL,
    visible boolean DEFAULT true NOT NULL,
    remark character varying(500),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_data_source_config_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_data_source_config_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_data_source_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_data_source_config (
    id bigint DEFAULT nextval('public.infra_data_source_config_seq'::regclass) NOT NULL,
    name character varying(100) DEFAULT ''::character varying NOT NULL,
    url character varying(500) DEFAULT ''::character varying NOT NULL,
    username character varying(100) DEFAULT ''::character varying NOT NULL,
    password character varying(500) DEFAULT ''::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_file_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_file_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_file; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_file (
    id bigint DEFAULT nextval('public.infra_file_seq'::regclass) NOT NULL,
    config_id bigint,
    name character varying(255),
    path character varying(512) DEFAULT ''::character varying NOT NULL,
    url character varying(1024) DEFAULT ''::character varying NOT NULL,
    type character varying(128),
    size integer DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_file_config_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_file_config_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_file_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_file_config (
    id bigint DEFAULT nextval('public.infra_file_config_seq'::regclass) NOT NULL,
    name character varying(100) DEFAULT ''::character varying NOT NULL,
    storage smallint DEFAULT 10 NOT NULL,
    master boolean DEFAULT false NOT NULL,
    config text DEFAULT '{}'::text NOT NULL,
    remark character varying(500),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_high_risk_port_rule_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_high_risk_port_rule_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_high_risk_port_rule; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_high_risk_port_rule (
    id bigint DEFAULT nextval('public.infra_high_risk_port_rule_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    protocol character varying(16) DEFAULT 'tcp'::character varying NOT NULL,
    port_start integer NOT NULL,
    port_end integer NOT NULL,
    severity character varying(32) NOT NULL,
    description text NOT NULL,
    solution text NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT infra_high_risk_port_rule_protocol_check CHECK (((protocol)::text = ANY ((ARRAY['tcp'::character varying, 'udp'::character varying, 'any'::character varying])::text[]))),
    CONSTRAINT infra_high_risk_port_rule_range_check CHECK ((((port_start >= 1) AND (port_start <= 65535)) AND ((port_end >= port_start) AND (port_end <= 65535)))),
    CONSTRAINT infra_high_risk_port_rule_severity_check CHECK (((severity)::text = ANY ((ARRAY['Critical'::character varying, 'High'::character varying, 'Medium'::character varying, 'Low'::character varying])::text[])))
);


--
-- Name: infra_inspection_baseline; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_inspection_baseline (
    ip inet NOT NULL,
    allowed_ports integer[] NOT NULL,
    reason text NOT NULL,
    updated_by character varying(64) NOT NULL,
    update_time timestamp without time zone DEFAULT now() NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_inspection_baseline_allowed_ports_check CHECK (((0 < ALL (allowed_ports)) AND (65536 > ALL (allowed_ports))))
);


--
-- Name: infra_inspection_result; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_inspection_result (
    id character varying(64) NOT NULL,
    task_id character varying(64) NOT NULL,
    ip inet NOT NULL,
    registered boolean NOT NULL,
    baseline_ports integer[],
    open_ports integer[] NOT NULL,
    uncertain_ports integer[] NOT NULL,
    differences jsonb NOT NULL,
    risk_ids text[] DEFAULT '{}'::text[] NOT NULL,
    create_time timestamp without time zone DEFAULT now() NOT NULL,
    tenant_id bigint
);


--
-- Name: infra_job_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_job_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_job; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_job (
    id bigint DEFAULT nextval('public.infra_job_seq'::regclass) NOT NULL,
    name character varying(100) DEFAULT ''::character varying NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    handler_name character varying(100) DEFAULT ''::character varying NOT NULL,
    handler_param character varying(255),
    cron_expression character varying(100) DEFAULT ''::character varying NOT NULL,
    retry_count integer DEFAULT 0 NOT NULL,
    retry_interval integer DEFAULT 0 NOT NULL,
    monitor_timeout integer DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_job_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_job_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_job_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_job_log (
    id bigint DEFAULT nextval('public.infra_job_log_seq'::regclass) NOT NULL,
    job_id bigint DEFAULT 0 NOT NULL,
    handler_name character varying(100) DEFAULT ''::character varying NOT NULL,
    handler_param character varying(255),
    execute_index integer DEFAULT 1 NOT NULL,
    begin_time timestamp without time zone,
    end_time timestamp without time zone,
    duration integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    result text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_machine_room_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_machine_room_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_machine_room; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_machine_room (
    id bigint DEFAULT nextval('public.infra_machine_room_seq'::regclass) NOT NULL,
    room_name character varying(128) NOT NULL,
    room_code character varying(64) NOT NULL,
    facility_type character varying(64) NOT NULL,
    address character varying(512) NOT NULL,
    provider_id bigint NOT NULL,
    room_type character varying(64) NOT NULL,
    contact_person character varying(64) NOT NULL,
    contact_phone character varying(32) NOT NULL,
    floor character varying(64),
    cabinet_count integer,
    area_size character varying(32),
    remarks text,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_network_policy_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_network_policy_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_network_policy; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_network_policy (
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
    tenant_id bigint
);


--
-- Name: infra_network_zone; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_network_zone (
    id character varying(64) NOT NULL,
    name character varying(128) NOT NULL,
    cidr character varying(64) NOT NULL,
    priority integer DEFAULT 0 NOT NULL,
    cloud_platform_id bigint,
    cloud_platform_name character varying(128),
    machine_room_id bigint,
    machine_room_name character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_network_zone_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_network_zone_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_physical_resource_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_physical_resource_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_physical_resource; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_physical_resource (
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


--
-- Name: infra_resource_ticket_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_resource_ticket_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_resource_ticket; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_resource_ticket (
    id bigint DEFAULT nextval('public.infra_resource_ticket_seq'::regclass) NOT NULL,
    resource_type character varying(32) NOT NULL,
    ecs_name character varying(256) NOT NULL,
    ticket_status character varying(32) NOT NULL,
    provider_id bigint,
    provider_name character varying(128),
    cloud_platform_id bigint,
    cloud_platform_name character varying(128),
    machine_room_id bigint,
    machine_room_name character varying(128),
    cloud_region character varying(128),
    cloud_category character varying(128),
    zone_name character varying(128),
    zone_cabinet character varying(128),
    rack_units integer DEFAULT 0 NOT NULL,
    customer_name character varying(128),
    application_name character varying(128),
    application_endpoint_id bigint,
    application_domain character varying(256),
    contract_name character varying(128),
    ecs_type character varying(128),
    ecs_os character varying(128),
    resource_count integer DEFAULT 0 NOT NULL,
    cpu_cores integer DEFAULT 0 NOT NULL,
    memory_gb integer DEFAULT 0 NOT NULL,
    system_disk character varying(64),
    system_disk_size_gb integer DEFAULT 0 NOT NULL,
    data_disk text,
    expire_at character varying(32),
    has_security_product integer DEFAULT 0 NOT NULL,
    security_products text,
    ip_address character varying(64),
    delivery_status character varying(32),
    remarks text,
    created_by character varying(64) NOT NULL,
    applicant_name character varying(64),
    organization_id bigint,
    organization_name character varying(128),
    department_id bigint,
    department_name character varying(128),
    approver character varying(64),
    approve_time character varying(32),
    approve_comment text,
    provisioner character varying(64),
    provision_time character varying(32),
    provision_details text,
    deliverer character varying(64),
    deliver_time character varying(32),
    deliver_comment text,
    fw_source_zone character varying(128),
    fw_source_address character varying(64),
    fw_source_port character varying(16),
    fw_dest_zone character varying(128),
    fw_dest_address character varying(64),
    fw_dest_port character varying(16),
    fw_protocol character varying(16),
    fw_port character varying(16),
    fw_direction character varying(16),
    fw_valid_until character varying(32),
    fw_firewall_name character varying(128),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    ticket_type character varying(32) DEFAULT 'create'::character varying NOT NULL,
    risk_level character varying(32) DEFAULT 'normal'::character varying NOT NULL,
    target_resource_id bigint,
    target_config text,
    maintenance_window character varying(64),
    allow_interruption boolean DEFAULT false NOT NULL,
    backup_confirmed boolean DEFAULT false NOT NULL,
    rollback_plan text,
    retention_until character varying(64),
    approval_stage integer DEFAULT 1 NOT NULL,
    approval_total integer DEFAULT 1 NOT NULL,
    current_approval_role character varying(64) DEFAULT '资源管理员'::character varying,
    apply_status character varying(32) DEFAULT 'none'::character varying,
    apply_log text,
    tf_outputs jsonb,
    tofu_workspace character varying(128),
    target_resource_type character varying(16),
    tenant_id bigint
);


--
-- Name: COLUMN infra_resource_ticket.apply_status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.infra_resource_ticket.apply_status IS 'none | planned | applied | failed — OpenTofu run tracking';


--
-- Name: infra_risk; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_risk (
    id character varying(64) NOT NULL,
    asset_ip character varying(256) NOT NULL,
    port integer NOT NULL,
    severity character varying(32) NOT NULL,
    description text NOT NULL,
    solution text,
    status character varying(32) NOT NULL,
    assigned_to character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    inspection_key text,
    tenant_id bigint,
    source_type character varying(64),
    source_id bigint,
    rule_id bigint
);


--
-- Name: infra_risk_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_risk_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_security_product_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_security_product_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_security_product; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_security_product (
    id bigint DEFAULT nextval('public.infra_security_product_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    category character varying(64) NOT NULL,
    vendor character varying(128) NOT NULL,
    model character varying(128) NOT NULL,
    version character varying(64) NOT NULL,
    serial_number character varying(128),
    license_type character varying(64) NOT NULL,
    license_expiry character varying(32),
    management_ip character varying(64),
    deployment_mode character varying(64) NOT NULL,
    cloud_platform_id bigint,
    machine_room_id bigint,
    provider_id bigint,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    features text,
    throughput character varying(64),
    contact_person character varying(64) NOT NULL,
    contact_phone character varying(32) NOT NULL,
    remarks text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_service_provider_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_service_provider_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: infra_service_provider; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_service_provider (
    id bigint DEFAULT nextval('public.infra_service_provider_seq'::regclass) NOT NULL,
    provider_name character varying(128) NOT NULL,
    provider_code character varying(64) NOT NULL,
    short_name character varying(64) NOT NULL,
    logo_url character varying(512),
    contact_person character varying(64) NOT NULL,
    contact_phone character varying(32) NOT NULL,
    contact_email character varying(128) NOT NULL,
    headquarters character varying(256) NOT NULL,
    service_area character varying(256) NOT NULL,
    business_license character varying(128) NOT NULL,
    remarks text,
    status character varying(32) DEFAULT 'active'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: infra_task; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.infra_task (
    id character varying(64) NOT NULL,
    name character varying(256) NOT NULL,
    target character varying(256) NOT NULL,
    status character varying(32) NOT NULL,
    start_time character varying(32),
    end_time character varying(32),
    found_assets integer DEFAULT 0 NOT NULL,
    found_risks integer DEFAULT 0 NOT NULL,
    port_policy character varying(32) NOT NULL,
    domain_brute integer DEFAULT 0 NOT NULL,
    service_detection integer DEFAULT 0 NOT NULL,
    os_detection integer DEFAULT 0 NOT NULL,
    site_identify integer DEFAULT 0 NOT NULL,
    created_by character varying(64),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    task_kind character varying(32) DEFAULT 'scan'::character varying NOT NULL,
    scan_ports integer[] DEFAULT '{}'::integer[] NOT NULL,
    total_targets integer DEFAULT 0 NOT NULL,
    completed_targets integer DEFAULT 0 NOT NULL,
    error_message text,
    tenant_id bigint,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    idempotency_key character varying(128),
    attempt_count integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 3 NOT NULL,
    timeout_seconds integer DEFAULT 300 NOT NULL,
    next_attempt_at timestamp with time zone DEFAULT now() NOT NULL,
    lease_owner character varying(128),
    lease_expires_at timestamp with time zone,
    heartbeat_at timestamp with time zone,
    cancel_requested boolean DEFAULT false NOT NULL,
    CONSTRAINT infra_task_retry_bounds_check CHECK (((attempt_count >= 0) AND ((max_attempts >= 1) AND (max_attempts <= 20)))),
    CONSTRAINT infra_task_timeout_bounds_check CHECK (((timeout_seconds >= 1) AND (timeout_seconds <= 86400)))
);


--
-- Name: infra_task_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.infra_task_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_dept; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_dept (
    id bigint NOT NULL,
    name character varying(30) DEFAULT ''::character varying NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    leader_user_id bigint,
    phone character varying(11) DEFAULT NULL::character varying,
    email character varying(50) DEFAULT NULL::character varying,
    status smallint NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_dept_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_dept_seq
    START WITH 118
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_dict_data; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_dict_data (
    id bigint NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    label character varying(100) DEFAULT ''::character varying NOT NULL,
    value character varying(100) DEFAULT ''::character varying NOT NULL,
    dict_type character varying(100) DEFAULT ''::character varying NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    color_type character varying(100) DEFAULT ''::character varying,
    css_class character varying(100) DEFAULT ''::character varying,
    remark character varying(500) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_dict_data_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_dict_data_seq
    START WITH 3449
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_dict_type; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_dict_type (
    id bigint NOT NULL,
    name character varying(100) DEFAULT ''::character varying NOT NULL,
    type character varying(100) DEFAULT ''::character varying NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    remark character varying(500) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    deleted_time timestamp without time zone
);


--
-- Name: system_dict_type_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_dict_type_seq
    START WITH 2139
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_login_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_login_log (
    id bigint NOT NULL,
    log_type bigint NOT NULL,
    trace_id character varying(64) DEFAULT ''::character varying NOT NULL,
    user_id bigint DEFAULT 0 NOT NULL,
    user_type smallint DEFAULT 0 NOT NULL,
    username character varying(50) DEFAULT ''::character varying NOT NULL,
    result smallint NOT NULL,
    user_ip character varying(50) NOT NULL,
    user_agent character varying(512) NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_login_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_login_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_mail_account; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_mail_account (
    id bigint NOT NULL,
    mail character varying(255) NOT NULL,
    username character varying(255) NOT NULL,
    password character varying(255) NOT NULL,
    host character varying(255) NOT NULL,
    port integer NOT NULL,
    ssl_enable boolean DEFAULT false NOT NULL,
    starttls_enable boolean DEFAULT false NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_mail_account_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_mail_account_seq
    START WITH 5
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_mail_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_mail_log (
    id bigint NOT NULL,
    user_id bigint,
    user_type smallint,
    to_mails character varying(1024) NOT NULL,
    cc_mails character varying(1024) DEFAULT NULL::character varying,
    bcc_mails character varying(1024) DEFAULT NULL::character varying,
    account_id bigint NOT NULL,
    from_mail character varying(255) NOT NULL,
    template_id bigint NOT NULL,
    template_code character varying(63) NOT NULL,
    template_nickname character varying(255) DEFAULT NULL::character varying,
    template_title character varying(255) NOT NULL,
    template_content text NOT NULL,
    template_params character varying(255) NOT NULL,
    send_status smallint DEFAULT 0 NOT NULL,
    send_time timestamp without time zone,
    send_message_id character varying(255) DEFAULT NULL::character varying,
    send_exception character varying(4096) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_mail_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_mail_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_mail_template; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_mail_template (
    id bigint NOT NULL,
    name character varying(63) NOT NULL,
    code character varying(63) NOT NULL,
    account_id bigint NOT NULL,
    nickname character varying(255) DEFAULT NULL::character varying,
    title character varying(255) NOT NULL,
    content character varying(10240) NOT NULL,
    params character varying(255) NOT NULL,
    status smallint NOT NULL,
    remark character varying(255) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_mail_template_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_mail_template_seq
    START WITH 16
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_menu; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_menu (
    id bigint NOT NULL,
    name character varying(50) NOT NULL,
    permission character varying(100) DEFAULT ''::character varying NOT NULL,
    type smallint NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    path character varying(200) DEFAULT ''::character varying,
    icon character varying(100) DEFAULT '#'::character varying,
    component character varying(255) DEFAULT NULL::character varying,
    component_name character varying(255) DEFAULT NULL::character varying,
    status smallint DEFAULT 0 NOT NULL,
    visible boolean DEFAULT true NOT NULL,
    keep_alive boolean DEFAULT true NOT NULL,
    always_show boolean DEFAULT true NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    active_menu_id bigint
);


--
-- Name: system_menu_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_menu_seq
    START WITH 5986
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_notice; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_notice (
    id bigint NOT NULL,
    title character varying(50) NOT NULL,
    content text NOT NULL,
    type smallint NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_notice_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_notice_seq
    START WITH 5
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_notify_message; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_notify_message (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint NOT NULL,
    template_id bigint NOT NULL,
    template_code character varying(64) NOT NULL,
    template_nickname character varying(63) NOT NULL,
    template_content character varying(1024) NOT NULL,
    template_type integer NOT NULL,
    template_params character varying(255) NOT NULL,
    read_status boolean NOT NULL,
    read_time timestamp without time zone,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_notify_message_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_notify_message_seq
    START WITH 11
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_notify_template; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_notify_template (
    id bigint NOT NULL,
    name character varying(63) NOT NULL,
    code character varying(64) NOT NULL,
    nickname character varying(255) NOT NULL,
    content character varying(1024) NOT NULL,
    type smallint NOT NULL,
    params character varying(255) DEFAULT NULL::character varying,
    status smallint NOT NULL,
    remark character varying(255) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_notify_template_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_notify_template_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_oauth2_access_token; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_oauth2_access_token (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint NOT NULL,
    user_info character varying(512) NOT NULL,
    access_token text NOT NULL,
    refresh_token character varying(32) NOT NULL,
    client_id character varying(255) NOT NULL,
    scopes character varying(255) DEFAULT NULL::character varying,
    expires_time timestamp without time zone NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_oauth2_access_token_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_oauth2_access_token_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_oauth2_approve; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_oauth2_approve (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint NOT NULL,
    client_id character varying(255) NOT NULL,
    scope character varying(255) DEFAULT ''::character varying NOT NULL,
    approved boolean DEFAULT false NOT NULL,
    expires_time timestamp without time zone NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_oauth2_approve_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_oauth2_approve_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_oauth2_client; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_oauth2_client (
    id bigint NOT NULL,
    client_id character varying(255) NOT NULL,
    secret character varying(255) NOT NULL,
    name character varying(255) NOT NULL,
    logo character varying(255) NOT NULL,
    description character varying(255) DEFAULT NULL::character varying,
    status smallint NOT NULL,
    access_token_validity_seconds integer NOT NULL,
    refresh_token_validity_seconds integer NOT NULL,
    redirect_uris character varying(255) NOT NULL,
    authorized_grant_types character varying(255) NOT NULL,
    scopes character varying(255) DEFAULT NULL::character varying,
    auto_approve_scopes character varying(255) DEFAULT NULL::character varying,
    authorities character varying(255) DEFAULT NULL::character varying,
    resource_ids character varying(255) DEFAULT NULL::character varying,
    additional_information character varying(4096) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_oauth2_client_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_oauth2_client_seq
    START WITH 43
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_oauth2_code; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_oauth2_code (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint NOT NULL,
    code character varying(32) NOT NULL,
    client_id character varying(255) NOT NULL,
    scopes character varying(255) DEFAULT ''::character varying,
    expires_time timestamp without time zone NOT NULL,
    redirect_uri character varying(255) DEFAULT NULL::character varying,
    state character varying(255) DEFAULT ''::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_oauth2_code_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_oauth2_code_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_oauth2_refresh_token; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_oauth2_refresh_token (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    refresh_token character varying(32) NOT NULL,
    user_type smallint NOT NULL,
    client_id character varying(255) NOT NULL,
    scopes character varying(255) DEFAULT NULL::character varying,
    expires_time timestamp without time zone NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_oauth2_refresh_token_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_oauth2_refresh_token_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_operate_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_operate_log (
    id bigint NOT NULL,
    trace_id character varying(64) DEFAULT ''::character varying NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint DEFAULT 0 NOT NULL,
    type character varying(50) NOT NULL,
    sub_type character varying(50) NOT NULL,
    biz_id bigint NOT NULL,
    action character varying(2000) DEFAULT ''::character varying NOT NULL,
    success boolean DEFAULT true NOT NULL,
    extra character varying(2000) DEFAULT ''::character varying NOT NULL,
    request_method character varying(16) DEFAULT ''::character varying,
    request_url character varying(255) DEFAULT ''::character varying,
    user_ip character varying(50) DEFAULT NULL::character varying,
    user_agent character varying(512) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_operate_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_operate_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_post; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_post (
    id bigint NOT NULL,
    code character varying(64) NOT NULL,
    name character varying(50) NOT NULL,
    sort integer NOT NULL,
    status smallint NOT NULL,
    remark character varying(500) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_post_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_post_seq
    START WITH 8
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_role; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_role (
    id bigint NOT NULL,
    name character varying(30) NOT NULL,
    code character varying(100) NOT NULL,
    sort integer NOT NULL,
    data_scope smallint DEFAULT 1 NOT NULL,
    data_scope_dept_ids character varying(500) DEFAULT ''::character varying NOT NULL,
    status smallint NOT NULL,
    type smallint NOT NULL,
    remark character varying(500) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_role_menu; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_role_menu (
    id bigint NOT NULL,
    role_id bigint NOT NULL,
    menu_id bigint NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_role_menu_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_role_menu_seq
    START WITH 6365
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_role_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_role_seq
    START WITH 156
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_sms_channel; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_sms_channel (
    id bigint NOT NULL,
    signature character varying(12) NOT NULL,
    code character varying(63) NOT NULL,
    status smallint NOT NULL,
    remark character varying(255) DEFAULT NULL::character varying,
    api_key character varying(128) NOT NULL,
    api_secret character varying(128) DEFAULT NULL::character varying,
    callback_url character varying(255) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_sms_channel_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_sms_channel_seq
    START WITH 8
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_sms_code; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_sms_code (
    id bigint NOT NULL,
    mobile character varying(11) NOT NULL,
    code character varying(6) NOT NULL,
    create_ip character varying(15) NOT NULL,
    scene smallint NOT NULL,
    today_index smallint NOT NULL,
    used smallint NOT NULL,
    used_time timestamp without time zone,
    used_ip character varying(255) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_sms_code_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_sms_code_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_sms_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_sms_log (
    id bigint NOT NULL,
    channel_id bigint NOT NULL,
    channel_code character varying(63) NOT NULL,
    template_id bigint NOT NULL,
    template_code character varying(63) NOT NULL,
    template_type smallint NOT NULL,
    template_content character varying(255) NOT NULL,
    template_params character varying(255) NOT NULL,
    api_template_id character varying(63) NOT NULL,
    mobile character varying(11) NOT NULL,
    user_id bigint,
    user_type smallint,
    send_status smallint DEFAULT 0 NOT NULL,
    send_time timestamp without time zone,
    api_send_code character varying(63) DEFAULT NULL::character varying,
    api_send_msg character varying(255) DEFAULT NULL::character varying,
    api_request_id character varying(255) DEFAULT NULL::character varying,
    api_serial_no character varying(255) DEFAULT NULL::character varying,
    receive_status smallint DEFAULT 0 NOT NULL,
    receive_time timestamp without time zone,
    api_receive_code character varying(63) DEFAULT NULL::character varying,
    api_receive_msg character varying(255) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_sms_log_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_sms_log_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_sms_template; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_sms_template (
    id bigint NOT NULL,
    type smallint NOT NULL,
    status smallint NOT NULL,
    code character varying(63) NOT NULL,
    name character varying(63) NOT NULL,
    content character varying(255) NOT NULL,
    params character varying(255) NOT NULL,
    remark character varying(255) DEFAULT NULL::character varying,
    api_template_id character varying(63) NOT NULL,
    channel_id bigint NOT NULL,
    channel_code character varying(63) NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_sms_template_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_sms_template_seq
    START WITH 20
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_social_client; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_social_client (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    social_type smallint NOT NULL,
    user_type smallint NOT NULL,
    client_id character varying(255) NOT NULL,
    client_secret character varying(255) NOT NULL,
    agent_id character varying(255) DEFAULT NULL::character varying,
    public_key character varying(2048) DEFAULT NULL::character varying,
    status smallint NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_social_client_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_social_client_seq
    START WITH 48
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_social_user; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_social_user (
    id bigint NOT NULL,
    type smallint NOT NULL,
    openid character varying(32) NOT NULL,
    token character varying(256) DEFAULT NULL::character varying,
    raw_token_info character varying(1024) NOT NULL,
    nickname character varying(32) NOT NULL,
    avatar character varying(255) DEFAULT NULL::character varying,
    raw_user_info character varying(1024) NOT NULL,
    code character varying(256) NOT NULL,
    state character varying(256) DEFAULT NULL::character varying,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_social_user_bind; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_social_user_bind (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    user_type smallint NOT NULL,
    social_type smallint NOT NULL,
    social_user_id bigint NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_social_user_bind_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_social_user_bind_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_social_user_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_social_user_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_tenant; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_tenant (
    id bigint NOT NULL,
    name character varying(30) NOT NULL,
    contact_user_id bigint,
    contact_name character varying(30) NOT NULL,
    contact_mobile character varying(500) DEFAULT NULL::character varying,
    status smallint DEFAULT 0 NOT NULL,
    websites character varying(1024) DEFAULT ''::character varying,
    package_id bigint NOT NULL,
    expire_time timestamp without time zone NOT NULL,
    account_count integer NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_tenant_package; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_tenant_package (
    id bigint CONSTRAINT system_tenant_package_id_not_null1 NOT NULL,
    name character varying(30) NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    remark character varying(256) DEFAULT ''::character varying,
    menu_ids character varying(4096) NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: system_tenant_package_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_tenant_package_seq
    START WITH 112
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_tenant_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_tenant_seq
    START WITH 123
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_user_post; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_user_post (
    id bigint NOT NULL,
    user_id bigint DEFAULT 0 NOT NULL,
    post_id bigint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_user_post_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_user_post_seq
    START WITH 130
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_user_role; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_user_role (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    role_id bigint NOT NULL,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: system_user_role_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_user_role_seq
    START WITH 55
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: system_users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_users (
    id bigint NOT NULL,
    username character varying(30) NOT NULL,
    password character varying(128) DEFAULT ''::character varying NOT NULL,
    nickname character varying(30) NOT NULL,
    remark character varying(500) DEFAULT NULL::character varying,
    dept_id bigint,
    post_ids character varying(255) DEFAULT NULL::character varying,
    email character varying(50) DEFAULT ''::character varying,
    mobile character varying(11) DEFAULT ''::character varying,
    sex smallint DEFAULT 0,
    avatar character varying(512) DEFAULT ''::character varying,
    status smallint DEFAULT 0 NOT NULL,
    login_ip character varying(50) DEFAULT ''::character varying,
    login_date timestamp without time zone,
    creator character varying(64) DEFAULT ''::character varying,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint DEFAULT 0 NOT NULL,
    identity_uuid uuid
);


--
-- Name: system_users_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.system_users_seq
    START WITH 145
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo01_contact_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.yudao_demo01_contact_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo01_contact; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.yudao_demo01_contact (
    id bigint DEFAULT nextval('public.yudao_demo01_contact_seq'::regclass) NOT NULL,
    name character varying(100),
    sex smallint,
    birthday date,
    description text,
    avatar character varying(1024),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: yudao_demo02_category_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.yudao_demo02_category_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo02_category; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.yudao_demo02_category (
    id bigint DEFAULT nextval('public.yudao_demo02_category_seq'::regclass) NOT NULL,
    name character varying(100),
    parent_id bigint,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: yudao_demo03_course_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.yudao_demo03_course_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo03_course; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.yudao_demo03_course (
    id bigint DEFAULT nextval('public.yudao_demo03_course_seq'::regclass) NOT NULL,
    student_id bigint DEFAULT 0 NOT NULL,
    name character varying(100),
    score integer,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: yudao_demo03_grade_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.yudao_demo03_grade_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo03_grade; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.yudao_demo03_grade (
    id bigint DEFAULT nextval('public.yudao_demo03_grade_seq'::regclass) NOT NULL,
    student_id bigint DEFAULT 0 NOT NULL,
    name character varying(100),
    teacher character varying(100),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Name: yudao_demo03_student_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.yudao_demo03_student_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: yudao_demo03_student; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.yudao_demo03_student (
    id bigint DEFAULT nextval('public.yudao_demo03_student_seq'::regclass) NOT NULL,
    name character varying(100),
    sex smallint,
    birthday date,
    description text,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL
);


--
-- Data for Name: chat_conversations; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.chat_conversations (id, user_id, title, pinned, role_id, model_id, temperature, max_tokens, max_contexts, system_message, create_time, update_time, tool_ids, knowledge_ids) FROM stdin;
\.


--
-- Data for Name: chat_messages; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.chat_messages (id, conversation_id, user_id, type, model_id, content, reasoning_content, tokens, segment_ids, attachment_urls, tool_calls, create_time) FROM stdin;
\.


--
-- Data for Name: chat_roles; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.chat_roles (id, user_id, model_id, name, avatar, category, sort, description, system_message, welcome_message, public_status, status, knowledge_ids, tool_ids, create_time, update_time) FROM stdin;
1784300000100	1	1784257814114	运维助理		Agent	100	CMDB / 资产台账 / 资源工单的智能运维 Agent	你是 RustSet 的运维助理 Agent。你可以：用 cmdb_model_list 和 cmdb_instance_query 查询 CMDB 模型与配置项；用 asset_query 查询资产采集台账；用 ticket_query 查询资源开通工单；用 ticket_create 创建云主机开通工单（会按自动审批规则流转）。回答保持简洁、基于工具返回的真实数据，不确定时先查询再回答。创建工单前先向用户确认规格（名称/CPU/内存/数量）。	你好，我是运维助理。可以问我“CMDB 里有哪些模型”“查一下 web-prod 相关资产”或“帮我开一台 4 核 8G 的机器”。	t	1	{}	{1784300000001,1784300000002,1784300000003,1784300000004,1784300000005}	1790506719106	1790506719106
\.


--
-- Data for Name: images; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.images (id, user_id, model_id, platform, model, prompt, width, height, status, public_status, pic_url, error_message, options, task_id, buttons, create_time, finish_time, parent_id, action_custom_id, poll_count, last_poll_time) FROM stdin;
\.


--
-- Data for Name: knowledge_bases; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.knowledge_bases (id, name, description, embedding_model_id, top_k, similarity_threshold, create_time, update_time) FROM stdin;
\.


--
-- Data for Name: knowledge_documents; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.knowledge_documents (id, knowledge_id, name, url, content, content_length, tokens, segment_max_tokens, retrieval_count, status, create_time, update_time) FROM stdin;
\.


--
-- Data for Name: knowledge_segments; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.knowledge_segments (id, document_id, knowledge_id, vector_id, content, content_length, tokens, retrieval_count, status, embedding, create_time, update_time) FROM stdin;
\.


--
-- Data for Name: model_catalog; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.model_catalog (platform, model, type, source, source_url, active, synced_at, missing_count, verified_at) FROM stdin;
Moonshot	kimi-k2.5	chat	preset		t	0	0	\N
Moonshot	kimi-k2-0711-preview	chat	preset		t	0	0	\N
Anthropic	claude-fable-5	chat	preset		t	0	0	\N
Anthropic	claude-opus-4-8	chat	preset		t	0	0	\N
Anthropic	claude-sonnet-5	chat	preset		t	0	0	\N
Anthropic	claude-haiku-4-5	chat	preset		t	0	0	\N
OpenAI	gpt-5.6-sol	chat	preset		t	0	0	\N
OpenAI	gpt-5.6-terra	chat	preset		t	0	0	\N
OpenAI	gpt-5.6-luna	chat	preset		t	0	0	\N
OpenAI	gpt-image-2	image	preset		t	0	0	\N
OpenAI	gpt-audio-1.5	speech	preset		t	0	0	\N
OpenAI	gpt-4o-transcribe	transcription	preset		t	0	0	\N
OpenAI	text-embedding-3-small	embedding	preset		t	0	0	\N
OpenAI	text-embedding-3-large	embedding	preset		t	0	0	\N
TongYi	qwen-max	chat	preset		t	0	0	\N
TongYi	qwen-plus	chat	preset		t	0	0	\N
TongYi	qwen-turbo	chat	preset		t	0	0	\N
TongYi	text-embedding-v4	embedding	preset		t	0	0	\N
TongYi	qwen3-vl-embedding	embedding	preset		t	0	0	\N
TongYi	qwen3-rerank	rerank	preset		t	0	0	\N
TongYi	qwen3-vl-rerank	rerank	preset		t	0	0	\N
TongYi	gte-rerank-v2	rerank	preset		t	0	0	\N
ZhiPu	glm-5.1	chat	preset		t	0	0	\N
ZhiPu	glm-5v-turbo	chat	preset		t	0	0	\N
ZhiPu	glm-image	image	preset		t	0	0	\N
ZhiPu	cogvideox-3	video	preset		t	0	0	\N
ZhiPu	glm-tts	speech	preset		t	0	0	\N
ZhiPu	glm-asr-2512	transcription	preset		t	0	0	\N
ZhiPu	embedding-3	embedding	preset		t	0	0	\N
Gemini	gemini-3.5-flash	chat	preset		t	0	0	\N
Gemini	gemini-3.1-pro-preview	chat	preset		t	0	0	\N
Gemini	gemini-3.1-flash-image	image	preset		t	0	0	\N
Gemini	veo-3.1-preview	video	preset		t	0	0	\N
Gemini	gemini-3.1-flash-tts-preview	speech	preset		t	0	0	\N
Gemini	lyria-3-pro-preview	music	preset		t	0	0	\N
Gemini	gemini-embedding-2	embedding	preset		t	0	0	\N
MiniMax	MiniMax-M2.7	chat	preset		t	0	0	\N
MiniMax	image-01	image	preset		t	0	0	\N
MiniMax	MiniMax-Hailuo-2.3	video	preset		t	0	0	\N
MiniMax	speech-2.8-hd	speech	preset		t	0	0	\N
MiniMax	music-2.6	music	preset		t	0	0	\N
Suno	V4	music	preset		t	0	0	\N
Suno	V4_5	music	preset		t	0	0	\N
Suno	V5	music	preset		t	0	0	\N
Grok	grok-4.5	chat	preset		t	0	0	\N
Grok	grok-4.3	chat	preset		t	0	0	\N
Grok	grok-imagine-image-quality	image	preset		t	0	0	\N
Grok	grok-imagine-video	video	preset		t	0	0	\N
StableDiffusion	stable-image-ultra	image	preset		t	0	0	\N
StableDiffusion	stable-image-core	image	preset		t	0	0	\N
StableDiffusion	sd3.5-large	image	preset		t	0	0	\N
Midjourney	midjourney-v8.1	image	preset		t	0	0	\N
Midjourney	midjourney-v7	image	preset		t	0	0	\N
Midjourney	niji-7	image	preset		t	0	0	\N
DeepSeek	deepseek-v4-flash	chat	discover	https://api.deepseek.com/models	t	1784257814339	0	\N
DeepSeek	deepseek-v4-pro	chat	discover	https://api.deepseek.com/models	t	1784257814339	0	\N
DouBao	doubao-seedream-5-0-lite-260128	image	preset	https://www.volcengine.com/docs/82379/1541523	t	0	0	\N
DouBao	doubao-pro-4k-functioncall-240615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-128k-240428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-128k-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-4k-240328	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-32k-240428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-4k-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-4k-character-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-text-240515	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	mistral-7b-instruct-v0.2	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-4k-character-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-4k-functioncall-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-4k-pretrain-character-240516	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-character-240528	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-4k-browsing-240524	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-functioncall-240515	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-browsing-240615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-240615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-32k-240628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-128k-240628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-text-240715	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-4k-character-240728	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-functioncall-240815	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-4k-character-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-character-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-32k-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-128k-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-browsing-240828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-functioncall-preview	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-large-text-240915	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-32k-character-241015	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-functioncall-241028	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-browsing-241115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-vision-pro-32k-241028	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	hyper3d-gen2-260112	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-vision-lite-32k-241015	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seaweed-241128	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-256k-241115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-character-241215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-pro-32k-241215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-lite-32k-250115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-pro-32k-250115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-vision-pro-32k-250115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-vision-241215	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-pro-256k-250115	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v3-241226	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-r1-distill-qwen-7b-250120	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-r1-distill-qwen-32b-250120	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-r1-250120	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-pro-32k-character-250228	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1.5-vision-lite-250315	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v3-250324	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1.5-vision-pro-250328	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-lite-32k-character-250228	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1.5-ui-tars-250328	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-vision-250328	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-thinking-pro-250415	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	wan2-1-14b-i2v-250225	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	wan2-1-14b-t2v-250225	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-thinking-pro-m-250415	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-1-0-lite-i2v-250428	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-1-0-lite-t2v-250428	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedream-3-0-t2i-250415	image	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	wan2-1-14b-flf2v-250417	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-thinking-vision-pro-250428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-thinking-pro-m-250428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-large-text-250515	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-ui-tars-250428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-r1-250528	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-flash-250615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-250615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-thinking-250615	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-1-0-pro-250528	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-vision-250615	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-thinking-250715	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-1-5-pro-32k-character-250715	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seededit-3-0-i2i-250628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-flash-250715	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	kimi-k2-250711	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-vision-250815	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v3-1-250821	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-flash-250828	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	glm-4-5-air-20250728	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	qwen3-8b-20250429	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	qwen3-32b-20250429	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	qwen2-5-72b-20240919	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedream-4-0-250828	image	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	kimi-k2-250905	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-translation-250915	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v3-1-terminus	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-smart-router-250928	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-251015	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-1-0-pro-fast-251015	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-6-lite-251015	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed3d-1-0-250928	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	kimi-k2-thinking-251104	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-code-preview-251028	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	qwen3-0-6b-20250429	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	qwen3-14b-20250429	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedream-4-5-251128	image	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-embedding-vision-251215	embedding	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v3-2-251201	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-1-5-pro-251215	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	glm-4-7-251222	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-1-8-251228	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-character-251128	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-lite-260215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-2-0-260128	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedream-5-0-260128	image	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-mini-260215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-pro-260215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-2-0-fast-260128	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-code-preview-260215	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	hitem3d-2-0-251223	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed3d-2-0-260328	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-mini-260428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-0-lite-260428	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v4-pro-260425	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	deepseek-v4-flash-260425	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedance-2-0-mini-260615	video	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-1-pro-260628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-2-1-turbo-260628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-character-260628	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seed-evolving	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	glm-5-2-260617	chat	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
DouBao	doubao-seedream-5-0-pro-260628	image	discover	https://ark.cn-beijing.volces.com/api/v3/models	t	1784613884361	0	\N
\.


--
-- Data for Name: model_configs; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.model_configs (id, name, key, platform, type, model, api_key, url, status, config, create_time, update_time) FROM stdin;
1784249635985	豆包 · doubao-seedance-1-5-pro-251215	doubao-doubao-seedance-1-5-pro-251215	DouBao	video	doubao-seedance-1-5-pro-251215	9aeae737-7355-4fd6-b614-aa7296907242	https://ark.cn-beijing.volces.com/api/v3	0	null	1784249635985	1784255725674
1784256674777	豆包 · doubao-seedream-4-5-251128	doubao-doubao-seedream-4-5-251128	DouBao	image	doubao-seedream-4-5-251128	9aeae737-7355-4fd6-b614-aa7296907242	https://ark.cn-beijing.volces.com/api/v3	0	null	1784256674777	1784257129492
1784257814114	DeepSeek · deepseek-v4-pro	deepseek-deepseek-v4-pro	DeepSeek	chat	deepseek-v4-pro	sk-25ab8537e0974c20a69b0fc9bf8ebcf4	https://api.deepseek.com	0	null	1784257814114	1784257814114
\.


--
-- Data for Name: model_platforms; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.model_platforms (platform, label, default_url, supported_types, enabled, update_time) FROM stdin;
TongYi	通义千问	https://dashscope.aliyuncs.com/compatible-mode/v1	{chat,image,video,speech,embedding,rerank}	t	0
YiYan	文心一言	https://qianfan.baidubce.com/v2	{chat}	t	0
DeepSeek	DeepSeek	https://api.deepseek.com	{chat}	t	0
ZhiPu	智谱清言	https://open.bigmodel.cn/api/paas/v4	{chat,image,video,speech,transcription,embedding}	t	0
XingHuo	讯飞星火	https://spark-api-open.xf-yun.com/v1	{chat}	t	0
DouBao	豆包	https://ark.cn-beijing.volces.com/api/v3	{chat,image,video,speech,transcription,embedding}	t	0
HunYuan	腾讯混元	https://api.hunyuan.cloud.tencent.com/v1	{chat}	t	0
SiliconFlow	硅基流动	https://api.siliconflow.cn/v1	{chat,image,video,speech,embedding,rerank}	t	0
MiniMax	MiniMax	https://api.minimax.chat/v1	{chat,image,video,speech,music}	t	0
Moonshot	Kimi	https://api.moonshot.cn/v1	{chat}	t	0
BaiChuan	百川智能	https://api.baichuan-ai.com/v1	{chat}	t	0
StepFun	阶跃星辰	https://api.stepfun.com/v1	{chat,image,speech,transcription}	t	0
OpenAI	OpenAI 官方	https://api.openai.com/v1	{chat,image,video,speech,transcription,embedding}	t	0
AzureOpenAI	微软 Azure（OpenAI）		{chat}	t	0
Anthropic	Claude	https://api.anthropic.com/v1	{chat}	t	0
Gemini	Gemini	https://generativelanguage.googleapis.com/v1beta	{chat,image,video,speech,music,embedding}	t	0
Ollama	Ollama 本地模型	http://127.0.0.1:11434/v1	{chat,embedding}	t	0
StableDiffusion	Stable Diffusion	https://api.stability.ai	{image}	t	0
Midjourney	Midjourney		{image}	t	0
Suno	Suno 音乐		{music}	t	0
Grok	Grok	https://api.x.ai/v1	{chat,image,video,speech,transcription}	t	0
OpenAICompatible	OpenAI 兼容平台		{chat,image,video,speech,transcription,music,embedding,rerank}	t	0
\.


--
-- Data for Name: music; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.music (id, user_id, model_id, title, lyric, image_url, audio_url, video_url, status, gpt_description_prompt, prompt, platform, model, generate_mode, tags, duration, public_status, task_id, error_message, create_time, finish_time, poll_count, last_poll_time) FROM stdin;
\.


--
-- Data for Name: tools; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.tools (id, name, description, status, input_schema, executor, create_time, update_time) FROM stdin;
1	current_time	获取指定时区的当前时间	1	{"type": "object", "required": ["utcOffset"], "properties": {"utcOffset": {"type": "string"}}}	{"kind": "builtin"}	0	0
2	weather_query	查询指定地点的天气	1	{"type": "object", "required": ["location"], "properties": {"location": {"type": "string"}}}	{"kind": "builtin"}	0	0
1784300000001	cmdb_model_list	列出 CMDB 中所有已启用的模型及其属性数和实例数	1	{"type": "object", "required": [], "properties": {}}	{"kind": "rust"}	1790506719106	1790506719106
1784300000002	cmdb_instance_query	按模型编码查询 CMDB 配置项实例，可带关键词过滤实例属性内容	1	{"type": "object", "required": ["model_code"], "properties": {"limit": {"type": "integer", "description": "返回条数，默认 10，最大 20"}, "keyword": {"type": "string", "description": "属性内容关键词，可选"}, "model_code": {"type": "string", "description": "CMDB 模型编码，如 server / cloud_ecs"}}}	{"kind": "rust"}	1790506719106	1790506719106
1784300000003	asset_query	查询资产采集台账（infra_asset），支持名称/IP/单位/应用关键词	1	{"type": "object", "required": [], "properties": {"limit": {"type": "integer", "description": "返回条数，默认 10，最大 20"}, "keyword": {"type": "string", "description": "名称、IP、单位或应用关键词，可选"}}}	{"kind": "rust"}	1790506719106	1790506719106
1784300000004	ticket_query	查询资源开通工单，可按状态（pending_approval/pending_provision/pending_delivery/delivered/rejected）和关键词过滤	1	{"type": "object", "required": [], "properties": {"limit": {"type": "integer", "description": "返回条数，默认 10，最大 20"}, "status": {"type": "string", "description": "工单状态，可选"}, "keyword": {"type": "string", "description": "主机名或应用名关键词，可选"}}}	{"kind": "rust"}	1790506719106	1790506719106
1784300000005	ticket_create	创建云主机开通工单（ecs）。命中自动审批规则时会直接进入待配置状态；开通执行需在工单页进行或由自动规则触发	1	{"type": "object", "required": ["ecs_name"], "properties": {"ecs_os": {"type": "string", "description": "操作系统镜像，可选"}, "ecs_name": {"type": "string", "description": "主机名"}, "ecs_type": {"type": "string", "description": "规格，可选"}, "applicant": {"type": "string", "description": "申请人，可选"}, "cpu_cores": {"type": "integer", "description": "CPU 核数，默认 4"}, "memory_gb": {"type": "integer", "description": "内存 GB，默认 8"}, "cloud_region": {"type": "string", "description": "地域，可选"}, "cloud_category": {"type": "string", "description": "云类别（demo/aliyun/tencent），可选"}, "resource_count": {"type": "integer", "description": "数量，默认 1"}, "application_name": {"type": "string", "description": "所属应用，可选"}}}	{"kind": "rust"}	1790506719106	1790506719106
\.


--
-- Data for Name: writes; Type: TABLE DATA; Schema: ai; Owner: -
--

COPY ai.writes (id, user_id, model_id, type, prompt, original_content, length, format, tone, language, platform, model, generated_content, error_message, create_time, finish_time) FROM stdin;
\.


--
-- Data for Name: _sqlx_migrations; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time) FROM stdin;
1	initial	2026-09-27 18:58:38.514056+08	t	\\x8860dd58a2513c79f25b099e1d588be3498805ab6669631aee10cde0fca29f295b9c3034c032e242659903866fcb0692	485362225
2	asset management	2026-09-27 18:58:39.001811+08	t	\\xe05a530c57ef242cfeea36462a3eea8fb8fbd23632897e6edf94848e30157ff4cc0628ffb2364c007aa0a55c65774a21	22741866
3	kairos asset permissions	2026-09-27 18:58:39.025521+08	t	\\xc66a44c1446984d92c51f282d1e55f28c56bfd9ef6556bc377303aacbaaaf0d1e4e2d1517673213818dd0d94acdcc342	8654655
4	remove baseline runtime messages	2026-09-27 18:58:39.034748+08	t	\\x75f1d354d4cfd4101236a4099b430401a5e3223328b6f695b3eb70f804fa2cf42bd1a6ea9174988d80d1b9c7c7fe2913	2726417
5	kairos ticket workflow fields	2026-09-27 18:58:39.038088+08	t	\\x15445c4f9dd94234ffb3ebfe67028537228d3b0c940848c3c0dca6727e7ba0a4c78e65820e7105e30262dd0dc5746d16	5075526
6	grant asset ops to super admin	2026-09-27 18:58:39.043865+08	t	\\xaf8d8b195f38d418d2696fb7d43ec43492571bcd668361f6f7f2b3dc4ab0dbcaa9598c90fd4116006da12ab55ccd00c3	8275821
7	asset inventory and network policy	2026-09-27 18:58:39.052932+08	t	\\x7a7d6850f0c5082ec1b40f22770709cf77b94e76fc6a982cecb37f9dea8ce85bd22faa4437361e040ce89591619505ef	8616933
8	drop toonflow business	2026-09-27 18:58:39.062029+08	t	\\x5c9e7c3801e2ee15aa0be2d126315084abd2ddddce5960981c4bac67387e7d990face46fbf49a1c98d1b847d69eae796	25272000
9	cmdb core	2026-09-27 18:58:39.088447+08	t	\\x964cc976706a22a71d731783025e59f01e0974f95b0005c84619699196a744fcde8429ee10abe0b921fd1edc398db195	11618086
10	tofu provision and approval rules	2026-09-27 18:58:39.100851+08	t	\\x07e0b0b93635741b5e45af5b4a51141ef1b384c68c2f629c794c72f5a64bcc934e671cf4f3d6b9c35b3bdf493bcdd946	4389655
11	ops agent tools	2026-09-27 18:58:39.105822+08	t	\\x17af5b9061d91050b559037d50909ef5db69b6eef5c39586dd40badd39ace9f085613c44c70ad68a0a4c6fc01be1b7cd	4277843
12	approval rule menu	2026-09-27 18:58:39.110657+08	t	\\x37aa46690fb6afed5c745d21c502dfdbe37e02c20c3b55e7fc28ba7cbe7345cf707cfa31ff49b0baad6abc35c9b0c202	3666956
13	cmdb roles and net zones	2026-09-27 18:58:39.114963+08	t	\\x45df944c33090eb9c89ffac693808faa98895ed0d24d3bacf12b1a454e7d648649352dec2f049bb8a54721c8710ebcf3	7420398
14	fix cmdb permission codes	2026-09-27 18:58:39.12294+08	t	\\xf7960334257fbb6ae9098d0cd9767c97b0de7eb043f0ba9d0e2875a31916b04c23125c7194aae747e4af0feb46c8a158	4036817
15	split asset operations menu	2026-09-27 18:58:39.127499+08	t	\\xbdc020e1be29c25139ebc1949a8ad93610c7e337e5ffbe204b900716f78f7603d6da90d04c04e6ed284dbd042b8e48d8	6745503
16	remove bpm and scope tenant net zones	2026-09-27 18:58:39.135334+08	t	\\x02321d041095fdf24d09ec2e7e7a91956f4f537ed03da6284fc5765b3ac50b5022bd0098748fd683510098d6dcc8abea	6111214
17	rebrand baseline data	2026-09-27 18:58:39.141962+08	t	\\xda12a4845170ff9ed12ebf191e155426723b5e125b782bc66e44be082e18e4f7bfbcc9233502da26b65223e1d4094ad2	3343297
18	gm crypto and identity	2026-09-27 18:58:39.145854+08	t	\\x7ac129ddfca4d58aa79a57557a14258c30e3865f1d405f6d873455e35873dd47c5cabfdd5ff91281bddccd5f38f76282	5046955
19	remove demo analytics menu	2026-09-27 18:58:39.151558+08	t	\\xd5694bb55a38446f2217c3818018d59c898b8c2b3b8bbd95d16fa4f9d69a6261e1855f32079d396f72a68643a39c8a09	2459789
20	split business resource menu	2026-09-27 18:58:39.154658+08	t	\\xdd2c4d9600e424923d3c256dd4c56d281d0a1ac84ef7c749d19006a050c17c3541a2dd59d3eba5f0efc3d21a302b1cbd	2861578
21	split resource tables	2026-09-27 18:58:39.158178+08	t	\\x7a3109c40576cab6b36ac1e1736d6ef67e3a1bf260080719e93ba0be7b0d607b29b907484a0767712684348c8b697ec6	11755213
22	asset inspection	2026-09-27 18:58:39.170703+08	t	\\x4c01a6eb663e1bf4d6269e373ebebf14049784cbf8cff77fcd0b44a49c69584f9606ebb44d0e1d69903ab7e34fc64d4d	6716990
23	retire codegen demo menus	2026-09-27 18:58:39.178077+08	t	\\x14079da493e20ac0f88225d5d3d31841219de31da8653d284b45e1eba126a64713cfd2311f1147c519cdf16f47f64b28	3857790
24	rename api docs menu	2026-09-27 18:58:39.182363+08	t	\\xe63206d0221c83250952ea7a67a597856e6060dcf1162a6244acf7ddc2ebda878e6b26fd94016ae38a9a3f335d097460	1978579
25	cmdb instance unique values	2026-09-27 18:58:39.184996+08	t	\\x7906d8d2d2cb9e2f35f3cebfc10e8facc24524d20cd56b06d55ff4caf2fac58c3b86ee3672287240f11ad25ba5e01fc9	4362530
26	cmdb tenant isolation	2026-09-27 18:58:39.190046+08	t	\\xcee55302d791554220b47bbc9eaf69bd5c919233c0f13bfc432f71e95b53518bc521ca7176cc00d813deaafb96e5d4a1	6438170
27	asset policy tenants	2026-09-27 18:58:39.197156+08	t	\\x38e8de9796c33ac2eb60e20c3ad2f1d47d6be3f4d3d87780c933d1142a9a68a34ae521eaf092af501941c9a819c6a070	7700627
28	network policy port risk	2026-09-27 18:58:39.205511+08	t	\\x4e753d6453de580ffcf03fdc31017e391ff138a7947ed2865c6a1d74bdbf1c82bef17176d1c956ce04d1340fe6a1a3a8	7215128
29	asset tenant unique ip	2026-09-27 18:58:39.213446+08	t	\\x2d8f6ad857dace6e18957d630c8b65ba618230a9a3815c63368269342008244f2458b2debf68ccf4c435c1521d6b68ac	2535109
30	recoverable scan tasks	2026-09-27 18:58:39.216496+08	t	\\x4f7980409db12c481c63eb474da1890adf58ac4d2c16621121e91faef755227ba774738899d035172f99c7e2797120b3	4908573
\.


--
-- Data for Name: cmdb_attribute; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_attribute (id, model_id, name, code, attr_type, required, choices, default_value, show_in_list, sort, status, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: cmdb_instance; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_instance (id, model_id, attributes, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: cmdb_instance_unique_value; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_instance_unique_value (instance_id, model_id, value, tenant_id) FROM stdin;
\.


--
-- Data for Name: cmdb_model; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_model (id, name, code, description, icon, unique_key, sort, status, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: cmdb_net_zone; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_net_zone (id, name, parent_id, zone_type, cidr, sort, description, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: cmdb_relation; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.cmdb_relation (id, source_id, target_id, relation, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_api_access_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_api_access_log (id, trace_id, user_id, user_type, application_name, request_method, request_url, request_params, response_body, user_ip, user_agent, operate_module, operate_name, operate_type, begin_time, end_time, duration, result_code, result_msg, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_api_error_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_api_error_log (id, trace_id, user_id, user_type, application_name, request_method, request_url, request_params, user_ip, user_agent, exception_time, exception_name, exception_message, exception_root_cause_message, exception_stack_trace, exception_class_name, exception_file_name, exception_method_name, exception_line_number, process_status, process_time, process_user_id, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_application_endpoint; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_application_endpoint (id, business_application_id, protocol, dest_ip, nat_ip, dest_port, domain, created_by, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_approval_rule; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_approval_rule (id, name, resource_type, max_cpu_cores, max_memory_gb, max_resource_count, auto_provision, status, remarks, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_asset; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_asset (id, name, ip, zone, ports, last_scanned, contact_person, contact_phone, created_by, updated_by, owner, weight, labels, os, device_type, creator, create_time, updater, update_time, deleted, city, district, organization_name, business_department, department_contact, application_name, server_name, hardware_configuration, operating_system, database_type, launch_date, decommission_date, application_type, network_environment, internet_ipv4, internet_ipv6, domain_address, internal_network_ip, government_extranet_ip, open_ports, publishing_endpoint, publishes_other_endpoint, other_endpoint_name, security_product_installation, development_vendor, development_vendor_contact, security_vendor, security_vendor_contact, operations_vendor, operations_vendor_contact, classified_protection_level, classified_protection_assessed, classified_protection_assessor, classified_protection_assessment_date, classified_protection_score, classified_protection_filed, classified_protection_filing_date, classified_protection_filing_number, classified_protection_filing_authority, cryptography_assessed, cryptography_assessment_level, cryptography_assessment_date, cryptography_assessment_number, net_zone_id, ownership_source, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_business_application; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_business_application (id, name, description, created_by, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_cloud_asset; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_cloud_asset (id, cloud_provider_config_id, provider_type, platform_name, region_id, instance_id, name, status, private_ip, public_ip, cpu_cores, memory_gb, instance_type, os_name, expire_time, raw_payload, synced_at, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_cloud_platform; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_cloud_platform (id, zone_id, platform_name, platform_code, description, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_cloud_provider_config; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_cloud_provider_config (id, zone_id, platform_id, provider, region_id, region_name, available_zones, account_name, access_key_id, access_key_secret, status, remarks, last_test_time, last_test_result, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_cloud_resource; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_cloud_resource (id, ecs_name, ecs_status, resource_id, cloud_region, cloud_category, cloud_provider_config_id, zone_name, platform_name, county_city, vdc_name, customer_name, application_name, contract_name, instance_id, ecs_type, ecs_os, cpu_cores, memory_gb, system_disk, system_disk_size_gb, data_disk, completion_time, release_time, has_security_product, ip_address, ecs_login_method, ecs_login_username, ecs_initial_password, bastion_address, bastion_admin_account, bastion_initial_password, bandwidth_mbps, bandwidth_type, public_ip_count, network_type, billing_method, purchase_duration, cost_center, project_name, project_code, business_owner, tech_owner, contact_phone, remarks, application_status, delivery_status, delivery_confirmed_at, delivery_confirmed_by, applicant, department, approver, approval_time, approval_remarks, rejection_reason, security_level, data_sensitivity, purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_cloud_zone; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_cloud_zone (id, zone_name, zone_code, description, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_codegen_column; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_codegen_column (id, table_id, column_name, data_type, column_comment, nullable, primary_key, ordinal_position, java_type, java_field, create_operation, update_operation, list_operation, list_operation_result, html_type, creator, create_time, updater, update_time, deleted) FROM stdin;
1	1	id	bigint	id	f	t	1	Long	id	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
2	1	category	character varying	category	f	f	2	String	category	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
3	1	type	smallint	type	f	f	3	Integer	type	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
4	1	name	character varying	name	f	f	4	String	name	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
5	1	config_key	character varying	config_key	f	f	5	String	configKey	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
6	1	value	character varying	value	f	f	6	String	value	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
7	1	visible	boolean	visible	f	f	7	Boolean	visible	t	t	t	t	radio		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
8	1	remark	character varying	remark	t	f	8	String	remark	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
9	1	creator	character varying	creator	f	f	9	String	creator	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
10	1	create_time	timestamp without time zone	create_time	f	f	10	LocalDateTime	createTime	t	t	t	t	datetime		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
11	1	updater	character varying	updater	f	f	11	String	updater	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
12	1	update_time	timestamp without time zone	update_time	f	f	12	LocalDateTime	updateTime	t	t	t	t	datetime		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
13	1	deleted	smallint	deleted	f	f	13	Integer	deleted	t	t	t	t	input		2026-07-16 07:30:13.363579		2026-07-16 07:30:13.521669	1
14	1	id	bigint	id	f	t	1	Long	id	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
15	1	category	character varying	category	f	f	2	String	category	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
16	1	type	smallint	type	f	f	3	Integer	type	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
17	1	name	character varying	name	f	f	4	String	name	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
18	1	config_key	character varying	config_key	f	f	5	String	configKey	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
19	1	value	character varying	value	f	f	6	String	value	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
20	1	visible	boolean	visible	f	f	7	Boolean	visible	t	t	t	t	radio		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
21	1	remark	character varying	remark	t	f	8	String	remark	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
22	1	creator	character varying	creator	f	f	9	String	creator	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
23	1	create_time	timestamp without time zone	create_time	f	f	10	LocalDateTime	createTime	t	t	t	t	datetime		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
24	1	updater	character varying	updater	f	f	11	String	updater	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
25	1	update_time	timestamp without time zone	update_time	f	f	12	LocalDateTime	updateTime	t	t	t	t	datetime		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
26	1	deleted	smallint	deleted	f	f	13	Integer	deleted	t	t	t	t	input		2026-07-16 07:30:13.521669		2026-07-16 07:30:13.521669	0
\.


--
-- Data for Name: infra_codegen_table; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_codegen_table (id, data_source_config_id, scene, table_name, table_comment, module_name, business_name, class_name, class_comment, author, template_type, front_type, parent_menu_id, creator, create_time, updater, update_time, deleted) FROM stdin;
1	0	1	infra_config	infra_config	infra	config	InfraConfig	infra_config	admin	1	20	\N		2026-07-16 07:30:13.35742		2026-07-16 07:30:13.35742	0
\.


--
-- Data for Name: infra_config; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_config (id, category, type, name, config_key, value, visible, remark, creator, create_time, updater, update_time, deleted) FROM stdin;
1	test	1	Codex Test2	codex.test.updated.1784190150145	2	t	smoke		2026-07-16 08:22:29.993205		2026-07-16 08:22:30.328864	1
2	test	1	Codex Test2	codex.test.updated.1784190297896	2	t	smoke		2026-07-16 08:24:57.790394		2026-07-16 08:24:58.074263	1
\.


--
-- Data for Name: infra_data_source_config; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_data_source_config (id, name, url, username, password, creator, create_time, updater, update_time, deleted) FROM stdin;
1	Codex DS2	jdbc:postgresql://localhost/test2	u2	enc:sm4:v2:PcfyYNzKJb/N5U/tCQt4feabqbQK0yxYGdINBJDaAWCLTtgWTb7kkTY+7uEoF/11		2026-07-16 08:22:30.56435		2026-07-16 08:22:30.951151	1
2	Codex DS2	jdbc:postgresql://localhost/test2	u2	enc:sm4:v2:t12zrkO0dDzCTRWAWB317thkd5P/51APYmnJcon+xOs/5pOA6aokJFLcOl2BTYFb		2026-07-16 08:24:58.257105		2026-07-16 08:24:58.52636	1
\.


--
-- Data for Name: infra_file; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_file (id, config_id, name, path, url, type, size, creator, create_time, updater, update_time, deleted) FROM stdin;
1	\N	upload-1784190151596	/upload/upload-1784190151596	/upload/upload-1784190151596	application/octet-stream	0		2026-07-16 08:22:31.598394		2026-07-16 08:22:31.811991	1
2	\N	hello.txt	/upload/20260716/1784190299062_hello.txt	/upload/20260716/1784190299062_hello.txt	text/plain	11		2026-07-16 08:24:59.066129		2026-07-16 08:24:59.323691	1
3	\N	hello.txt	/upload/20260716/1784190421868_hello.txt	/upload/20260716/1784190421868_hello.txt	text/plain	11		2026-07-16 08:27:01.871695		2026-07-16 08:27:02.075376	1
\.


--
-- Data for Name: infra_file_config; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_file_config (id, name, storage, master, config, remark, creator, create_time, updater, update_time, deleted) FROM stdin;
1	Codex Local	10	t	{"basePath":"/tmp/rustset-upload"}	smoke		2026-07-16 08:22:31.17725		2026-07-16 08:22:31.911776	1
2	Codex Local	10	t	{"basePath":"storage/uploads"}	smoke		2026-07-16 08:24:58.710186		2026-07-16 08:24:59.430503	1
3	Codex Local Final	10	f	{"basePath":"storage/uploads"}	smoke		2026-07-16 08:27:01.760067		2026-07-16 08:27:02.170941	1
\.


--
-- Data for Name: infra_high_risk_port_rule; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_high_risk_port_rule (id, name, protocol, port_start, port_end, severity, description, solution, enabled, creator, create_time, updater, update_time, deleted) FROM stdin;
1	FTP 明文服务	tcp	21	21	High	FTP 使用明文认证与传输，暴露后易造成凭据和数据泄露。	关闭公网访问；确需使用时限制可信源地址并迁移到 SFTP。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
2	SSH 远程管理	tcp	22	22	High	远程管理端口暴露会增加口令爆破和未授权访问风险。	限制到运维网或堡垒机，启用密钥认证和多因素认证。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
3	Telnet 明文管理	tcp	23	23	Critical	Telnet 以明文传输认证信息，不应跨不可信网络开放。	立即关闭 Telnet 并迁移到 SSH，通过管理网或堡垒机访问。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
4	Windows RPC	tcp	135	135	High	RPC 端口暴露可能泄露主机信息并扩大远程攻击面。	限制到必要的管理网段，并通过主机和边界防火墙双重控制。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
5	NetBIOS	any	137	139	High	NetBIOS 服务暴露会泄露名称与共享信息，并增加横向移动风险。	关闭跨安全域访问；无法关闭时仅允许受控内网源地址。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
6	SMB 文件共享	tcp	445	445	Critical	SMB 暴露存在勒索软件传播、凭据窃取和远程利用风险。	禁止公网和非必要跨域访问，限制可信源并及时修补系统。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
7	Microsoft SQL Server	tcp	1433	1433	High	数据库端口暴露会增加爆破、数据泄露和漏洞利用风险。	仅允许应用服务器网段访问，禁止用户网与公网直连。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
8	Oracle Database	tcp	1521	1521	High	数据库监听端口暴露会增加未授权访问和数据泄露风险。	仅允许指定应用源地址访问，并启用数据库访问审计。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
9	NFS 文件共享	any	2049	2049	High	NFS 暴露可能导致未授权挂载和敏感文件泄露。	限制到必要的受控主机，核查导出权限并禁用匿名映射。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
10	Docker Remote API	tcp	2375	2375	Critical	未加密 Docker API 可能允许直接控制主机和容器。	关闭 2375；如需远程管理，使用双向 TLS 的 2376 并限制源地址。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
11	MySQL	tcp	3306	3306	High	数据库端口暴露会增加爆破、数据泄露和漏洞利用风险。	仅允许指定应用网段访问，使用最小权限账号并启用审计。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
12	RDP 远程桌面	tcp	3389	3389	Critical	远程桌面暴露容易遭受口令爆破、凭据攻击和远程利用。	仅通过 VPN 或堡垒机访问，限制源地址并启用网络级认证和多因素认证。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
13	PostgreSQL	tcp	5432	5432	High	数据库端口暴露会增加未授权访问和数据泄露风险。	仅允许指定应用服务器访问，并核查 pg_hba.conf 与账号权限。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
14	VNC 远程控制	tcp	5900	5900	Critical	VNC 暴露可能导致弱口令攻击、会话窃听和远程控制。	关闭直接暴露，通过堡垒机或 VPN 访问并启用强认证。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
15	Redis	tcp	6379	6379	Critical	Redis 暴露可能导致未授权读写、数据泄露或主机命令执行。	绑定内网地址，启用认证和 ACL，仅允许必要应用源地址访问。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
16	Elasticsearch	tcp	9200	9200	High	Elasticsearch 接口暴露可能导致索引数据泄露和未授权操作。	启用认证与 TLS，仅允许受控应用和运维网段访问。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
17	Memcached	any	11211	11211	Critical	Memcached 暴露可能泄露缓存数据，并被用于反射放大攻击。	绑定内网地址，关闭 UDP，并限制为必要应用源地址。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
18	MongoDB	tcp	27017	27017	Critical	MongoDB 暴露可能导致未授权访问和业务数据泄露。	启用认证与 TLS，仅允许指定应用服务器访问。	t		2026-09-27 10:58:39.205511		2026-09-27 10:58:39.205511	0
\.


--
-- Data for Name: infra_inspection_baseline; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_inspection_baseline (ip, allowed_ports, reason, updated_by, update_time, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_inspection_result; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_inspection_result (id, task_id, ip, registered, baseline_ports, open_ports, uncertain_ports, differences, risk_ids, create_time, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_job; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_job (id, name, status, handler_name, handler_param, cron_expression, retry_count, retry_interval, monitor_timeout, creator, create_time, updater, update_time, deleted) FROM stdin;
1	Codex Job2	0	codexHandler	{}	0 0/10 * * * ?	1	1	10		2026-07-16 08:22:32.197157		2026-07-16 08:22:32.899056	1
2	Codex Job2	1	codexHandler	{}	0 0/10 * * * ?	1	1	10		2026-07-16 08:24:59.610099		2026-07-16 08:25:00.250999	1
3	Codex Job Final	1	codexHandler	{}	0 0/10 * * * ?	0	0	0		2026-07-16 08:27:02.268192		2026-07-16 08:27:02.541902	1
\.


--
-- Data for Name: infra_job_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_job_log (id, job_id, handler_name, handler_param, execute_index, begin_time, end_time, duration, status, result, creator, create_time, updater, update_time, deleted) FROM stdin;
1	1	codexHandler	{}	1	2026-07-16 08:22:32.594164	2026-07-16 08:22:32.594164	0	1	manual trigger		2026-07-16 08:22:32.597449		2026-07-16 08:22:32.597449	0
2	2	codexHandler	{}	1	2026-07-16 08:24:59.973995	2026-07-16 08:24:59.973995	0	1	manual trigger		2026-07-16 08:24:59.975323		2026-07-16 08:24:59.975323	0
\.


--
-- Data for Name: infra_machine_room; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_machine_room (id, room_name, room_code, facility_type, address, provider_id, room_type, contact_person, contact_phone, floor, cabinet_count, area_size, remarks, status, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_network_policy; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_network_policy (id, firewall_name, destination_organization, destination_project, source_organization, source_project, source_security_zone, source_ip, destination_security_zone, destination_ip, service_port, applicant, application_date, traffic_direction, action, implementer, implementation_date, delivery_date, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_network_zone; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_network_zone (id, name, cidr, priority, cloud_platform_id, cloud_platform_name, machine_room_id, machine_room_name, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_physical_resource; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_physical_resource (id, ecs_name, ecs_status, cloud_region, cloud_category, customer_name, machine_room_id, deployment_type, management_ip, business_ip, network_cidr, gateway, vlan_id, dns_servers, mac_address, serial_number, hardware_model, rack_location, warranty_expiry, agent_status, ipmi_address, cpu_cores, memory_gb, has_security_product, application_name, contract_name, completion_time, release_time, project_name, project_code, business_owner, tech_owner, contact_phone, remarks, application_status, delivery_status, delivery_confirmed_at, delivery_confirmed_by, applicant, department, approver, approval_time, approval_remarks, rejection_reason, security_level, data_sensitivity, purpose, expected_delivery_time, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_resource_ticket; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_resource_ticket (id, resource_type, ecs_name, ticket_status, provider_id, provider_name, cloud_platform_id, cloud_platform_name, machine_room_id, machine_room_name, cloud_region, cloud_category, zone_name, zone_cabinet, rack_units, customer_name, application_name, application_endpoint_id, application_domain, contract_name, ecs_type, ecs_os, resource_count, cpu_cores, memory_gb, system_disk, system_disk_size_gb, data_disk, expire_at, has_security_product, security_products, ip_address, delivery_status, remarks, created_by, applicant_name, organization_id, organization_name, department_id, department_name, approver, approve_time, approve_comment, provisioner, provision_time, provision_details, deliverer, deliver_time, deliver_comment, fw_source_zone, fw_source_address, fw_source_port, fw_dest_zone, fw_dest_address, fw_dest_port, fw_protocol, fw_port, fw_direction, fw_valid_until, fw_firewall_name, creator, create_time, updater, update_time, deleted, ticket_type, risk_level, target_resource_id, target_config, maintenance_window, allow_interruption, backup_confirmed, rollback_plan, retention_until, approval_stage, approval_total, current_approval_role, apply_status, apply_log, tf_outputs, tofu_workspace, target_resource_type, tenant_id) FROM stdin;
\.


--
-- Data for Name: infra_risk; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_risk (id, asset_ip, port, severity, description, solution, status, assigned_to, creator, create_time, updater, update_time, deleted, inspection_key, tenant_id, source_type, source_id, rule_id) FROM stdin;
\.


--
-- Data for Name: infra_security_product; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_security_product (id, name, category, vendor, model, version, serial_number, license_type, license_expiry, management_ip, deployment_mode, cloud_platform_id, machine_room_id, provider_id, status, features, throughput, contact_person, contact_phone, remarks, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_service_provider; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_service_provider (id, provider_name, provider_code, short_name, logo_url, contact_person, contact_phone, contact_email, headquarters, service_area, business_license, remarks, status, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: infra_task; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.infra_task (id, name, target, status, start_time, end_time, found_assets, found_risks, port_policy, domain_brute, service_detection, os_detection, site_identify, created_by, creator, create_time, updater, update_time, deleted, task_kind, scan_ports, total_targets, completed_targets, error_message, tenant_id, payload, idempotency_key, attempt_count, max_attempts, timeout_seconds, next_attempt_at, lease_owner, lease_expires_at, heartbeat_at, cancel_requested) FROM stdin;
\.


--
-- Data for Name: system_dept; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_dept (id, name, parent_id, sort, leader_user_id, phone, email, status, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
110	新部门	0	1	\N	\N	\N	0	110	2022-02-23 20:46:30	110	2022-02-23 20:46:30	0	121
111	顶级部门	0	1	\N	\N	\N	0	113	2022-03-07 21:44:50	113	2022-03-07 21:44:50	0	122
112	产品部门	101	100	1	\N	\N	1	1	2023-12-02 09:45:13	1	2023-12-02 09:45:31	0	1
113	支持部门	102	3	104	\N	\N	1	1	2023-12-02 09:47:38	1	2025-03-29 15:00:56	0	1
116	某个子部门	0	1	\N	\N	\N	0	1	2025-12-08 14:51:12	1	2025-12-08 14:51:12	0	1
117	某个子部门 2	0	2	\N	\N	\N	0	1	2025-12-08 14:51:25	1	2025-12-08 14:51:25	0	1
101	深圳总公司	100	1	104	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
102	长沙分公司	100	2	\N	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
103	研发部门	101	1	104	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
104	市场部门	101	2	\N	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
105	测试部门	101	3	\N	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
106	财务部门	101	4	103	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
107	运维部门	101	5	1	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
108	市场部门	102	1	\N	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
109	财务部门	102	2	\N	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
100	RustSet	0	0	1	15888888888		0	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1
\.


--
-- Data for Name: system_dict_data; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_dict_data (id, sort, label, value, dict_type, status, color_type, css_class, remark, creator, create_time, updater, update_time, deleted) FROM stdin;
1	1	男	1	system_user_sex	0	primary	A	性别男	admin	2021-01-05 17:03:48	1	2025-12-10 13:19:26	0
2	2	女	2	system_user_sex	0	success		性别女	admin	2021-01-05 17:03:48	1	2023-11-15 23:30:37	0
8	1	正常	1	infra_job_status	0	success		正常状态	admin	2021-01-05 17:03:48	1	2022-02-16 19:33:38	0
9	2	暂停	2	infra_job_status	0	danger		停用状态	admin	2021-01-05 17:03:48	1	2022-02-16 19:33:45	0
12	1	系统内置	1	infra_config_type	0	danger		参数类型 - 系统内置	admin	2021-01-05 17:03:48	1	2022-02-16 19:06:02	0
13	2	自定义	2	infra_config_type	0	primary		参数类型 - 自定义	admin	2021-01-05 17:03:48	1	2022-02-16 19:06:07	0
14	1	通知	1	system_notice_type	0	success		通知	admin	2021-01-05 17:03:48	1	2022-02-16 13:05:57	0
15	2	公告	2	system_notice_type	0	info		公告	admin	2021-01-05 17:03:48	1	2022-02-16 13:06:01	0
16	0	其它	0	infra_operate_type	0	default		其它操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:19	0
17	1	查询	1	infra_operate_type	0	info		查询操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:20	0
18	2	新增	2	infra_operate_type	0	primary		新增操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:21	0
19	3	修改	3	infra_operate_type	0	warning		修改操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:22	0
20	4	删除	4	infra_operate_type	0	danger		删除操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:23	0
22	5	导出	5	infra_operate_type	0	default		导出操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:24	0
23	6	导入	6	infra_operate_type	0	default		导入操作	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:25	0
27	1	开启	0	common_status	0	primary		开启状态	admin	2021-01-05 17:03:48	1	2022-02-16 08:00:39	0
28	2	关闭	1	common_status	0	info		关闭状态	admin	2021-01-05 17:03:48	1	2022-02-16 08:00:44	0
29	1	目录	1	system_menu_type	0			目录	admin	2021-01-05 17:03:48		2022-02-01 16:43:45	0
30	2	菜单	2	system_menu_type	0			菜单	admin	2021-01-05 17:03:48		2022-02-01 16:43:41	0
31	3	按钮	3	system_menu_type	0			按钮	admin	2021-01-05 17:03:48		2022-02-01 16:43:39	0
32	1	内置	1	system_role_type	0	danger		内置角色	admin	2021-01-05 17:03:48	1	2022-02-16 13:02:08	0
33	2	自定义	2	system_role_type	0	primary		自定义角色	admin	2021-01-05 17:03:48	1	2022-02-16 13:02:12	0
34	1	全部数据权限	1	system_data_scope	0			全部数据权限	admin	2021-01-05 17:03:48		2022-02-01 16:47:17	0
35	2	指定部门数据权限	2	system_data_scope	0			指定部门数据权限	admin	2021-01-05 17:03:48		2022-02-01 16:47:18	0
36	3	本部门数据权限	3	system_data_scope	0			本部门数据权限	admin	2021-01-05 17:03:48		2022-02-01 16:47:16	0
37	4	本部门及以下数据权限	4	system_data_scope	0			本部门及以下数据权限	admin	2021-01-05 17:03:48		2022-02-01 16:47:21	0
38	5	仅本人数据权限	5	system_data_scope	0			仅本人数据权限	admin	2021-01-05 17:03:48		2022-02-01 16:47:23	0
39	0	成功	0	system_login_result	0	success		登陆结果 - 成功		2021-01-18 06:17:36	1	2022-02-16 13:23:49	0
40	10	账号或密码不正确	10	system_login_result	0	primary		登陆结果 - 账号或密码不正确		2021-01-18 06:17:54	1	2022-02-16 13:24:27	0
41	20	用户被禁用	20	system_login_result	0	warning		登陆结果 - 用户被禁用		2021-01-18 06:17:54	1	2022-02-16 13:23:57	0
42	30	验证码不存在	30	system_login_result	0	info		登陆结果 - 验证码不存在		2021-01-18 06:17:54	1	2022-02-16 13:24:07	0
43	31	验证码不正确	31	system_login_result	0	info		登陆结果 - 验证码不正确		2021-01-18 06:17:54	1	2022-02-16 13:24:11	0
44	100	未知异常	100	system_login_result	0	danger		登陆结果 - 未知异常		2021-01-18 06:17:54	1	2022-02-16 13:24:23	0
45	1	是	true	infra_boolean_string	0	danger		Boolean 是否类型 - 是		2021-01-19 03:20:55	1	2022-03-15 23:01:45	0
46	1	否	false	infra_boolean_string	0	info		Boolean 是否类型 - 否		2021-01-19 03:20:55	1	2022-03-15 23:09:45	0
50	1	单表（增删改查）	1	infra_codegen_template_type	0			\N		2021-02-05 07:09:06		2022-03-10 16:33:15	0
51	2	树表（增删改查）	2	infra_codegen_template_type	0			\N		2021-02-05 07:14:46		2022-03-10 16:33:19	0
53	0	初始化中	0	infra_job_status	0	primary		\N		2021-02-07 07:46:49	1	2022-02-16 19:33:29	0
57	0	运行中	0	infra_job_log_status	0	primary		RUNNING		2021-02-08 10:04:24	1	2022-02-16 19:07:48	0
58	1	成功	1	infra_job_log_status	0	success		\N		2021-02-08 10:06:57	1	2022-02-16 19:07:52	0
59	2	失败	2	infra_job_log_status	0	warning		失败		2021-02-08 10:07:38	1	2022-02-16 19:07:56	0
60	1	会员	1	user_type	0	primary		\N		2021-02-26 00:16:27	1	2022-02-16 10:22:19	0
61	2	管理员	2	user_type	0	success		\N		2021-02-26 00:16:34	1	2025-04-06 18:37:43	0
62	0	未处理	0	infra_api_error_log_process_status	0	primary		\N		2021-02-26 07:07:19	1	2022-02-16 20:14:17	0
63	1	已处理	1	infra_api_error_log_process_status	0	success		\N		2021-02-26 07:07:26	1	2022-02-16 20:14:08	0
64	2	已忽略	2	infra_api_error_log_process_status	0	danger		\N		2021-02-26 07:07:34	1	2022-02-16 20:14:14	0
66	1	阿里云	ALIYUN	system_sms_channel_code	0	primary		\N	1	2021-04-05 01:05:26	1	2024-07-22 22:23:25	0
67	1	验证码	1	system_sms_template_type	0	warning		\N	1	2021-04-05 21:50:57	1	2022-02-16 12:48:30	0
68	2	通知	2	system_sms_template_type	0	primary		\N	1	2021-04-05 21:51:08	1	2022-02-16 12:48:27	0
69	0	营销	3	system_sms_template_type	0	danger		\N	1	2021-04-05 21:51:15	1	2022-02-16 12:48:22	0
70	0	初始化	0	system_sms_send_status	0	primary		\N	1	2021-04-11 20:18:33	1	2022-02-16 10:26:07	0
71	1	发送成功	10	system_sms_send_status	0	success		\N	1	2021-04-11 20:18:43	1	2022-02-16 10:25:56	0
72	2	发送失败	20	system_sms_send_status	0	danger		\N	1	2021-04-11 20:18:49	1	2022-02-16 10:26:03	0
73	3	不发送	30	system_sms_send_status	0	info		\N	1	2021-04-11 20:19:44	1	2022-02-16 10:26:10	0
74	0	等待结果	0	system_sms_receive_status	0	primary		\N	1	2021-04-11 20:27:43	1	2022-02-16 10:28:24	0
75	1	接收成功	10	system_sms_receive_status	0	success		\N	1	2021-04-11 20:29:25	1	2022-02-16 10:28:28	0
76	2	接收失败	20	system_sms_receive_status	0	danger		\N	1	2021-04-11 20:29:31	1	2022-02-16 10:28:32	0
77	0	调试(钉钉)	DEBUG_DING_TALK	system_sms_channel_code	0	info		\N	1	2021-04-13 00:20:37	1	2022-02-16 10:10:00	0
80	100	账号登录	100	system_login_type	0	primary		账号登录	1	2021-10-06 00:52:02	1	2022-02-16 13:11:34	0
81	101	社交登录	101	system_login_type	0	info		社交登录	1	2021-10-06 00:52:17	1	2022-02-16 13:11:40	0
83	200	主动登出	200	system_login_type	0	primary		主动登出	1	2021-10-06 00:52:58	1	2022-02-16 13:11:49	0
85	202	强制登出	202	system_login_type	0	danger		强制退出	1	2021-10-06 00:53:41	1	2022-02-16 13:11:57	0
86	0	病假	1	bpm_oa_leave_type	0	primary		\N	1	2021-09-21 22:35:28	1	2022-02-16 10:00:41	0
87	1	事假	2	bpm_oa_leave_type	0	info		\N	1	2021-09-21 22:36:11	1	2022-02-16 10:00:49	0
88	2	婚假	3	bpm_oa_leave_type	0	warning		\N	1	2021-09-21 22:36:38	1	2022-02-16 10:00:53	0
112	0	微信 Wap 网站支付	wx_wap	pay_channel_code	0	success		微信 Wap 网站支付	1	2023-07-19 20:08:06	1	2023-07-19 20:09:08	0
113	1	微信公众号支付	wx_pub	pay_channel_code	0	success		微信公众号支付	1	2021-12-03 10:40:24	1	2023-07-19 20:08:47	0
114	2	微信小程序支付	wx_lite	pay_channel_code	0	success		微信小程序支付	1	2021-12-03 10:41:06	1	2023-07-19 20:08:50	0
115	3	微信 App 支付	wx_app	pay_channel_code	0	success		微信 App 支付	1	2021-12-03 10:41:20	1	2023-07-19 20:08:56	0
116	10	支付宝 PC 网站支付	alipay_pc	pay_channel_code	0	primary		支付宝 PC 网站支付	1	2021-12-03 10:42:09	1	2023-07-19 20:09:12	0
117	11	支付宝 Wap 网站支付	alipay_wap	pay_channel_code	0	primary		支付宝 Wap 网站支付	1	2021-12-03 10:42:26	1	2023-07-19 20:09:16	0
118	12	支付宝 App 支付	alipay_app	pay_channel_code	0	primary		支付宝 App 支付	1	2021-12-03 10:42:55	1	2023-07-19 20:09:20	0
119	14	支付宝扫码支付	alipay_qr	pay_channel_code	0	primary		支付宝扫码支付	1	2021-12-03 10:43:10	1	2023-07-19 20:09:28	0
120	10	通知成功	10	pay_notify_status	0	success		通知成功	1	2021-12-03 11:02:41	1	2023-07-19 10:08:19	0
121	20	通知失败	20	pay_notify_status	0	danger		通知失败	1	2021-12-03 11:02:59	1	2023-07-19 10:08:21	0
122	0	等待通知	0	pay_notify_status	0	info		未通知	1	2021-12-03 11:03:10	1	2023-07-19 10:08:24	0
123	10	支付成功	10	pay_order_status	0	success		支付成功	1	2021-12-03 11:18:29	1	2023-07-19 18:04:28	0
124	30	支付关闭	30	pay_order_status	0	info		支付关闭	1	2021-12-03 11:18:42	1	2023-07-19 18:05:07	0
125	0	等待支付	0	pay_order_status	0	info		未支付	1	2021-12-03 11:18:18	1	2023-07-19 18:04:15	0
600	5	首页	1	promotion_banner_position	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
601	4	秒杀活动页	2	promotion_banner_position	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
602	3	砍价活动页	3	promotion_banner_position	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
603	2	限时折扣页	4	promotion_banner_position	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
604	1	满减送页	5	promotion_banner_position	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
1118	0	等待退款	0	pay_refund_status	0	info		等待退款	1	2021-12-10 16:44:59	1	2023-07-19 10:14:39	0
1119	20	退款失败	20	pay_refund_status	0	danger		退款失败	1	2021-12-10 16:45:10	1	2023-07-19 10:15:10	0
1124	10	退款成功	10	pay_refund_status	0	success		退款成功	1	2021-12-10 16:46:26	1	2023-07-19 10:15:00	0
1127	1	审批中	1	bpm_process_instance_status	0	default		流程实例的状态 - 进行中	1	2022-01-07 23:47:22	1	2024-03-16 16:11:45	0
1128	2	审批通过	2	bpm_process_instance_status	0	success		流程实例的状态 - 已完成	1	2022-01-07 23:47:49	1	2024-03-16 16:11:54	0
1129	1	审批中	1	bpm_task_status	0	primary		流程实例的结果 - 处理中	1	2022-01-07 23:48:32	1	2024-03-08 22:41:37	0
1130	2	审批通过	2	bpm_task_status	0	success		流程实例的结果 - 通过	1	2022-01-07 23:48:45	1	2024-03-08 22:41:38	0
1131	3	审批不通过	3	bpm_task_status	0	danger		流程实例的结果 - 不通过	1	2022-01-07 23:48:55	1	2024-03-08 22:41:38	0
1132	4	已取消	4	bpm_task_status	0	info		流程实例的结果 - 撤销	1	2022-01-07 23:49:06	1	2024-03-08 22:41:39	0
1133	10	流程表单	10	bpm_model_form_type	0			流程的表单类型 - 流程表单	103	2022-01-11 23:51:30	103	2022-01-11 23:51:30	0
1134	20	业务表单	20	bpm_model_form_type	0			流程的表单类型 - 业务表单	103	2022-01-11 23:51:47	103	2022-01-11 23:51:47	0
1135	10	角色	10	bpm_task_candidate_strategy	0	info		任务分配规则的类型 - 角色	103	2022-01-12 23:21:22	1	2024-03-06 02:53:16	0
1136	20	部门的成员	20	bpm_task_candidate_strategy	0	primary		任务分配规则的类型 - 部门的成员	103	2022-01-12 23:21:47	1	2024-03-06 02:53:17	0
1137	21	部门的负责人	21	bpm_task_candidate_strategy	0	primary		任务分配规则的类型 - 部门的负责人	103	2022-01-12 23:33:36	1	2024-03-06 02:53:18	0
1138	30	用户	30	bpm_task_candidate_strategy	0	info		任务分配规则的类型 - 用户	103	2022-01-12 23:34:02	1	2024-03-06 02:53:19	0
1139	40	用户组	40	bpm_task_candidate_strategy	0	warning		任务分配规则的类型 - 用户组	103	2022-01-12 23:34:21	1	2024-03-06 02:53:20	0
1140	60	流程表达式	60	bpm_task_candidate_strategy	0	danger		任务分配规则的类型 - 流程表达式	103	2022-01-12 23:34:43	1	2024-03-06 02:53:20	0
1141	22	岗位	22	bpm_task_candidate_strategy	0	success		任务分配规则的类型 - 岗位	103	2022-01-14 18:41:55	1	2024-03-06 02:53:21	0
1145	1	管理后台	1	infra_codegen_scene	0			代码生成的场景枚举 - 管理后台	1	2022-02-02 13:15:06	1	2022-03-10 16:32:59	0
1146	2	用户 APP	2	infra_codegen_scene	0			代码生成的场景枚举 - 用户 APP	1	2022-02-02 13:15:19	1	2022-03-10 16:33:03	0
1150	1	数据库	1	infra_file_storage	0	default		\N	1	2022-03-15 00:25:28	1	2022-03-15 00:25:28	0
1151	10	本地磁盘	10	infra_file_storage	0	default		\N	1	2022-03-15 00:25:41	1	2022-03-15 00:25:56	0
1152	11	FTP 服务器	11	infra_file_storage	0	default		\N	1	2022-03-15 00:26:06	1	2022-03-15 00:26:10	0
1153	12	SFTP 服务器	12	infra_file_storage	0	default		\N	1	2022-03-15 00:26:22	1	2022-03-15 00:26:22	0
1154	20	S3 对象存储	20	infra_file_storage	0	default		\N	1	2022-03-15 00:26:31	1	2022-03-15 00:26:45	0
1155	103	短信登录	103	system_login_type	0	default		\N	1	2022-05-09 23:57:58	1	2022-05-09 23:58:09	0
1156	1	password	password	system_oauth2_grant_type	0	default		密码模式	1	2022-05-12 00:22:05	1	2022-05-11 16:26:01	0
1157	2	authorization_code	authorization_code	system_oauth2_grant_type	0	primary		授权码模式	1	2022-05-12 00:22:59	1	2022-05-11 16:26:02	0
1158	3	implicit	implicit	system_oauth2_grant_type	0	success		简化模式	1	2022-05-12 00:23:40	1	2022-05-11 16:26:05	0
1159	4	client_credentials	client_credentials	system_oauth2_grant_type	0	default		客户端模式	1	2022-05-12 00:23:51	1	2022-05-11 16:26:08	0
1160	5	refresh_token	refresh_token	system_oauth2_grant_type	0	info		刷新模式	1	2022-05-12 00:24:02	1	2022-05-11 16:26:11	0
1162	1	销售中	1	product_spu_status	0	success		商品 SPU 状态 - 销售中	1	2022-10-24 21:19:47	1	2022-10-24 21:20:38	0
1163	0	仓库中	0	product_spu_status	0	info		商品 SPU 状态 - 仓库中	1	2022-10-24 21:20:54	1	2022-10-24 21:21:22	0
1164	0	回收站	-1	product_spu_status	0	default		商品 SPU 状态 - 回收站	1	2022-10-24 21:21:11	1	2022-10-24 21:21:11	0
1165	1	满减	1	promotion_discount_type	0	success		优惠类型 - 满减	1	2022-11-01 12:46:41	1	2022-11-01 12:50:11	0
1166	2	折扣	2	promotion_discount_type	0	primary		优惠类型 - 折扣	1	2022-11-01 12:46:51	1	2022-11-01 12:50:08	0
1167	1	固定日期	1	promotion_coupon_template_validity_type	0	default		优惠劵模板的有限期类型 - 固定日期	1	2022-11-02 00:07:34	1	2022-11-04 00:07:49	0
1168	2	领取之后	2	promotion_coupon_template_validity_type	0	default		优惠劵模板的有限期类型 - 领取之后	1	2022-11-02 00:07:54	1	2022-11-04 00:07:52	0
1169	1	通用劵	1	promotion_product_scope	0	default		营销的商品范围 - 全部商品参与	1	2022-11-02 00:28:22	1	2023-09-28 00:27:42	0
1170	2	商品劵	2	promotion_product_scope	0	default		营销的商品范围 - 指定商品参与	1	2022-11-02 00:28:34	1	2023-09-28 00:27:44	0
1171	1	未使用	1	promotion_coupon_status	0	primary		优惠劵的状态 - 已领取	1	2022-11-04 00:15:08	1	2023-10-03 12:54:38	0
1172	2	已使用	2	promotion_coupon_status	0	success		优惠劵的状态 - 已使用	1	2022-11-04 00:15:21	1	2022-11-04 19:16:08	0
1173	3	已过期	3	promotion_coupon_status	0	info		优惠劵的状态 - 已过期	1	2022-11-04 00:15:43	1	2022-11-04 19:16:12	0
1174	1	直接领取	1	promotion_coupon_take_type	0	primary		优惠劵的领取方式 - 直接领取	1	2022-11-04 19:13:00	1	2022-11-04 19:13:25	0
1175	2	指定发放	2	promotion_coupon_take_type	0	success		优惠劵的领取方式 - 指定发放	1	2022-11-04 19:13:13	1	2022-11-04 19:14:48	0
1176	10	未开始	10	promotion_activity_status	0	primary		促销活动的状态枚举 - 未开始	1	2022-11-04 22:54:49	1	2022-11-04 22:55:53	0
1177	20	进行中	20	promotion_activity_status	0	success		促销活动的状态枚举 - 进行中	1	2022-11-04 22:55:06	1	2022-11-04 22:55:20	0
1178	30	已结束	30	promotion_activity_status	0	info		促销活动的状态枚举 - 已结束	1	2022-11-04 22:55:41	1	2022-11-04 22:55:41	0
1179	40	已关闭	40	promotion_activity_status	0	warning		促销活动的状态枚举 - 已关闭	1	2022-11-04 22:56:10	1	2022-11-04 22:56:18	0
1180	10	满 N 元	10	promotion_condition_type	0	primary		营销的条件类型 - 满 N 元	1	2022-11-04 22:59:45	1	2022-11-04 22:59:45	0
1181	20	满 N 件	20	promotion_condition_type	0	success		营销的条件类型 - 满 N 件	1	2022-11-04 23:00:02	1	2022-11-04 23:00:02	0
1182	10	申请售后	10	trade_after_sale_status	0	primary		交易售后状态 - 申请售后	1	2022-11-19 20:53:33	1	2022-11-19 20:54:42	0
1183	20	商品待退货	20	trade_after_sale_status	0	primary		交易售后状态 - 商品待退货	1	2022-11-19 20:54:36	1	2022-11-19 20:58:58	0
1184	30	商家待收货	30	trade_after_sale_status	0	primary		交易售后状态 - 商家待收货	1	2022-11-19 20:56:56	1	2022-11-19 20:59:20	0
1185	40	等待退款	40	trade_after_sale_status	0	primary		交易售后状态 - 等待退款	1	2022-11-19 20:59:54	1	2022-11-19 21:00:01	0
1186	50	退款成功	50	trade_after_sale_status	0	default		交易售后状态 - 退款成功	1	2022-11-19 21:00:33	1	2022-11-19 21:00:33	0
1187	61	买家取消	61	trade_after_sale_status	0	info		交易售后状态 - 买家取消	1	2022-11-19 21:01:29	1	2022-11-19 21:01:29	0
1188	62	商家拒绝	62	trade_after_sale_status	0	info		交易售后状态 - 商家拒绝	1	2022-11-19 21:02:17	1	2022-11-19 21:02:17	0
1189	63	商家拒收货	63	trade_after_sale_status	0	info		交易售后状态 - 商家拒收货	1	2022-11-19 21:02:37	1	2022-11-19 21:03:07	0
1190	10	售中退款	10	trade_after_sale_type	0	success		交易售后的类型 - 售中退款	1	2022-11-19 21:05:05	1	2022-11-19 21:38:23	0
1191	20	售后退款	20	trade_after_sale_type	0	primary		交易售后的类型 - 售后退款	1	2022-11-19 21:05:32	1	2022-11-19 21:38:32	0
1192	10	仅退款	10	trade_after_sale_way	0	primary		交易售后的方式 - 仅退款	1	2022-11-19 21:39:19	1	2022-11-19 21:39:19	0
1193	20	退货退款	20	trade_after_sale_way	0	success		交易售后的方式 - 退货退款	1	2022-11-19 21:39:38	1	2022-11-19 21:39:49	0
1194	10	微信小程序	10	terminal	0	default		终端 - 微信小程序	1	2022-12-10 10:51:11	1	2022-12-10 10:51:57	0
1195	20	H5 网页	20	terminal	0	default		终端 - H5 网页	1	2022-12-10 10:51:30	1	2022-12-10 10:51:59	0
1196	11	微信公众号	11	terminal	0	default		终端 - 微信公众号	1	2022-12-10 10:54:16	1	2022-12-10 10:52:01	0
1197	31	苹果 App	31	terminal	0	default		终端 - 苹果 App	1	2022-12-10 10:54:42	1	2022-12-10 10:52:18	0
1198	32	安卓 App	32	terminal	0	default		终端 - 安卓 App	1	2022-12-10 10:55:02	1	2022-12-10 10:59:17	0
1199	0	普通订单	0	trade_order_type	0	default		交易订单的类型 - 普通订单	1	2022-12-10 16:34:14	1	2022-12-10 16:34:14	0
1200	1	秒杀订单	1	trade_order_type	0	default		交易订单的类型 - 秒杀订单	1	2022-12-10 16:34:26	1	2022-12-10 16:34:26	0
1201	2	砍价订单	2	trade_order_type	0	default		交易订单的类型 - 拼团订单	1	2022-12-10 16:34:36	1	2024-09-07 14:18:39	0
1202	3	拼团订单	3	trade_order_type	0	default		交易订单的类型 - 砍价订单	1	2022-12-10 16:34:48	1	2024-09-07 14:18:32	0
1203	0	待支付	0	trade_order_status	0	default		交易订单状态 - 待支付	1	2022-12-10 16:49:29	1	2022-12-10 16:49:29	0
1204	10	待发货	10	trade_order_status	0	primary		交易订单状态 - 待发货	1	2022-12-10 16:49:53	1	2022-12-10 16:51:17	0
1205	20	已发货	20	trade_order_status	0	primary		交易订单状态 - 已发货	1	2022-12-10 16:50:13	1	2022-12-10 16:51:31	0
1206	30	已完成	30	trade_order_status	0	success		交易订单状态 - 已完成	1	2022-12-10 16:50:30	1	2022-12-10 16:51:06	0
1207	40	已取消	40	trade_order_status	0	danger		交易订单状态 - 已取消	1	2022-12-10 16:50:50	1	2022-12-10 16:51:00	0
1208	0	未售后	0	trade_order_item_after_sale_status	0	info		交易订单项的售后状态 - 未售后	1	2022-12-10 20:58:42	1	2022-12-10 20:59:29	0
1209	10	售后中	10	trade_order_item_after_sale_status	0	primary		交易订单项的售后状态 - 售后中	1	2022-12-10 20:59:21	1	2024-07-21 17:01:24	0
1210	20	已退款	20	trade_order_item_after_sale_status	0	success		交易订单项的售后状态 - 已退款	1	2022-12-10 20:59:46	1	2024-07-21 17:01:35	0
1369	2	申请提现	2	brokerage_record_biz_type	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1211	1	完全匹配	1	mp_auto_reply_request_match	0	primary		公众号自动回复的请求关键字匹配模式 - 完全匹配	1	2023-01-16 23:30:39	1	2023-01-16 23:31:00	0
1212	2	半匹配	2	mp_auto_reply_request_match	0	success		公众号自动回复的请求关键字匹配模式 - 半匹配	1	2023-01-16 23:30:55	1	2023-01-16 23:31:10	0
1213	1	文本	text	mp_message_type	0	default		公众号的消息类型 - 文本	1	2023-01-17 22:17:32	1	2023-01-17 22:17:39	0
1214	2	图片	image	mp_message_type	0	default		公众号的消息类型 - 图片	1	2023-01-17 22:17:32	1	2023-01-17 14:19:47	0
1215	3	语音	voice	mp_message_type	0	default		公众号的消息类型 - 语音	1	2023-01-17 22:17:32	1	2023-01-17 14:20:08	0
1216	4	视频	video	mp_message_type	0	default		公众号的消息类型 - 视频	1	2023-01-17 22:17:32	1	2023-01-17 14:21:08	0
1217	5	小视频	shortvideo	mp_message_type	0	default		公众号的消息类型 - 小视频	1	2023-01-17 22:17:32	1	2023-01-17 14:19:59	0
1218	6	图文	news	mp_message_type	0	default		公众号的消息类型 - 图文	1	2023-01-17 22:17:32	1	2023-01-17 14:22:54	0
1219	7	音乐	music	mp_message_type	0	default		公众号的消息类型 - 音乐	1	2023-01-17 22:17:32	1	2023-01-17 14:22:54	0
1220	8	地理位置	location	mp_message_type	0	default		公众号的消息类型 - 地理位置	1	2023-01-17 22:17:32	1	2023-01-17 14:23:51	0
1221	9	链接	link	mp_message_type	0	default		公众号的消息类型 - 链接	1	2023-01-17 22:17:32	1	2023-01-17 14:24:49	0
1222	10	事件	event	mp_message_type	0	default		公众号的消息类型 - 事件	1	2023-01-17 22:17:32	1	2023-01-17 14:24:49	0
1223	0	初始化	0	system_mail_send_status	0	primary		邮件发送状态 - 初始化\\n	1	2023-01-26 09:53:49	1	2023-01-26 16:36:14	0
1224	10	发送成功	10	system_mail_send_status	0	success		邮件发送状态 - 发送成功	1	2023-01-26 09:54:28	1	2023-01-26 16:36:22	0
1225	20	发送失败	20	system_mail_send_status	0	danger		邮件发送状态 - 发送失败	1	2023-01-26 09:54:50	1	2023-01-26 16:36:26	0
1226	30	不发送	30	system_mail_send_status	0	info		邮件发送状态 -  不发送	1	2023-01-26 09:55:06	1	2023-01-26 16:36:36	0
1227	1	通知公告	1	system_notify_template_type	0	primary		站内信模版的类型 - 通知公告	1	2023-01-28 10:35:59	1	2023-01-28 10:35:59	0
1228	2	系统消息	2	system_notify_template_type	0	success		站内信模版的类型 - 系统消息	1	2023-01-28 10:36:20	1	2023-01-28 10:36:25	0
1230	13	支付宝条码支付	alipay_bar	pay_channel_code	0	primary		支付宝条码支付	1	2023-02-18 23:32:24	1	2023-07-19 20:09:23	0
1231	10	Vue2 Element UI 标准模版	10	infra_codegen_front_type	0				1	2023-04-13 00:03:55	1	2023-04-13 00:03:55	0
1232	20	Vue3 Element Plus 标准模版	20	infra_codegen_front_type	0				1	2023-04-13 00:04:08	1	2023-04-13 00:04:08	0
1234	30	Vben2.0 Ant Design Schema 模版	30	infra_codegen_front_type	1				1	2023-04-13 00:04:26	1	2025-07-27 10:55:14	0
1244	0	按件	1	trade_delivery_express_charge_mode	0				1	2023-05-21 22:46:40	1	2023-05-21 22:46:40	0
1245	1	按重量	2	trade_delivery_express_charge_mode	0				1	2023-05-21 22:46:58	1	2023-05-21 22:46:58	0
1246	2	按体积	3	trade_delivery_express_charge_mode	0				1	2023-05-21 22:47:18	1	2023-05-21 22:47:18	0
1335	11	订单积分抵扣	11	member_point_biz_type	0				1	2023-06-10 12:15:27	1	2023-10-11 07:41:43	0
1336	1	签到	1	member_point_biz_type	0				1	2023-06-10 12:15:48	1	2023-08-20 11:59:53	0
1341	20	已退款	20	pay_order_status	0	danger		已退款	1	2023-07-19 18:05:37	1	2023-07-19 18:05:37	0
1342	21	请求成功，但是结果失败	21	pay_notify_status	0	warning		请求成功，但是结果失败	1	2023-07-19 18:10:47	1	2023-07-19 18:11:38	0
1343	22	请求失败	22	pay_notify_status	0	warning		\N	1	2023-07-19 18:11:05	1	2023-07-19 18:11:27	0
1344	4	微信扫码支付	wx_native	pay_channel_code	0	success		微信扫码支付	1	2023-07-19 20:07:47	1	2023-07-19 20:09:03	0
1345	5	微信条码支付	wx_bar	pay_channel_code	0	success		微信条码支付\\n	1	2023-07-19 20:08:06	1	2023-07-19 20:09:08	0
1346	1	支付单	1	pay_notify_type	0	primary		支付单	1	2023-07-20 12:23:17	1	2023-07-20 12:23:17	0
1347	2	退款单	2	pay_notify_type	0	danger		\N	1	2023-07-20 12:23:26	1	2023-07-20 12:23:26	0
1348	20	模拟支付	mock	pay_channel_code	0	default		模拟支付	1	2023-07-29 11:10:51	1	2023-07-29 03:14:10	0
1349	12	订单积分抵扣（整单取消）	12	member_point_biz_type	0				1	2023-08-20 12:00:03	1	2023-10-11 07:42:01	0
1350	0	管理员调整	0	member_experience_biz_type	0			\N		2023-08-22 12:41:01		2023-08-22 12:41:01	0
1351	1	邀新奖励	1	member_experience_biz_type	0			\N		2023-08-22 12:41:01		2023-08-22 12:41:01	0
1352	11	下单奖励	11	member_experience_biz_type	0	success		\N		2023-08-22 12:41:01	1	2023-10-11 07:45:09	0
1353	12	下单奖励（整单取消）	12	member_experience_biz_type	0	warning		\N		2023-08-22 12:41:01	1	2023-10-11 07:45:01	0
1354	4	签到奖励	4	member_experience_biz_type	0			\N		2023-08-22 12:41:01		2023-08-22 12:41:01	0
1355	5	抽奖奖励	5	member_experience_biz_type	0			\N		2023-08-22 12:41:01		2023-08-22 12:41:01	0
1356	1	快递发货	1	trade_delivery_type	0				1	2023-08-23 00:04:55	1	2023-08-23 00:04:55	0
1357	2	用户自提	2	trade_delivery_type	0				1	2023-08-23 00:05:05	1	2023-08-23 00:05:05	0
1358	3	品类劵	3	promotion_product_scope	0	default			1	2023-09-01 23:43:07	1	2023-09-28 00:27:47	0
1359	1	人人分销	1	brokerage_enabled_condition	0			所有用户都可以分销		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1360	2	指定分销	2	brokerage_enabled_condition	0			仅可后台手动设置推广员		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1361	1	首次绑定	1	brokerage_bind_mode	0			只要用户没有推广人，随时都可以绑定推广关系		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1362	2	注册绑定	2	brokerage_bind_mode	0			仅新用户注册时才能绑定推广关系		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1363	3	覆盖绑定	3	brokerage_bind_mode	0			如果用户已经有推广人，推广人会被变更		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1364	1	钱包	1	brokerage_withdraw_type	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1365	2	银行卡	2	brokerage_withdraw_type	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1366	3	微信收款码	3	brokerage_withdraw_type	0			手动打款		2023-09-28 02:46:05	1	2025-05-10 08:24:25	0
1367	4	支付宝收款码	4	brokerage_withdraw_type	0			手动打款		2023-09-28 02:46:05	1	2025-05-10 08:24:37	0
1368	1	订单返佣	1	brokerage_record_biz_type	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1370	3	申请提现驳回	3	brokerage_record_biz_type	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1371	0	待结算	0	brokerage_record_status	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1372	1	已结算	1	brokerage_record_status	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1373	2	已取消	2	brokerage_record_status	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1374	0	审核中	0	brokerage_withdraw_status	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1375	10	审核通过	10	brokerage_withdraw_status	0	success		\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1376	11	提现成功	11	brokerage_withdraw_status	0	success		\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1377	20	审核不通过	20	brokerage_withdraw_status	0	danger		\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1378	21	提现失败	21	brokerage_withdraw_status	0	danger		\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1379	0	工商银行	0	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1380	1	建设银行	1	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1381	2	农业银行	2	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1382	3	中国银行	3	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1383	4	交通银行	4	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1384	5	招商银行	5	brokerage_bank_name	0			\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0
1385	21	钱包	wallet	pay_channel_code	0	primary			1	2023-10-01 21:46:19	1	2023-10-01 21:48:01	0
1386	1	砍价中	1	promotion_bargain_record_status	0	default			1	2023-10-05 10:41:26	1	2023-10-05 10:41:26	0
1387	2	砍价成功	2	promotion_bargain_record_status	0	success			1	2023-10-05 10:41:39	1	2023-10-05 10:41:39	0
1388	3	砍价失败	3	promotion_bargain_record_status	0	warning			1	2023-10-05 10:41:57	1	2023-10-05 10:41:57	0
1389	0	拼团中	0	promotion_combination_record_status	0				1	2023-10-08 07:24:44	1	2024-10-13 10:08:17	0
1390	1	拼团成功	1	promotion_combination_record_status	0	success			1	2023-10-08 07:24:56	1	2024-10-13 10:08:20	0
1391	2	拼团失败	2	promotion_combination_record_status	0	warning			1	2023-10-08 07:25:11	1	2024-10-13 10:08:24	0
1392	2	管理员修改	2	member_point_biz_type	0	default			1	2023-10-11 07:41:34	1	2023-10-11 07:41:34	0
1393	13	订单积分抵扣（单个退款）	13	member_point_biz_type	0				1	2023-10-11 07:42:29	1	2023-10-11 07:42:29	0
1394	21	订单积分奖励	21	member_point_biz_type	0	default			1	2023-10-11 07:42:44	1	2023-10-11 07:42:44	0
1395	22	订单积分奖励（整单取消）	22	member_point_biz_type	0	default			1	2023-10-11 07:42:55	1	2023-10-11 07:43:01	0
1396	23	订单积分奖励（单个退款）	23	member_point_biz_type	0	default			1	2023-10-11 07:43:16	1	2023-10-11 07:43:16	0
1397	13	下单奖励（单个退款）	13	member_experience_biz_type	0	warning			1	2023-10-11 07:45:24	1	2023-10-11 07:45:38	0
1398	5	网上转账	5	crm_receivable_return_type	0	default			1	2023-10-18 21:55:24	1	2023-10-18 21:55:24	0
1399	6	支付宝	6	crm_receivable_return_type	0	default			1	2023-10-18 21:55:38	1	2023-10-18 21:55:38	0
1400	7	微信支付	7	crm_receivable_return_type	0	default			1	2023-10-18 21:55:53	1	2023-10-18 21:55:53	0
1401	8	其他	8	crm_receivable_return_type	0	default			1	2023-10-18 21:56:06	1	2023-10-18 21:56:06	0
1402	1	IT	1	crm_customer_industry	0	default			1	2023-10-28 23:02:15	1	2024-02-18 23:30:38	0
1403	2	金融业	2	crm_customer_industry	0	default			1	2023-10-28 23:02:29	1	2024-02-18 23:30:43	0
1404	3	房地产	3	crm_customer_industry	0	default			1	2023-10-28 23:02:41	1	2024-02-18 23:30:48	0
1405	4	商业服务	4	crm_customer_industry	0	default			1	2023-10-28 23:02:54	1	2024-02-18 23:30:54	0
1406	5	运输/物流	5	crm_customer_industry	0	default			1	2023-10-28 23:03:03	1	2024-02-18 23:31:00	0
1407	6	生产	6	crm_customer_industry	0	default			1	2023-10-28 23:03:13	1	2024-02-18 23:31:08	0
1408	7	政府	7	crm_customer_industry	0	default			1	2023-10-28 23:03:27	1	2024-02-18 23:31:13	0
1409	8	文化传媒	8	crm_customer_industry	0	default			1	2023-10-28 23:03:37	1	2024-02-18 23:31:20	0
1422	1	A （重点客户）	1	crm_customer_level	0	primary			1	2023-10-28 23:07:13	1	2023-10-28 23:07:13	0
1423	2	B （普通客户）	2	crm_customer_level	0	info			1	2023-10-28 23:07:35	1	2023-10-28 23:07:35	0
1424	3	C （非优先客户）	3	crm_customer_level	0	default			1	2023-10-28 23:07:53	1	2023-10-28 23:07:53	0
1425	1	促销	1	crm_customer_source	0	default			1	2023-10-28 23:08:29	1	2023-10-28 23:08:29	0
1426	2	搜索引擎	2	crm_customer_source	0	default			1	2023-10-28 23:08:39	1	2023-10-28 23:08:39	0
1427	3	广告	3	crm_customer_source	0	default			1	2023-10-28 23:08:47	1	2023-10-28 23:08:47	0
1428	4	转介绍	4	crm_customer_source	0	default			1	2023-10-28 23:08:58	1	2023-10-28 23:08:58	0
1429	5	线上注册	5	crm_customer_source	0	default			1	2023-10-28 23:09:12	1	2023-10-28 23:09:12	0
1430	6	线上咨询	6	crm_customer_source	0	default			1	2023-10-28 23:09:22	1	2023-10-28 23:09:22	0
1431	7	预约上门	7	crm_customer_source	0	default			1	2023-10-28 23:09:39	1	2023-10-28 23:09:39	0
1432	8	陌拜	8	crm_customer_source	0	default			1	2023-10-28 23:10:04	1	2023-10-28 23:10:04	0
1433	9	电话咨询	9	crm_customer_source	0	default			1	2023-10-28 23:10:18	1	2023-10-28 23:10:18	0
1434	10	邮件咨询	10	crm_customer_source	0	default			1	2023-10-28 23:10:33	1	2023-10-28 23:10:33	0
1435	10	Gitee	10	system_social_type	0				1	2023-11-04 13:04:42	1	2023-11-04 13:04:42	0
1436	20	钉钉	20	system_social_type	0				1	2023-11-04 13:04:54	1	2023-11-04 13:04:54	0
1437	30	企业微信	30	system_social_type	0				1	2023-11-04 13:05:09	1	2023-11-04 13:05:09	0
1438	31	微信公众平台	31	system_social_type	0				1	2023-11-04 13:05:18	1	2023-11-04 13:05:18	0
1439	32	微信开放平台	32	system_social_type	0				1	2023-11-04 13:05:30	1	2023-11-04 13:05:30	0
1440	34	微信小程序	34	system_social_type	0				1	2023-11-04 13:05:38	1	2023-11-04 13:07:16	0
1441	1	上架	1	crm_product_status	0	success			1	2023-10-30 21:49:34	1	2023-10-30 21:49:34	0
1442	0	下架	0	crm_product_status	0	success			1	2023-10-30 21:49:13	1	2023-10-30 21:49:13	0
1443	15	子表	15	infra_codegen_template_type	0	default			1	2023-11-13 23:06:16	1	2023-11-13 23:06:16	0
1444	10	主表（标准模式）	10	infra_codegen_template_type	0	default			1	2023-11-14 12:32:49	1	2023-11-14 12:32:49	0
1445	11	主表（ERP 模式）	11	infra_codegen_template_type	0	default			1	2023-11-14 12:33:05	1	2023-11-14 12:33:05	0
1446	12	主表（内嵌模式）	12	infra_codegen_template_type	0				1	2023-11-14 12:33:31	1	2023-11-14 12:33:31	0
1447	1	负责人	1	crm_permission_level	0	default			1	2023-11-30 09:53:12	1	2023-11-30 09:53:12	0
1448	2	只读	2	crm_permission_level	0				1	2023-11-30 09:53:29	1	2023-11-30 09:53:29	0
1449	3	读写	3	crm_permission_level	0				1	2023-11-30 09:53:36	1	2023-11-30 09:53:36	0
1450	0	未提交	0	crm_audit_status	0				1	2023-11-30 18:56:59	1	2023-11-30 18:56:59	0
1451	10	审批中	10	crm_audit_status	0				1	2023-11-30 18:57:10	1	2023-11-30 18:57:10	0
1452	20	审核通过	20	crm_audit_status	0				1	2023-11-30 18:57:24	1	2023-11-30 18:57:24	0
1453	30	审核不通过	30	crm_audit_status	0				1	2023-11-30 18:57:32	1	2023-11-30 18:57:32	0
1454	40	已取消	40	crm_audit_status	0				1	2023-11-30 18:57:42	1	2023-11-30 18:57:42	0
1456	1	支票	1	crm_receivable_return_type	0	default			1	2023-10-18 21:54:29	1	2023-10-18 21:54:29	0
1457	2	现金	2	crm_receivable_return_type	0	default			1	2023-10-18 21:54:41	1	2023-10-18 21:54:41	0
1458	3	邮政汇款	3	crm_receivable_return_type	0	default			1	2023-10-18 21:54:53	1	2023-10-18 21:54:53	0
1459	4	电汇	4	crm_receivable_return_type	0	default			1	2023-10-18 21:55:07	1	2023-10-18 21:55:07	0
1461	1	个	1	crm_product_unit	0				1	2023-12-05 23:02:26	1	2023-12-05 23:02:26	0
1462	2	块	2	crm_product_unit	0				1	2023-12-05 23:02:34	1	2023-12-05 23:02:34	0
1463	3	只	3	crm_product_unit	0				1	2023-12-05 23:02:57	1	2023-12-05 23:02:57	0
1464	4	把	4	crm_product_unit	0				1	2023-12-05 23:03:05	1	2023-12-05 23:03:05	0
1465	5	枚	5	crm_product_unit	0				1	2023-12-05 23:03:14	1	2023-12-05 23:03:14	0
1466	6	瓶	6	crm_product_unit	0				1	2023-12-05 23:03:20	1	2023-12-05 23:03:20	0
1467	7	盒	7	crm_product_unit	0				1	2023-12-05 23:03:30	1	2023-12-05 23:03:30	0
1468	8	台	8	crm_product_unit	0				1	2023-12-05 23:03:41	1	2023-12-05 23:03:41	0
1469	9	吨	9	crm_product_unit	0				1	2023-12-05 23:03:48	1	2023-12-05 23:03:48	0
1470	10	千克	10	crm_product_unit	0				1	2023-12-05 23:04:03	1	2023-12-05 23:04:03	0
1471	11	米	11	crm_product_unit	0				1	2023-12-05 23:04:12	1	2023-12-05 23:04:12	0
1472	12	箱	12	crm_product_unit	0				1	2023-12-05 23:04:25	1	2023-12-05 23:04:25	0
1473	13	套	13	crm_product_unit	0				1	2023-12-05 23:04:34	1	2023-12-05 23:04:34	0
1474	1	打电话	1	crm_follow_up_type	0				1	2024-01-15 20:48:20	1	2024-01-15 20:48:20	0
1475	2	发短信	2	crm_follow_up_type	0				1	2024-01-15 20:48:31	1	2024-01-15 20:48:31	0
1476	3	上门拜访	3	crm_follow_up_type	0				1	2024-01-15 20:49:07	1	2024-01-15 20:49:07	0
1477	4	微信沟通	4	crm_follow_up_type	0				1	2024-01-15 20:49:15	1	2024-01-15 20:49:15	0
1482	4	转账失败	20	pay_transfer_status	0	warning			1	2023-10-28 16:24:16	1	2025-05-08 12:59:01	0
1483	3	转账成功	10	pay_transfer_status	0	success			1	2023-10-28 16:23:50	1	2025-05-08 12:58:58	0
1484	2	转账进行中	5	pay_transfer_status	0	info			1	2023-10-28 16:23:12	1	2025-05-08 12:58:54	0
1485	1	等待转账	0	pay_transfer_status	0	default			1	2023-10-28 16:21:43	1	2023-10-28 16:23:22	0
1486	10	其它入库	10	erp_stock_record_biz_type	0				1	2024-02-05 18:07:25	1	2024-02-05 18:07:43	0
1487	11	其它入库（作废）	11	erp_stock_record_biz_type	0	danger			1	2024-02-05 18:08:07	1	2024-02-05 19:20:16	0
1488	20	其它出库	20	erp_stock_record_biz_type	0				1	2024-02-05 18:08:51	1	2024-02-05 18:08:51	0
1489	21	其它出库（作废）	21	erp_stock_record_biz_type	0	danger			1	2024-02-05 18:09:00	1	2024-02-05 19:20:10	0
1490	10	未审核	10	erp_audit_status	0	default			1	2024-02-06 00:00:21	1	2024-02-06 00:00:21	0
1491	20	已审核	20	erp_audit_status	0	success			1	2024-02-06 00:00:35	1	2024-02-06 00:00:35	0
1492	30	调拨入库	30	erp_stock_record_biz_type	0				1	2024-02-07 20:34:19	1	2024-02-07 12:36:31	0
1493	31	调拨入库（作废）	31	erp_stock_record_biz_type	0	danger			1	2024-02-07 20:34:29	1	2024-02-07 20:37:11	0
1494	32	调拨出库	32	erp_stock_record_biz_type	0				1	2024-02-07 20:34:38	1	2024-02-07 12:36:33	0
1495	33	调拨出库（作废）	33	erp_stock_record_biz_type	0	danger			1	2024-02-07 20:34:49	1	2024-02-07 20:37:06	0
1496	40	盘盈入库	40	erp_stock_record_biz_type	0				1	2024-02-08 08:53:00	1	2024-02-08 08:53:09	0
1497	41	盘盈入库（作废）	41	erp_stock_record_biz_type	0	danger			1	2024-02-08 08:53:39	1	2024-02-16 19:40:54	0
1498	42	盘亏出库	42	erp_stock_record_biz_type	0				1	2024-02-08 08:54:16	1	2024-02-08 08:54:16	0
1499	43	盘亏出库（作废）	43	erp_stock_record_biz_type	0	danger			1	2024-02-08 08:54:31	1	2024-02-16 19:40:46	0
1500	50	销售出库	50	erp_stock_record_biz_type	0				1	2024-02-11 21:47:25	1	2024-02-11 21:50:40	0
1501	51	销售出库（作废）	51	erp_stock_record_biz_type	0	danger			1	2024-02-11 21:47:37	1	2024-02-11 21:51:12	0
1502	60	销售退货入库	60	erp_stock_record_biz_type	0				1	2024-02-12 06:51:05	1	2024-02-12 06:51:05	0
1503	61	销售退货入库（作废）	61	erp_stock_record_biz_type	0	danger			1	2024-02-12 06:51:18	1	2024-02-12 06:51:18	0
1504	70	采购入库	70	erp_stock_record_biz_type	0				1	2024-02-16 13:10:02	1	2024-02-16 13:10:02	0
1505	71	采购入库（作废）	71	erp_stock_record_biz_type	0	danger			1	2024-02-16 13:10:10	1	2024-02-16 19:40:40	0
1506	80	采购退货出库	80	erp_stock_record_biz_type	0				1	2024-02-16 13:10:17	1	2024-02-16 13:10:17	0
1507	81	采购退货出库（作废）	81	erp_stock_record_biz_type	0	danger			1	2024-02-16 13:10:26	1	2024-02-16 19:40:33	0
1509	3	审批不通过	3	bpm_process_instance_status	0	danger			1	2024-03-16 16:12:06	1	2024-03-16 16:12:06	0
1510	4	已取消	4	bpm_process_instance_status	0	warning			1	2024-03-16 16:12:22	1	2024-03-16 16:12:22	0
1511	5	已退回	5	bpm_task_status	0	warning			1	2024-03-16 19:10:46	1	2024-03-08 22:41:40	0
1512	6	委派中	6	bpm_task_status	0	primary			1	2024-03-17 10:06:22	1	2024-03-08 22:41:40	0
1513	7	审批通过中	7	bpm_task_status	0	success			1	2024-03-17 10:06:47	1	2024-03-08 22:41:41	0
1514	0	待审批	0	bpm_task_status	0	info			1	2024-03-17 10:07:11	1	2024-03-08 22:41:42	0
1515	35	发起人自选	35	bpm_task_candidate_strategy	0				1	2024-03-22 19:45:16	1	2024-03-22 19:45:16	0
1516	1	执行监听器	execution	bpm_process_listener_type	0	primary			1	2024-03-23 12:54:03	1	2024-03-23 19:14:19	0
1517	1	任务监听器	task	bpm_process_listener_type	0	success			1	2024-03-23 12:54:13	1	2024-03-23 19:14:24	0
1526	1	Java 类	class	bpm_process_listener_value_type	0	primary			1	2024-03-23 15:08:45	1	2024-03-23 19:14:32	0
1527	2	表达式	expression	bpm_process_listener_value_type	0	success			1	2024-03-23 15:09:06	1	2024-03-23 19:14:38	0
1528	3	代理表达式	delegateExpression	bpm_process_listener_value_type	0	info			1	2024-03-23 15:11:23	1	2024-03-23 19:14:41	0
1529	1	天	1	date_interval	0				1	2024-03-29 22:50:26	1	2024-03-29 22:50:26	0
1530	2	周	2	date_interval	0				1	2024-03-29 22:50:36	1	2024-03-29 22:50:36	0
1531	3	月	3	date_interval	0				1	2024-03-29 22:50:46	1	2024-03-29 22:50:54	0
1532	4	季度	4	date_interval	0				1	2024-03-29 22:51:01	1	2024-03-29 22:51:01	0
1533	5	年	5	date_interval	0				1	2024-03-29 22:51:07	1	2024-03-29 22:51:07	0
1534	1	赢单	1	crm_business_end_status_type	0	success			1	2024-04-13 23:26:57	1	2024-04-13 23:26:57	0
1535	2	输单	2	crm_business_end_status_type	0	primary			1	2024-04-13 23:27:31	1	2024-04-13 23:27:31	0
1536	3	无效	3	crm_business_end_status_type	0	info			1	2024-04-13 23:27:59	1	2024-04-13 23:27:59	0
1537	1	OpenAI	OpenAI	ai_platform	0				1	2024-05-09 22:33:47	1	2024-05-09 22:58:46	0
1538	2	Ollama	Ollama	ai_platform	0				1	2024-05-17 23:02:55	1	2024-05-17 23:02:55	0
1539	3	文心一言	YiYan	ai_platform	0				1	2024-05-18 09:24:20	1	2024-05-18 09:29:01	0
1540	4	讯飞星火	XingHuo	ai_platform	0				1	2024-05-18 10:08:56	1	2024-05-18 10:08:56	0
1541	5	通义千问	TongYi	ai_platform	0				1	2024-05-18 10:32:29	1	2024-07-06 15:42:29	0
1542	6	StableDiffusion	StableDiffusion	ai_platform	0				1	2024-06-01 15:09:31	1	2024-06-01 15:10:25	0
1543	10	进行中	10	ai_image_status	0	primary			1	2024-06-26 20:51:41	1	2024-06-26 20:52:48	0
1544	20	已完成	20	ai_image_status	0	success			1	2024-06-26 20:52:07	1	2024-06-26 20:52:41	0
1545	30	已失败	30	ai_image_status	0	warning			1	2024-06-26 20:52:25	1	2024-06-26 20:52:35	0
1546	7	Midjourney	Midjourney	ai_platform	0				1	2024-06-26 22:14:46	1	2024-06-26 22:14:46	0
1547	10	进行中	10	ai_music_status	0	primary			1	2024-06-27 22:45:22	1	2024-06-28 00:56:17	0
1548	20	已完成	20	ai_music_status	0	success			1	2024-06-27 22:45:33	1	2024-06-28 00:56:18	0
1549	30	已失败	30	ai_music_status	0	danger			1	2024-06-27 22:45:44	1	2024-06-28 00:56:19	0
1550	1	歌词模式	1	ai_generate_mode	0				1	2024-06-27 22:46:31	1	2024-06-28 01:22:25	0
1551	2	描述模式	2	ai_generate_mode	0				1	2024-06-27 22:46:37	1	2024-06-28 01:22:24	0
1552	8	Suno	Suno	ai_platform	0				1	2024-06-29 09:13:36	1	2024-06-29 09:13:41	0
1553	9	DeepSeek	DeepSeek	ai_platform	0				1	2024-07-06 12:04:30	1	2024-07-06 12:05:20	0
1554	13	智谱	ZhiPu	ai_platform	0				1	2024-07-06 18:00:35	1	2025-02-24 20:18:41	0
1555	4	长	4	ai_write_length	0				1	2024-07-07 15:49:03	1	2024-07-07 15:49:03	0
1556	5	段落	5	ai_write_format	0				1	2024-07-07 15:49:54	1	2024-07-07 15:49:54	0
1557	6	文章	6	ai_write_format	0				1	2024-07-07 15:50:05	1	2024-07-07 15:50:05	0
1558	7	博客文章	7	ai_write_format	0				1	2024-07-07 15:50:23	1	2024-07-07 15:50:23	0
1559	8	想法	8	ai_write_format	0				1	2024-07-07 15:50:31	1	2024-07-07 15:50:31	0
1560	9	大纲	9	ai_write_format	0				1	2024-07-07 15:50:37	1	2024-07-07 15:50:37	0
1561	1	自动	1	ai_write_tone	0				1	2024-07-07 15:51:06	1	2024-07-07 15:51:06	0
1562	2	友善	2	ai_write_tone	0				1	2024-07-07 15:51:19	1	2024-07-07 15:51:19	0
1563	3	随意	3	ai_write_tone	0				1	2024-07-07 15:51:27	1	2024-07-07 15:51:27	0
1564	4	友好	4	ai_write_tone	0				1	2024-07-07 15:51:37	1	2024-07-07 15:51:37	0
1565	5	专业	5	ai_write_tone	0				1	2024-07-07 15:51:49	1	2024-07-07 15:52:02	0
1566	6	诙谐	6	ai_write_tone	0				1	2024-07-07 15:52:15	1	2024-07-07 15:52:15	0
1567	7	有趣	7	ai_write_tone	0				1	2024-07-07 15:52:24	1	2024-07-07 15:52:24	0
1568	8	正式	8	ai_write_tone	0				1	2024-07-07 15:54:33	1	2024-07-07 15:54:33	0
1570	1	自动	1	ai_write_format	0				1	2024-07-07 15:19:34	1	2024-07-07 15:19:34	0
1571	2	电子邮件	2	ai_write_format	0				1	2024-07-07 15:19:50	1	2024-07-07 15:49:30	0
1572	3	消息	3	ai_write_format	0				1	2024-07-07 15:20:01	1	2024-07-07 15:49:38	0
1573	4	评论	4	ai_write_format	0				1	2024-07-07 15:20:13	1	2024-07-07 15:49:45	0
1574	1	自动	1	ai_write_language	0				1	2024-07-07 15:44:18	1	2024-07-07 15:44:18	0
1575	2	中文	2	ai_write_language	0				1	2024-07-07 15:44:28	1	2024-07-07 15:44:28	0
1576	3	英文	3	ai_write_language	0				1	2024-07-07 15:44:37	1	2024-07-07 15:44:37	0
1577	4	韩语	4	ai_write_language	0				1	2024-07-07 15:46:28	1	2024-07-07 15:46:28	0
1578	5	日语	5	ai_write_language	0				1	2024-07-07 15:46:44	1	2024-07-07 15:46:44	0
1579	1	自动	1	ai_write_length	0				1	2024-07-07 15:48:34	1	2024-07-07 15:48:34	0
1580	2	短	2	ai_write_length	0				1	2024-07-07 15:48:44	1	2024-07-07 15:48:44	0
1581	3	中等	3	ai_write_length	0				1	2024-07-07 15:48:52	1	2024-07-07 15:48:52	0
1584	1	撰写	1	ai_write_type	0				1	2024-07-10 21:26:00	1	2024-07-10 21:26:00	0
1585	2	回复	2	ai_write_type	0				1	2024-07-10 21:26:06	1	2024-07-10 21:26:06	0
1586	2	腾讯云	TENCENT	system_sms_channel_code	0				1	2024-07-22 22:23:16	1	2024-07-22 22:23:16	0
1587	3	华为云	HUAWEI	system_sms_channel_code	0				1	2024-07-22 22:23:46	1	2024-07-22 22:23:53	0
1588	1	OpenAI 微软	AzureOpenAI	ai_platform	0				1	2024-08-10 14:07:41	1	2024-08-10 14:07:41	0
1589	10	BPMN 设计器	10	bpm_model_type	0	primary			1	2024-08-26 15:22:17	1	2024-08-26 16:46:02	0
1590	20	SIMPLE 设计器	20	bpm_model_type	0	success			1	2024-08-26 15:22:27	1	2024-08-26 16:45:58	0
1591	4	七牛云	QINIU	system_sms_channel_code	0				1	2024-08-31 08:45:03	1	2024-08-31 08:45:24	0
1592	3	新人券	3	promotion_coupon_take_type	0	info		新人注册后，自动发放	1	2024-09-03 11:57:16	1	2024-09-03 11:57:28	0
1593	5	微信零钱	5	brokerage_withdraw_type	0			API 打款	1	2024-10-13 11:06:48	1	2025-05-10 08:24:55	0
1683	10	字节豆包	DouBao	ai_platform	0				1	2025-02-23 19:51:40	1	2025-02-23 19:52:02	0
1684	11	腾讯混元	HunYuan	ai_platform	0				1	2025-02-23 20:58:04	1	2025-02-23 20:58:04	0
1685	12	硅基流动	SiliconFlow	ai_platform	0				1	2025-02-24 20:19:09	1	2025-02-24 20:19:09	0
1686	1	聊天	1	ai_model_type	0				1	2025-03-03 12:26:34	1	2025-03-03 12:26:34	0
1687	2	图像	2	ai_model_type	0				1	2025-03-03 12:27:23	1	2025-03-03 12:27:23	0
1688	3	音频	3	ai_model_type	0				1	2025-03-03 12:27:51	1	2025-03-03 12:27:51	0
1689	4	视频	4	ai_model_type	0				1	2025-03-03 12:28:03	1	2025-03-03 12:28:03	0
1690	5	向量	5	ai_model_type	0				1	2025-03-03 12:28:15	1	2025-03-03 12:28:15	0
1691	6	重排	6	ai_model_type	0				1	2025-03-03 12:28:26	1	2025-03-03 12:28:26	0
1692	14	MiniMax	MiniMax	ai_platform	0				1	2025-03-11 20:04:51	1	2025-03-11 20:04:51	0
1693	15	月之暗面	Moonshot	ai_platform	0				1	2025-03-11 20:05:08	1	2025-11-24 07:17:39	0
2002	0	直连设备	0	iot_product_device_type	0	default			1	2024-08-10 11:54:58	1	2025-03-17 09:28:22	0
2003	2	网关设备	2	iot_product_device_type	0	default			1	2024-08-10 11:55:08	1	2025-03-17 09:28:28	0
2004	1	网关子设备	1	iot_product_device_type	0	default			1	2024-08-10 11:55:20	1	2025-03-17 09:28:31	0
2005	1	已发布	1	iot_product_status	0	success			1	2024-08-10 12:10:33	1	2025-03-17 09:28:34	0
2006	0	开发中	0	iot_product_status	0	default			1	2024-08-10 14:19:18	1	2025-03-17 09:28:39	0
2009	0	Wi-Fi	0	iot_net_type	0				1	2024-09-06 22:04:47	1	2025-03-17 09:28:47	0
2010	1	移动网络	1	iot_net_type	0				1	2024-09-06 22:05:14	1	2025-06-12 23:27:19	0
2011	2	以太网	2	iot_net_type	0				1	2024-09-06 22:05:35	1	2025-03-17 09:28:51	0
2012	3	其他	3	iot_net_type	0				1	2024-09-06 22:05:52	1	2025-03-17 09:28:54	0
2018	0	未激活	0	iot_device_state	0				1	2024-09-21 08:13:34	1	2025-03-17 09:29:09	0
2019	1	在线	1	iot_device_state	0				1	2024-09-21 08:13:48	1	2025-03-17 09:29:12	0
2020	2	离线	2	iot_device_state	0				1	2024-09-21 08:13:59	1	2025-03-17 09:29:14	0
2021	1	属性	1	iot_thing_model_type	0				1	2024-09-29 20:03:01	1	2025-03-17 09:29:24	0
2022	2	服务	2	iot_thing_model_type	0				1	2024-09-29 20:03:11	1	2025-03-17 09:29:27	0
2023	3	事件	3	iot_thing_model_type	0				1	2024-09-29 20:03:20	1	2025-03-17 09:29:29	0
2030	1	升每分钟	L/min	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:34:24	0
2031	2	毫克每千克	mg/kg	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:34:27	0
2032	3	浊度	NTU	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:34:31	0
2033	4	PH值	pH	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:34:36	0
2034	5	土壤EC值	dS/m	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:34:43	0
2035	6	太阳总辐射	W/㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:36:20	0
2036	7	降雨量	mm/hour	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:36:24	0
2037	8	乏	var	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:36:27	0
2038	9	厘泊	cP	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:36:33	0
2039	10	饱和度	aw	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:11	0
2040	11	个	pcs	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:19	0
2041	12	厘斯	cst	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:22	0
2042	13	巴	bar	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:24	0
2043	14	纳克每升	ppt	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:27	0
2044	15	十亿分之一	ppb	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2026-04-05 15:53:29	0
2045	16	微西每厘米	uS/cm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:34	0
2046	17	牛顿每库仑	N/C	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:38	0
2047	18	伏特每米	V/m	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:43	0
2048	19	滴速	ml/min	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:46	0
2049	20	毫米汞柱	mmHg	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:48	0
2050	21	血糖	mmol/L	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:37:54	0
2051	22	毫米每秒	mm/s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:02	0
2052	23	转每米	turn/m	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2026-04-05 15:53:29	0
2053	24	次	count	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:09	0
2054	25	档	gear	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:11	0
2055	26	步	stepCount	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:13	0
2056	27	标准立方米每小时	Nm3/h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:15	0
2057	28	千伏	kV	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:20	0
2058	29	千伏安	kVA	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:38:24	0
2060	30	千乏	kVar	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2061	31	微瓦每平方厘米	uw/cm2	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2062	32	只	只	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2063	33	相对湿度	%RH	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2064	34	立方米每秒	m³/s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2065	35	公斤每秒	kg/s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2066	36	转每分钟	r/min	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2067	37	吨每小时	t/h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2068	38	千卡每小时	KCL/h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2069	39	升每秒	L/s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2070	40	兆帕	MPa	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2026-04-05 15:53:29	0
2071	41	立方米每小时	m³/h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2072	42	千乏时	kvarh	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2073	43	微克每升	μg/L	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2074	44	千卡路里	kcal	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2075	45	吉字节	GB	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2076	46	兆字节	MB	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2077	47	千字节	KB	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2078	48	字节	B	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2079	49	微克每平方分米每天	μg/(d㎡·d)	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2080	50	无		iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2081	51	百万分率	ppm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2082	52	像素	pixel	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2083	53	照度	Lux	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2084	54	重力加速度	grav	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2085	55	分贝	dB	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2086	56	百分比	%	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2087	57	流明	lm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2088	58	比特	bit	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2089	59	克每毫升	g/mL	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2090	60	克每升	g/L	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2091	61	毫克每升	mg/L	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2092	62	微克每立方米	μg/m³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2093	63	毫克每立方米	mg/m³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2094	64	克每立方米	g/m³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2095	65	千克每立方米	kg/m³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2096	66	纳法	nF	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2097	67	皮法	pF	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2098	68	微法	μF	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2099	69	法拉	F	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2100	70	欧姆	Ω	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2101	71	微安	μA	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2102	72	毫安	mA	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2103	73	千安	kA	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2104	74	安培	A	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2105	75	毫伏	mV	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2106	76	伏特	V	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2107	77	毫秒	ms	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2108	78	秒	s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2109	79	分钟	min	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2110	80	小时	h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2111	81	日	day	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2112	82	周	week	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2113	83	月	month	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2114	84	年	year	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2115	85	节	kn	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2116	86	千米每小时	km/h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2117	87	米每秒	m/s	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2118	88	角秒	″	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2026-04-05 15:53:29	0
2119	89	分	′	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2120	90	度	°	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2121	91	弧度	rad	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2122	92	赫兹	Hz	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2123	93	微瓦	μW	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2124	94	毫瓦	mW	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2125	95	千瓦特	kW	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2126	96	瓦特	W	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2127	97	卡路里	cal	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2128	98	千瓦时	kW·h	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2129	99	瓦时	Wh	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2130	100	电子伏	eV	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2131	101	千焦	kJ	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2132	102	焦耳	J	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2133	103	华氏度	℉	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2134	104	开尔文	K	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2135	105	吨	t	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2136	106	摄氏度	°C	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2137	107	毫帕	1e-3Pa	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2026-04-05 15:53:29	0
2138	108	百帕	hPa	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2139	109	千帕	kPa	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2140	110	帕斯卡	Pa	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2141	111	毫克	mg	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2142	112	克	g	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2143	113	千克	kg	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2144	114	牛	N	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2145	115	毫升	mL	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2146	116	升	L	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2147	117	立方毫米	mm³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2148	118	立方厘米	cm³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2149	119	立方千米	km³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2150	120	立方米	m³	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2151	121	公顷	h㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2152	122	平方厘米	c㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2153	123	平方毫米	m㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2154	124	平方千米	k㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2155	125	平方米	㎡	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2156	126	纳米	nm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2157	127	微米	μm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2158	128	毫米	mm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2159	129	厘米	cm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2160	130	分米	dm	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2161	131	千米	km	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2162	132	米	m	iot_thing_model_unit	0				1	2024-12-13 11:08:41	1	2025-03-17 09:40:46	0
2165	1	HTTP	1	iot_data_sink_type_enum	0	default			1	2025-03-09 12:39:54	1	2025-06-24 12:44:47	0
2166	2	TCP	2	iot_data_sink_type_enum	0	default			1	2025-03-09 12:40:06	1	2025-06-24 12:44:46	0
2167	3	WebSocket	3	iot_data_sink_type_enum	0	default			1	2025-03-09 12:40:24	1	2025-06-24 12:44:45	0
2168	10	MQTT	10	iot_data_sink_type_enum	0	default			1	2025-03-09 12:40:37	1	2025-06-24 12:44:44	0
2169	20	Database	20	iot_data_sink_type_enum	0	default			1	2025-03-09 12:41:05	1	2025-06-24 12:44:44	0
2170	21	Redis Stream	21	iot_data_sink_type_enum	0	default			1	2025-03-09 12:41:18	1	2025-06-24 12:44:43	0
2171	30	RocketMQ	30	iot_data_sink_type_enum	0	default			1	2025-03-09 12:41:30	1	2025-06-24 12:44:42	0
2172	31	RabbitMQ	31	iot_data_sink_type_enum	0	default			1	2025-03-09 12:41:47	1	2025-06-24 12:44:41	0
2173	32	Kafka	32	iot_data_sink_type_enum	0	default			1	2025-03-09 12:41:59	1	2025-06-24 12:44:39	0
2174	1	设备上下线变更	1	iot_rule_scene_trigger_type_enum	0	primary			1	2025-03-20 15:00:01	"1"	2025-07-06 10:28:16	0
2175	2	物模型属性上报	2	iot_rule_scene_trigger_type_enum	0	primary			1	2025-03-20 15:00:09	"1"	2025-07-06 10:28:22	0
2176	1	设备状态	state	iot_device_message_type_enum	0	primary			1	2025-03-20 15:24:58	1	2025-03-20 15:24:58	0
2177	2	设备属性	property	iot_device_message_type_enum	0	primary			1	2025-03-20 15:25:09	1	2025-03-20 15:25:09	0
2178	3	设备事件	event	iot_device_message_type_enum	0	primary			1	2025-03-20 15:25:23	1	2025-03-20 15:25:23	0
2179	4	设备服务	service	iot_device_message_type_enum	0	primary			1	2025-03-20 15:25:39	1	2025-03-20 15:25:39	0
2180	5	设备配置	config	iot_device_message_type_enum	0	primary			1	2025-03-20 15:25:51	1	2025-03-20 15:25:57	0
2181	6	设备 OTA	ota	iot_device_message_type_enum	0	primary			1	2025-03-20 15:26:17	1	2025-03-20 15:26:17	0
2182	7	设备注册	register	iot_device_message_type_enum	0	primary			1	2025-03-20 15:26:35	1	2025-03-20 15:26:35	0
2183	8	设备拓扑	topology	iot_device_message_type_enum	0	primary			1	2025-03-20 15:26:46	1	2025-03-20 15:26:46	0
2184	1	设备属性设置	1	iot_rule_scene_action_type_enum	0	primary			1	2025-03-28 15:27:12	"1"	2025-07-06 10:37:33	0
2185	2	设备服务调用	2	iot_rule_scene_action_type_enum	0	primary			1	2025-03-28 15:27:25	"1"	2025-07-06 10:37:41	0
2186	100	告警触发	100	iot_rule_scene_action_type_enum	0	primary			1	2025-03-28 15:27:35	"1"	2025-07-06 10:37:50	0
3000	16	百川智能	BaiChuan	ai_platform	0				1	2025-03-23 12:15:46	1	2025-03-23 12:15:46	0
3001	40	Vben5.0 Ant Design Schema 模版	40	infra_codegen_front_type	0			\N	1	2025-04-23 21:47:47	1	2025-09-04 23:25:12	0
3002	6	支付宝余额	6	brokerage_withdraw_type	0			API 打款	1	2025-05-10 08:24:49	1	2025-05-10 08:24:49	0
3004	3	WARN	3	iot_alert_level	0	warning			1	2025-06-27 20:32:22	1	2025-06-27 20:34:31	0
3005	1	INFO	1	iot_alert_level	0	primary			1	2025-06-27 20:33:28	1	2025-06-27 20:34:35	0
3006	5	ERROR	5	iot_alert_level	0	danger			1	2025-06-27 20:33:50	1	2025-06-27 20:33:50	0
3007	1	短信	1	iot_alert_receive_type	0				1	2025-06-27 22:49:30	1	2025-06-27 22:49:30	0
3008	2	邮箱	2	iot_alert_receive_type	0				1	2025-06-27 22:49:39	1	2025-06-27 22:50:07	0
3009	3	站内信	3	iot_alert_receive_type	0				1	2025-06-27 22:50:20	1	2025-06-27 22:50:20	0
3010	1	全部设备	1	iot_ota_task_device_scope	0				1	2025-07-02 09:43:09	1	2025-07-02 09:43:09	0
3011	2	指定设备	2	iot_ota_task_device_scope	0				1	2025-07-02 09:43:15	1	2025-07-02 09:43:15	0
3012	10	进行中	10	iot_ota_task_status	0	primary			1	2025-07-02 09:44:01	"1"	2025-07-02 09:44:21	0
3013	20	已结束	20	iot_ota_task_status	0	success			1	2025-07-02 09:44:14	"1"	2025-07-02 23:56:12	0
3014	30	已取消	30	iot_ota_task_status	0	danger			1	2025-07-02 09:44:36	1	2025-07-02 09:44:36	0
3015	0	待推送	0	iot_ota_task_record_status	0				1	2025-07-02 09:45:16	1	2025-07-02 09:45:16	0
3016	10	已推送	10	iot_ota_task_record_status	0				1	2025-07-02 09:45:25	1	2025-07-02 09:45:25	0
3017	20	升级中	20	iot_ota_task_record_status	0	primary			1	2025-07-02 09:45:37	1	2025-07-02 09:45:37	0
3018	30	升级成功	30	iot_ota_task_record_status	0	success			1	2025-07-02 09:45:47	1	2025-07-02 09:45:47	0
3019	40	升级失败	40	iot_ota_task_record_status	0	danger			1	2025-07-02 09:46:02	1	2025-07-02 09:46:02	0
3020	50	升级取消	50	iot_ota_task_record_status	0	warning			1	2025-07-02 09:46:09	"1"	2025-07-02 09:46:27	0
3024	3	设备事件上报	3	iot_rule_scene_trigger_type_enum	0				1	2025-07-06 10:28:29	1	2025-07-06 10:28:29	0
3025	4	设备服务调用	4	iot_rule_scene_trigger_type_enum	0				1	2025-07-06 10:28:35	1	2025-07-06 10:28:35	0
3026	100	定时触发	100	iot_rule_scene_trigger_type_enum	0				1	2025-07-06 10:28:48	1	2025-07-06 10:28:48	0
3027	101	告警恢复	101	iot_rule_scene_action_type_enum	0				1	2025-07-06 10:37:57	1	2025-07-06 10:37:57	0
3028	2	Anthropic	Anthropic	ai_platform	0				1	2025-08-21 22:54:24	1	2025-08-21 22:57:58	0
3029	2	谷歌 Gemini	Gemini	ai_platform	0				1	2025-08-22 22:39:35	1	2025-08-22 22:44:49	0
3030	1	文件系统	filesystem	ai_mcp_client_name	0				1	2025-08-28 13:58:43	1	2025-08-28 21:19:42	0
3031	41	Vben5.0 Ant Design 标准模版	41	infra_codegen_front_type	0				1	2025-09-04 23:26:07	1	2025-09-04 23:26:07	0
3032	50	Vben5.0 Element Plus Schema 模版	50	infra_codegen_front_type	0				1	2025-09-04 23:26:38	1	2025-09-04 23:26:38	0
3033	51	Vben5.0 Element Plus 标准模版	51	infra_codegen_front_type	0				1	2025-09-04 23:26:49	1	2025-09-04 23:26:49	0
3034	1	ttt	tt	iot_ota_task_record_status	0	success		\N	1	2025-09-06 00:02:21	1	2025-09-06 00:02:31	0
3035	40	支付宝小程序	40	system_social_type	0				1	2023-11-04 13:05:38	1	2023-11-04 13:07:16	0
3036	60	Admin Uniapp 移动端	60	infra_codegen_front_type	0			\N	1	2025-12-16 19:25:51	1	2025-12-17 09:46:15	0
3037	42	Vben5.0 Antdv Next Schema 模版	42	infra_codegen_front_type	0				1	2026-07-14 04:45:56.4759	1	2026-07-14 04:45:56.4759	0
3038	43	Vben5.0 Antdv Next 标准模版	43	infra_codegen_front_type	0				1	2026-07-14 04:45:56.4759	1	2026-07-14 04:45:56.4759	0
3040	1	UDP	udp	iot_protocol_type	0			UDP 协议	1	2026-02-04 00:32:47	1	2026-02-04 00:32:47	0
3041	2	WebSocket	websocket	iot_protocol_type	0			WebSocket 协议	1	2026-02-04 00:32:55	1	2026-02-04 00:32:55	0
3042	3	HTTP	http	iot_protocol_type	0			HTTP 协议	1	2026-02-04 00:32:55	1	2026-02-04 00:32:55	0
3043	4	MQTT	mqtt	iot_protocol_type	0	success		MQTT 协议	1	2026-02-04 00:32:55	1	2026-02-04 00:32:55	0
3044	5	EMQX	emqx	iot_protocol_type	0	success		EMQX 协议	1	2026-02-04 00:32:55	1	2026-02-04 00:32:55	0
3045	6	CoAP	coap	iot_protocol_type	0			CoAP 协议	1	2026-02-04 00:32:55	1	2026-02-04 00:32:55	0
3046	7	Modbus TCP Server	modbus_tcp_server	iot_protocol_type	0			Modbus TCP Server 协议	1	2026-02-04 00:32:55	1	2026-02-12 15:16:45	0
3047	0	JSON	json	iot_serialize_type	0	success		JSON 格式	1	2026-02-04 00:33:19	1	2026-02-04 00:33:19	0
3048	1	二进制	binary	iot_serialize_type	0	warning		二进制格式	1	2026-02-04 00:33:19	1	2026-02-04 00:33:19	0
3049	8	Modbus TCP Client	modbus_tcp_client	iot_protocol_type	0			Modbus TCP Client 协议	1	2026-02-08 18:29:46	1	2026-02-12 15:16:32	0
3050	2	边缘采集	2	iot_modbus_mode	0	success		设备主动上报数据，无需轮询	1	2025-06-12 22:56:06	1	2026-02-09 13:03:23	0
3051	1	Modbus TCP	1	iot_modbus_frame_format	0	default		MBAP 头部格式	1	2025-06-12 22:56:06	1	2025-06-12 22:56:06	0
3052	2	Modbus RTU	2	iot_modbus_frame_format	0	warning		CRC16 校验格式	1	2025-06-12 22:56:06	1	2025-06-12 22:56:06	0
3053	1	云端轮询	1	iot_modbus_mode	0	primary		网关主动轮询读取设备寄存器	1	2025-06-12 22:56:06	1	2025-06-12 22:56:06	0
3054	1	企业客户	1	mes_client_type	0	primary			1	2026-02-15 14:38:25	1	2026-02-15 14:38:25	0
3055	2	个人	2	mes_client_type	0	success			1	2026-02-15 14:38:25	1	2026-02-15 14:38:25	0
3056	1	优质供应商	A	mes_vendor_level	0	success			1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0
3057	2	正常	B	mes_vendor_level	0	primary			1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0
3058	3	重点关注	C	mes_vendor_level	0	warning			1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0
3059	4	劣质供应商	D	mes_vendor_level	0	danger			1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0
3060	5	黑名单	E	mes_vendor_level	0	info			1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0
3061	1	假期	2	mes_cal_holiday_type	0	success			1	2026-02-16 07:35:58	1	2026-02-16 11:20:42	0
3062	2	工作日	1	mes_cal_holiday_type	0	primary			1	2026-02-16 07:35:58	1	2026-02-16 11:20:40	0
3063	1	在库	1	mes_tm_tool_status	0	success			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3064	2	领用中	2	mes_tm_tool_status	0	primary			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3065	3	维修中	3	mes_tm_tool_status	0	warning			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3066	4	报废	4	mes_tm_tool_status	0	danger			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3067	1	定期维护	1	mes_tm_mainten_type	0	primary			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3068	2	按使用次数维护	2	mes_tm_mainten_type	0	success			1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0
3069	1	停机	1	mes_dv_machinery_status	0	success			1	2026-02-17 01:00:06	1	2026-02-17 03:28:27	0
3070	2	生产中	2	mes_dv_machinery_status	0	info			1	2026-02-17 01:00:06	1	2026-02-17 03:28:33	0
3071	3	维护中	3	mes_dv_machinery_status	0	danger			1	2026-02-17 01:00:06	1	2026-02-17 03:28:41	0
3072	1	尺寸	1	mes_indicator_type	0				1	2026-02-17 02:18:18	1	2026-04-09 14:38:53	0
3073	2	外观	2	mes_indicator_type	0				1	2026-02-17 02:18:18	1	2026-04-09 14:38:53	0
3074	3	重量	3	mes_indicator_type	0				1	2026-02-17 02:18:18	1	2026-04-09 14:38:53	0
3075	4	性能	4	mes_indicator_type	0				1	2026-02-17 02:18:18	1	2026-04-09 14:38:53	0
3076	5	成分	5	mes_indicator_type	0				1	2026-02-17 02:18:18	1	2026-04-09 14:38:53	0
3077	1	致命缺陷	1	mes_defect_level	0	danger			1	2026-02-17 02:18:18	1	2026-02-21 12:21:12	0
3078	2	严重缺陷	2	mes_defect_level	0	warning			1	2026-02-17 02:18:18	1	2026-02-21 12:21:15	0
3079	3	轻微缺陷	3	mes_defect_level	0	info			1	2026-02-17 02:18:18	1	2026-02-21 12:21:19	0
3080	1	单白班	1	mes_cal_shift_type	0	primary			1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3081	2	两班倒	2	mes_cal_shift_type	0	success			1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3082	3	三班倒	3	mes_cal_shift_type	0	warning			1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3083	1	按季度	1	mes_cal_shift_method	0				1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3084	2	按月	2	mes_cal_shift_method	0				1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3085	3	按周	3	mes_cal_shift_method	0				1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3086	4	按天	4	mes_cal_shift_method	0				1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3089	0	草稿	0	mes_cal_plan_status	0	info			1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3090	1	已确认	1	mes_cal_plan_status	0	success			1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0
3100	0	草稿	0	mes_pro_work_order_status	0	info			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3101	1	已确认	1	mes_pro_work_order_status	0	primary			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3102	2	已完成	2	mes_pro_work_order_status	0	success			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3103	3	已取消	3	mes_pro_work_order_status	0	warning			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3104	1	客户订单	1	mes_pro_work_order_source_type	0	primary			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3105	2	库存备货	2	mes_pro_work_order_source_type	0	success			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3106	1	自行生产	1	mes_pro_work_order_type	0	primary			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3107	2	代工	2	mes_pro_work_order_type	0	warning			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3108	3	采购	3	mes_pro_work_order_type	0	info			1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0
3121	1	IQC（来料检验）	1	mes_qc_type	0	primary		来料质量检验	1	2026-02-18 14:12:05	1	2026-02-18 14:12:05	0
3122	2	IPQC（过程检验）	2	mes_qc_type	0	warning		生产制程质量检验	1	2026-02-18 14:12:05	1	2026-03-24 15:21:34	0
3123	3	OQC（出货检验）	3	mes_qc_type	0	success		出货质量检验	1	2026-02-18 14:12:05	1	2026-02-18 14:12:05	0
3124	4	RQC（退料检验）	4	mes_qc_type	0	danger		退货质量检验	1	2026-02-18 14:12:05	1	2026-03-24 15:22:00	0
3125	0	开始-开始(SS)	0	mes_pro_link_type	0	default		前序开始后，后序可以开始	1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3126	1	结束-结束(FF)	1	mes_pro_link_type	0	default		前序结束后，后序才能结束	1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3127	2	开始-结束(SF)	2	mes_pro_link_type	0	default		前序开始后，后序才能结束	1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3128	3	结束-开始(FS)	3	mes_pro_link_type	0	default		前序结束后，后序才能开始	1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3129	1	分钟	MINUTE	mes_time_unit_type	0	default			1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3130	2	小时	HOUR	mes_time_unit_type	0	default			1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3131	3	天	DAY	mes_time_unit_type	0	default			1	2026-02-19 04:24:53	1	2026-02-19 04:24:53	0
3137	1	设备点检	1	mes_dv_subject_type	0	info			1	2026-02-20 01:42:58	1	2026-02-20 01:42:58	0
3138	2	设备保养	2	mes_dv_subject_type	0	success			1	2026-02-20 01:42:58	1	2026-02-20 01:42:58	0
3139	1	待保养	0	mes_mainten_record_status	0	info		\N	admin	2026-02-20 02:59:55	1	2026-04-16 05:32:37	0
3140	2	已完成	4	mes_mainten_record_status	0	success		\N	admin	2026-02-20 02:59:55	1	2026-04-16 05:32:37	0
3141	1	正常	1	mes_mainten_status	0	success		\N	admin	2026-02-20 02:59:55	admin	2026-02-20 02:59:55	0
3142	2	异常	0	mes_mainten_status	0	danger		\N	admin	2026-02-20 02:59:55	admin	2026-02-20 02:59:55	0
3143	1	天	1	mes_dv_cycle_type	0	default			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3144	2	周	2	mes_dv_cycle_type	0	default			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3145	3	月	3	mes_dv_cycle_type	0	default			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3146	4	年	4	mes_dv_cycle_type	0	default			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3147	0	草稿	0	mes_dv_check_plan_status	0	info			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3148	1	已启用	1	mes_dv_check_plan_status	0	success			1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0
3149	1	待点检	10	mes_dv_check_record_status	0	info		\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0
3150	2	已完成	20	mes_dv_check_record_status	0	success		\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0
3151	1	正常	1	mes_dv_check_result	0	success		\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0
3152	2	异常	2	mes_dv_check_result	0	danger		\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0
3157	1	修复成功	1	mes_dv_repair_result	0	success			1	2026-02-20 10:56:24	1	2026-02-20 10:56:24	0
3158	2	报废	2	mes_dv_repair_result	0	danger			1	2026-02-20 10:56:24	1	2026-02-20 10:56:24	0
3161	1	校验通过	1	mes_qc_check_result	0	success			1	2026-02-20 11:23:35	1	2026-02-20 16:15:54	0
3162	2	校验不通过	2	mes_qc_check_result	0	danger			1	2026-02-20 11:23:35	1	2026-02-20 16:15:52	0
3166	0	未处置	0	mes_pro_andon_status	0	danger			1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0
3167	1	已处置	1	mes_pro_andon_status	0	success			1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0
3168	1	一级	1	mes_pro_andon_level	0	danger			1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0
3169	2	二级	2	mes_pro_andon_level	0	warning			1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0
3170	3	三级	3	mes_pro_andon_level	0	info			1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0
3171	0	草稿	0	mes_pro_feedback_status	0	info			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3172	2	审批中	2	mes_pro_feedback_status	0	primary			1	2026-02-21 00:50:32	1	2026-03-19 00:51:54	0
3173	3	待检验	3	mes_pro_feedback_status	0	warning			1	2026-02-21 00:50:32	1	2026-03-19 00:51:54	0
3174	4	已完成	4	mes_pro_feedback_status	0	success			1	2026-02-21 00:50:32	1	2026-03-19 00:51:54	0
3176	1	自行报工	1	mes_pro_feedback_type	0	primary			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3177	2	统一报工	2	mes_pro_feedback_type	0	success			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3178	1	PC	PC	mes_pro_feedback_channel	0	primary			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3179	2	APP	APP	mes_pro_feedback_channel	0	success			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3180	3	PDA	PDA	mes_pro_feedback_channel	0	info			1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0
3181	1	浮点	1	mes_qc_result_type	0	primary			1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0
3182	2	整数	2	mes_qc_result_type	0	success			1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0
3183	3	文本	3	mes_qc_result_type	0	info			1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0
3184	4	字典	4	mes_qc_result_type	0	warning			1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0
3185	5	文件	5	mes_qc_result_type	0	danger			1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0
3186	1	生产退料	1	mes_rqc_type	0	default		生产退料检验	1	2026-02-22 06:44:09	1	2026-02-22 06:44:09	0
3187	2	销售退货	2	mes_rqc_type	0	default		销售退货检验	1	2026-02-22 06:44:09	1	2026-02-22 06:44:09	0
3188	1	自制工序检验	1	mes_ipqc_type	0	primary			1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0
3189	2	首检	2	mes_ipqc_type	0	success			1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0
3190	3	巡检	3	mes_ipqc_type	0	warning			1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0
3191	4	自检	4	mes_ipqc_type	0	info			1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0
3192	5	成品检验	5	mes_ipqc_type	0	danger			1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0
3205	0	草稿	0	mes_wm_arrival_notice_status	0	info			1	2026-02-22 14:53:18	1	2026-02-22 14:53:18	0
3206	2	待质检	2	mes_wm_arrival_notice_status	0	warning			1	2026-02-22 14:53:18	1	2026-02-26 05:24:40	0
3207	3	待入库	3	mes_wm_arrival_notice_status	0	success			1	2026-02-22 14:53:18	1	2026-02-26 05:24:47	0
3208	4	已完成	4	mes_wm_arrival_notice_status	0	primary			1	2026-02-22 14:53:18	1	2026-02-26 05:24:52	0
3209	0	草稿	0	mes_wm_item_receipt_status	0	info			1	2026-02-22 14:54:05	1	2026-02-22 14:54:05	0
3210	1	待上架	2	mes_wm_item_receipt_status	0	warning			1	2026-02-22 14:54:05	1	2026-02-26 08:03:35	0
3211	2	待执行入库	3	mes_wm_item_receipt_status	0	success			1	2026-02-22 14:54:05	1	2026-02-26 08:03:31	0
3212	3	已完成	4	mes_wm_item_receipt_status	0	primary			1	2026-02-22 14:54:05	1	2026-02-26 08:03:20	0
3213	4	已取消	5	mes_wm_item_receipt_status	0	danger			1	2026-02-22 14:54:05	1	2026-02-26 08:03:24	0
3214	1	草稿	0	mes_order_status	0	info			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3215	2	已确认	1	mes_order_status	0	primary			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3216	3	审批中	2	mes_order_status	0	warning			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3217	4	已审批	3	mes_order_status	0	success			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3218	5	已完成	4	mes_order_status	0	success			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3219	6	已取消	5	mes_order_status	0	danger			1	2026-02-23 21:16:03	1	2026-02-23 21:16:03	0
3220	1	草稿	0	mes_wm_issue_status	0	info		草稿状态，未完成	1	2026-02-26 15:54:25	1	2026-02-26 15:54:25	0
3221	2	已完成	4	mes_wm_issue_status	0	success		已完成出库	1	2026-02-26 15:54:25	1	2026-02-26 15:54:25	0
3222	1	草稿	0	mes_wm_product_issue_status	0	info		草稿状态，可编辑	1	2026-02-26 16:39:12	1	2026-03-23 13:18:02	0
3223	2	待拣货	2	mes_wm_product_issue_status	0	warning		审批中，可执行拣货	1	2026-02-26 16:39:12	1	2026-03-23 13:18:02	0
3224	3	待执行领出	3	mes_wm_product_issue_status	0	primary		已审批，拣货完成	1	2026-02-26 16:39:12	1	2026-03-23 13:18:02	0
3225	4	已完成	4	mes_wm_product_issue_status	0	success		已完成出库	1	2026-02-26 16:39:12	1	2026-03-23 13:18:02	0
3226	5	已取消	5	mes_wm_product_issue_status	0	success		已完成出库	1	2026-02-26 16:39:12	1	2026-03-23 13:18:02	0
3232	1	草稿	0	mes_wm_return_issue_status	0	info		草稿状态，可编辑	1	2026-02-28 14:11:12	1	2026-02-28 14:28:24	0
3233	2	待检验	1	mes_wm_return_issue_status	0	default		已确认，等待质检	1	2026-02-28 14:11:12	1	2026-02-28 14:28:28	0
3234	3	待上架	2	mes_wm_return_issue_status	0	warning		检验完成，等待仓库上架	1	2026-02-28 14:11:12	1	2026-02-28 14:28:31	0
3235	4	待执行退料	3	mes_wm_return_issue_status	0	primary		上架完成，等待执行退料操作	1	2026-02-28 14:11:12	1	2026-02-28 14:28:34	0
3236	5	已完成	4	mes_wm_return_issue_status	0	success		退料执行完成，库存已更新	1	2026-02-28 14:11:12	1	2026-02-28 14:28:37	0
3237	6	已取消	5	mes_wm_return_issue_status	0	danger		已取消	1	2026-02-28 14:11:12	1	2026-02-28 14:28:40	0
3238	1	余料退料	1	mes_wm_return_issue_type	0	success		余料退回，直接合格	1	2026-02-28 14:11:12	1	2026-02-28 14:27:47	0
3239	2	不良退料	2	mes_wm_return_issue_type	0	danger		不良品退回	1	2026-02-28 14:11:12	1	2026-02-28 14:27:49	0
3240	3	其他退料	3	mes_wm_return_issue_type	0	info		其他原因退料	1	2026-02-28 14:11:12	1	2026-02-28 14:27:55	0
3241	1	待检	0	mes_wm_quality_status	0	warning		待检状态	1	2026-02-28 15:00:53	1	2026-02-28 15:00:53	0
3242	2	合格	1	mes_wm_quality_status	0	success		合格状态	1	2026-02-28 15:00:53	1	2026-02-28 15:00:53	0
3243	3	不合格	2	mes_wm_quality_status	0	danger		不合格状态	1	2026-02-28 15:00:53	1	2026-02-28 15:00:53	0
3244	1	草稿	0	mes_wm_product_receipt_status	0	info		草稿状态	1	2026-03-01 06:03:07	1	2026-03-01 06:03:07	0
3245	2	待上架	2	mes_wm_product_receipt_status	0	primary		待上架	1	2026-03-01 06:03:07	1	2026-03-01 06:03:07	0
3246	3	待执行入库	3	mes_wm_product_receipt_status	0	warning		待执行入库	1	2026-03-01 06:03:07	1	2026-03-01 06:03:07	0
3247	4	已完成	4	mes_wm_product_receipt_status	0	success		已完成	1	2026-03-01 06:03:07	1	2026-03-01 06:03:07	0
3248	5	已取消	5	mes_wm_product_receipt_status	0	danger		已取消	1	2026-03-01 06:03:07	1	2026-03-01 06:03:07	0
3252	1	草稿	0	mes_wm_product_sales_status	0	info		草稿状态	1	2026-03-02 08:55:11	1	2026-03-02 08:55:11	0
3253	3	待拣货	2	mes_wm_product_sales_status	0	warning		待拣货状态	1	2026-03-02 08:55:11	1	2026-03-27 11:44:48	0
3254	4	待出库	3	mes_wm_product_sales_status	0	primary		待出库状态	1	2026-03-02 08:55:11	1	2026-03-27 11:44:48	0
3255	5	已完成	4	mes_wm_product_sales_status	0	success		已完成状态	1	2026-03-02 08:55:11	1	2026-03-27 11:44:48	0
3256	6	已取消	5	mes_wm_product_sales_status	0	danger		已取消状态	1	2026-03-02 08:55:11	1	2026-03-27 11:44:48	0
3272	1	草稿	0	mes_wm_misc_receipt_status	0	info		草稿状态	1	2026-03-03 07:33:41	1	2026-03-03 07:33:41	0
3273	2	待执行入库	3	mes_wm_misc_receipt_status	0	primary		待执行入库状态	1	2026-03-03 07:33:41	1	2026-03-03 07:37:34	0
3274	3	已完成	4	mes_wm_misc_receipt_status	0	success		已完成状态	1	2026-03-03 07:33:41	1	2026-03-03 07:33:41	0
3275	4	已取消	5	mes_wm_misc_receipt_status	0	danger		已取消状态	1	2026-03-03 07:33:41	1	2026-03-03 07:33:41	0
3277	1	库存调整	1	mes_wm_misc_receipt_type	0	primary		库存调整入库	1	2026-03-03 07:34:33	1	2026-03-03 07:34:33	0
3278	1	库存调整	1	mes_wm_misc_issue_type	0	primary		库存调整出库	1	2026-03-03 07:34:33	1	2026-03-03 07:34:33	0
3279	2	报废出库	2	mes_wm_misc_issue_type	0	danger		报废出库	1	2026-03-03 07:36:13	1	2026-03-03 07:36:13	0
3280	1	草稿	0	mes_wm_outsource_receipt_status	0	info		草稿状态	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3281	2	待检验	1	mes_wm_outsource_receipt_status	0	warning		已确认，等待质检	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3282	3	待上架	2	mes_wm_outsource_receipt_status	0	primary		检验完成，等待仓库上架	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3283	4	待执行入库	3	mes_wm_outsource_receipt_status	0	warning		上架完成，等待执行入库操作	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3284	5	已完成	4	mes_wm_outsource_receipt_status	0	success		入库执行完成，库存已更新	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3285	6	已取消	5	mes_wm_outsource_receipt_status	0	danger		已取消	1	2026-03-03 14:03:57	1	2026-03-03 14:03:57	0
3286	1	草稿	0	mes_wm_outsource_issue_status	0	info		草稿状态，可编辑、删除、执行出库	1	2026-03-03 16:31:00	1	2026-03-03 16:31:00	0
3287	2	待拣货	2	mes_wm_outsource_issue_status	0	warning		待拣货状态	1	2026-03-03 16:31:00	1	2026-03-03 16:31:00	0
3288	3	待执行出库	3	mes_wm_outsource_issue_status	0	primary		待执行出库状态	1	2026-03-03 16:31:00	1	2026-03-03 16:31:00	0
3289	4	已完成	4	mes_wm_outsource_issue_status	0	success		已完成，库存已扣减	1	2026-03-03 16:31:00	1	2026-03-03 16:31:00	0
3290	5	已取消	5	mes_wm_outsource_issue_status	0	danger		已取消状态	1	2026-03-03 16:31:00	1	2026-03-03 16:31:00	0
3301	1	输入字符	1	mes_md_auto_code_part_type	0	default		输入字符	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3302	2	当前日期	2	mes_md_auto_code_part_type	0	primary		当前日期时间	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3303	3	固定字符	3	mes_md_auto_code_part_type	0	success		固定字符	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3304	4	流水号	4	mes_md_auto_code_part_type	0	warning		流水号	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3305	1	左补齐	1	mes_md_auto_code_padded_method	0	primary		左补齐	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3306	2	右补齐	2	mes_md_auto_code_padded_method	0	success		右补齐	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3307	1	按年	1	mes_md_auto_code_cycle_method	0	default		按年循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3308	2	按月	2	mes_md_auto_code_cycle_method	0	primary		按月循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3309	3	按天	3	mes_md_auto_code_cycle_method	0	success		按天循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3310	4	按小时	4	mes_md_auto_code_cycle_method	0	warning		按小时循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3311	5	按分钟	5	mes_md_auto_code_cycle_method	0	danger		按分钟循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3312	10	按传入字符	10	mes_md_auto_code_cycle_method	0	info		按传入字符循环	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0
3313	1	二维码	1	mes_wm_barcode_format	0	primary		QR_CODE	1	2026-03-05 14:37:20	1	2026-03-06 13:18:21	0
3314	2	EAN13 商品条码	2	mes_wm_barcode_format	0	success		EAN13	1	2026-03-05 14:37:20	1	2026-03-06 13:18:23	0
3315	3	CODE39 工业条码	3	mes_wm_barcode_format	0	info		CODE39	1	2026-03-05 14:37:20	1	2026-03-06 13:18:25	0
3316	4	UPC-A 美国商品码	4	mes_wm_barcode_format	0	warning		UPC_A	1	2026-03-05 14:37:20	1	2026-03-06 13:18:28	0
3318	3	库位	104	mes_wm_barcode_biz_type	0	default		AREA	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3319	4	装箱单	105	mes_wm_barcode_biz_type	0	default		PACKAGE	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3320	5	库存	106	mes_wm_barcode_biz_type	0	default		STOCK	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3321	6	批次	107	mes_wm_barcode_biz_type	0	default		BATCH	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3322	7	流转卡	300	mes_wm_barcode_biz_type	0	primary		PROCARD	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3323	8	工单	301	mes_wm_barcode_biz_type	0	primary		WORKORDER	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3324	9	流转单	302	mes_wm_barcode_biz_type	0	primary		TRANSORDER	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3325	10	设备	400	mes_wm_barcode_biz_type	0	success		MACHINERY	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3327	12	产品物料	600	mes_wm_barcode_biz_type	0	info		ITEM	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3328	13	供应商	601	mes_wm_barcode_biz_type	0	info		VENDOR	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3329	14	工作站	602	mes_wm_barcode_biz_type	0	info		WORKSTATION	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3330	15	车间	603	mes_wm_barcode_biz_type	0	info		WORKSHOP	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3331	16	人员	604	mes_wm_barcode_biz_type	0	info		USER	1	2026-03-05 14:37:20	1	2026-03-07 06:25:19	0
3351	1	仓库	102	mes_wm_barcode_biz_type	0			\N		2026-03-07 06:22:27		2026-03-07 06:25:19	0
3352	2	库区	103	mes_wm_barcode_biz_type	0			\N		2026-03-07 06:22:27		2026-03-07 06:25:19	0
3353	11	工具	500	mes_wm_barcode_biz_type	0			\N		2026-03-07 06:22:27		2026-03-07 06:22:27	0
3354	17	客户	605	mes_wm_barcode_biz_type	0			\N		2026-03-07 06:22:27		2026-03-07 06:25:19	0
3355	1	草稿	0	mes_wm_package_status	0	info		草稿状态，可编辑	1	2026-03-08 02:05:46	1	2026-03-08 02:05:46	0
3356	2	已完成	4	mes_wm_package_status	0	success		装箱已完成	1	2026-03-08 02:05:46	1	2026-03-08 02:05:46	0
3357	1	草稿	0	mes_wm_transfer_status	0	info		草稿状态，可编辑	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3358	2	待确认	1	mes_wm_transfer_status	0	warning		外部调拨待确认到货	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3359	3	待上架	2	mes_wm_transfer_status	0	primary		待维护目标库位明细	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3360	4	待执行	3	mes_wm_transfer_status	0	success		目标库位已分配，待执行调拨	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3361	5	已完成	4	mes_wm_transfer_status	0	success		调拨已完成	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3362	6	已取消	5	mes_wm_transfer_status	0	danger		调拨已取消	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3363	1	内部调拨	1	mes_wm_transfer_type	0	success		内部仓储调拨	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3364	2	外部调拨	2	mes_wm_transfer_type	0	warning		外部配送/外部收货调拨	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0
3365	1	静态盘点	1	mes_wm_stock_taking_type	0	primary		静态盘点	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3366	2	动态盘点	2	mes_wm_stock_taking_type	0	success		动态盘点	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3367	1	仓库	102	mes_wm_stock_taking_plan_param_type	0	primary		按仓库盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3368	2	库区	103	mes_wm_stock_taking_plan_param_type	0	success		按库区盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3369	3	库位	104	mes_wm_stock_taking_plan_param_type	0	info		按库位盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3370	4	物料	600	mes_wm_stock_taking_plan_param_type	0	warning		按物料盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3371	5	批次	107	mes_wm_stock_taking_plan_param_type	0	danger		按批次盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3372	1	草稿	0	mes_wm_stock_taking_task_status	0	info		草稿	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3373	2	审批中	2	mes_wm_stock_taking_task_status	0	primary		盘点任务审批中	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3374	3	已完成	4	mes_wm_stock_taking_task_status	0	success		已完成	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3375	4	已取消	5	mes_wm_stock_taking_task_status	0	danger		已取消	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3377	1	正常	1	mes_wm_stock_taking_task_line_status	0	success		正常	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3378	2	盘盈	2	mes_wm_stock_taking_task_line_status	0	primary		盘盈	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3379	3	盘亏	3	mes_wm_stock_taking_task_line_status	0	danger		盘亏	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0
3380	6	质量状态	900	mes_wm_stock_taking_plan_param_type	0	default		按质量状态盘点	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0
3381	1	物料	ITEM	mes_md_item_or_product	0	info			1	2026-03-15 01:55:06	1	2026-03-15 01:55:06	0
3382	2	产品	PRODUCT	mes_md_item_or_product	0	success			1	2026-03-15 01:55:06	1	2026-03-15 01:55:06	0
3383	1	草稿	0	mes_wm_item_consume_status	0	info		草稿状态	1	2026-03-19 15:06:23	1	2026-03-19 15:06:23	0
3384	2	已完成	4	mes_wm_item_consume_status	0	success		已完成	1	2026-03-19 15:06:23	1	2026-03-19 15:06:23	0
3385	1	到货通知单	100	mes_qc_source_doc_type	0	primary		IQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3386	2	外协入库单	121	mes_qc_source_doc_type	0	warning		IQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3387	3	生产报工	304	mes_qc_source_doc_type	0	success		IPQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3388	4	销售出库单	118	mes_qc_source_doc_type	0	info		OQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3389	5	生产退料单	116	mes_qc_source_doc_type	0	danger		RQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3390	6	销售退货单	119	mes_qc_source_doc_type	0	default		RQC	1	2026-03-26 13:01:09	1	2026-03-26 13:01:09	0
3397	2	待检测	1	mes_wm_product_sales_status	0	warning		OQC 检验中	1	2026-03-27 11:44:48	1	2026-03-27 11:44:48	0
3398	0	草稿	0	mes_wm_return_vendor_status	0	info		\N		2026-03-29 13:49:57		2026-04-05 15:53:46	0
3399	1	待拣货	2	mes_wm_return_vendor_status	0	primary		\N		2026-03-29 13:49:57		2026-04-05 15:53:46	0
3400	2	待执行退货	3	mes_wm_return_vendor_status	0	warning		\N		2026-03-29 13:49:57		2026-04-05 15:53:46	0
3401	3	已完成	4	mes_wm_return_vendor_status	0	success		\N		2026-03-29 13:49:57		2026-04-05 15:53:46	0
3402	4	已取消	5	mes_wm_return_vendor_status	0	danger		\N		2026-03-29 13:49:57		2026-04-05 15:53:46	0
3403	1	草稿	0	mes_wm_sales_notice_status	0	info		草稿状态，可以修改和删除	1	2026-03-30 08:54:30	1	2026-04-05 15:53:46	0
3404	2	待出库	3	mes_wm_sales_notice_status	0	success		已提交状态，不可修改和删除	1	2026-03-30 08:54:30	1	2026-04-05 15:53:46	0
3405	3	已完成	4	mes_wm_sales_notice_status	0			\N	1	2026-03-30 10:02:10	1	2026-04-05 15:53:46	0
3406	1	草稿	0	mes_wm_misc_issue_status	0	info		草稿状态	1	2026-03-30 15:00:18	1	2026-03-30 15:00:18	0
3407	2	待出库	3	mes_wm_misc_issue_status	0	warning		待出库状态	1	2026-03-30 15:00:18	1	2026-03-30 15:00:18	0
3408	3	已完成	4	mes_wm_misc_issue_status	0	success		执行出库后的状态	1	2026-03-30 15:00:18	1	2026-03-30 15:00:18	0
3409	4	已取消	5	mes_wm_misc_issue_status	0	danger		已取消状态	1	2026-03-30 15:00:18	1	2026-03-30 15:00:18	0
3415	1	注塑	1	mes_cal_calendar_type	0	primary			1	2026-04-01 15:23:14	1	2026-04-01 16:08:31	0
3416	2	机加工	2	mes_cal_calendar_type	0	success			1	2026-04-01 15:23:14	1	2026-04-01 16:08:32	0
3417	3	组装	3	mes_cal_calendar_type	0	warning			1	2026-04-01 15:23:14	1	2026-04-01 16:08:33	0
3418	4	仓库	4	mes_cal_calendar_type	0	danger			1	2026-04-01 15:23:14	1	2026-04-01 16:08:34	0
3419	0	草稿	0	mes_dv_repair_status	0	info			1	2026-04-03 17:20:23	1	2026-04-03 17:20:23	0
3420	1	维修中	1	mes_dv_repair_status	0	primary			1	2026-04-03 17:20:23	1	2026-04-03 17:20:23	0
3421	2	待验收	2	mes_dv_repair_status	0	warning			1	2026-04-03 17:20:23	1	2026-04-03 17:20:23	0
3422	3	已确认	4	mes_dv_repair_status	0	success			1	2026-04-03 17:20:23	1	2026-04-03 17:20:23	0
3423	0	草稿	0	mes_wm_return_sales_status	0	info			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3424	1	待检验	1	mes_wm_return_sales_status	0	warning			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3425	2	待执行	2	mes_wm_return_sales_status	0	warning			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3426	3	待上架	3	mes_wm_return_sales_status	0	primary			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3427	4	已完成	4	mes_wm_return_sales_status	0	success			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3428	5	已取消	5	mes_wm_return_sales_status	0	danger			1	2026-04-03 17:20:25	1	2026-04-03 17:20:25	0
3429	1	尺寸	1	mes_defect_type	0				1	2026-04-04 12:49:51	1	2026-04-09 15:03:20	0
3430	2	外观	2	mes_defect_type	0				1	2026-04-04 12:49:51	1	2026-04-09 15:03:20	0
3431	3	重量	3	mes_defect_type	0				1	2026-04-04 12:49:51	1	2026-04-09 15:03:20	0
3432	4	性能	4	mes_defect_type	0				1	2026-04-04 12:49:51	1	2026-04-09 15:03:20	0
3433	5	成分	5	mes_defect_type	0				1	2026-04-04 12:49:51	1	2026-04-09 15:03:20	0
3436	1	上工	1	mes_pro_work_record_type	0	success			1	2026-04-05 14:07:27	1	2026-04-05 14:07:27	0
3437	2	下工	2	mes_pro_work_record_type	0	danger			1	2026-04-05 14:07:27	1	2026-04-05 14:07:27	0
3443	1	草稿	0	mes_wm_product_produce_status	0	info		草稿状态	1	2026-04-05 15:53:46	1	2026-04-05 15:53:46	0
3444	2	已完成	4	mes_wm_product_produce_status	0	success		已完成状态	1	2026-04-05 15:53:46	1	2026-04-05 15:53:46	0
3445	3	已取消	5	mes_wm_product_produce_status	0	danger		已取消状态	1	2026-04-05 15:53:46	1	2026-04-05 15:53:46	0
3446	0	草稿	0	mes_pro_task_status	0			\N	1	2026-04-16 09:47:00	1	2026-04-16 09:47:00	0
3447	1	已完成	4	mes_pro_task_status	0			\N	1	2026-04-16 09:47:00	1	2026-04-16 09:47:00	0
3448	2	已取消	5	mes_pro_task_status	0			\N	1	2026-04-16 09:47:00	1	2026-04-16 09:47:00	0
\.


--
-- Data for Name: system_dict_type; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_dict_type (id, name, type, status, remark, creator, create_time, updater, update_time, deleted, deleted_time) FROM stdin;
1	用户性别	system_user_sex	0	\N	admin	2021-01-05 17:03:48	1	2022-05-16 20:29:32	0	\N
6	参数类型	infra_config_type	0	\N	admin	2021-01-05 17:03:48		2022-02-01 16:36:54	0	\N
7	通知类型	system_notice_type	0	\N	admin	2021-01-05 17:03:48		2022-02-01 16:35:26	0	\N
9	操作类型	infra_operate_type	0	\N	admin	2021-01-05 17:03:48	1	2024-03-14 12:44:01	0	\N
10	系统状态	common_status	0	\N	admin	2021-01-05 17:03:48		2022-02-01 16:21:28	0	\N
11	Boolean 是否类型	infra_boolean_string	0	boolean 转是否		2021-01-19 03:20:08		2022-02-01 16:37:10	0	\N
104	登陆结果	system_login_result	0	登陆结果		2021-01-18 06:17:11		2022-02-01 16:36:00	0	\N
106	代码生成模板类型	infra_codegen_template_type	0	\N		2021-02-05 07:08:06	1	2022-05-16 20:26:50	0	\N
107	定时任务状态	infra_job_status	0	\N		2021-02-07 07:44:16		2022-02-01 16:51:11	0	\N
108	定时任务日志状态	infra_job_log_status	0	\N		2021-02-08 10:03:51		2022-02-01 16:50:43	0	\N
109	用户类型	user_type	0	\N		2021-02-26 00:15:51		2021-02-26 00:15:51	0	\N
110	API 异常数据的处理状态	infra_api_error_log_process_status	0	\N		2021-02-26 07:07:01		2022-02-01 16:50:53	0	\N
111	短信渠道编码	system_sms_channel_code	0	\N	1	2021-04-05 01:04:50	1	2022-02-16 02:09:08	0	\N
112	短信模板的类型	system_sms_template_type	0	\N	1	2021-04-05 21:50:43	1	2022-02-01 16:35:06	0	\N
113	短信发送状态	system_sms_send_status	0	\N	1	2021-04-11 20:18:03	1	2022-02-01 16:35:09	0	\N
114	短信接收状态	system_sms_receive_status	0	\N	1	2021-04-11 20:27:14	1	2022-02-01 16:35:14	0	\N
116	登陆日志的类型	system_login_type	0	登陆日志的类型	1	2021-10-06 00:50:46	1	2022-02-01 16:35:56	0	\N
117	OA 请假类型	bpm_oa_leave_type	0	\N	1	2021-09-21 22:34:33	1	2022-01-22 10:41:37	0	\N
130	支付渠道编码类型	pay_channel_code	0	支付渠道的编码	1	2021-12-03 10:35:08	1	2023-07-10 10:11:39	0	\N
131	支付回调状态	pay_notify_status	0	支付回调状态（包括退款回调）	1	2021-12-03 10:53:29	1	2023-07-19 18:09:43	0	\N
132	支付订单状态	pay_order_status	0	支付订单状态	1	2021-12-03 11:17:50	1	2021-12-03 11:17:50	0	\N
134	退款订单状态	pay_refund_status	0	退款订单状态	1	2021-12-10 16:42:50	1	2023-07-19 10:13:17	0	\N
139	流程实例的状态	bpm_process_instance_status	0	流程实例的状态	1	2022-01-07 23:46:42	1	2022-01-07 23:46:42	0	\N
140	流程实例的结果	bpm_task_status	0	流程实例的结果	1	2022-01-07 23:48:10	1	2024-03-08 22:42:03	0	\N
141	流程的表单类型	bpm_model_form_type	0	流程的表单类型	103	2022-01-11 23:50:45	103	2022-01-11 23:50:45	0	\N
142	任务分配规则的类型	bpm_task_candidate_strategy	0	BPM 任务的候选人的策略	103	2022-01-12 23:21:04	103	2024-03-06 02:53:59	0	\N
144	代码生成的场景枚举	infra_codegen_scene	0	代码生成的场景枚举	1	2022-02-02 13:14:45	1	2022-03-10 16:33:46	0	\N
145	角色类型	system_role_type	0	角色类型	1	2022-02-16 13:01:46	1	2022-02-16 13:01:46	0	\N
146	文件存储器	infra_file_storage	0	文件存储器	1	2022-03-15 00:24:38	1	2022-03-15 00:24:38	0	\N
147	OAuth 2.0 授权类型	system_oauth2_grant_type	0	OAuth 2.0 授权类型（模式）	1	2022-05-12 00:20:52	1	2022-05-11 16:25:49	0	\N
149	商品 SPU 状态	product_spu_status	0	商品 SPU 状态	1	2022-10-24 21:19:04	1	2022-10-24 21:19:08	0	\N
150	优惠类型	promotion_discount_type	0	优惠类型	1	2022-11-01 12:46:06	1	2022-11-01 12:46:06	0	\N
151	优惠劵模板的有限期类型	promotion_coupon_template_validity_type	0	优惠劵模板的有限期类型	1	2022-11-02 00:06:20	1	2022-11-04 00:08:26	0	\N
152	营销的商品范围	promotion_product_scope	0	营销的商品范围	1	2022-11-02 00:28:01	1	2022-11-02 00:28:01	0	\N
153	优惠劵的状态	promotion_coupon_status	0	优惠劵的状态	1	2022-11-04 00:14:49	1	2022-11-04 00:14:49	0	\N
154	优惠劵的领取方式	promotion_coupon_take_type	0	优惠劵的领取方式	1	2022-11-04 19:12:27	1	2022-11-04 19:12:27	0	\N
155	促销活动的状态	promotion_activity_status	0	促销活动的状态	1	2022-11-04 22:54:23	1	2022-11-04 22:54:23	0	\N
156	营销的条件类型	promotion_condition_type	0	营销的条件类型	1	2022-11-04 22:59:23	1	2022-11-04 22:59:23	0	\N
157	交易售后状态	trade_after_sale_status	0	交易售后状态	1	2022-11-19 20:52:56	1	2022-11-19 20:52:56	0	\N
158	交易售后的类型	trade_after_sale_type	0	交易售后的类型	1	2022-11-19 21:04:09	1	2022-11-19 21:04:09	0	\N
159	交易售后的方式	trade_after_sale_way	0	交易售后的方式	1	2022-11-19 21:39:04	1	2022-11-19 21:39:04	0	\N
160	终端	terminal	0	终端	1	2022-12-10 10:50:50	1	2022-12-10 10:53:11	0	\N
161	交易订单的类型	trade_order_type	0	交易订单的类型	1	2022-12-10 16:33:54	1	2022-12-10 16:33:54	0	\N
162	交易订单的状态	trade_order_status	0	交易订单的状态	1	2022-12-10 16:48:44	1	2022-12-10 16:48:44	0	\N
163	交易订单项的售后状态	trade_order_item_after_sale_status	0	交易订单项的售后状态	1	2022-12-10 20:58:08	1	2022-12-10 20:58:08	0	\N
164	公众号自动回复的请求关键字匹配模式	mp_auto_reply_request_match	0	公众号自动回复的请求关键字匹配模式	1	2023-01-16 23:29:56	1	2023-01-16 23:29:56	0	1970-01-01 00:00:00
165	公众号的消息类型	mp_message_type	0	公众号的消息类型	1	2023-01-17 22:17:09	1	2023-01-17 22:17:09	0	1970-01-01 00:00:00
166	邮件发送状态	system_mail_send_status	0	邮件发送状态	1	2023-01-26 09:53:13	1	2023-01-26 09:53:13	0	1970-01-01 00:00:00
167	站内信模版的类型	system_notify_template_type	0	站内信模版的类型	1	2023-01-28 10:35:10	1	2023-01-28 10:35:10	0	1970-01-01 00:00:00
168	代码生成的前端类型	infra_codegen_front_type	0		1	2023-04-12 23:57:52	1	2023-04-12 23:57:52	0	1970-01-01 00:00:00
170	快递计费方式	trade_delivery_express_charge_mode	0	用于商城交易模块配送管理	1	2023-05-21 22:45:03	1	2023-05-21 22:45:03	0	1970-01-01 00:00:00
171	积分业务类型	member_point_biz_type	0		1	2023-06-10 12:15:00	1	2023-06-28 13:48:20	0	1970-01-01 00:00:00
173	支付通知类型	pay_notify_type	0	\N	1	2023-07-20 12:23:03	1	2023-07-20 12:23:03	0	1970-01-01 00:00:00
174	会员经验业务类型	member_experience_biz_type	0	\N		2023-08-22 12:41:01		2023-08-22 12:41:01	0	\N
175	交易配送类型	trade_delivery_type	0		1	2023-08-23 00:03:14	1	2023-08-23 00:03:14	0	1970-01-01 00:00:00
176	分佣模式	brokerage_enabled_condition	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
177	分销关系绑定模式	brokerage_bind_mode	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
178	佣金提现类型	brokerage_withdraw_type	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
179	佣金记录业务类型	brokerage_record_biz_type	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
180	佣金记录状态	brokerage_record_status	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
181	佣金提现状态	brokerage_withdraw_status	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
182	佣金提现银行	brokerage_bank_name	0	\N		2023-09-28 02:46:05		2023-09-28 02:46:05	0	\N
183	砍价记录的状态	promotion_bargain_record_status	0		1	2023-10-05 10:41:08	1	2023-10-05 10:41:08	0	1970-01-01 00:00:00
184	拼团记录的状态	promotion_combination_record_status	0		1	2023-10-08 07:24:25	1	2023-10-08 07:24:25	0	1970-01-01 00:00:00
185	回款-回款方式	crm_receivable_return_type	0	回款-回款方式	1	2023-10-18 21:54:10	1	2023-10-18 21:54:10	0	1970-01-01 00:00:00
186	CRM 客户行业	crm_customer_industry	0	CRM 客户所属行业	1	2023-10-28 22:57:07	1	2024-02-18 23:30:22	0	\N
187	客户等级	crm_customer_level	0	CRM 客户等级	1	2023-10-28 22:59:12	1	2023-10-28 15:11:16	0	\N
188	客户来源	crm_customer_source	0	CRM 客户来源	1	2023-10-28 23:00:34	1	2023-10-28 15:11:16	0	\N
600	Banner 位置	promotion_banner_position	0		1	2023-10-08 07:24:25	1	2023-11-04 13:04:02	0	1970-01-01 00:00:00
601	社交类型	system_social_type	0		1	2023-11-04 13:03:54	1	2023-11-04 13:03:54	0	1970-01-01 00:00:00
604	产品状态	crm_product_status	0		1	2023-10-30 21:47:59	1	2023-10-30 21:48:45	0	1970-01-01 00:00:00
605	CRM 数据权限的级别	crm_permission_level	0		1	2023-11-30 09:51:59	1	2023-11-30 09:51:59	0	1970-01-01 00:00:00
606	CRM 审批状态	crm_audit_status	0		1	2023-11-30 18:56:23	1	2023-11-30 18:56:23	0	1970-01-01 00:00:00
607	CRM 产品单位	crm_product_unit	0		1	2023-12-05 23:01:51	1	2023-12-05 23:01:51	0	1970-01-01 00:00:00
608	CRM 跟进方式	crm_follow_up_type	0		1	2024-01-15 20:48:05	1	2024-01-15 20:48:05	0	1970-01-01 00:00:00
610	转账订单状态	pay_transfer_status	0		1	2023-10-28 16:18:32	1	2023-10-28 16:18:32	0	1970-01-01 00:00:00
611	ERP 库存明细的业务类型	erp_stock_record_biz_type	0	ERP 库存明细的业务类型	1	2024-02-05 18:07:02	1	2024-02-05 18:07:02	0	1970-01-01 00:00:00
612	ERP 审批状态	erp_audit_status	0		1	2024-02-06 00:00:07	1	2024-02-06 00:00:07	0	1970-01-01 00:00:00
613	BPM 监听器类型	bpm_process_listener_type	0		1	2024-03-23 12:52:24	1	2024-03-09 15:54:28	0	1970-01-01 00:00:00
615	BPM 监听器值类型	bpm_process_listener_value_type	0		1	2024-03-23 13:00:31	1	2024-03-23 13:00:31	0	1970-01-01 00:00:00
616	时间间隔	date_interval	0		1	2024-03-29 22:50:09	1	2024-03-29 22:50:09	0	1970-01-01 00:00:00
619	CRM 商机结束状态类型	crm_business_end_status_type	0		1	2024-04-13 23:23:00	1	2024-04-13 23:23:00	0	1970-01-01 00:00:00
620	AI 模型平台	ai_platform	0		1	2024-05-09 22:27:38	1	2024-05-09 22:27:38	0	1970-01-01 00:00:00
621	AI 绘画状态	ai_image_status	0		1	2024-06-26 20:51:23	1	2024-06-26 20:51:23	0	1970-01-01 00:00:00
622	AI 音乐状态	ai_music_status	0		1	2024-06-27 22:45:07	1	2024-06-28 00:56:27	0	1970-01-01 00:00:00
623	AI 音乐生成模式	ai_generate_mode	0		1	2024-06-27 22:46:21	1	2024-06-28 01:22:29	0	1970-01-01 00:00:00
624	写作语气	ai_write_tone	0		1	2024-07-07 15:19:02	1	2024-07-07 15:19:02	0	1970-01-01 00:00:00
625	写作语言	ai_write_language	0		1	2024-07-07 15:18:52	1	2024-07-07 15:18:52	0	1970-01-01 00:00:00
626	写作长度	ai_write_length	0		1	2024-07-07 15:18:41	1	2024-07-07 15:18:41	0	1970-01-01 00:00:00
627	写作格式	ai_write_format	0		1	2024-07-07 15:14:34	1	2024-07-07 15:14:34	0	1970-01-01 00:00:00
628	AI 写作类型	ai_write_type	0		1	2024-07-10 21:25:29	1	2024-07-10 21:25:29	0	1970-01-01 00:00:00
629	BPM 流程模型类型	bpm_model_type	0		1	2024-08-26 15:21:43	1	2024-08-26 15:21:43	0	1970-01-01 00:00:00
640	AI 模型类型	ai_model_type	0		1	2025-03-03 12:24:07	1	2025-03-03 12:24:07	0	1970-01-01 00:00:00
1001	IoT 产品设备类型	iot_product_device_type	0		1	2024-08-10 11:54:30	1	2025-03-17 09:25:08	0	1970-01-01 00:00:00
1002	IoT 产品状态	iot_product_status	0		1	2024-08-10 12:06:09	1	2025-03-17 09:25:10	0	1970-01-01 00:00:00
1004	IoT 联网方式	iot_net_type	0		1	2024-09-06 22:04:13	1	2025-03-17 09:25:14	0	1970-01-01 00:00:00
1006	IoT 设备状态	iot_device_state	0		1	2024-09-21 08:12:55	1	2025-03-17 09:25:19	0	1970-01-01 00:00:00
1007	IoT 物模型功能类型	iot_thing_model_type	0		1	2024-09-29 20:02:36	1	2025-03-17 09:25:24	0	1970-01-01 00:00:00
1011	IoT 物模型单位	iot_thing_model_unit	0		1	2024-12-25 17:36:46	1	2025-03-17 09:25:35	0	1970-01-01 00:00:00
1013	IoT 数据流转目的的类型枚举	iot_data_sink_type_enum	0		1	2025-03-09 12:39:36	1	2025-06-24 12:45:24	0	1970-01-01 00:00:00
1014	IoT 场景流转的触发类型枚举	iot_rule_scene_trigger_type_enum	0		1	2025-03-20 14:59:44	1	2025-03-20 14:59:44	0	1970-01-01 00:00:00
1015	IoT 设备消息类型枚举	iot_device_message_type_enum	0		1	2025-03-20 15:01:15	1	2025-03-20 15:01:15	0	1970-01-01 00:00:00
1016	IoT 规则场景的触发类型枚举	iot_rule_scene_action_type_enum	0		1	2025-03-28 15:26:54	1	2025-03-28 15:29:13	0	1970-01-01 00:00:00
1017	MES 物料消耗记录状态	mes_wm_item_consume_status	0	MES 物料消耗记录状态	1	2026-03-19 15:06:23	1	2026-03-19 15:06:23	0	\N
2001	IoT 告警级别	iot_alert_level	0		1	2025-06-27 20:30:57	1	2025-06-27 20:30:57	0	1970-01-01 00:00:00
2002	IoT 告警	iot_alert_receive_type	0		1	2025-06-27 22:49:19	1	2025-06-27 22:49:19	0	1970-01-01 00:00:00
2003	IoT 固件设备范围	iot_ota_task_device_scope	0		1	2025-07-02 09:42:49	1	2025-07-02 09:42:49	0	1970-01-01 00:00:00
2004	IoT 固件升级任务状态	iot_ota_task_status	0		1	2025-07-02 09:43:43	1	2025-07-02 09:43:43	0	1970-01-01 00:00:00
2005	IoT 固件升级记录状态	iot_ota_task_record_status	0		1	2025-07-02 09:45:02	1	2025-07-02 09:45:02	0	1970-01-01 00:00:00
2007	AI MCP 客户端名字	ai_mcp_client_name	0		1	2025-08-28 13:57:40	1	2025-08-28 13:57:40	0	1970-01-01 00:00:00
2008	IoT 协议类型	iot_protocol_type	0	IoT 设备接入协议类型	1	2026-02-04 00:31:33	1	2026-02-04 00:31:33	0	1970-01-01 00:00:00
2009	IoT 序列化类型	iot_serialize_type	0	IoT 设备消息序列化类型	1	2026-02-04 00:33:16	1	2026-02-04 00:33:16	0	1970-01-01 00:00:00
2010	IoT Modbus 工作模式	iot_modbus_mode	0	Modbus 设备数据采集模式	1	2025-06-12 22:55:46	1	2025-06-12 22:55:46	0	1970-01-01 00:00:00
2011	IoT Modbus 帧格式	iot_modbus_frame_format	0	Modbus 数据帧协议格式	1	2025-06-12 22:55:46	1	2025-06-12 22:55:46	0	1970-01-01 00:00:00
2012	MES 客户类型	mes_client_type	0		1	2026-02-15 14:38:25	1	2026-02-15 14:38:25	0	\N
2013	MES 供应商级别	mes_vendor_level	0		1	2026-02-15 15:59:15	1	2026-02-15 15:59:15	0	\N
2014	MES 假期类型	mes_cal_holiday_type	0	MES 日历排班 - 假期类型（HOLIDAY=假期，WORKDAY=工作日）	1	2026-02-16 07:35:58	1	2026-02-16 07:35:58	0	\N
2015	MES 工具状态	mes_tm_tool_status	0	MES 工具管理 - 工具状态（1=在库，2=领用中，3=维修中，4=报废）	1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0	\N
2016	MES 保养维护类型	mes_tm_mainten_type	0	MES 工具管理 - 保养维护类型（1=定期维护，2=按使用次数维护）	1	2026-02-16 11:10:55	1	2026-02-16 11:10:55	0	\N
2017	MES 设备状态	mes_dv_machinery_status	0	MES 设备管理 - 设备状态（1=运行中，2=停机，3=故障）	1	2026-02-17 01:00:06	1	2026-02-17 01:00:06	0	\N
2018	MES 检测项类型	mes_indicator_type	0		1	2026-02-17 02:16:22	1	2026-02-21 15:25:04	0	\N
2019	MES 缺陷等级	mes_defect_level	0		1	2026-02-17 02:16:22	1	2026-02-17 02:16:22	0	\N
2020	MES 轮班方式	mes_cal_shift_type	0	MES 日历排班 - 轮班方式	1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0	\N
2021	MES 倒班方式	mes_cal_shift_method	0	MES 日历排班 - 倒班方式	1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0	\N
2022	MES 班组类型	mes_cal_calendar_type	0	MES 日历排班 - 班组类型	1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0	\N
2023	MES 排班计划状态	mes_cal_plan_status	0	MES 日历排班 - 排班计划状态	1	2026-02-17 03:40:09	1	2026-02-17 03:40:09	0	\N
2026	MES 检测种类	mes_qc_type	0	IQC/IPQC/OQC/RQC	1	2026-02-17 08:34:40	1	2026-02-17 08:34:40	0	\N
2027	MES 生产工单状态	mes_pro_work_order_status	0	MES 生产管理 - 工单状态（0=草稿，1=已确认，2=已完成，3=已取消）	1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0	\N
2028	MES 工单来源类型	mes_pro_work_order_source_type	0	MES 生产管理 - 工单来源类型（1=客户订单，2=库存备货）	1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0	\N
2029	MES 工单类型	mes_pro_work_order_type	0	MES 生产管理 - 工单类型（1=自行生产，2=代工，3=采购）	1	2026-02-17 11:43:47	1	2026-02-17 11:43:47	0	\N
2036	MES 工序关系类型	mes_pro_link_type	0	工艺路线中工序之间的关系类型	1	2026-02-19 04:24:53	1	2026-04-05 15:05:07	0	\N
2037	MES 时间单位	mes_time_unit_type	0	生产时间的计量单位	1	2026-02-19 04:24:53	1	2026-04-05 15:04:57	0	\N
2038	MES 生产任务状态	mes_pro_task_status	0	MES 生产管理 - 任务状态（0=草稿，1=进行中，2=暂停，3=已完成，4=已取消）	1	2026-02-19 15:25:27	1	2026-02-19 15:25:27	0	\N
2039	MES 点检保养项目类型	mes_dv_subject_type	0	MES 设备管理 - 点检保养项目类型（1=设备点检，2=设备保养）	1	2026-02-20 01:42:58	1	2026-02-20 01:42:58	0	\N
2040	MES 保养记录状态	mes_mainten_record_status	0	\N	admin	2026-02-20 02:59:55	admin	2026-02-20 02:59:55	0	\N
2041	MES 保养结果	mes_mainten_status	0	\N	admin	2026-02-20 02:59:55	admin	2026-02-20 02:59:55	0	\N
2042	MES 点检保养周期类型	mes_dv_cycle_type	0	MES 设备管理 - 点检保养周期类型（1=天，2=周，3=月，4=年）	1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0	\N
2043	MES 点检保养方案状态	mes_dv_check_plan_status	0	MES 设备管理 - 点检保养方案状态（0=草稿，1=已启用）	1	2026-02-20 07:11:43	1	2026-02-20 07:11:43	0	\N
2044	MES 点检记录状态	mes_dv_check_record_status	0	\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0	\N
2045	MES 点检结果	mes_dv_check_result	0	\N	admin	2026-02-20 09:46:19	admin	2026-02-20 09:46:19	0	\N
2046	MES 维修工单状态	mes_dv_repair_status	0	MES 设备管理 - 维修工单状态（10=待维修，20=维修中，30=已完成，40=已验收）	1	2026-02-20 10:56:24	1	2026-02-20 10:56:24	0	\N
2047	MES 维修结果	mes_dv_repair_result	0	MES 设备管理 - 维修结果（1=修复成功，2=报废）	1	2026-02-20 10:56:24	1	2026-02-20 10:56:24	0	\N
2049	MES 检测结果	mes_qc_check_result	0	来料检验的最终结果判定	1	2026-02-20 11:23:35	1	2026-02-20 11:23:35	0	\N
2050	MES 来源单据类型	mes_qc_source_doc_type	0	IQC 来料检验的来源单据类型	1	2026-02-20 11:23:35	1	2026-02-20 11:23:35	0	\N
2051	MES 安灯处置状态	mes_pro_andon_status	0	MES 生产管理 - 安灯处置状态（0=未处置，1=已处置）	1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0	\N
2052	MES 安灯级别	mes_pro_andon_level	0	MES 生产管理 - 安灯级别（1=一级，2=二级，3=三级）	1	2026-02-21 00:08:38	1	2026-02-21 00:08:38	0	\N
2053	MES 生产报工状态	mes_pro_feedback_status	0	MES 生产管理 - 报工状态（0=草稿，1=审批中，2=待检验，3=已完成，4=已取消）	1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0	\N
2054	MES 生产报工类型	mes_pro_feedback_type	0	MES 生产管理 - 报工类型（1=自行报工，2=统一报工）	1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0	\N
2055	MES 生产报工途径	mes_pro_feedback_channel	0	MES 生产管理 - 报工途径（PC/APP/PDA）	1	2026-02-21 00:50:32	1	2026-02-21 00:50:32	0	\N
2056	MES 质检值类型	mes_qc_result_type	0	检验结果明细的值类型：浮点/整数/文本/字典/文件	1	2026-02-21 13:37:17	1	2026-02-21 13:37:17	0	\N
2057	MES 退货检验类型	mes_rqc_type	0	MES 退货检验类型	1	2026-02-22 06:43:18	1	2026-02-22 06:43:18	0	\N
2062	MES IPQC 检验类型	mes_ipqc_type	0	IPQC 过程检验的检验类型	1	2026-02-22 07:01:04	1	2026-02-22 07:01:04	0	\N
2066	MES 到货通知单状态	mes_wm_arrival_notice_status	0	MES 到货通知单状态	1	2026-02-22 14:53:18	1	2026-02-22 14:53:18	0	\N
2067	MES 采购入库单状态	mes_wm_item_receipt_status	0	MES 采购入库单状态	1	2026-02-22 14:54:05	1	2026-02-22 14:54:05	0	\N
2068	MES 单据状态	mes_order_status	0		1	2026-02-23 21:16:03	1	2026-02-23 21:17:37	0	\N
2069	MES 领料出库单状态	mes_wm_product_issue_status	0	MES 领料出库单状态	1	2026-02-26 16:39:44	1	2026-04-05 15:05:11	0	\N
2073	MES 生产退料单状态	mes_wm_return_issue_status	0	MES 生产退料单状态	1	2026-02-28 14:11:09	1	2026-04-05 15:05:14	0	\N
2074	MES 生产退料类型	mes_wm_return_issue_type	0	MES 生产退料类型	1	2026-02-28 14:11:09	1	2026-04-05 15:05:16	0	\N
2075	MES 质量状态	mes_wm_quality_status	0	MES 质量状态（待检/合格/不合格）	1	2026-02-28 15:00:53	1	2026-02-28 15:00:53	0	\N
2100	MES 产品入库单状态	mes_wm_product_receipt_status	0	MES 产品入库单状态	1	2026-03-01 06:03:04	1	2026-04-05 15:05:49	0	\N
2102	MES 销售出库单状态	mes_wm_product_sales_status	0	MES 销售出库单状态	1	2026-03-02 08:55:11	1	2026-04-05 15:05:18	0	\N
2105	MES 杂项入库类型	mes_wm_misc_receipt_type	0	杂项入库类型	1	2026-03-03 07:18:12	1	2026-04-05 15:05:23	0	\N
2106	MES 杂项入库状态	mes_wm_misc_receipt_status	0	杂项入库状态	1	2026-03-03 07:18:12	1	2026-04-05 15:05:25	0	\N
2109	MES 杂项出库类型	mes_wm_misc_issue_type	0	MES 杂项出库类型	1	2026-03-03 07:34:33	1	2026-03-03 07:34:33	0	\N
2110	MES 外协入库单状态	mes_wm_outsource_receipt_status	0	MES 外协入库单状态	1	2026-03-03 14:03:20	1	2026-03-03 14:03:20	0	\N
2112	MES 外协发料单状态	mes_wm_outsource_issue_status	0	MES 外协发料单状态	1	2026-03-03 16:30:56	1	2026-04-05 15:05:47	0	\N
2113	MES 编码规则分段类型	mes_md_auto_code_part_type	0	MES 编码规则分段类型	1	2026-03-04 14:45:46	1	2026-03-04 15:24:40	0	\N
2115	MES 编码规则补齐方式	mes_md_auto_code_padded_method	0	MES 编码规则补齐方式	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0	\N
2116	MES 编码规则循环方式	mes_md_auto_code_cycle_method	0	MES 编码规则循环方式	1	2026-03-04 14:46:22	1	2026-03-04 15:24:40	0	\N
2117	MES 条码格式	mes_wm_barcode_format	0	MES 条码格式	1	2026-03-05 14:37:20	1	2026-04-05 15:05:27	0	\N
2118	MES 条码业务类型	mes_wm_barcode_biz_type	0	MES 条码业务类型	1	2026-03-05 14:37:20	1	2026-04-05 15:05:29	0	\N
2121	MES 装箱单状态	mes_wm_package_status	0	MES 装箱单状态	1	2026-03-08 02:05:46	1	2026-04-05 15:05:35	0	\N
2122	MES 调拨单状态	mes_wm_transfer_status	0	MES 调拨单状态	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0	\N
2123	MES 调拨类型	mes_wm_transfer_type	0	MES 调拨类型	1	2026-03-08 11:55:25	1	2026-03-08 11:55:25	0	\N
2124	MES 盘点类型	mes_wm_stock_taking_type	0	MES 盘点类型	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0	\N
2125	MES 盘点方案参数类型	mes_wm_stock_taking_plan_param_type	0	MES 盘点方案参数类型	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0	\N
2126	MES 盘点任务状态	mes_wm_stock_taking_task_status	0	MES 盘点任务状态	1	2026-03-09 00:00:00	1	2026-03-09 00:00:00	0	\N
2127	MES 盘点任务行状态	mes_wm_stock_taking_task_line_status	0	MES 盘点任务行状态	1	2026-03-09 00:00:00	1	2026-04-05 15:02:18	0	\N
2129	MES 物料产品标识	mes_md_item_or_product	0	物料分类：物料(ITEM) / 产品(PRODUCT)	1	2026-03-15 01:55:06	1	2026-03-15 01:55:06	0	\N
2130	MES 供应商退货单状态	mes_wm_return_vendor_status	0	采购退货单状态		2026-03-29 13:49:57		2026-04-05 15:53:46	0	\N
2131	MES 发货通知单状态	mes_wm_sales_notice_status	0	MES 发货通知单状态	1	2026-03-30 08:54:30	1	2026-04-05 15:53:46	0	\N
2132	MES 杂项出库单状态	mes_wm_misc_issue_status	0	杂项出库单状态	1	2026-03-30 15:00:18	1	2026-04-05 15:05:41	0	\N
2133	MES 销售退货单状态	mes_wm_return_sales_status	0	MES 销售退货单状态枚举	1	2026-04-03 17:20:25	1	2026-04-05 15:05:39	0	\N
2134	MES 缺陷检测项类型	mes_defect_type	0	缺陷模块的检测项类型字典	1	2026-04-04 12:49:51	1	2026-04-04 12:49:51	0	\N
2135	MES 上下工状态类型	mes_pro_work_record_type	0	MES 上下工状态类型	1	2026-04-05 14:07:27	1	2026-04-05 14:07:27	0	\N
2138	MES 生产入库单状态	mes_wm_product_produce_status	0	MES 生产入库单状态	1	2026-04-05 15:53:46	1	2026-04-05 15:53:46	0	1970-01-01 00:00:00
\.


--
-- Data for Name: system_login_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_login_log (id, log_type, trace_id, user_id, user_type, username, result, user_ip, user_agent, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_mail_account; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_mail_account (id, mail, username, password, host, port, ssl_enable, starttls_enable, creator, create_time, updater, update_time, deleted) FROM stdin;
1	7684413@qq.com	7684413@qq.com	enc:sm4:v2:Tpnlc4jTjeUhzNQAfR+K1Zx2Qbfvb1eRClwXThjK1MBZ4sMnQZS67Y1yz8gClUKP	127.0.0.1	8080	f	f	1	2023-01-25 17:39:52	1	2026-07-16 07:29:32.31865	0
2	ydym_test@163.com	ydym_test@163.com	enc:sm4:v2:Y7MQnUONNTSjw7iSaiLxhrIf62J6VhCNv36lRl3BYfGEQXal2+iXu7zBBLojWT5z	smtp.163.com	465	t	f	1	2023-01-26 01:26:03	1	2026-07-16 07:29:32.31865	0
3	76854114@qq.com	3335	enc:sm4:v2:FAqts/i4oybhm3hAxxAVocK9BPv/qPatpz72HIrei5HGKsA7/GHGDWu42L+sASq+	yunai1.cn	466	f	f	1	2023-01-27 15:06:38	1	2026-07-16 07:29:32.31865	1
4	7685413x@qq.com	2	enc:sm4:v2:05FpuNGPizJXwGCjfa20HHRbnrquGzn/llMwHDwQOjFshz7EwINkJUKZLjteyy68	4	5	t	f	1	2023-04-12 23:05:06	1	2026-07-16 07:29:32.31865	1
\.


--
-- Data for Name: system_mail_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_mail_log (id, user_id, user_type, to_mails, cc_mails, bcc_mails, account_id, from_mail, template_id, template_code, template_nickname, template_title, template_content, template_params, send_status, send_time, send_message_id, send_exception, creator, create_time, updater, update_time, deleted) FROM stdin;
2	\N	2	nobody@example.com	\N	\N	2	ydym_test@163.com	14	test_01	芋艿	一个标题	<p>你是 A 吗？</p><p><br></p><p>是的话，赶紧 B 一下！</p>	{"key01":"A","key02":"B"}	30	2026-07-16 07:29:59.563274	\N	mail provider is not configured	admin	2026-07-16 07:29:59.563274	admin	2026-07-16 07:29:59.563274	0
3	\N	2	test@example.com	\N	\N	1	7684413@qq.com	13	admin-sms-login	奥特曼	你猜我猜	<p>您的验证码是{code}，名字是Codex</p>	{"name":"Codex"}	30	2026-07-16 07:41:03.257612	\N	mail provider is not configured	admin	2026-07-16 07:41:03.257612	admin	2026-07-16 07:41:03.257612	0
\.


--
-- Data for Name: system_mail_template; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_mail_template (id, name, code, account_id, nickname, title, content, params, status, remark, creator, create_time, updater, update_time, deleted) FROM stdin;
13	后台用户短信登录	admin-sms-login	1	奥特曼	你猜我猜	<p>您的验证码是{code}，名字是{name}</p>	["code","name"]	0	3	1	2021-10-11 08:10:00	1	2023-12-02 19:51:14	0
14	测试模版	test_01	2	芋艿	一个标题	<p>你是 {key01} 吗？</p><p><br></p><p>是的话，赶紧 {key02} 一下！</p>	["key01","key02"]	0	\N	1	2023-01-26 01:27:40	1	2025-07-26 21:48:45	0
15	3	2	2	7	4	<p>45</p>	[]	1	80	1	2023-01-27 15:50:35	1	2025-07-26 21:47:49	1
\.


--
-- Data for Name: system_menu; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_menu (id, name, permission, type, sort, parent_id, path, icon, component, component_name, status, visible, keep_alive, always_show, creator, create_time, updater, update_time, deleted, active_menu_id) FROM stdin;
100	用户管理	system:user:list	2	1	1	user	ep:avatar	system/user/index	SystemUser	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-01-01 18:43:01	0	\N
101	角色管理		2	2	1	role	ep:user	system/role/index	SystemRole	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-01-05 19:30:33	0	\N
102	菜单管理		2	3	1	menu	ep:menu	system/menu/index	SystemMenu	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 01:03:50	0	\N
103	部门管理		2	4	1	dept	fa:address-card	system/dept/index	SystemDept	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 01:06:28	0	\N
104	岗位管理		2	5	1	post	fa:address-book-o	system/post/index	SystemPost	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 01:06:39	0	\N
105	字典管理		2	6	1	dict	ep:collection	system/dict/index	SystemDictType	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 01:07:12	0	\N
106	配置管理		2	8	2	config	fa:connectdevelop	infra/config/index	InfraConfig	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-04-23 00:02:45	0	\N
110	定时任务		2	7	2	job	fa-solid:tasks	infra/job/index	InfraJob	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 08:57:36	0	\N
114	表单构建	infra:build:list	2	2	2	build	fa:wpforms	infra/build/index	InfraBuild	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 08:51:35	0	\N
115	代码生成	infra:codegen:query	2	1	2	codegen	ep:document-copy	infra/codegen/index	InfraCodegen	0	t	t	t	admin	2021-01-05 17:03:48	1	2024-02-29 08:51:06	0	\N
1001	用户查询	system:user:query	3	1	100		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1002	用户新增	system:user:create	3	2	100				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1003	用户修改	system:user:update	3	3	100				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1004	用户删除	system:user:delete	3	4	100				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1005	用户导出	system:user:export	3	5	100		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1006	用户导入	system:user:import	3	6	100		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1007	重置密码	system:user:update-password	3	7	100				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1008	角色查询	system:role:query	3	1	101		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1009	角色新增	system:role:create	3	2	101				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1010	角色修改	system:role:update	3	3	101				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1011	角色删除	system:role:delete	3	4	101				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1012	角色导出	system:role:export	3	5	101		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1013	菜单查询	system:menu:query	3	1	102		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1014	菜单新增	system:menu:create	3	2	102		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1015	菜单修改	system:menu:update	3	3	102		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1016	菜单删除	system:menu:delete	3	4	102		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1017	部门查询	system:dept:query	3	1	103		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1018	部门新增	system:dept:create	3	2	103				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1019	部门修改	system:dept:update	3	3	103				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1020	部门删除	system:dept:delete	3	4	103				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1021	岗位查询	system:post:query	3	1	104		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1022	岗位新增	system:post:create	3	2	104				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1023	岗位修改	system:post:update	3	3	104				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1024	岗位删除	system:post:delete	3	4	104				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1025	岗位导出	system:post:export	3	5	104		#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1026	字典查询	system:dict:query	3	1	105	#	#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1027	字典新增	system:dict:create	3	2	105				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1028	字典修改	system:dict:update	3	3	105				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1029	字典删除	system:dict:delete	3	4	105				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1030	字典导出	system:dict:export	3	5	105	#	#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1031	配置查询	infra:config:query	3	1	106				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1032	配置新增	infra:config:create	3	2	106				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
5	OA 示例		1	40	1185	oa	fa:road	\N	\N	0	t	t	t	admin	2021-09-20 16:26:19	system	2026-07-17 01:44:04.417394	1	\N
111	PostgreSQL 监控		2	1	2740	postgresql	lucide:database	infra/druid/index	InfraPostgreSql	0	t	t	t	admin	2021-01-05 17:03:48	system	2026-07-17 05:04:14.954699	0	\N
113	Redis 监控		2	2	2740	redis	lucide:database-zap	infra/redis/index	InfraRedis	0	t	t	t	admin	2021-01-05 17:03:48	system	2026-07-17 05:04:14.954699	0	\N
112	Rust 监控		2	3	2740	rust	lucide:server-cog	infra/server/index	InfraRustServer	0	t	t	t	admin	2021-01-05 17:03:48	system	2026-07-17 05:08:19.419206	0	\N
1033	配置修改	infra:config:update	3	3	106				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1034	配置删除	infra:config:delete	3	4	106				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1035	配置导出	infra:config:export	3	5	106				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1036	公告查询	system:notice:query	3	1	107	#	#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1037	公告新增	system:notice:create	3	2	107				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1038	公告修改	system:notice:update	3	3	107				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1039	公告删除	system:notice:delete	3	4	107				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1040	操作查询	system:operate-log:query	3	1	500				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1042	日志导出	system:operate-log:export	3	2	500				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1043	登录查询	system:login-log:query	3	1	501	#	#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1045	日志导出	system:login-log:export	3	3	501	#	#		\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1046	令牌列表	system:oauth2-token:page	3	1	109				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-05-09 23:54:42	0	\N
1048	令牌删除	system:oauth2-token:delete	3	2	109				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-05-09 23:54:53	0	\N
1050	任务新增	infra:job:create	3	2	110				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1051	任务修改	infra:job:update	3	3	110				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1052	任务删除	infra:job:delete	3	4	110				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1053	状态修改	infra:job:update	3	5	110				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1054	任务导出	infra:job:export	3	7	110				\N	0	t	t	t	admin	2021-01-05 17:03:48		2022-04-20 17:03:10	0	\N
1056	生成修改	infra:codegen:update	3	2	115				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1057	生成删除	infra:codegen:delete	3	3	115				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1058	导入代码	infra:codegen:create	3	2	115				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1059	预览代码	infra:codegen:preview	3	4	115				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1060	生成代码	infra:codegen:download	3	5	115				\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2022-04-20 17:03:10	0	\N
1063	设置角色菜单权限	system:permission:assign-role-menu	3	6	101				\N	0	t	t	t		2021-01-06 17:53:44		2022-04-20 17:03:10	0	\N
1064	设置角色数据权限	system:permission:assign-role-data-scope	3	7	101				\N	0	t	t	t		2021-01-06 17:56:31		2022-04-20 17:03:10	0	\N
1065	设置用户角色	system:permission:assign-user-role	3	8	101				\N	0	t	t	t		2021-01-07 10:23:28		2022-04-20 17:03:10	0	\N
1066	获得 Redis 监控信息	infra:redis:get-monitor-info	3	1	113				\N	0	t	t	t		2021-01-26 01:02:31		2022-04-20 17:03:10	0	\N
1067	获得 Redis Key 列表	infra:redis:get-key-list	3	2	113				\N	0	t	t	t		2021-01-26 01:02:52		2022-04-20 17:03:10	0	\N
1075	任务触发	infra:job:trigger	3	8	110				\N	0	t	t	t		2021-02-07 13:03:10		2022-04-20 17:03:10	0	\N
1078	访问日志		2	1	1083	api-access-log	ep:place	infra/apiAccessLog/index	InfraApiAccessLog	0	t	t	t		2021-02-26 01:32:59	1	2024-02-29 08:54:57	0	\N
1082	日志导出	infra:api-access-log:export	3	2	1078				\N	0	t	t	t		2021-02-26 01:32:59	1	2022-04-20 17:03:10	0	\N
1083	API 日志		2	4	2	log	fa:tasks	\N	\N	0	t	t	t		2021-02-26 02:18:24	1	2024-04-22 23:58:36	0	\N
1084	错误日志	infra:api-error-log:query	2	2	1083	api-error-log	ep:warning-filled	infra/apiErrorLog/index	InfraApiErrorLog	0	t	t	t		2021-02-26 07:53:20	1	2024-02-29 08:55:17	0	\N
1085	日志处理	infra:api-error-log:update-status	3	2	1084				\N	0	t	t	t		2021-02-26 07:53:20	1	2022-04-20 17:03:10	0	\N
1086	日志导出	infra:api-error-log:export	3	3	1084				\N	0	t	t	t		2021-02-26 07:53:20	1	2022-04-20 17:03:10	0	\N
1087	任务查询	infra:job:query	3	1	110				\N	0	t	t	t	1	2021-03-10 01:26:19	1	2022-04-20 17:03:10	0	\N
1088	日志查询	infra:api-access-log:query	3	1	1078				\N	0	t	t	t	1	2021-03-10 01:28:04	1	2022-04-20 17:03:10	0	\N
1089	日志查询	infra:api-error-log:query	3	1	1084				\N	0	t	t	t	1	2021-03-10 01:29:09	1	2022-04-20 17:03:10	0	\N
1090	文件列表		2	5	1243	file	ep:upload-filled	infra/file/index	InfraFile	0	t	t	t		2021-03-12 20:16:20	1	2024-02-29 08:53:02	0	\N
1091	文件查询	infra:file:query	3	1	1090				\N	0	t	t	t		2021-03-12 20:16:20		2022-04-20 17:03:10	0	\N
1092	文件删除	infra:file:delete	3	4	1090				\N	0	t	t	t		2021-03-12 20:16:20		2022-04-20 17:03:10	0	\N
1095	短信渠道查询	system:sms-channel:query	3	1	1094				\N	0	t	t	t		2021-04-01 11:07:15		2022-04-20 17:03:10	0	\N
1096	短信渠道创建	system:sms-channel:create	3	2	1094				\N	0	t	t	t		2021-04-01 11:07:15		2022-04-20 17:03:10	0	\N
1097	短信渠道更新	system:sms-channel:update	3	3	1094				\N	0	t	t	t		2021-04-01 11:07:15		2022-04-20 17:03:10	0	\N
1098	短信渠道删除	system:sms-channel:delete	3	4	1094				\N	0	t	t	t		2021-04-01 11:07:15		2022-04-20 17:03:10	0	\N
1101	短信模板查询	system:sms-template:query	3	1	1100				\N	0	t	t	t		2021-04-01 17:35:17		2022-04-20 17:03:10	0	\N
1102	短信模板创建	system:sms-template:create	3	2	1100				\N	0	t	t	t		2021-04-01 17:35:17		2022-04-20 17:03:10	0	\N
1103	短信模板更新	system:sms-template:update	3	3	1100				\N	0	t	t	t		2021-04-01 17:35:17		2022-04-20 17:03:10	0	\N
1093	短信管理		1	1	2739	sms	ep:message	\N	\N	0	t	t	t	1	2021-04-05 01:10:16	1	2026-07-17 01:35:39.386526	1	\N
1077	请求链路		2	4	2740	traces	lucide:route	infra/skywalking/index	InfraRequestTraces	0	t	t	t		2021-02-08 20:41:31	system	2026-07-17 05:04:14.954699	0	\N
1104	短信模板删除	system:sms-template:delete	3	4	1100				\N	0	t	t	t		2021-04-01 17:35:17		2022-04-20 17:03:10	0	\N
1105	短信模板导出	system:sms-template:export	3	5	1100				\N	0	t	t	t		2021-04-01 17:35:17		2022-04-20 17:03:10	0	\N
1106	发送测试短信	system:sms-template:send-sms	3	6	1100				\N	0	t	t	t	1	2021-04-11 00:26:40	1	2022-04-20 17:03:10	0	\N
1108	短信日志查询	system:sms-log:query	3	1	1107				\N	0	t	t	t		2021-04-11 08:37:05		2022-04-20 17:03:10	0	\N
1109	短信日志导出	system:sms-log:export	3	5	1107				\N	0	t	t	t		2021-04-11 08:37:05		2022-04-20 17:03:10	0	\N
1139	租户查询	system:tenant:query	3	1	1138				\N	0	t	t	t		2021-12-14 12:31:44		2022-04-20 17:03:10	0	\N
1140	租户创建	system:tenant:create	3	2	1138				\N	0	t	t	t		2021-12-14 12:31:44		2022-04-20 17:03:10	0	\N
1141	租户更新	system:tenant:update	3	3	1138				\N	0	t	t	t		2021-12-14 12:31:44		2022-04-20 17:03:10	0	\N
1142	租户删除	system:tenant:delete	3	4	1138				\N	0	t	t	t		2021-12-14 12:31:44		2022-04-20 17:03:10	0	\N
1143	租户导出	system:tenant:export	3	5	1138				\N	0	t	t	t		2021-12-14 12:31:44		2022-04-20 17:03:10	0	\N
1118	请假查询		2	0	5	leave	fa:leanpub	bpm/oa/leave/index	BpmOALeave	0	t	t	t		2021-09-20 08:51:03	system	2026-07-17 01:44:04.417394	1	\N
1119	请假申请查询	bpm:oa-leave:query	3	1	1118				\N	0	t	t	t		2021-09-20 08:51:03	system	2026-07-17 01:44:04.417394	1	\N
1120	请假申请创建	bpm:oa-leave:create	3	2	1118				\N	0	t	t	t		2021-09-20 08:51:03	system	2026-07-17 01:44:04.417394	1	\N
6124	商机跟进	bid:opportunity:follow	3	4	6102				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
1226	租户套餐查询	system:tenant-package:query	3	1	1225				\N	0	t	t	t		2022-02-19 17:44:06		2022-04-20 17:03:10	0	\N
1227	租户套餐创建	system:tenant-package:create	3	2	1225				\N	0	t	t	t		2022-02-19 17:44:06		2022-04-20 17:03:10	0	\N
1228	租户套餐更新	system:tenant-package:update	3	3	1225				\N	0	t	t	t		2022-02-19 17:44:06		2022-04-20 17:03:10	0	\N
1229	租户套餐删除	system:tenant-package:delete	3	4	1225				\N	0	t	t	t		2022-02-19 17:44:06		2022-04-20 17:03:10	0	\N
1237	文件配置		2	0	1243	file-config	fa-solid:file-signature	infra/fileConfig/index	InfraFileConfig	0	t	t	t		2022-03-15 14:35:28	1	2024-02-29 08:52:54	0	\N
1238	文件配置查询	infra:file-config:query	3	1	1237				\N	0	t	t	t		2022-03-15 14:35:28		2022-04-20 17:03:10	0	\N
1239	文件配置创建	infra:file-config:create	3	2	1237				\N	0	t	t	t		2022-03-15 14:35:28		2022-04-20 17:03:10	0	\N
1240	文件配置更新	infra:file-config:update	3	3	1237				\N	0	t	t	t		2022-03-15 14:35:28		2022-04-20 17:03:10	0	\N
1241	文件配置删除	infra:file-config:delete	3	4	1237				\N	0	t	t	t		2022-03-15 14:35:28		2022-04-20 17:03:10	0	\N
1242	文件配置导出	infra:file-config:export	3	5	1237				\N	0	t	t	t		2022-03-15 14:35:28		2022-04-20 17:03:10	0	\N
1243	文件管理		2	6	2	file	ep:files	\N		0	t	t	t	1	2022-03-16 23:47:40	1	2024-04-23 00:02:11	0	\N
1255	数据源配置		2	1	2	data-source-config	ep:data-analysis	infra/dataSourceConfig/index	InfraDataSourceConfig	0	t	t	t		2022-04-27 14:37:32	1	2024-02-29 08:51:25	0	\N
1256	数据源配置查询	infra:data-source-config:query	3	1	1255				\N	0	t	t	t		2022-04-27 14:37:32		2022-04-27 14:37:32	0	\N
1257	数据源配置创建	infra:data-source-config:create	3	2	1255				\N	0	t	t	t		2022-04-27 14:37:32		2022-04-27 14:37:32	0	\N
1258	数据源配置更新	infra:data-source-config:update	3	3	1255				\N	0	t	t	t		2022-04-27 14:37:32		2022-04-27 14:37:32	0	\N
1259	数据源配置删除	infra:data-source-config:delete	3	4	1255				\N	0	t	t	t		2022-04-27 14:37:32		2022-04-27 14:37:32	0	\N
1260	数据源配置导出	infra:data-source-config:export	3	5	1255				\N	0	t	t	t		2022-04-27 14:37:32		2022-04-27 14:37:32	0	\N
1264	客户端查询	system:oauth2-client:query	3	1	1263				\N	0	t	t	t		2022-05-10 16:26:33	1	2022-05-11 00:31:06	0	\N
1265	客户端创建	system:oauth2-client:create	3	2	1263				\N	0	t	t	t		2022-05-10 16:26:33	1	2022-05-11 00:31:23	0	\N
1266	客户端更新	system:oauth2-client:update	3	3	1263				\N	0	t	t	t		2022-05-10 16:26:33	1	2022-05-11 00:31:28	0	\N
1267	客户端删除	system:oauth2-client:delete	3	4	1263				\N	0	t	t	t		2022-05-10 16:26:33	1	2022-05-11 00:31:33	0	\N
2132	账号查询	system:mail-account:query	3	1	2131				\N	0	t	t	t		2023-01-25 09:33:48		2023-01-25 09:33:48	0	\N
2133	账号创建	system:mail-account:create	3	2	2131				\N	0	t	t	t		2023-01-25 09:33:48		2023-01-25 09:33:48	0	\N
2134	账号更新	system:mail-account:update	3	3	2131				\N	0	t	t	t		2023-01-25 09:33:48		2023-01-25 09:33:48	0	\N
2135	账号删除	system:mail-account:delete	3	4	2131				\N	0	t	t	t		2023-01-25 09:33:48		2023-01-25 09:33:48	0	\N
6145	爬虫配置管理	bid:crawler:config	3	5	6104				\N	0	t	t	t	system	2026-07-15 00:57:21.707523	system	2026-07-17 01:44:04.417394	1	\N
6151	预警处理	bid:subscription:process	3	5	6105				\N	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6111	公告新增	bid:notice:create	3	1	6101				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6112	公告修改	bid:notice:update	3	2	6101				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6113	公告删除	bid:notice:delete	3	3	6101				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6121	商机新增	bid:opportunity:create	3	1	6102				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6122	商机指派	bid:opportunity:assign	3	2	6102				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6123	商机决策	bid:opportunity:decision	3	3	6102				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
2137	模版查询	system:mail-template:query	3	1	2136				\N	0	t	t	t		2023-01-25 12:05:31		2023-01-25 12:05:31	0	\N
2138	模版创建	system:mail-template:create	3	2	2136				\N	0	t	t	t		2023-01-25 12:05:31		2023-01-25 12:05:31	0	\N
2139	模版更新	system:mail-template:update	3	3	2136				\N	0	t	t	t		2023-01-25 12:05:31		2023-01-25 12:05:31	0	\N
2140	模版删除	system:mail-template:delete	3	4	2136				\N	0	t	t	t		2023-01-25 12:05:31		2023-01-25 12:05:31	0	\N
2142	日志查询	system:mail-log:query	3	1	2141				\N	0	t	t	t		2023-01-26 02:16:50		2023-01-26 02:16:50	0	\N
2143	发送测试邮件	system:mail-template:send-mail	3	5	2136				\N	0	t	t	t	1	2023-01-26 23:29:15	1	2023-01-26 23:29:15	0	\N
2146	站内信模板查询	system:notify-template:query	3	1	2145				\N	0	t	t	t		2023-01-28 02:26:42		2023-01-28 02:26:42	0	\N
2147	站内信模板创建	system:notify-template:create	3	2	2145				\N	0	t	t	t		2023-01-28 02:26:42		2023-01-28 02:26:42	0	\N
2148	站内信模板更新	system:notify-template:update	3	3	2145				\N	0	t	t	t		2023-01-28 02:26:42		2023-01-28 02:26:42	0	\N
2149	站内信模板删除	system:notify-template:delete	3	4	2145				\N	0	t	t	t		2023-01-28 02:26:42		2023-01-28 02:26:42	0	\N
2150	发送测试站内信	system:notify-template:send-notify	3	5	2145				\N	0	t	t	t	1	2023-01-28 10:54:43	1	2023-01-28 10:54:43	0	\N
2152	站内信消息查询	system:notify-message:query	3	1	2151				\N	0	t	t	t		2023-01-28 04:28:22		2023-01-28 04:28:22	0	\N
2449	三方应用查询	system:social-client:query	3	1	2448					0	t	t	t	1	2023-11-04 12:43:12	1	2023-11-04 12:43:33	0	\N
2450	三方应用创建	system:social-client:create	3	2	2448					0	t	t	t	1	2023-11-04 12:43:58	1	2023-11-04 12:43:58	0	\N
2451	三方应用更新	system:social-client:update	3	3	2448					0	t	t	t	1	2023-11-04 12:44:27	1	2023-11-04 12:44:27	0	\N
2452	三方应用删除	system:social-client:delete	3	4	2448					0	t	t	t	1	2023-11-04 12:44:43	1	2023-11-04 12:44:43	0	\N
2525	WebSocket		2	5	2	websocket	ep:connection	infra/webSocket/index	InfraWebSocket	0	t	t	t	1	2023-11-23 19:41:55	1	2024-04-23 00:02:00	0	\N
2740	监控中心		1	10	2	monitors	ep:monitor			0	t	t	t	1	2024-04-23 00:04:44	1	2024-04-23 00:04:44	0	\N
2759	AI 对话		2	1	2758	chat	ep:message	ai/chat/index/index.vue	AiChat	0	t	t	t	1	2024-05-07 15:09:14	1	2024-07-07 17:15:36	0	\N
2768	聊天模型查询	ai:model:query	3	1	2767					0	t	t	t		2024-05-10 14:42:48	1	2025-03-03 09:19:46	0	\N
2769	聊天模型创建	ai:model:create	3	2	2767					0	t	t	t		2024-05-10 14:42:48	1	2025-03-03 09:20:10	0	\N
2770	聊天模型更新	ai:model:update	3	3	2767					0	t	t	t		2024-05-10 14:42:48	1	2025-03-03 09:20:14	0	\N
2771	聊天模型删除	ai:model:delete	3	4	2767					0	t	t	t		2024-05-10 14:42:48	1	2025-03-03 09:20:27	0	\N
2774	聊天角色查询	ai:chat-role:query	3	1	2773				\N	0	t	t	t		2024-05-13 12:39:28		2024-05-13 12:39:28	0	\N
2775	聊天角色创建	ai:chat-role:create	3	2	2773				\N	0	t	t	t		2024-05-13 12:39:28		2024-05-13 12:39:28	0	\N
2776	聊天角色更新	ai:chat-role:update	3	3	2773				\N	0	t	t	t		2024-05-13 12:39:28		2024-05-13 12:39:28	0	\N
2777	聊天角色删除	ai:chat-role:delete	3	4	2773					0	t	t	t	1	2024-05-13 21:43:38	1	2024-05-13 21:43:38	0	\N
2779	会话查询	ai:chat-conversation:query	3	1	2778					0	t	t	t		2024-05-24 15:39:18	1	2024-05-25 08:38:30	0	\N
2780	会话删除	ai:chat-conversation:delete	3	2	2778					0	t	t	t		2024-05-24 15:39:18	1	2024-05-25 08:38:40	0	\N
2781	消息查询	ai:chat-message:query	3	11	2778					0	t	t	t	1	2024-05-25 08:38:56	1	2024-05-25 08:38:56	0	\N
2782	消息删除	ai:chat-message:delete	3	12	2778					0	t	t	t	1	2024-05-25 08:39:10	1	2024-05-25 08:39:10	0	\N
2783	AI 绘画		2	2	2758	image	ep:picture-rounded	ai/image/index/index.vue	AiImage	0	t	t	t	1	2024-05-26 11:45:17	1	2024-07-07 17:18:59	0	\N
2785	绘画查询	ai:image:query	3	1	2784					0	t	t	t		2024-06-26 13:32:31	1	2024-06-26 22:21:57	0	\N
2786	绘画删除	ai:image:delete	3	4	2784					0	t	t	t		2024-06-26 13:32:31	1	2024-06-26 22:22:08	0	\N
2787	绘图更新	ai:image:update	3	2	2784					0	t	t	t	1	2024-06-26 22:47:56	1	2024-08-31 09:21:35	0	\N
2789	音乐查询	ai:music:query	3	1	2788				\N	0	t	t	t		2024-06-27 15:03:33		2024-06-27 15:03:33	0	\N
2790	音乐更新	ai:music:update	3	3	2788				\N	0	t	t	t		2024-06-27 15:03:33		2024-06-27 15:03:33	0	\N
2791	音乐删除	ai:music:delete	3	4	2788				\N	0	t	t	t		2024-06-27 15:03:33		2024-06-27 15:03:33	0	\N
2792	AI 写作		2	3	2758	write	fa-solid:book-reader	ai/write/index/index.vue	AiWrite	0	t	t	t	1	2024-07-08 09:26:44	1	2024-07-16 13:03:06	0	\N
2758	AI 大模型		1	30	0	/ai	tabler:ai		Ai	0	t	t	t	1	2024-05-07 15:07:56	1	2026-07-17 01:35:39.386526	0	\N
2739	消息中心		1	7	1	messages	ep:chat-dot-round			0	t	t	t	1	2024-04-22 23:54:30	1	2026-07-17 01:35:39.386526	1	\N
2762	API 密钥查询	ai:api-key:query	3	1	2761					0	t	t	t		2024-05-09 14:52:56	system	2026-07-17 01:44:04.417394	1	\N
2763	API 密钥创建	ai:api-key:create	3	2	2761					0	t	t	t		2024-05-09 14:52:56	system	2026-07-17 01:44:04.417394	1	\N
2764	API 密钥更新	ai:api-key:update	3	3	2761					0	t	t	t		2024-05-09 14:52:56	system	2026-07-17 01:44:04.417394	1	\N
2497	主子表（ERP）		2	11	1070	demo03-erp	ep:calendar	infra/demo/demo03/erp/index	Demo03StudentERP	0	t	t	t		2023-11-16 15:50:59	migration	2026-09-27 10:58:39.178077	1	\N
2765	API 密钥删除	ai:api-key:delete	3	4	2761					0	t	t	t		2024-05-09 14:52:56	system	2026-07-17 01:44:04.417394	1	\N
2794	AI 写作查询	ai:write:query	3	1	2793				\N	0	t	t	t		2024-07-10 13:24:34		2024-07-10 13:24:34	0	\N
2795	AI 写作删除	ai:write:delete	3	4	2793				\N	0	t	t	t		2024-07-10 13:24:34		2024-07-10 13:24:34	0	\N
2796	AI 音乐		2	4	2758	music	fa:music	ai/music/index/index.vue	AiMusic	0	t	t	t	1	2024-07-17 09:21:12	1	2024-07-29 21:11:52	0	\N
2915	AI 知识库		2	5	2758	knowledge	ep:notebook	ai/knowledge/knowledge/index	AiKnowledge	0	t	t	t		2025-02-28 07:04:21	1	2025-03-02 18:58:37	0	\N
2916	AI 知识库查询	ai:knowledge:query	3	1	2915				\N	0	t	t	t		2025-02-28 07:04:21		2025-02-28 07:04:21	0	\N
2917	AI 知识库创建	ai:knowledge:create	3	2	2915				\N	0	t	t	t		2025-02-28 07:04:21		2025-02-28 07:04:21	0	\N
2918	AI 知识库更新	ai:knowledge:update	3	3	2915				\N	0	t	t	t		2025-02-28 07:04:21		2025-02-28 07:04:21	0	\N
2919	AI 知识库删除	ai:knowledge:delete	3	4	2915				\N	0	t	t	t		2025-02-28 07:04:21		2025-02-28 07:04:21	0	\N
2921	工具查询	ai:tool:query	3	1	2920				\N	0	t	t	t		2025-03-14 11:19:29		2025-03-14 11:19:29	0	\N
2922	工具创建	ai:tool:create	3	2	2920				\N	0	t	t	t		2025-03-14 11:19:29		2025-03-14 11:19:29	0	\N
2923	工具更新	ai:tool:update	3	3	2920				\N	0	t	t	t		2025-03-14 11:19:29		2025-03-14 11:19:29	0	\N
2924	工具删除	ai:tool:delete	3	4	2920				\N	0	t	t	t		2025-03-14 11:19:29		2025-03-14 11:19:29	0	\N
5010	租户切换	system:tenant:visit	3	999	1138					0	t	t	t	1	2025-05-05 15:25:32	1	2025-05-05 15:25:32	0	\N
2798	AI 思维导图		2	6	2758	mind-map	fa:sitemap	ai/mindmap/index/index.vue	AiMindMap	0	t	t	t	1	2024-07-29 21:31:59	system	2026-07-17 01:44:04.417394	1	\N
30000	AI 大模型		1	40	0	/ai	tabler:ai		Ai	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30001	AI 对话		2	1	30000	chat	lucide:message-circle	ai/chat/index/index	AiChat	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30002	AI 绘图		2	2	30000	image	lucide:image	ai/image/index/index	AiImage	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30003	AI 写作		2	3	30000	write	lucide:pen-line	ai/write/index/index	AiWrite	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
2920	工具管理		2	0	2760	tool	lucide:wrench	ai/model/tool/index.vue	AiTool	0	t	t	t		2025-03-14 11:19:29	system	2026-07-17 01:13:48.097106	0	\N
2800	思维导图查询	ai:mind-map:query	3	1	2799				\N	0	t	t	t		2024-08-10 09:15:09	system	2026-07-17 01:44:04.417394	1	\N
2801	思维导图删除	ai:mind-map:delete	3	4	2799				\N	0	t	t	t		2024-08-10 09:15:09	system	2026-07-17 01:44:04.417394	1	\N
5000	AI 工作流		2	5	2758	workflow	fa:hand-grab-o	ai/workflow/index.vue	AiWorkflow	0	t	t	t	1	2025-03-25 09:50:27	system	2026-07-17 01:44:04.417394	1	\N
5001	AI 工作流查询	ai:workflow:query	3	1	5000					0	t	t	t	1	2025-03-25 09:51:11	system	2026-07-17 01:44:04.417394	1	\N
30004	AI 音乐		2	4	30000	music	lucide:music	ai/music/index/index	AiMusic	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30005	知识库	ai:knowledge:query	2	5	30000	knowledge	lucide:database	ai/knowledge/knowledge/index	AiKnowledge	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30006	模型管理	ai:model:query	2	6	30000	model	lucide:brain-circuit	ai/model/model/index	AiModel	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30007	聊天角色	ai:chat-role:query	2	7	30000	model/chat-role	lucide:bot	ai/model/chatRole/index	AiModelChatRole	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
30008	工具管理	ai:tool:query	2	8	30000	model/tool	lucide:wrench	ai/model/tool/index	AiModelTool	0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-16 08:12:35.316268	1	\N
2760	控制台		1	100	2758	console	lucide:settings-2			0	t	t	t	1	2024-05-09 22:39:09	system	2026-07-17 01:13:48.097106	0	\N
2913	流程清理	bpm:model:clean	3	7	1193					0	t	t	t	1	2025-01-17 19:32:06	migration	2026-09-27 10:58:39.135334	1	\N
2767	模型配置		2	0	2760	model	lucide:brain-circuit	ai/model/model/index.vue	AiModel	0	t	t	t		2024-05-10 14:42:48	system	2026-07-17 01:13:48.097106	0	\N
2773	聊天角色		2	0	2760	chat-role	lucide:bot	ai/model/chatRole/index.vue	AiChatRole	0	t	t	t		2024-05-13 12:39:28	system	2026-07-17 01:13:48.097106	0	\N
2778	聊天管理		2	10	2760	chat-conversation	lucide:messages-square	ai/chat/manager/index.vue	AiChatManager	0	t	t	t		2024-05-24 15:39:18	system	2026-07-17 01:13:48.097106	0	\N
2784	绘画管理		2	11	2760	image	lucide:images	ai/image/manager/index.vue	AiImageManager	0	t	t	t		2024-06-26 13:32:31	system	2026-07-17 01:13:48.097106	0	\N
2788	音乐管理		2	12	2760	music	lucide:list-music	ai/music/manager/index.vue	AiMusicManager	0	t	t	t		2024-06-27 15:03:33	system	2026-07-17 01:13:48.097106	0	\N
2793	写作管理		2	13	2760	write	lucide:book-text	ai/write/manager/index.vue	AiWriteManager	0	t	t	t		2024-07-10 13:24:34	system	2026-07-17 01:13:48.097106	0	\N
30101	模型查询	ai:model:query	3	1	30006					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:39.834971	1	\N
30111	知识库创建	ai:knowledge:create	3	1	30005					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:41.726946	1	\N
30121	角色创建	ai:chat-role:create	3	1	30007					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:43.474946	1	\N
30131	工具创建	ai:tool:create	3	1	30008					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:45.030092	1	\N
30102	模型创建	ai:model:create	3	2	30006					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:46.999118	1	\N
30104	模型删除	ai:model:delete	3	4	30006					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:51.00945	1	\N
30123	角色删除	ai:chat-role:delete	3	3	30007					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:53.230952	1	\N
30133	工具删除	ai:tool:delete	3	3	30008					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:55.362036	1	\N
30113	知识库删除	ai:knowledge:delete	3	3	30005					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:57.281626	1	\N
30103	模型更新	ai:model:update	3	3	30006					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:21:59.506947	1	\N
30132	工具更新	ai:tool:update	3	2	30008					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:22:01.05254	1	\N
30122	角色更新	ai:chat-role:update	3	2	30007					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:22:02.775135	1	\N
30112	知识库更新	ai:knowledge:update	3	2	30005					0	t	t	t	system	2026-07-16 05:03:23.711366	system	2026-07-17 01:22:04.945537	1	\N
30200	仪表盘		1	-10	0	/dashboard	lucide:layout-dashboard		Dashboard	0	t	t	t	system	2026-07-17 01:29:09.910057	system	2026-07-17 01:29:09.910057	0	\N
30201	工作台		2	1	30200	/workspace	carbon:workspace	dashboard/workspace/index	Workspace	0	t	t	t	system	2026-07-17 01:29:09.910057	system	2026-07-17 01:29:09.910057	0	\N
30203	个人中心		2	99	1	/profile	lucide:user-round	_core/profile/index	Profile	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 01:29:09.910057	0	\N
2761	API 密钥		2	0	2760	api-key	lucide:key-round	ai/model/apiKey/index.vue	AiApiKey	0	t	t	t		2024-05-09 14:52:56	system	2026-07-17 01:44:04.417394	1	\N
30210	绘图作品	ai:image:query	2	90	2758	image/square	lucide:images	ai/image/square/index	AiImageSquare	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2783
30211	知识库文档	ai:knowledge:query	2	91	2758	knowledge/document	lucide:files	ai/knowledge/document/index	AiKnowledgeDocument	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2915
30212	创建文档	ai:knowledge:create	2	92	2758	knowledge/document/create	lucide:file-plus-2	ai/knowledge/document/form/index	AiKnowledgeDocumentCreate	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2915
500	操作日志		2	8	1	operatelog	ep:position	system/operatelog/index	SystemOperateLog	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
501	登录日志		2	9	1	loginlog	ep:promotion	system/loginlog/index	SystemLoginLog	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
107	通知公告		2	7	1	notice	ep:takeaway-box	system/notice/index	SystemNotice	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
1138	租户管理		2	1	30240	/system/tenant	ep:house	system/tenant/index	SystemTenant	0	t	t	t		2021-12-14 12:31:43	1	2026-07-17 01:35:39.386526	0	\N
1225	租户套餐		2	2	30240	/system/tenant-package	fa:bars	system/tenantPackage/index	SystemTenantPackage	0	t	t	t		2022-02-19 17:44:06	1	2026-07-17 01:35:39.386526	0	\N
2083	地区管理		2	3	30240	/system/area	fa:map-marker	system/area/index	SystemArea	0	t	t	t	1	2022-12-23 17:35:05	1	2026-07-17 01:35:39.386526	0	\N
2448	社交客户端		2	4	30240	/system/social-client	ep:set-up	system/social/client/index.vue	SystemSocialClient	0	t	t	t	1	2023-11-04 12:17:19	1	2026-07-17 01:35:39.386526	0	\N
2453	社交用户	system:social-user:query	2	5	30240	/system/social-user	ep:avatar	system/social/user/index.vue	SystemSocialUser	0	t	t	t	1	2023-11-04 14:01:05	1	2026-07-17 01:35:39.386526	0	\N
1263	OAuth2 客户端		2	6	30240	/system/oauth2-client	fa:hdd-o	system/oauth2/client/index	SystemOauth2Client	0	t	t	t		2022-05-10 16:26:33	1	2026-07-17 01:35:39.386526	0	\N
109	OAuth2 令牌		2	7	30240	/system/oauth2-token	fa:key	system/oauth2/token/index	SystemOauth2Token	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
2145	站内信模板		2	1	30241	/system/notify-template	fa:archive	system/notify/template/index	SystemNotifyTemplate	0	t	t	t		2023-01-28 02:26:42	1	2026-07-17 01:35:39.386526	0	\N
2151	站内信消息		2	2	30241	/system/notify-message-list	fa:edit	system/notify/message/index	SystemNotifyMessage	0	t	t	t		2023-01-28 04:28:22	1	2026-07-17 01:35:39.386526	0	\N
2131	邮箱账号		2	3	30241	/system/mail-account	fa:universal-access	system/mail/account/index	SystemMailAccount	0	t	t	t		2023-01-25 09:33:48	1	2026-07-17 01:35:39.386526	0	\N
2136	邮件模板		2	4	30241	/system/mail-template	fa:tag	system/mail/template/index	SystemMailTemplate	0	t	t	t		2023-01-25 12:05:31	1	2026-07-17 01:35:39.386526	0	\N
1185	工作流		1	50	0	/bpm	fa:medium	\N	bpm	0	t	t	t	1	2021-12-30 20:26:36	migration	2026-09-27 10:58:39.135334	1	\N
30202	分析页		2	2	30200	/analytics	lucide:area-chart	dashboard/analytics/index	Analytics	0	t	t	t	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.151558	1	\N
2141	邮件日志		2	5	30241	/system/mail-log	fa:edit	system/mail/log/index	SystemMailLog	0	t	t	t		2023-01-26 02:16:50	1	2026-07-17 01:35:39.386526	0	\N
1094	短信渠道		2	6	30241	/system/sms-channel	fa:stack-exchange	system/sms/channel/index	SystemSmsChannel	0	t	t	t		2021-04-01 11:07:15	1	2026-07-17 01:35:39.386526	0	\N
1100	短信模板		2	7	30241	/system/sms-template	ep:connection	system/sms/template/index	SystemSmsTemplate	0	t	t	t		2021-04-01 17:35:17	1	2026-07-17 01:35:39.386526	0	\N
1107	短信日志		2	8	30241	/system/sms-log	fa:edit	system/sms/log/index	SystemSmsLog	0	t	t	t		2021-04-11 08:37:05	1	2026-07-17 01:35:39.386526	0	\N
108	审计日志		1	9	1	log	ep:document-copy		\N	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	1	\N
1224	租户管理		2	0	1	tenant	fa-solid:house-user	\N	\N	0	t	t	t	1	2022-02-20 01:41:13	1	2026-07-17 01:35:39.386526	1	\N
1261	OAuth 2.0		2	10	1	oauth2	fa:dashcube	\N	\N	0	t	t	t	1	2022-05-09 23:38:17	1	2026-07-17 01:35:39.386526	1	\N
2130	邮箱管理		2	2	2739	mail	fa-solid:mail-bulk	\N	\N	0	t	t	t	1	2023-01-25 17:27:44	1	2026-07-17 01:35:39.386526	1	\N
2144	站内信管理		1	3	2739	notify	ep:message-box	\N	\N	0	t	t	t	1	2023-01-28 10:25:18	1	2026-07-17 01:35:39.386526	1	\N
2447	三方登录		1	10	1	social	fa:rocket			0	t	t	t	1	2023-11-04 12:12:01	1	2026-07-17 01:35:39.386526	1	\N
6101	招标公告	bid:notice:query	2	1	6100	notice	ant-design:notification-outlined	bid/notice/index	BidNotice	0	t	t	t	system	2026-07-14 06:07:11.723882	system	2026-07-17 01:44:04.417394	1	\N
6100	招标管理		1	22	0	/bid	ant-design:solution-outlined	\N	\N	0	t	t	t	system	2026-07-14 06:07:11.723882	system	2026-07-17 01:44:04.417394	1	\N
6102	投标商机	bid:opportunity:query	2	2	6100	opportunity	ant-design:bulb-outlined	bid/opportunity/index	BidOpportunity	0	t	t	t	system	2026-07-14 06:07:11.723882	system	2026-07-17 01:44:04.417394	1	\N
6103	投标项目	bid:project:query	2	3	6100	project	ant-design:project-outlined	bid/project/index	BidProject	0	t	t	t	system	2026-07-14 06:07:11.723882	system	2026-07-17 01:44:04.417394	1	\N
6125	商机转项目	bid:opportunity:convert	3	5	6102				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6131	项目修改	bid:project:update	3	1	6103				\N	0	t	t	t	system	2026-07-14 08:22:10.528054	system	2026-07-17 01:44:04.417394	1	\N
6104	爬虫管理	bid:crawler:query	2	4	6100	crawler	ant-design:bug-outlined	bid/crawler/index	BidCrawler	0	t	t	t	system	2026-07-15 00:40:22.525741	system	2026-07-17 01:44:04.417394	1	\N
6141	爬虫查询	bid:crawler:query	3	1	6104				\N	0	t	t	t	system	2026-07-15 00:40:22.525741	system	2026-07-17 01:44:04.417394	1	\N
6142	爬虫任务创建	bid:crawler:create	3	2	6104				\N	0	t	t	t	system	2026-07-15 00:40:22.525741	system	2026-07-17 01:44:04.417394	1	\N
6143	爬虫任务重试	bid:crawler:retry	3	3	6104				\N	0	t	t	t	system	2026-07-15 00:40:22.525741	system	2026-07-17 01:44:04.417394	1	\N
6144	爬虫调度管理	bid:crawler:schedule	3	4	6104				\N	0	t	t	t	system	2026-07-15 00:40:22.525741	system	2026-07-17 01:44:04.417394	1	\N
6105	订阅预警	bid:subscription:query	2	5	6100	subscription	ant-design:bell-outlined	bid/subscription/index	BidSubscription	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6147	订阅预警查询	bid:subscription:query	3	1	6105				\N	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6148	订阅规则创建	bid:subscription:create	3	2	6105				\N	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6149	订阅规则修改	bid:subscription:update	3	3	6105				\N	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6150	订阅规则删除	bid:subscription:delete	3	4	6105				\N	0	t	t	t	system	2026-07-15 10:45:40.244018	system	2026-07-17 01:44:04.417394	1	\N
6146	爬虫登录态管理	bid:crawler:login	3	6	6104				\N	0	t	t	t	system	2026-07-15 01:11:30.159654	system	2026-07-17 01:44:04.417394	1	\N
5002	AI 工作流创建	ai:workflow:create	3	2	5000					0	t	t	t	1	2025-03-25 09:51:28	system	2026-07-17 01:44:04.417394	1	\N
5003	AI 工作流更新	ai:workflow:update	3	3	5000					0	t	t	t	1	2025-03-25 09:51:42	system	2026-07-17 01:44:04.417394	1	\N
5004	AI 工作流删除	ai:workflow:delete	3	4	5000					0	t	t	t	1	2025-03-25 09:51:55	system	2026-07-17 01:44:04.417394	1	\N
5005	AI 工作流测试	ai:workflow:test	3	5	5000					0	t	t	t	1	2025-03-30 10:29:41	system	2026-07-17 01:44:04.417394	1	\N
2799	导图管理		2	14	2760	mind-map	lucide:network	ai/mindmap/manager/index	AiMindMapManager	0	t	t	t		2024-08-10 09:15:09	system	2026-07-17 01:44:04.417394	1	\N
30213	修改文档	ai:knowledge:update	2	93	2758	knowledge/document/update	lucide:file-pen-line	ai/knowledge/document/form/index	AiKnowledgeDocumentUpdate	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2915
30214	文档召回测试	ai:knowledge:query	2	94	2758	knowledge/retrieval	lucide:search-check	ai/knowledge/knowledge/retrieval/index	AiKnowledgeRetrieval	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2915
30215	知识库分段	ai:knowledge:query	2	95	2758	knowledge/segment	lucide:blocks	ai/knowledge/segment/index	AiKnowledgeSegment	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2915
30230	调度日志	infra:job:query	2	90	2	/infra/job/log	lucide:scroll-text	infra/job/logger/index	InfraJobLog	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	110
30231	生成配置修改	infra:codegen:update	2	91	2	/infra/codegen/edit	lucide:file-cog	infra/codegen/edit/index	InfraCodegenEdit	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	115
30232	我的站内信	system:notify-message:query	2	90	1	/system/notify-message	lucide:mail	system/notify/my/index	MyNotifyMessage	0	f	f	f	system	2026-07-17 01:29:09.910057	system	2026-07-17 02:02:24.52913	0	2151
1189	表单创建	bpm:form:create	3	2	1187				\N	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
30256	资产查询	infra:asset:query	3	1	30243					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30257	资产创建	infra:asset:create	3	2	30243					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30258	资产更新	infra:asset:update	3	3	30243					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30259	资产删除	infra:asset:delete	3	4	30243					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30260	服务商查询	infra:service-provider:query	3	1	30244					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30261	服务商创建	infra:service-provider:create	3	2	30244					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30262	服务商更新	infra:service-provider:update	3	3	30244					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30263	服务商删除	infra:service-provider:delete	3	4	30244					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30264	机房查询	infra:machine-room:query	3	1	30245					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30265	机房创建	infra:machine-room:create	3	2	30245					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30266	机房更新	infra:machine-room:update	3	3	30245					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30267	机房删除	infra:machine-room:delete	3	4	30245					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30268	云资源区查询	infra:cloud-zone:query	3	1	30246					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30269	云资源区创建	infra:cloud-zone:create	3	2	30246					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30270	云资源区更新	infra:cloud-zone:update	3	3	30246					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30271	云资源区删除	infra:cloud-zone:delete	3	4	30246					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30272	云平台查询	infra:cloud-platform:query	3	1	30247					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30273	云平台创建	infra:cloud-platform:create	3	2	30247					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30274	云平台更新	infra:cloud-platform:update	3	3	30247					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30275	云平台删除	infra:cloud-platform:delete	3	4	30247					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30276	云对接查询	infra:cloud-provider-config:query	3	1	30248					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30277	云对接创建	infra:cloud-provider-config:create	3	2	30248					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30278	云对接更新	infra:cloud-provider-config:update	3	3	30248					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30279	云对接删除	infra:cloud-provider-config:delete	3	4	30248					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30280	网络区域查询	infra:network-zone:query	3	1	30249					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30281	网络区域创建	infra:network-zone:create	3	2	30249					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30282	网络区域更新	infra:network-zone:update	3	3	30249					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30283	网络区域删除	infra:network-zone:delete	3	4	30249					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30284	安全产品查询	infra:security-product:query	3	1	30250					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30285	安全产品创建	infra:security-product:create	3	2	30250					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30286	安全产品更新	infra:security-product:update	3	3	30250					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30287	安全产品删除	infra:security-product:delete	3	4	30250					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30288	业务应用查询	infra:business-application:query	3	1	30251					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30289	业务应用创建	infra:business-application:create	3	2	30251					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30290	业务应用更新	infra:business-application:update	3	3	30251					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30291	业务应用删除	infra:business-application:delete	3	4	30251					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30296	资源工单查询	infra:resource-ticket:query	3	1	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30297	资源工单创建	infra:resource-ticket:create	3	2	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30298	资源工单更新	infra:resource-ticket:update	3	3	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30299	资源工单删除	infra:resource-ticket:delete	3	4	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30300	资源工单审批	infra:resource-ticket:approve	3	5	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30301	资源工单配置	infra:resource-ticket:provision	3	6	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30302	资源工单交付	infra:resource-ticket:deliver	3	7	30253					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30303	任务查询	infra:task:query	3	1	30254					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30304	任务创建	infra:task:create	3	2	30254					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30305	任务更新	infra:task:update	3	3	30254					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30306	任务删除	infra:task:delete	3	4	30254					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30307	任务执行	infra:task:execute	3	5	30254					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30308	风险查询	infra:risk:query	3	1	30255					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30309	风险更新	infra:risk:update	3	2	30255					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30310	风险处置	infra:risk:resolve	3	3	30255					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.025521	0	\N
30312	网络策略更新	infra:network-policy:update	3	3	30311					0	t	t	t	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	\N
30313	网络策略创建	infra:network-policy:create	3	2	30311					0	t	t	t	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	\N
30314	网络策略查询	infra:network-policy:query	3	1	30311					0	t	t	t	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	\N
30315	网络策略删除	infra:network-policy:delete	3	4	30311					0	t	t	t	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	\N
30317	模型管理		2	1	30316	model	lucide:shapes	cmdb/model/index	CmdbModelManagement	0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30318	实例管理		2	2	30316	instance	lucide:shapes	cmdb/instance/index	CmdbInstanceManagement	0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30323	属性创建	cmdb:attribute:create	3	5	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30324	属性更新	cmdb:attribute:update	3	6	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30325	属性删除	cmdb:attribute:delete	3	7	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30331	审批规则查询	infra:approval-rule:query	3	1	30330					0	t	t	t	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	\N
30332	审批规则创建	infra:approval-rule:create	3	2	30330					0	t	t	t	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	\N
30333	审批规则更新	infra:approval-rule:update	3	3	30330					0	t	t	t	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	\N
30316	配置管理		1	10	0	/cmdb	lucide:database			0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30311	网络策略		2	2	30342	network-policy	lucide:shield-check	asset-ops/network-policy/index	AssetOpsNetworkPolicy	0	t	t	t	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.127499	0	\N
30330	审批规则		2	2	30344	approval-rule	lucide:git-pull-request-arrow	asset-ops/approval-rule/index	AssetOpsApprovalRule	0	t	t	t	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.127499	0	\N
30292	云资源查询	infra:cloud-resource:query	3	1	30252					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.158178	0	\N
30293	云资源创建	infra:cloud-resource:create	3	2	30252					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.158178	0	\N
30294	云资源更新	infra:cloud-resource:update	3	3	30252					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.158178	0	\N
30295	云资源删除	infra:cloud-resource:delete	3	4	30252					0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.158178	0	\N
30334	审批规则删除	infra:approval-rule:delete	3	4	30330					0	t	t	t	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	\N
30319	模型查询	cmdb:model:query	3	1	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30320	模型创建	cmdb:model:create	3	2	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30321	模型更新	cmdb:model:update	3	3	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30322	模型删除	cmdb:model:delete	3	4	30317					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30326	实例查询	cmdb:instance:query	3	1	30318					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30327	实例创建	cmdb:instance:create	3	2	30318					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30328	实例更新	cmdb:instance:update	3	3	30318					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30329	实例删除	cmdb:instance:delete	3	4	30318					0	t	t	t	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	\N
30340	基础设施		1	11	0	/infra-center	lucide:server-cog			0	t	t	t	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	\N
30341	云管中心		1	12	0	/cloud-center	lucide:cloud-cog			0	t	t	t	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	\N
30342	资产中心		1	13	0	/asset-center	lucide:boxes			0	t	t	t	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	\N
30343	业务管理		1	14	0	/biz-center	lucide:briefcase			0	t	t	t	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	\N
30344	运营流程		1	15	0	/ops-center	lucide:workflow			0	t	t	t	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	\N
30243	资产管理		2	1	30342	assets	lucide:hard-drive	asset-ops/asset/index	AssetOpsAsset	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30244	服务商		2	1	30340	service-provider	lucide:briefcase-business	asset-ops/service-provider/index	AssetOpsServiceProvider	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30245	物理机房		2	2	30340	machine-room	lucide:warehouse	asset-ops/machine-room/index	AssetOpsMachineRoom	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30246	云资源区		2	2	30341	cloud-zone	lucide:map-pin	asset-ops/cloud-platform/zone	AssetOpsCloudZone	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30247	云平台		2	1	30341	cloud-platform	lucide:cloud-cog	asset-ops/cloud-platform/platform	AssetOpsCloudPlatform	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30248	云厂商对接		2	3	30341	cloud-provider-config	lucide:plug-zap	asset-ops/cloud-provider-config/index	AssetOpsCloudProviderConfig	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30249	网络区域		2	3	30340	zone	lucide:globe	asset-ops/zone/index	AssetOpsNetworkZone	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30250	安全产品		2	4	30340	security-product	lucide:shield	asset-ops/security-product/index	AssetOpsSecurityProduct	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30251	业务应用		2	1	30343	business-application	lucide:layout-grid	asset-ops/business-application/index	AssetOpsBusinessApplication	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30253	资源工单		2	1	30344	resource-ticket	lucide:ticket	asset-ops/resource-ticket/index	AssetOpsResourceTicket	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30254	扫描任务		2	3	30342	task	lucide:list-checks	asset-ops/task/index	AssetOpsTask	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30255	风险管理		2	4	30342	risk	lucide:triangle-alert	asset-ops/risk/index	AssetOpsRisk	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	0	\N
30242	资产运营		1	35	0	/asset-ops	lucide:boxes		AssetOps	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.127499	1	\N
1	系统功能		1	20	0	/system	lucide:settings	\N	System	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
30240	基础设置		1	21	0	/system/basic	lucide:sliders-horizontal		SystemBasic	0	t	t	t	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	\N
30241	信息中心		1	22	0	/system/message	lucide:mail		SystemMessage	0	t	t	t	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	\N
2	基础功能		1	25	0	/infra	lucide:blocks	\N	Infra	0	t	t	t	admin	2021-01-05 17:03:48	1	2026-07-17 01:35:39.386526	0	\N
30336	组织网段查询	cmdb:net-zone:query	3	1	1138					0	t	t	t	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.135334	0	\N
30337	组织网段创建	cmdb:net-zone:create	3	2	1138					0	t	t	t	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.135334	0	\N
30338	组织网段更新	cmdb:net-zone:update	3	3	1138					0	t	t	t	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.135334	0	\N
30339	组织网段删除	cmdb:net-zone:delete	3	4	1138					0	t	t	t	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.135334	0	\N
30335	组织网段		2	3	30316	net-zone	lucide:network	cmdb/net-zone/index	CmdbNetZone	0	t	t	t	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.135334	1	\N
1186	流程管理		1	10	1185	manager	fa:dedent	\N	\N	0	t	t	t	1	2021-12-30 20:28:30	migration	2026-09-27 10:58:39.135334	1	\N
1187	流程表单		2	2	1186	form	fa:hdd-o	bpm/form/index	BpmForm	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
1188	表单查询	bpm:form:query	3	1	1187				\N	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
30252	云资源		2	2	30343	cloud-resource	lucide:cloud	asset-ops/business-resource/cloud/index	AssetOpsCloudResource	0	t	t	t	migration	2026-09-27 10:58:39.025521	migration	2026-09-27 10:58:39.154658	0	\N
1190	表单更新	bpm:form:update	3	3	1187				\N	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
1191	表单删除	bpm:form:delete	3	4	1187				\N	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
1192	表单导出	bpm:form:export	3	5	1187				\N	0	t	t	t		2021-12-30 12:38:22	migration	2026-09-27 10:58:39.135334	1	\N
1193	流程模型		2	1	1186	model	fa-solid:project-diagram	bpm/model/index	BpmModel	0	t	t	t	1	2021-12-31 23:24:58	migration	2026-09-27 10:58:39.135334	1	\N
1194	模型查询	bpm:model:query	3	1	1193				\N	0	t	t	t	1	2022-01-03 19:01:10	migration	2026-09-27 10:58:39.135334	1	\N
1195	模型创建	bpm:model:create	3	2	1193				\N	0	t	t	t	1	2022-01-03 19:01:24	migration	2026-09-27 10:58:39.135334	1	\N
1197	模型更新	bpm:model:update	3	4	1193				\N	0	t	t	t	1	2022-01-03 19:02:28	migration	2026-09-27 10:58:39.135334	1	\N
1198	模型删除	bpm:model:delete	3	5	1193				\N	0	t	t	t	1	2022-01-03 19:02:43	migration	2026-09-27 10:58:39.135334	1	\N
1199	模型发布	bpm:model:deploy	3	6	1193				\N	0	t	t	t	1	2022-01-03 19:03:24	migration	2026-09-27 10:58:39.135334	1	\N
1200	审批中心		2	20	1185	task	fa:tasks	\N	\N	0	t	t	t	1	2022-01-07 23:51:48	migration	2026-09-27 10:58:39.135334	1	\N
1201	我的流程		2	1	1200	my	fa-solid:book	bpm/processInstance/index	BpmProcessInstanceMy	0	t	t	t		2022-01-07 15:53:44	migration	2026-09-27 10:58:39.135334	1	\N
1202	流程实例的查询	bpm:process-instance:query	3	1	1201				\N	0	t	t	t		2022-01-07 15:53:44	migration	2026-09-27 10:58:39.135334	1	\N
1207	待办任务		2	10	1200	todo	fa:slack	bpm/task/todo/index	BpmTodoTask	0	t	t	t	1	2022-01-08 10:33:37	migration	2026-09-27 10:58:39.135334	1	\N
1208	已办任务		2	20	1200	done	fa:delicious	bpm/task/done/index	BpmDoneTask	0	t	t	t	1	2022-01-08 10:34:13	migration	2026-09-27 10:58:39.135334	1	\N
1209	用户分组		2	4	1186	user-group	fa:user-secret	bpm/group/index	BpmUserGroup	0	t	t	t		2022-01-14 02:14:20	migration	2026-09-27 10:58:39.135334	1	\N
1210	用户组查询	bpm:user-group:query	3	1	1209				\N	0	t	t	t		2022-01-14 02:14:20	migration	2026-09-27 10:58:39.135334	1	\N
1211	用户组创建	bpm:user-group:create	3	2	1209				\N	0	t	t	t		2022-01-14 02:14:20	migration	2026-09-27 10:58:39.135334	1	\N
1212	用户组更新	bpm:user-group:update	3	3	1209				\N	0	t	t	t		2022-01-14 02:14:20	migration	2026-09-27 10:58:39.135334	1	\N
1213	用户组删除	bpm:user-group:delete	3	4	1209				\N	0	t	t	t		2022-01-14 02:14:20	migration	2026-09-27 10:58:39.135334	1	\N
1215	流程定义查询	bpm:process-definition:query	3	10	1193				\N	0	t	t	t	1	2022-01-23 00:21:43	migration	2026-09-27 10:58:39.135334	1	\N
1216	流程任务分配规则查询	bpm:task-assign-rule:query	3	20	1193				\N	0	t	t	t	1	2022-01-23 00:26:53	migration	2026-09-27 10:58:39.135334	1	\N
1217	流程任务分配规则创建	bpm:task-assign-rule:create	3	21	1193				\N	0	t	t	t	1	2022-01-23 00:28:15	migration	2026-09-27 10:58:39.135334	1	\N
1218	流程任务分配规则更新	bpm:task-assign-rule:update	3	22	1193				\N	0	t	t	t	1	2022-01-23 00:28:41	migration	2026-09-27 10:58:39.135334	1	\N
1219	流程实例的创建	bpm:process-instance:create	3	2	1201				\N	0	t	t	t	1	2022-01-23 00:36:15	migration	2026-09-27 10:58:39.135334	1	\N
1220	流程实例的取消	bpm:process-instance:cancel	3	3	1201				\N	0	t	t	t	1	2022-01-23 00:36:33	migration	2026-09-27 10:58:39.135334	1	\N
1221	流程任务的查询	bpm:task:query	3	1	1207				\N	0	t	t	t	1	2022-01-23 00:38:52	migration	2026-09-27 10:58:39.135334	1	\N
1222	流程任务的更新	bpm:task:update	3	2	1207				\N	0	t	t	t	1	2022-01-23 00:39:24	migration	2026-09-27 10:58:39.135334	1	\N
2713	抄送我的	bpm:process-instance-cc:query	2	30	1200	copy	ep:copy-document	bpm/task/copy/index	BpmProcessInstanceCopy	0	t	t	t	1	2024-03-17 21:50:23	migration	2026-09-27 10:58:39.135334	1	\N
2714	流程分类		2	3	1186	category	fa:object-ungroup	bpm/category/index	BpmCategory	0	t	t	t		2024-03-08 02:00:51	migration	2026-09-27 10:58:39.135334	1	\N
2715	分类查询	bpm:category:query	3	1	2714					0	t	t	t		2024-03-08 02:00:51	migration	2026-09-27 10:58:39.135334	1	\N
2716	分类创建	bpm:category:create	3	2	2714					0	t	t	t		2024-03-08 02:00:51	migration	2026-09-27 10:58:39.135334	1	\N
2717	分类更新	bpm:category:update	3	3	2714					0	t	t	t		2024-03-08 02:00:51	migration	2026-09-27 10:58:39.135334	1	\N
2718	分类删除	bpm:category:delete	3	4	2714					0	t	t	t		2024-03-08 02:00:51	migration	2026-09-27 10:58:39.135334	1	\N
2720	发起流程		2	0	1200	create	fa-solid:grin-stars	bpm/processInstance/create/index	BpmProcessInstanceCreate	0	t	f	t	1	2024-03-19 19:46:05	migration	2026-09-27 10:58:39.135334	1	\N
2721	流程实例		2	10	1186	process-instance/manager	fa:square	bpm/processInstance/manager/index	BpmProcessInstanceManager	0	t	t	t	1	2024-03-21 23:57:30	migration	2026-09-27 10:58:39.135334	1	\N
2722	流程实例的查询（管理员）	bpm:process-instance:manager-query	3	1	2721					0	t	t	t	1	2024-03-22 08:18:27	migration	2026-09-27 10:58:39.135334	1	\N
2723	流程实例的取消（管理员）	bpm:process-instance:cancel-by-admin	3	2	2721					0	t	t	t	1	2024-03-22 08:19:25	migration	2026-09-27 10:58:39.135334	1	\N
2724	流程任务		2	11	1186	process-tasnk	ep:collection-tag	bpm/task/manager/index	BpmManagerTask	0	t	t	t	1	2024-03-22 08:43:22	migration	2026-09-27 10:58:39.135334	1	\N
2725	流程任务的查询（管理员）	bpm:task:manager-query	3	1	2724					0	t	t	t	1	2024-03-22 08:43:49	migration	2026-09-27 10:58:39.135334	1	\N
2726	流程监听器		2	5	1186	process-listener	fa:assistive-listening-systems	bpm/processListener/index	BpmProcessListener	0	t	t	t		2024-03-09 16:05:34	migration	2026-09-27 10:58:39.135334	1	\N
2727	流程监听器查询	bpm:process-listener:query	3	1	2726				\N	0	t	t	t		2024-03-09 16:05:34	migration	2026-09-27 10:58:39.135334	1	\N
2728	流程监听器创建	bpm:process-listener:create	3	2	2726				\N	0	t	t	t		2024-03-09 16:05:34	migration	2026-09-27 10:58:39.135334	1	\N
2729	流程监听器更新	bpm:process-listener:update	3	3	2726				\N	0	t	t	t		2024-03-09 16:05:34	migration	2026-09-27 10:58:39.135334	1	\N
2730	流程监听器删除	bpm:process-listener:delete	3	4	2726				\N	0	t	t	t		2024-03-09 16:05:34	migration	2026-09-27 10:58:39.135334	1	\N
2731	流程表达式		2	6	1186	process-expression	fa:wpexplorer	bpm/processExpression/index	BpmProcessExpression	0	t	t	t		2024-03-09 22:35:08	migration	2026-09-27 10:58:39.135334	1	\N
2732	流程表达式查询	bpm:process-expression:query	3	1	2731				\N	0	t	t	t		2024-03-09 22:35:08	migration	2026-09-27 10:58:39.135334	1	\N
2733	流程表达式创建	bpm:process-expression:create	3	2	2731				\N	0	t	t	t		2024-03-09 22:35:08	migration	2026-09-27 10:58:39.135334	1	\N
2734	流程表达式更新	bpm:process-expression:update	3	3	2731				\N	0	t	t	t		2024-03-09 22:35:08	migration	2026-09-27 10:58:39.135334	1	\N
2735	流程表达式删除	bpm:process-expression:delete	3	4	2731				\N	0	t	t	t		2024-03-09 22:35:08	migration	2026-09-27 10:58:39.135334	1	\N
30220	流程详情	bpm:process-instance:query	2	90	1185	process-instance/detail	lucide:file-search	bpm/processInstance/detail/index	BpmProcessInstanceDetail	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1201
30221	设计流程表单	bpm:form:update	2	91	1185	manager/form/edit	lucide:file-pen-line	bpm/form/designer/index	BpmFormEditor	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1187
30222	创建流程	bpm:model:create	2	92	1185	manager/model/create	lucide:workflow	bpm/model/form/index	BpmModelCreate	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1193
30223	修改流程	bpm:model:update	2	93	1185	manager/model/:type/:id	lucide:workflow	bpm/model/form/index	BpmModelUpdate	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1193
30224	流程定义	bpm:definition:query	2	94	1185	manager/definition	lucide:file-cog	bpm/model/definition/index	BpmProcessDefinition	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1193
30225	流程数据报表	bpm:process-instance:query	2	95	1185	process-instance/report	lucide:chart-no-axes-combined	bpm/processInstance/report/index	BpmProcessInstanceReport	0	f	f	f	system	2026-07-17 01:29:09.910057	migration	2026-09-27 10:58:39.135334	1	1193
30345	物理资源		2	3	30343	physical-resource	lucide:hard-drive	asset-ops/business-resource/physical/index	AssetOpsPhysicalResource	0	t	t	t	migration	2026-09-27 10:58:39.154658	migration	2026-09-27 10:58:39.154658	0	\N
30349	物理资源删除	infra:physical-resource:delete	3	4	30345				\N	0	t	t	t	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	\N
30346	物理资源查询	infra:physical-resource:query	3	1	30345				\N	0	t	t	t	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	\N
30348	物理资源更新	infra:physical-resource:update	3	3	30345				\N	0	t	t	t	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	\N
30347	物理资源创建	infra:physical-resource:create	3	2	30345				\N	0	t	t	t	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	\N
30350	资产核查		2	5	30342	inspection	lucide:scan-search	asset-ops/inspection/index	AssetInspection	0	t	t	t	migration	2026-09-27 10:58:39.170703	migration	2026-09-27 10:58:39.170703	0	\N
1070	代码生成案例		1	1	2	demo	ep:aim	infra/testDemo/index	\N	0	t	t	t		2021-02-06 12:42:49	migration	2026-09-27 10:58:39.178077	1	\N
2472	主子表（内嵌）		2	12	1070	demo03-inner	fa:power-off	infra/demo/demo03/inner/index	Demo03StudentInner	0	t	t	t		2023-11-13 04:39:51	migration	2026-09-27 10:58:39.178077	1	\N
2478	单表（增删改查）		2	1	1070	demo01-contact	ep:bicycle	infra/demo/demo01/index	Demo01Contact	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2479	示例联系人查询	infra:demo01-contact:query	3	1	2478				\N	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2480	示例联系人创建	infra:demo01-contact:create	3	2	2478				\N	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2481	示例联系人更新	infra:demo01-contact:update	3	3	2478				\N	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2482	示例联系人删除	infra:demo01-contact:delete	3	4	2478				\N	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2483	示例联系人导出	infra:demo01-contact:export	3	5	2478				\N	0	t	t	t		2023-11-15 14:42:30	migration	2026-09-27 10:58:39.178077	1	\N
2484	树表（增删改查）		2	2	1070	demo02-category	fa:tree	infra/demo/demo02/index	Demo02Category	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2485	示例分类查询	infra:demo02-category:query	3	1	2484				\N	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2486	示例分类创建	infra:demo02-category:create	3	2	2484				\N	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2487	示例分类更新	infra:demo02-category:update	3	3	2484				\N	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2488	示例分类删除	infra:demo02-category:delete	3	4	2484				\N	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2489	示例分类导出	infra:demo02-category:export	3	5	2484				\N	0	t	t	t		2023-11-16 12:18:27	migration	2026-09-27 10:58:39.178077	1	\N
2490	主子表（标准）		2	10	1070	demo03-normal	fa:battery-3	infra/demo/demo03/normal/index	Demo03StudentNormal	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
2491	学生查询	infra:demo03-student:query	3	1	2490				\N	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
2492	学生创建	infra:demo03-student:create	3	2	2490				\N	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
2493	学生更新	infra:demo03-student:update	3	3	2490				\N	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
2494	学生删除	infra:demo03-student:delete	3	4	2490				\N	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
2495	学生导出	infra:demo03-student:export	3	5	2490				\N	0	t	t	t		2023-11-16 12:53:37	migration	2026-09-27 10:58:39.178077	1	\N
116	API 接口	infra:swagger:list	2	3	2	api-docs	fa:fighter-jet	infra/api-docs/index	InfraApiDocs	0	t	t	t	admin	2021-01-05 17:03:48	migration	2026-09-27 10:58:39.182363	0	\N
\.


--
-- Data for Name: system_notice; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_notice (id, title, content, type, status, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
4	我是测试标题	<p>哈哈哈哈123</p>	1	0	110	2022-02-22 01:01:25	110	2022-02-22 01:01:46	0	121
1	芋道的公众	<p>新版本内容133222</p>	1	0	admin	2021-01-05 17:03:48	migration	2026-09-27 10:58:39.141962	1	1
2	维护通知：2018-07-01 系统凌晨维护	<p><img src="http://test.yudao.iocoder.cn/b7cb3cf49b4b3258bf7309a09dd2f4e5.jpg" alt="" data-href="">11112222<img src="http://test.yudao.iocoder.cn/fe44fc7bdb82ca421184b2eebbaee9e2148d4a1827479a4eb4521e11d2a062ba.png" alt="image" data-href="http://test.yudao.iocoder.cn/fe44fc7bdb82ca421184b2eebbaee9e2148d4a1827479a4eb4521e11d2a062ba.png">3333</p>	2	1	admin	2021-01-05 17:03:48	migration	2026-09-27 10:58:39.141962	1	1
\.


--
-- Data for Name: system_notify_message; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_notify_message (id, user_id, user_type, template_id, template_code, template_nickname, template_content, template_type, template_params, read_status, read_time, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_notify_template; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_notify_template (id, name, code, nickname, content, type, params, status, remark, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: system_oauth2_access_token; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_oauth2_access_token (id, user_id, user_type, user_info, access_token, refresh_token, client_id, scopes, expires_time, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_oauth2_approve; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_oauth2_approve (id, user_id, user_type, client_id, scope, approved, expires_time, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_oauth2_client; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_oauth2_client (id, client_id, secret, name, logo, description, status, access_token_validity_seconds, refresh_token_validity_seconds, redirect_uris, authorized_grant_types, scopes, auto_approve_scopes, authorities, resource_ids, additional_information, creator, create_time, updater, update_time, deleted) FROM stdin;
1	default	admin123	RustSet		RustSet 默认 OAuth2 客户端	0	1800	2592000	["http://127.0.0.1:5666"]	["password","authorization_code","implicit","refresh_token","client_credentials"]	["user.read","user.write"]	[]	["user.read","user.write"]	[]	{}	1	2022-05-11 21:47:12	migration	2026-09-27 10:58:39.141962	0
40	test	test2	biubiu	http://test.yudao.iocoder.cn/20251227/javayuanma_1766829882970.jpg	啦啦啦啦	0	1800	43200	["https://www.iocoder.cn"]	["password","authorization_code","implicit"]	["user_info","projects"]	["user_info"]	[]	[]	{}	1	2022-05-12 00:28:20	migration	2026-09-27 10:58:39.141962	1
41	yudao-sso-demo-by-code	test	基于授权码模式，如何实现 SSO 单点登录？	http://test.yudao.iocoder.cn/it/20250502/sign_1746181948685.png	\N	0	1800	43200	["http://127.0.0.1:18080"]	["authorization_code","refresh_token"]	["user.read","user.write"]	[]	[]	[]	\N	1	2022-09-29 13:28:31	migration	2026-09-27 10:58:39.141962	1
42	yudao-sso-demo-by-password	test	基于密码模式，如何实现 SSO 单点登录？	http://test.yudao.iocoder.cn/20251025/images (3)_1761360515810.jpeg	\N	0	1800	43200	["http://127.0.0.1:18080"]	["password","refresh_token"]	["user.read","user.write"]	[]	[]	[]	\N	1	2022-10-04 17:40:16	migration	2026-09-27 10:58:39.141962	1
\.


--
-- Data for Name: system_oauth2_code; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_oauth2_code (id, user_id, user_type, code, client_id, scopes, expires_time, redirect_uri, state, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_oauth2_refresh_token; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_oauth2_refresh_token (id, user_id, refresh_token, user_type, client_id, scopes, expires_time, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_operate_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_operate_log (id, trace_id, user_id, user_type, type, sub_type, biz_id, action, success, extra, request_method, request_url, user_ip, user_agent, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_post; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_post (id, code, name, sort, status, remark, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
2	se	项目经理	2	0		admin	2021-01-05 17:03:48	1	2025-12-15 22:38:43	0	1
4	user	普通员工	4	0	111222	admin	2021-01-05 17:03:48	1	2025-03-24 21:32:40	0	1
5	HR	人力资源	5	0	`	1	2024-03-24 20:45:40	1	2025-03-29 19:08:10	0	1
7	test	测试	10	0	\N	1	2025-09-02 08:45:57	1	2025-09-02 08:45:57	0	1
\.


--
-- Data for Name: system_role; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_role (id, name, code, sort, data_scope, data_scope_dept_ids, status, type, remark, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
1	超级管理员	super_admin	1	1		0	1	超级管理员	admin	2021-01-05 17:03:48		2022-02-22 05:08:21	0	1
2	普通角色	common	2	2		0	1	普通角色	admin	2021-01-05 17:03:48		2022-02-22 05:08:20	0	1
3	CRM 管理员	crm_admin	2	1		0	1	CRM 专属角色	1	2024-02-24 10:51:13	1	2024-02-24 02:51:32	0	1
109	租户管理员	tenant_admin	0	1		0	1	系统自动生成	1	2022-02-22 00:56:14	1	2022-02-22 00:56:14	0	121
111	租户管理员	tenant_admin	0	1		0	1	系统自动生成	1	2022-03-07 21:37:58	1	2022-03-07 21:37:58	0	122
155	测试数据权限1	test-dp	4	2	[112,100,102,103,104,105,107,108]	0	2	1111	1	2025-03-31 14:58:06	1	2025-12-04 23:29:40	0	1
157	CMDB 管理员	cmdb_admin	20	1		0	1	\N	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
158	CMDB 用户	cmdb_user	21	1		0	1	\N	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
\.


--
-- Data for Name: system_role_menu; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_role_menu (id, role_id, menu_id, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
263	109	1	1	2022-02-22 00:56:14	1	2022-02-22 00:56:14	0	121
434	2	1	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
454	2	1093	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
455	2	1094	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
460	2	1100	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
467	2	1107	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
6367	1	6143	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
477	2	100	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
478	2	101	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
479	2	102	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
6368	1	6144	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
481	2	103	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
483	2	104	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
485	2	105	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
488	2	107	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
490	2	108	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
492	2	109	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
498	2	1138	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
523	2	1224	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
524	2	1225	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
541	2	500	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
543	2	501	1	2022-02-22 13:09:12	1	2022-02-22 13:09:12	0	1
675	2	2	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
689	2	1077	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
690	2	1078	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
692	2	1083	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
693	2	1084	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
699	2	1090	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
703	2	106	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
704	2	110	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
705	2	111	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
706	2	112	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
707	2	113	1	2022-02-22 13:16:57	1	2022-02-22 13:16:57	0	1
1296	110	1	110	2022-02-23 00:23:55	110	2022-02-23 00:23:55	0	121
1578	111	1	1	2022-03-07 21:37:58	1	2022-03-07 21:37:58	0	122
1729	109	100	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1730	109	101	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1731	109	1063	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1732	109	1064	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1733	109	1001	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1734	109	1065	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1735	109	1002	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1736	109	1003	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1737	109	1004	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1738	109	1005	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1739	109	1006	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1740	109	1007	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1741	109	1008	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1742	109	1009	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1743	109	1010	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1744	109	1011	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1745	109	1012	1	2022-09-21 22:08:51	1	2022-09-21 22:08:51	0	121
1746	111	100	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1747	111	101	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1748	111	1063	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1749	111	1064	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1750	111	1001	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1751	111	1065	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1752	111	1002	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1753	111	1003	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1754	111	1004	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1755	111	1005	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1756	111	1006	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1757	111	1007	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1758	111	1008	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1759	111	1009	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1760	111	1010	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1761	111	1011	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1762	111	1012	1	2022-09-21 22:08:52	1	2022-09-21 22:08:52	0	122
1763	109	100	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1764	109	101	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1765	109	1063	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1766	109	1064	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1767	109	1001	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1768	109	1065	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1769	109	1002	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1770	109	1003	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1771	109	1004	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1772	109	1005	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1773	109	1006	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1774	109	1007	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1775	109	1008	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1776	109	1009	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1777	109	1010	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1778	109	1011	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1779	109	1012	1	2022-09-21 22:08:53	1	2022-09-21 22:08:53	0	121
1780	111	100	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1781	111	101	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1782	111	1063	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1783	111	1064	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1784	111	1001	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1785	111	1065	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1786	111	1002	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1787	111	1003	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1788	111	1004	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1789	111	1005	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1790	111	1006	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1791	111	1007	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1792	111	1008	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1793	111	1009	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1794	111	1010	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1795	111	1011	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1796	111	1012	1	2022-09-21 22:08:54	1	2022-09-21 22:08:54	0	122
1797	109	100	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1798	109	101	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1799	109	1063	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1800	109	1064	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1801	109	1001	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1802	109	1065	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1803	109	1002	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1804	109	1003	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1805	109	1004	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1806	109	1005	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1807	109	1006	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1808	109	1007	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1809	109	1008	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1810	109	1009	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1811	109	1010	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1812	109	1011	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1813	109	1012	1	2022-09-21 22:08:55	1	2022-09-21 22:08:55	0	121
1814	111	100	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1815	111	101	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1816	111	1063	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1817	111	1064	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1818	111	1001	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1819	111	1065	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1820	111	1002	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1821	111	1003	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1822	111	1004	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1823	111	1005	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1824	111	1006	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1825	111	1007	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1826	111	1008	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1827	111	1009	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1828	111	1010	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1829	111	1011	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1830	111	1012	1	2022-09-21 22:08:56	1	2022-09-21 22:08:56	0	122
1831	109	103	1	2022-09-21 22:43:23	1	2022-09-21 22:43:23	0	121
1832	109	1017	1	2022-09-21 22:43:23	1	2022-09-21 22:43:23	0	121
1833	109	1018	1	2022-09-21 22:43:23	1	2022-09-21 22:43:23	0	121
1834	109	1019	1	2022-09-21 22:43:23	1	2022-09-21 22:43:23	0	121
1835	109	1020	1	2022-09-21 22:43:23	1	2022-09-21 22:43:23	0	121
1836	111	103	1	2022-09-21 22:43:24	1	2022-09-21 22:43:24	0	122
1837	111	1017	1	2022-09-21 22:43:24	1	2022-09-21 22:43:24	0	122
1838	111	1018	1	2022-09-21 22:43:24	1	2022-09-21 22:43:24	0	122
1839	111	1019	1	2022-09-21 22:43:24	1	2022-09-21 22:43:24	0	122
1840	111	1020	1	2022-09-21 22:43:24	1	2022-09-21 22:43:24	0	122
1841	109	1036	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	121
1842	109	1037	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	121
1843	109	1038	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	121
1844	109	1039	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	121
1845	109	107	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	121
1846	111	1036	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	122
1847	111	1037	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	122
1848	111	1038	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	122
1849	111	1039	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	122
1850	111	107	1	2022-09-21 22:48:13	1	2022-09-21 22:48:13	0	122
1991	2	1024	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1992	2	1025	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1993	2	1026	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1994	2	1027	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1995	2	1028	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1996	2	1029	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1997	2	1030	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1998	2	1031	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
1999	2	1032	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2000	2	1033	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2001	2	1034	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2002	2	1035	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2003	2	1036	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2004	2	1037	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2005	2	1038	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2006	2	1039	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2007	2	1040	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2008	2	1042	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2009	2	1043	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2010	2	1045	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2011	2	1046	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2012	2	1048	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2013	2	1050	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2014	2	1051	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2015	2	1052	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2016	2	1053	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2017	2	1054	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2018	2	1056	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2019	2	1057	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2020	2	1058	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2021	2	2083	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2022	2	1059	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2023	2	1060	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2024	2	1063	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2025	2	1064	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2026	2	1065	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2027	2	1066	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2028	2	1067	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2034	2	1075	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2036	2	1082	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2037	2	1085	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2038	2	1086	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2039	2	1087	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2040	2	1088	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2041	2	1089	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2042	2	1091	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2043	2	1092	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2044	2	1095	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2045	2	1096	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2046	2	1097	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2047	2	1098	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2048	2	1101	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2049	2	1102	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2050	2	1103	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2051	2	1104	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2052	2	1105	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2053	2	1106	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2054	2	1108	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2055	2	1109	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
6369	1	6142	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
6370	1	6104	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
6371	1	6100	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
6372	1	6141	system	2026-07-15 00:40:28.757045	system	2026-07-15 00:40:28.757045	0	1
2072	2	114	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2073	2	1139	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2074	2	115	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2075	2	1140	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2076	2	116	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2077	2	1141	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2078	2	1142	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2079	2	1143	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2099	2	1226	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2100	2	1227	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2101	2	1228	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2102	2	1229	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2103	2	1237	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2104	2	1238	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2105	2	1239	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2106	2	1240	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2107	2	1241	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2108	2	1242	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2109	2	1243	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2117	2	1255	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2118	2	1256	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2119	2	1257	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2120	2	1258	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2121	2	1259	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2122	2	1260	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2123	2	1261	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2124	2	1263	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2125	2	1264	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2126	2	1265	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2127	2	1266	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2128	2	1267	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2129	2	1001	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2130	2	1002	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2131	2	1003	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2132	2	1004	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2133	2	1005	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2134	2	1006	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2135	2	1007	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2136	2	1008	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2137	2	1009	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2138	2	1010	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2139	2	1011	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2140	2	1012	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2141	2	1013	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2143	2	1015	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2145	2	1017	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2146	2	1018	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2147	2	1019	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2148	2	1020	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2149	2	1021	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2150	2	1022	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
2151	2	1023	1	2023-01-25 08:42:52	1	2023-01-25 08:42:52	0	1
6373	1	6145	system	2026-07-15 00:57:21.724071	system	2026-07-15 00:57:21.724071	0	1
6375	1	6151	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
6376	1	6148	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
6377	1	6147	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
6378	1	6105	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
6379	1	6149	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
6380	1	6150	system	2026-07-15 10:45:40.244018	system	2026-07-15 10:45:40.244018	0	1
2929	109	1224	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2930	109	1225	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2931	109	1226	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2932	109	1227	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2933	109	1228	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2934	109	1229	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2935	109	1138	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2936	109	1139	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2937	109	1140	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2938	109	1141	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2939	109	1142	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2940	109	1143	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	121
2941	111	1224	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2942	111	1225	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2943	111	1226	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2944	111	1227	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2945	111	1228	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2946	111	1229	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2947	111	1138	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2948	111	1139	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2949	111	1140	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2950	111	1141	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2951	111	1142	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2952	111	1143	1	2023-12-02 23:19:40	1	2023-12-02 23:19:40	0	122
2993	109	2	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2994	109	1031	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2995	109	1032	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2996	109	1033	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2997	109	1034	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2998	109	1035	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
2999	109	1050	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3000	109	1051	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3001	109	1052	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3002	109	1053	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3003	109	1054	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3004	109	1056	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3005	109	1057	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3006	109	1058	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3007	109	1059	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3008	109	1060	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3009	109	1066	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3010	109	1067	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3012	109	1075	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3014	109	1077	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3015	109	1078	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3016	109	1082	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3017	109	1083	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3018	109	1084	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3019	109	1085	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3020	109	1086	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3021	109	1087	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3022	109	1088	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3023	109	1089	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3024	109	1090	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3025	109	1091	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3026	109	1092	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3027	109	106	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3028	109	110	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3029	109	111	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3030	109	112	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3031	109	113	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3032	109	114	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3033	109	115	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3034	109	116	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3055	109	1237	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3056	109	1238	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3057	109	1239	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3058	109	1240	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3059	109	1241	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3060	109	1242	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3061	109	1243	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3062	109	2525	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3063	109	1255	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3064	109	1256	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3065	109	1257	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3066	109	1258	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3067	109	1259	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3068	109	1260	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	121
3069	111	2	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3070	111	1031	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3071	111	1032	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3072	111	1033	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3073	111	1034	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3074	111	1035	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3075	111	1050	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3076	111	1051	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3077	111	1052	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3078	111	1053	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3079	111	1054	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3080	111	1056	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3081	111	1057	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3082	111	1058	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3083	111	1059	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3084	111	1060	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3085	111	1066	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3086	111	1067	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3088	111	1075	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3090	111	1077	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3091	111	1078	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3092	111	1082	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3093	111	1083	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3094	111	1084	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3095	111	1085	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3096	111	1086	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3097	111	1087	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3098	111	1088	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3099	111	1089	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3100	111	1090	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3101	111	1091	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3102	111	1092	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3103	111	106	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3104	111	110	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3105	111	111	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3106	111	112	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3107	111	113	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3108	111	114	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3109	111	115	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3110	111	116	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3131	111	1237	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3132	111	1238	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3133	111	1239	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3134	111	1240	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3135	111	1241	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3136	111	1242	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3137	111	1243	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3138	111	2525	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3139	111	1255	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3140	111	1256	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3141	111	1257	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3142	111	1258	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3143	111	1259	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3144	111	1260	1	2023-12-02 23:41:02	1	2023-12-02 23:41:02	0	122
3221	109	102	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	121
3222	109	1013	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	121
3223	109	1014	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	121
3224	109	1015	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	121
3225	109	1016	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	121
3226	111	102	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	122
3227	111	1013	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	122
3228	111	1014	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	122
3229	111	1015	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	122
3230	111	1016	1	2023-12-30 11:42:36	1	2023-12-30 11:42:36	0	122
4163	109	5	1	2024-03-30 17:53:17	1	2024-03-30 17:53:17	0	121
4164	109	1118	1	2024-03-30 17:53:17	1	2024-03-30 17:53:17	0	121
4165	109	1119	1	2024-03-30 17:53:17	1	2024-03-30 17:53:17	0	121
4166	109	1120	1	2024-03-30 17:53:17	1	2024-03-30 17:53:17	0	121
3130	111	2497	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3111	111	2472	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3112	111	2478	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3118	111	2484	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3124	111	2490	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3113	111	2479	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3114	111	2480	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3115	111	2481	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3116	111	2482	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3117	111	2483	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3119	111	2485	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3120	111	2486	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3121	111	2487	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3122	111	2488	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3123	111	2489	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3125	111	2491	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
4222	111	5	1	2024-03-30 17:53:18	1	2024-03-30 17:53:18	0	122
4223	111	1118	1	2024-03-30 17:53:18	1	2024-03-30 17:53:18	0	122
4224	111	1119	1	2024-03-30 17:53:18	1	2024-03-30 17:53:18	0	122
4225	111	1120	1	2024-03-30 17:53:18	1	2024-03-30 17:53:18	0	122
5779	2	2739	1	2024-07-07 20:39:38	1	2024-07-07 20:39:38	0	1
5780	2	2740	1	2024-07-07 20:39:38	1	2024-07-07 20:39:38	0	1
5781	2	2758	1	2024-07-07 20:39:38	1	2024-07-07 20:39:38	0	1
5782	2	2759	1	2024-07-07 20:39:38	1	2024-07-07 20:39:38	0	1
6374	1	6146	system	2026-07-15 01:11:30.173694	system	2026-07-15 01:11:30.173694	0	1
5789	109	2739	1	2024-07-13 22:37:24	1	2024-07-13 22:37:24	0	121
5790	109	2740	1	2024-07-13 22:37:24	1	2024-07-13 22:37:24	0	121
5791	111	2739	1	2024-07-13 22:37:24	1	2024-07-13 22:37:24	0	122
5792	111	2740	1	2024-07-13 22:37:24	1	2024-07-13 22:37:24	0	122
6293	2	5	1	2026-01-04 18:09:41	1	2026-01-04 18:09:41	0	1
6294	2	1118	1	2026-01-04 18:09:41	1	2026-01-04 18:09:41	0	1
6295	2	1119	1	2026-01-04 18:09:41	1	2026-01-04 18:09:41	0	1
6296	2	1120	1	2026-01-04 18:09:41	1	2026-01-04 18:09:41	0	1
330000	1	30000	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330001	1	30001	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330002	1	30002	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330003	1	30003	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330004	1	30004	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330005	1	30005	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330006	1	30006	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330007	1	30007	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330008	1	30008	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330101	1	30101	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330102	1	30102	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330103	1	30103	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330104	1	30104	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330111	1	30111	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330112	1	30112	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330113	1	30113	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330121	1	30121	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330122	1	30122	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330123	1	30123	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330131	1	30131	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330132	1	30132	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330133	1	30133	system	2026-07-16 05:03:23.720981	system	2026-07-16 05:03:23.720981	0	1
330134	2	30240	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330135	109	30240	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330136	111	30240	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330137	2	30241	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330138	109	30241	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330139	111	30241	system	2026-07-17 01:35:39.386526	system	2026-07-17 01:35:39.386526	0	0
330140	1	30242	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330141	1	30243	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330142	1	30244	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330143	1	30245	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330144	1	30246	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330145	1	30247	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330146	1	30248	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330147	1	30249	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330148	1	30250	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330149	1	30251	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330150	1	30252	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330151	1	30253	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330152	1	30254	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330153	1	30255	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330154	1	30256	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330155	1	30257	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330156	1	30258	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330157	1	30259	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330158	1	30260	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330159	1	30261	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330160	1	30262	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330161	1	30263	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330162	1	30264	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330163	1	30265	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330164	1	30266	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330165	1	30267	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330166	1	30268	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330167	1	30269	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330168	1	30270	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330169	1	30271	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330170	1	30272	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330171	1	30273	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330172	1	30274	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330173	1	30275	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330174	1	30276	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330175	1	30277	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330176	1	30278	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330177	1	30279	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330178	1	30280	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330179	1	30281	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330180	1	30282	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330181	1	30283	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330182	1	30284	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330183	1	30285	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330184	1	30286	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330185	1	30287	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330186	1	30288	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330187	1	30289	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330188	1	30290	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330189	1	30291	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330190	1	30292	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330191	1	30293	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330192	1	30294	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330193	1	30295	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330194	1	30296	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330195	1	30297	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330196	1	30298	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330197	1	30299	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330198	1	30300	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330199	1	30301	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330200	1	30302	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330201	1	30303	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330202	1	30304	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330203	1	30305	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330204	1	30306	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330205	1	30307	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330206	1	30308	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330207	1	30309	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330208	1	30310	migration	2026-09-27 10:58:39.043865	migration	2026-09-27 10:58:39.043865	0	1
330209	1	30311	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	1
330210	1	30312	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	1
330211	1	30313	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	1
330212	1	30314	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	1
330213	1	30315	migration	2026-09-27 10:58:39.052932	migration	2026-09-27 10:58:39.052932	0	1
330214	1	30316	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330215	1	30317	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330216	1	30318	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330217	1	30319	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330218	1	30320	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330219	1	30321	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330220	1	30322	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330221	1	30323	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330222	1	30324	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330223	1	30325	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330224	1	30326	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330225	1	30327	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330226	1	30328	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330227	1	30329	migration	2026-09-27 10:58:39.088447	migration	2026-09-27 10:58:39.088447	0	1
330228	1	30330	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	1
330229	1	30331	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	1
330230	1	30332	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	1
330231	1	30333	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	1
330232	1	30334	migration	2026-09-27 10:58:39.110657	migration	2026-09-27 10:58:39.110657	0	1
330233	157	30319	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330234	157	30320	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330235	157	30321	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330236	157	30322	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330237	157	30323	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330238	157	30324	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330239	157	30325	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330240	157	30326	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330241	157	30327	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330242	157	30328	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330243	157	30329	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330244	1	30335	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	1
330245	1	30336	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	1
330246	1	30337	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	1
330247	1	30338	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	1
330248	1	30339	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	1
330249	157	30335	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330250	157	30336	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330251	157	30337	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330252	157	30338	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330253	157	30339	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330254	158	30335	migration	2026-09-27 10:58:39.114963	migration	2026-09-27 10:58:39.114963	0	0
330259	158	30319	migration	2026-09-27 10:58:39.12294	migration	2026-09-27 10:58:39.12294	0	0
330260	158	30326	migration	2026-09-27 10:58:39.12294	migration	2026-09-27 10:58:39.12294	0	0
330261	158	30327	migration	2026-09-27 10:58:39.12294	migration	2026-09-27 10:58:39.12294	0	0
330262	158	30328	migration	2026-09-27 10:58:39.12294	migration	2026-09-27 10:58:39.12294	0	0
330263	1	30340	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	1
330264	1	30341	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	1
330265	1	30342	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	1
330266	1	30343	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	1
330267	1	30344	migration	2026-09-27 10:58:39.127499	migration	2026-09-27 10:58:39.127499	0	1
4167	109	2713	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4168	109	2714	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4169	109	2715	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4170	109	2716	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4171	109	2717	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4172	109	2718	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4173	109	2720	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4174	109	1185	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4175	109	2721	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4176	109	1186	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4177	109	2722	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4178	109	1187	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4179	109	2723	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4180	109	1188	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4181	109	2724	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4182	109	1189	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4183	109	2725	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4184	109	1190	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4185	109	2726	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4186	109	1191	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4187	109	2727	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4188	109	1192	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4189	109	2728	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4190	109	1193	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4191	109	2729	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4192	109	1194	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4193	109	2730	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4194	109	1195	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4195	109	2731	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4197	109	2732	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4198	109	1197	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4199	109	2733	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4200	109	1198	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4201	109	2734	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4202	109	1199	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4203	109	2735	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4204	109	1200	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4205	109	1201	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4206	109	1202	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4207	109	1207	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4208	109	1208	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4209	109	1209	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4210	109	1210	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4211	109	1211	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4212	109	1212	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4213	109	1213	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4214	109	1215	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4215	109	1216	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4216	109	1217	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4217	109	1218	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4218	109	1219	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4219	109	1220	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4220	109	1221	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4221	109	1222	1	2024-03-30 17:53:17	migration	2026-09-27 10:58:39.135334	1	121
4226	111	2713	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4227	111	2714	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4228	111	2715	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4229	111	2716	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4230	111	2717	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4231	111	2718	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4232	111	2720	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4233	111	1185	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4234	111	2721	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4235	111	1186	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4236	111	2722	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4237	111	1187	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4238	111	2723	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4239	111	1188	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4240	111	2724	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4241	111	1189	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4242	111	2725	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4243	111	1190	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4244	111	2726	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4245	111	1191	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4246	111	2727	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4247	111	1192	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4248	111	2728	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4249	111	1193	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4250	111	2729	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4251	111	1194	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4252	111	2730	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4253	111	1195	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4254	111	2731	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4256	111	2732	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4257	111	1197	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4258	111	2733	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4259	111	1198	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4260	111	2734	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4261	111	1199	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4262	111	2735	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4263	111	1200	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4264	111	1201	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4265	111	1202	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4266	111	1207	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4267	111	1208	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4268	111	1209	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4269	111	1210	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4270	111	1211	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4271	111	1212	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4272	111	1213	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4273	111	1215	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4274	111	1216	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4275	111	1217	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4276	111	1218	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4277	111	1219	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4278	111	1220	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4279	111	1221	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
4280	111	1222	1	2024-03-30 17:53:18	migration	2026-09-27 10:58:39.135334	1	122
6297	2	2713	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6298	2	2714	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6299	2	2715	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6300	2	2716	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6301	2	2717	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6302	2	2718	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6303	2	2720	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6304	2	1185	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6305	2	2721	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6306	2	1186	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6307	2	2722	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6308	2	1187	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6309	2	2723	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6310	2	1188	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6311	2	2724	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6312	2	1189	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6313	2	2725	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6314	2	1190	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6315	2	2726	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6316	2	1191	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6317	2	2727	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6318	2	1192	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6319	2	2728	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6320	2	1193	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6321	2	2729	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6322	2	1194	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6323	2	2730	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6324	2	1195	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6325	2	2731	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6326	2	2732	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6327	2	1197	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6328	2	2733	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6329	2	1198	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6330	2	2734	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6331	2	1199	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6332	2	2735	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6333	2	1200	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6334	2	1201	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6335	2	1202	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6336	2	1207	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6337	2	1208	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6338	2	1209	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6339	2	1210	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6340	2	1211	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6341	2	1212	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6342	2	1213	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6343	2	1215	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6344	2	1216	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6345	2	1217	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6346	2	1218	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6347	2	1219	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6348	2	1220	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6349	2	1221	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6350	2	1222	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
6351	2	2913	1	2026-01-04 18:09:41	migration	2026-09-27 10:58:39.135334	1	1
330268	1	30345	migration	2026-09-27 10:58:39.154658	migration	2026-09-27 10:58:39.154658	0	1
330269	1	30346	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	1
330270	1	30347	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	1
330271	1	30348	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	1
330272	1	30349	migration	2026-09-27 10:58:39.158178	migration	2026-09-27 10:58:39.158178	0	1
330273	1	30350	migration	2026-09-27 10:58:39.170703	migration	2026-09-27 10:58:39.170703	0	1
3087	111	1070	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3011	109	1070	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
2029	2	1070	1	2023-01-25 08:42:52	migration	2026-09-27 10:58:39.178077	1	1
3054	109	2497	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3035	109	2472	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3036	109	2478	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3042	109	2484	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3048	109	2490	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3037	109	2479	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3038	109	2480	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3039	109	2481	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3040	109	2482	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3041	109	2483	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3043	109	2485	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3044	109	2486	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3045	109	2487	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3046	109	2488	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3047	109	2489	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3049	109	2491	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3126	111	2492	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3050	109	2492	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3127	111	2493	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3051	109	2493	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3128	111	2494	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3052	109	2494	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
3129	111	2495	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	122
3053	109	2495	1	2023-12-02 23:41:02	migration	2026-09-27 10:58:39.178077	1	121
\.


--
-- Data for Name: system_sms_channel; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_sms_channel (id, signature, code, status, remark, api_key, api_secret, callback_url, creator, create_time, updater, update_time, deleted) FROM stdin;
2	Ballcat	ALIYUN	0	你要改哦，只有我可以用！！！！	enc:sm4:v2:FE8ercajBVifj3mqLWipFEwG/M98RA0UrKbcYJzCQIjAjX+04uno/Z3IFKvF3cFP	enc:sm4:v2:l84j7zWPF+756+aGEXMjzsVR1joEPz0o7EKppOD9XT/98Wch2DamzdY9n+jWlUp/	\N		2021-03-31 11:53:10	1	2026-07-16 07:29:32.31865	0
4	测试渠道	DEBUG_DING_TALK	0	123	enc:sm4:v2:kwqYsxwMfw7nILox9EHbllCigUa7/ZSEL65JZBmJ30I3MUHHwEMZp/pQ1m+IQ6o5	enc:sm4:v2:daOafQTreIJjHo/OJmlZuzbjEI9bi2C8b3pHrJf0LoQGsbymlk6p5akkDxkhmhoc	\N	1	2021-04-13 00:23:14	1	2026-07-16 07:29:32.31865	0
7	mock腾讯云	TENCENT	0	123	enc:sm4:v2:B+x5vhvQiPG6bcIoNXCdoUudOsBL9W4vhY7DkspZ5cJbmKaipEzcY0HRWb9i5T9x	enc:sm4:v2:gKzSJ+izf0xglm2KxGO6FXtCAXWskw+gkNo4v8G98Oc0QxPL94oONlqUBsfbNK7T		1	2024-09-30 08:53:45	1	2026-07-16 07:29:32.31865	0
\.


--
-- Data for Name: system_sms_code; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_sms_code (id, mobile, code, create_ip, scene, today_index, used, used_time, used_ip, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_sms_log; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_sms_log (id, channel_id, channel_code, template_id, template_code, template_type, template_content, template_params, api_template_id, mobile, user_id, user_type, send_status, send_time, api_send_code, api_send_msg, api_request_id, api_serial_no, receive_status, receive_time, api_receive_code, api_receive_msg, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: system_sms_template; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_sms_template (id, type, status, code, name, content, params, remark, api_template_id, channel_id, channel_code, creator, create_time, updater, update_time, deleted) FROM stdin;
2	1	0	test_01	测试验证码短信	正在进行登录操作{operation}，您的验证码是{code}	["operation","code"]	测试备注	4383920	4	DEBUG_DING_TALK		2021-03-31 10:49:38	1	2024-08-18 11:57:18	0
3	1	0	test_02	公告通知	您的验证码{code}，该验证码5分钟内有效，请勿泄漏于他人！	["code"]	\N	SMS_207945135	2	ALIYUN		2021-03-31 11:56:30	1	2021-04-10 01:22:02	0
6	3	0	test-01	测试模板	哈哈哈 {name}	["name"]	f哈哈哈	4383920	4	DEBUG_DING_TALK	1	2021-04-10 01:07:21	1	2024-08-18 11:57:07	0
7	3	0	test-04	测试下	老鸡{name}，牛逼{code}	["name","code"]	哈哈哈哈	suibian	7	DEBUG_DING_TALK	1	2021-04-13 00:29:53	1	2024-09-30 00:56:24	0
8	1	0	user-sms-login	前台用户短信登录	您的验证码是{code}	["code"]	\N	4372216	4	DEBUG_DING_TALK	1	2021-10-11 08:10:00	1	2024-08-18 11:57:06	0
9	2	0	bpm_task_assigned	【工作流】任务被分配	您收到了一条新的待办任务：{processInstanceName}-{taskName}，申请人：{startUserNickname}，处理链接：{detailUrl}	["processInstanceName","taskName","startUserNickname","detailUrl"]	\N	suibian	4	DEBUG_DING_TALK	1	2022-01-21 22:31:19	1	2022-01-22 00:03:36	0
10	2	0	bpm_process_instance_reject	【工作流】流程被不通过	您的流程被审批不通过：{processInstanceName}，原因：{reason}，查看链接：{detailUrl}	["processInstanceName","reason","detailUrl"]	\N	suibian	4	DEBUG_DING_TALK	1	2022-01-22 00:03:31	1	2022-05-01 12:33:14	0
11	2	0	bpm_process_instance_approve	【工作流】流程被通过	您的流程被审批通过：{processInstanceName}，查看链接：{detailUrl}	["processInstanceName","detailUrl"]	\N	suibian	4	DEBUG_DING_TALK	1	2022-01-22 00:04:31	1	2022-03-27 20:32:21	0
12	2	0	demo	演示模板	我就是测试一下下	[]	\N	biubiubiu	4	DEBUG_DING_TALK	1	2022-04-10 23:22:49	1	2024-08-18 11:57:04	0
14	1	0	user-update-mobile	会员用户 - 修改手机	您的验证码{code}，该验证码 5 分钟内有效，请勿泄漏于他人！	["code"]		null	4	DEBUG_DING_TALK	1	2023-08-19 18:58:01	1	2023-08-19 11:34:04	0
15	1	0	user-update-password	会员用户 - 修改密码	您的验证码{code}，该验证码 5 分钟内有效，请勿泄漏于他人！	["code"]		null	4	DEBUG_DING_TALK	1	2023-08-19 18:58:01	1	2023-08-19 11:34:18	0
16	1	0	user-reset-password	会员用户 - 重置密码	您的验证码{code}，该验证码 5 分钟内有效，请勿泄漏于他人！	["code"]		null	4	DEBUG_DING_TALK	1	2023-08-19 18:58:01	1	2023-12-02 22:35:27	0
17	2	0	bpm_task_timeout	【工作流】任务审批超时	您收到了一条超时的待办任务：{processInstanceName}-{taskName}，处理链接：{detailUrl}	["processInstanceName","taskName","detailUrl"]		X	4	DEBUG_DING_TALK	1	2024-08-16 21:59:15	1	2024-08-16 21:59:34	0
18	1	0	admin-reset-password	后台用户 - 忘记密码	您的验证码{code}，该验证码 5 分钟内有效，请勿泄漏于他人！	["code"]		null	4	DEBUG_DING_TALK	1	2025-03-16 14:19:34	1	2025-03-16 14:19:45	0
19	1	0	admin-sms-login	后台用户短信登录	您的验证码是{code}	["code"]		4372216	4	DEBUG_DING_TALK	1	2025-04-08 09:36:03	1	2025-04-08 09:36:17	0
\.


--
-- Data for Name: system_social_client; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_social_client (id, name, social_type, user_type, client_id, client_secret, agent_id, public_key, status, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
1	钉钉	20	2	dingvrnreaje3yqvzhxg	i8E6iZyDvZj51JIb0tYsYfVQYOks9Cq1lgryEjFRqC79P3iJcrxEwT6Qk2QvLrLI	\N	\N	0		2023-10-18 11:21:18	1	2023-12-20 21:28:26	1	1
2	钉钉（王土豆）	20	2	dingtsu9hpepjkbmthhw	FP_bnSq_HAHKCSncmJjw5hxhnzs6vaVDSZZn3egj6rdqTQ_hu5tQVJyLMpgCakdP	\N	\N	0		2023-10-18 11:21:18		2023-12-20 21:28:26	1	121
3	微信公众号	31	1	wx5b23ba7a5589ecbb	2a7b3b20c537e52e74afd395eb85f61f	\N	\N	0		2023-10-18 16:07:46	1	2023-12-20 21:28:23	1	1
43	微信小程序	34	1	wx63c280fe3248a3e7	6f270509224a7ae1296bbf1c8cb97aed	\N	\N	0		2023-10-19 13:37:41	1	2023-12-20 21:28:25	1	1
44	1	10	1	2	3	\N	\N	0	1	2025-04-06 20:36:28	1	2025-04-06 20:43:12	1	1
45	1	10	1	2	3	\N	\N	1	1	2025-09-06 20:26:15	1	2025-09-06 20:27:55	1	1
46	1	10	1	2	3	\N	\N	0	1	2025-11-29 16:04:23	1	2025-11-29 16:04:26	1	1
47	123	10	1	1	2	3	\N	0	1	2025-12-21 10:27:02	1	2025-12-21 10:27:20	1	1
\.


--
-- Data for Name: system_social_user; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_social_user (id, type, openid, token, raw_token_info, nickname, avatar, raw_user_info, code, state, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_social_user_bind; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_social_user_bind (id, user_id, user_type, social_type, social_user_id, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
\.


--
-- Data for Name: system_tenant; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_tenant (id, name, contact_user_id, contact_name, contact_mobile, status, websites, package_id, expire_time, account_count, creator, create_time, updater, update_time, deleted) FROM stdin;
1	RustSet	\N	管理员	17321315478	0		0	2099-02-19 17:14:16	9999	1	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.141962	0
121	小租户	110	小王2	15601691300	0		111	2026-07-10 00:00:00	30	1	2022-02-22 00:56:14	migration	2026-09-27 10:58:39.141962	0
122	测试租户	113	管理员	15601691300	0		111	2023-04-29 00:00:00	50	1	2022-03-07 21:37:58	migration	2026-09-27 10:58:39.141962	0
\.


--
-- Data for Name: system_tenant_package; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_tenant_package (id, name, status, remark, menu_ids, creator, create_time, updater, update_time, deleted) FROM stdin;
111	普通套餐	0	小功能	[1,2,5,1031,1032,1033,1034,1035,1036,1037,1038,1039,1050,1051,1052,1053,1054,1056,1057,1058,1059,1060,1063,1064,1065,1066,1067,1070,1075,1077,1078,1082,1083,1084,1085,1086,1087,1088,1089,1090,1091,1092,1117,1118,1119,1120,100,101,102,1126,103,1127,1128,1129,106,1130,107,1132,1133,110,1134,111,1135,112,1136,113,1137,2161,114,1138,1139,115,1140,116,1141,1142,1143,1150,1161,1162,1166,1173,1174,2713,2714,1178,2715,2716,2717,2718,2720,2721,1185,2722,1186,1187,2723,1188,2724,1189,2725,1190,2726,1191,2727,1192,2728,2729,1193,1194,2730,1195,2731,2732,1197,2733,1198,2734,1199,2735,1200,1201,1202,2739,2740,1207,1208,1209,2745,1210,2746,1211,2747,1212,2748,1213,1215,1216,1217,1218,1219,1220,2756,1221,2757,1222,1224,1225,1226,1227,1228,1229,1237,1238,2262,1239,1240,1241,1242,1243,2275,2276,2277,1255,1256,1257,2281,1258,2282,1259,2283,1260,2284,2285,2287,2288,2293,2294,2297,2300,2301,2302,2317,2318,2319,2320,2321,2322,2323,2324,2325,2326,2327,2328,2329,2330,2331,2332,2333,2334,2335,2363,2364,5011,5012,2472,2478,2479,2480,2481,2482,2483,2484,2485,2486,2487,2488,2489,2490,2491,2492,2493,2494,2495,2497,2525,1001,1002,1003,1004,1005,1006,1007,1008,1009,1010,1011,1012,1013,2549,1014,2550,1015,2551,1016,2552,1017,2553,1018,2554,1019,2555,1020,2556,2557,2558,2559]	1	2022-02-22 00:54:00	1	2025-09-06 20:52:25	0
\.


--
-- Data for Name: system_user_post; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_user_post (id, user_id, post_id, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
112	1	1	admin	2022-05-02 07:25:24	admin	2022-05-02 07:25:24	0	1
113	100	1	admin	2022-05-02 07:25:24	admin	2022-05-02 07:25:24	0	1
115	104	1	1	2022-05-16 19:36:28	1	2022-05-16 19:36:28	0	1
116	117	2	1	2022-07-09 17:40:26	1	2022-07-09 17:40:26	0	1
117	118	1	1	2022-07-09 17:44:44	1	2022-07-09 17:44:44	0	1
119	114	5	1	2024-03-24 20:45:51	1	2024-03-24 20:45:51	0	1
123	115	1	1	2024-04-04 09:37:14	1	2024-04-04 09:37:14	0	1
124	115	2	1	2024-04-04 09:37:14	1	2024-04-04 09:37:14	0	1
125	1	2	1	2024-07-13 22:31:39	1	2024-07-13 22:31:39	0	1
128	139	2	1	2025-12-05 21:43:27	1	2025-12-05 21:43:27	0	1
129	139	4	1	2025-12-05 21:43:27	1	2025-12-05 21:43:27	0	1
130	104	2		2026-07-16 06:02:21.121236		2026-07-16 06:02:21.121236	0	1
\.


--
-- Data for Name: system_user_role; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_user_role (id, user_id, role_id, creator, create_time, updater, update_time, deleted, tenant_id) FROM stdin;
1	1	1		2022-01-11 13:19:45		2022-05-12 12:35:17	0	1
2	2	2		2022-01-11 13:19:45		2022-05-12 12:35:13	0	1
5	100	1		2022-01-11 13:19:45		2022-05-12 12:35:12	0	1
6	100	2		2022-01-11 13:19:45		2022-05-12 12:35:11	0	1
10	103	1	1	2022-01-11 13:19:45	1	2022-01-11 13:19:45	0	1
14	110	109	1	2022-02-22 00:56:14	1	2022-02-22 00:56:14	0	121
15	111	110	110	2022-02-23 13:14:38	110	2022-02-23 13:14:38	0	121
16	113	111	1	2022-03-07 21:37:58	1	2022-03-07 21:37:58	0	122
18	1	2	1	2022-05-12 20:39:29	1	2022-05-12 20:39:29	0	1
22	115	2	1	2022-07-21 22:08:30	1	2022-07-21 22:08:30	0	1
35	112	1	1	2024-03-15 20:00:24	1	2024-03-15 20:00:24	0	1
36	118	1	1	2024-03-17 09:12:08	1	2024-03-17 09:12:08	0	1
46	117	1	1	2024-10-02 10:16:11	1	2024-10-02 10:16:11	0	1
47	104	2	1	2025-01-04 10:40:33	1	2025-01-04 10:40:33	0	1
48	100	155	1	2025-04-04 10:41:14	1	2025-04-04 10:41:14	0	1
49	142	1	1	2025-07-23 09:11:42	1	2025-07-23 09:11:42	0	1
50	142	2	1	2025-10-07 20:50:37	1	2025-10-07 20:50:37	0	1
51	139	1	1	2025-12-05 22:36:57	1	2025-12-05 22:36:57	0	1
52	139	2	1	2025-12-05 22:37:00	1	2025-12-05 22:37:00	0	1
53	114	2	1	2026-01-04 18:15:40	1	2026-01-04 18:15:40	0	1
54	114	3	1	2026-01-04 18:16:19	1	2026-01-04 18:16:19	0	1
\.


--
-- Data for Name: system_users; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_users (id, username, password, nickname, remark, dept_id, post_ids, email, mobile, sex, avatar, status, login_ip, login_date, creator, create_time, updater, update_time, deleted, tenant_id, identity_uuid) FROM stdin;
1	admin	$sm3$16384$7a7092f2a3f9a71facbcb3a1666a9110$f5e509bec314404aa6f771a1378b9e6645f386b88c692675267f89789ebc5010	RustSet 管理员	管理员	103	[1,2]	13aoteman@126.com	18818260272	1		0		\N	admin	2021-01-05 17:03:47	migration	2026-09-27 10:58:39.145854	0	1	4003435d-fee2-433d-8288-8bd84101bf16
104	test	$2a$04$BrwaYn303hjA/6TnXqdGoOLhyHOAA0bVrAFu6.1dJKycqKUnIoRz2	测试号	\N	107	[1,2]	111@qq.com	15601691200	1	\N	0		\N		2021-01-21 02:13:53	\N	2026-01-04 18:09:54	0	1	2d10acdc-a623-4526-822f-295b76331f61
110	admin110	$2a$10$mRMIYLDtRHlf6.9ipiqH1.Z.bh/R9dO9d5iHiGYPigi6r5KOoR2Wm	小王	\N	\N	\N		15601691300	0	\N	0		\N	1	2022-02-22 00:56:14	\N	2026-07-16 06:25:39.039366	0	121	9c330aef-242c-4349-8f10-e9f8a4f27a4d
111	test	$2a$10$mRMIYLDtRHlf6.9ipiqH1.Z.bh/R9dO9d5iHiGYPigi6r5KOoR2Wm	测试用户	\N	\N	[]			0	\N	0		\N	110	2022-02-23 13:14:33	\N	2026-07-16 06:12:57.86539	0	121	d81ad3bf-9a1f-4420-97f1-11e17779468c
112	newobject	$2a$04$dB0z8Q819fJWz0hbaLe6B.VfHCjYgWx6LFfET5lyz3JwcqlyCkQ4C	新对象	\N	100	[]		15601691235	1	\N	0		\N	1	2022-02-23 19:08:03	\N	2025-04-21 14:23:08	0	1	56c8c8f0-832e-4bcf-a4de-cb6d3f9b317f
113	aoteman	$2a$10$0acJOIk2D25/oC87nyclE..0lzeu9DtQ/n3geP4fkun/zIVRhHJIO	RustSet 用户	\N	\N	\N		15601691300	0	\N	0		\N	1	2022-03-07 21:37:58	migration	2026-09-27 10:58:39.141962	0	122	f762824c-863d-4d6a-9652-d67d07dff70d
114	hrmgr	$2a$10$TR4eybBioGRhBmDBWkqWLO6NIh3mzYa8KBKDDB5woiGYFVlRAi.fu	hr 小姐姐	\N	\N	[5]		15601691236	1	\N	0		\N	1	2022-03-19 21:50:58	\N	2026-01-04 18:16:01	0	1	d1bdf970-446b-4dc2-abf4-2f4c969d2298
115	aotemane	$2a$04$GcyP0Vyzb2F2Yni5PuIK9ueGxM0tkZGMtDwVRwrNbtMvorzbpNsV2	阿呆	11222	102	[1,2]	7648@qq.com	15601691229	2	\N	0		\N	1	2022-04-30 02:55:43	1	2025-04-21 14:23:08	0	1	23c9ef7b-1947-4f3a-a06e-71488de6b507
117	admin123	$2a$04$sEtimsHu9YCkYY4/oqElHem2Ijc9ld20eYO6lN.g/21NfLUTDLB9W	测试号02	1111	100	[2]		15601691234	1	\N	0		\N	1	2022-07-09 17:40:26	1	2025-05-14 09:56:04	0	1	783c8126-ed7b-4f47-b01c-74bfc7358601
118	goudan	$2a$04$3suGZjnA6rM5bErf38u1felbgqbsPHGdRG3l9NkxPCEt2ah9Y6aJi	狗蛋	\N	103	[1]		15601691239	1	\N	0		\N	1	2022-07-09 17:44:43	\N	2025-11-23 15:28:25	0	1	1337338d-a3ec-4b36-9489-61bc4deb8c8a
107	admin107	$2a$10$dYOOBKMO93v/.ReCqzyFg.o67Tqk.bbc2bhrpyBGkIw9aypCtr2pm	RustSet 用户	\N	\N	\N		15601691300	0	\N	0		\N	1	2022-02-20 22:59:33	migration	2026-09-27 10:58:39.145854	0	118	dfdc2f63-0643-4bf7-996e-d66b197ade02
108	admin108	$2a$10$y6mfvKoNYL1GXWak8nYwVOH.kCWqjactkzdoIDgiKl93WN3Ejg.Lu	RustSet 用户	\N	\N	\N		15601691300	0	\N	0		\N	1	2022-02-20 23:00:50	migration	2026-09-27 10:58:39.145854	0	119	34001a93-ad57-4df9-b797-86810bc7c88d
139	wwbwwb	$2a$04$FJLIyg8lbPytP29pbZaiU.LesJvCsYfEaHqQfB0pGQhK3e9BeZmLy	小秃头	123	108	[2,4]			1	\N	0		\N	\N	2024-09-10 21:03:58	1	2025-12-15 22:38:15	0	1	51591a2c-4497-46d5-9158-f9d3a61220d6
141	admin1	$2a$04$oj6F6d7HrZ70kYVD3TNzEu.m3TPUzajOVuC66zdKna8KRerK1FmVa	新用户	\N	\N	\N			0		0		\N	1	2025-04-08 13:09:07	1	2025-05-14 19:11:48	0	1	445f0226-93a6-44ab-9553-61304601f847
142	test01	$2a$04$4bCYWZkjxxOC4QE0LY2M9uEEKWeJbLfs489NFtQoyidL5I0FndRaO	test01		\N	[]		19021719925	1		0		\N	1	2025-07-09 21:07:10	\N	2025-12-02 13:23:11	0	1	a11d3ba6-9ddc-4914-816e-1d1b0f63423e
143	a00001	$2a$04$GhVHFviOw/SsTmiQtifHJesDYFlHMeGK7OWh7aGCCjGGVCmbHVAwa	a00001	\N	104	\N			0		0		\N	\N	2025-12-01 16:10:13	1	2025-12-05 21:34:05	0	1	4f0cadfd-bd5e-4ed2-aa93-92eff0caff71
144	aoteman001	$2a$04$omQOmhz8OyUFBKw77nr8KOtMp6xdvoQ1gWStjk9r8.OYT3Bv6oEYe	aoteman001	\N	116	\N			0		1		\N	1	2025-12-01 17:05:27	1	2025-12-15 15:55:54	0	1	a4edfe32-96f1-4a81-84d3-59553729bb02
109	admin109	$2a$10$JAqvH0tEc0I7dfDVBI7zyuB4E3j.uH6daIjV53.vUS6PknFkDJkuK	RustSet 用户	\N	\N	\N		15601691300	0	\N	0		\N	1	2022-02-20 23:11:50	migration	2026-09-27 10:58:39.145854	0	120	3e44df26-102a-4bb6-8b4c-1c06acee548b
100	rustset-demo	$2a$04$h.aaPKgO.odHepnk5PCsWeEwKdojFWdTItxGKfx1r0e1CSeBzsTJ6	RustSet 用户	\N	104	[1]		15601691300	1	\N	0		\N		2021-01-07 09:07:17	migration	2026-09-27 10:58:39.145854	0	1	0ba69165-1297-4983-a7e3-bbc36cf3b3a5
103	yuanma	$2a$04$fUBSmjKCPYAUmnMzOb6qE.eZCGPhHi1JmAKclODbfS/O7fHOl2bH6	示例用户	\N	106	\N		15601701300	0	\N	0		\N		2021-01-13 23:50:35	migration	2026-09-27 10:58:39.145854	0	1	b20b44a9-e18e-4a7b-824d-e50b779fa649
\.


--
-- Data for Name: yudao_demo01_contact; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.yudao_demo01_contact (id, name, sex, birthday, description, avatar, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: yudao_demo02_category; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.yudao_demo02_category (id, name, parent_id, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: yudao_demo03_course; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.yudao_demo03_course (id, student_id, name, score, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: yudao_demo03_grade; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.yudao_demo03_grade (id, student_id, name, teacher, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Data for Name: yudao_demo03_student; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.yudao_demo03_student (id, name, sex, birthday, description, creator, create_time, updater, update_time, deleted) FROM stdin;
\.


--
-- Name: cmdb_attribute_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.cmdb_attribute_seq', 1, false);


--
-- Name: cmdb_instance_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.cmdb_instance_seq', 1, false);


--
-- Name: cmdb_model_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.cmdb_model_seq', 1, false);


--
-- Name: cmdb_net_zone_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.cmdb_net_zone_seq', 1, false);


--
-- Name: cmdb_relation_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.cmdb_relation_seq', 1, false);


--
-- Name: infra_api_access_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_api_access_log_seq', 44792, true);


--
-- Name: infra_api_error_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_api_error_log_seq', 1, false);


--
-- Name: infra_application_endpoint_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_application_endpoint_seq', 1, false);


--
-- Name: infra_approval_rule_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_approval_rule_seq', 1, false);


--
-- Name: infra_asset_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_asset_seq', 1, false);


--
-- Name: infra_business_application_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_business_application_seq', 1, false);


--
-- Name: infra_cloud_asset_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_cloud_asset_seq', 1, false);


--
-- Name: infra_cloud_platform_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_cloud_platform_seq', 1, false);


--
-- Name: infra_cloud_provider_config_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_cloud_provider_config_seq', 1, false);


--
-- Name: infra_cloud_resource_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_cloud_resource_seq', 1, true);


--
-- Name: infra_cloud_zone_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_cloud_zone_seq', 1, false);


--
-- Name: infra_codegen_column_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_codegen_column_seq', 26, true);


--
-- Name: infra_codegen_table_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_codegen_table_seq', 1, true);


--
-- Name: infra_config_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_config_seq', 2, true);


--
-- Name: infra_data_source_config_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_data_source_config_seq', 2, true);


--
-- Name: infra_file_config_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_file_config_seq', 3, true);


--
-- Name: infra_file_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_file_seq', 3, true);


--
-- Name: infra_high_risk_port_rule_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_high_risk_port_rule_seq', 36, true);


--
-- Name: infra_job_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_job_log_seq', 2, true);


--
-- Name: infra_job_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_job_seq', 3, true);


--
-- Name: infra_machine_room_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_machine_room_seq', 1, false);


--
-- Name: infra_network_policy_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_network_policy_seq', 1, false);


--
-- Name: infra_network_zone_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_network_zone_seq', 1, false);


--
-- Name: infra_physical_resource_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_physical_resource_seq', 1, true);


--
-- Name: infra_resource_ticket_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_resource_ticket_seq', 1, false);


--
-- Name: infra_risk_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_risk_seq', 1, false);


--
-- Name: infra_security_product_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_security_product_seq', 1, false);


--
-- Name: infra_service_provider_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_service_provider_seq', 1, false);


--
-- Name: infra_task_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.infra_task_seq', 1, false);


--
-- Name: system_dept_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_dept_seq', 118, true);


--
-- Name: system_dict_data_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_dict_data_seq', 3449, true);


--
-- Name: system_dict_type_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_dict_type_seq', 2139, true);


--
-- Name: system_login_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_login_log_seq', 167, true);


--
-- Name: system_mail_account_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_mail_account_seq', 5, true);


--
-- Name: system_mail_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_mail_log_seq', 3, true);


--
-- Name: system_mail_template_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_mail_template_seq', 16, true);


--
-- Name: system_menu_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_menu_seq', 30350, true);


--
-- Name: system_notice_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_notice_seq', 5, true);


--
-- Name: system_notify_message_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_notify_message_seq', 12, true);


--
-- Name: system_notify_template_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_notify_template_seq', 2, true);


--
-- Name: system_oauth2_access_token_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_oauth2_access_token_seq', 324, true);


--
-- Name: system_oauth2_approve_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_oauth2_approve_seq', 1, true);


--
-- Name: system_oauth2_client_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_oauth2_client_seq', 43, true);


--
-- Name: system_oauth2_code_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_oauth2_code_seq', 1, true);


--
-- Name: system_oauth2_refresh_token_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_oauth2_refresh_token_seq', 308, true);


--
-- Name: system_operate_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_operate_log_seq', 162, true);


--
-- Name: system_post_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_post_seq', 8, true);


--
-- Name: system_role_menu_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_role_menu_seq', 330273, true);


--
-- Name: system_role_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_role_seq', 158, true);


--
-- Name: system_sms_channel_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_sms_channel_seq', 8, true);


--
-- Name: system_sms_code_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_sms_code_seq', 1, true);


--
-- Name: system_sms_log_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_sms_log_seq', 3, true);


--
-- Name: system_sms_template_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_sms_template_seq', 20, true);


--
-- Name: system_social_client_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_social_client_seq', 48, true);


--
-- Name: system_social_user_bind_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_social_user_bind_seq', 1, true);


--
-- Name: system_social_user_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_social_user_seq', 1, true);


--
-- Name: system_tenant_package_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_tenant_package_seq', 112, true);


--
-- Name: system_tenant_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_tenant_seq', 123, true);


--
-- Name: system_user_post_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_user_post_seq', 132, true);


--
-- Name: system_user_role_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_user_role_seq', 57, true);


--
-- Name: system_users_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.system_users_seq', 146, true);


--
-- Name: yudao_demo01_contact_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.yudao_demo01_contact_seq', 1, false);


--
-- Name: yudao_demo02_category_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.yudao_demo02_category_seq', 1, false);


--
-- Name: yudao_demo03_course_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.yudao_demo03_course_seq', 1, false);


--
-- Name: yudao_demo03_grade_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.yudao_demo03_grade_seq', 1, false);


--
-- Name: yudao_demo03_student_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.yudao_demo03_student_seq', 1, false);


--
-- Name: chat_conversations chat_conversations_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_conversations
    ADD CONSTRAINT chat_conversations_pkey PRIMARY KEY (id);


--
-- Name: chat_messages chat_messages_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_messages
    ADD CONSTRAINT chat_messages_pkey PRIMARY KEY (id);


--
-- Name: chat_roles chat_roles_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_roles
    ADD CONSTRAINT chat_roles_pkey PRIMARY KEY (id);


--
-- Name: images images_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.images
    ADD CONSTRAINT images_pkey PRIMARY KEY (id);


--
-- Name: knowledge_bases knowledge_bases_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_bases
    ADD CONSTRAINT knowledge_bases_pkey PRIMARY KEY (id);


--
-- Name: knowledge_documents knowledge_documents_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_documents
    ADD CONSTRAINT knowledge_documents_pkey PRIMARY KEY (id);


--
-- Name: knowledge_segments knowledge_segments_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_segments
    ADD CONSTRAINT knowledge_segments_pkey PRIMARY KEY (id);


--
-- Name: model_catalog model_catalog_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.model_catalog
    ADD CONSTRAINT model_catalog_pkey PRIMARY KEY (platform, model);


--
-- Name: model_configs model_configs_key_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.model_configs
    ADD CONSTRAINT model_configs_key_key UNIQUE (key);


--
-- Name: model_configs model_configs_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.model_configs
    ADD CONSTRAINT model_configs_pkey PRIMARY KEY (id);


--
-- Name: model_platforms model_platforms_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.model_platforms
    ADD CONSTRAINT model_platforms_pkey PRIMARY KEY (platform);


--
-- Name: music music_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.music
    ADD CONSTRAINT music_pkey PRIMARY KEY (id);


--
-- Name: tools tools_name_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.tools
    ADD CONSTRAINT tools_name_key UNIQUE (name);


--
-- Name: tools tools_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.tools
    ADD CONSTRAINT tools_pkey PRIMARY KEY (id);


--
-- Name: writes writes_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.writes
    ADD CONSTRAINT writes_pkey PRIMARY KEY (id);


--
-- Name: _sqlx_migrations _sqlx_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public._sqlx_migrations
    ADD CONSTRAINT _sqlx_migrations_pkey PRIMARY KEY (version);


--
-- Name: cmdb_attribute cmdb_attribute_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_attribute
    ADD CONSTRAINT cmdb_attribute_pkey PRIMARY KEY (id);


--
-- Name: cmdb_instance cmdb_instance_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance
    ADD CONSTRAINT cmdb_instance_pkey PRIMARY KEY (id);


--
-- Name: cmdb_instance_unique_value cmdb_instance_unique_value_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance_unique_value
    ADD CONSTRAINT cmdb_instance_unique_value_key UNIQUE NULLS NOT DISTINCT (tenant_id, model_id, value);


--
-- Name: cmdb_instance_unique_value cmdb_instance_unique_value_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance_unique_value
    ADD CONSTRAINT cmdb_instance_unique_value_pkey PRIMARY KEY (instance_id);


--
-- Name: cmdb_model cmdb_model_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_model
    ADD CONSTRAINT cmdb_model_pkey PRIMARY KEY (id);


--
-- Name: cmdb_net_zone cmdb_net_zone_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_net_zone
    ADD CONSTRAINT cmdb_net_zone_pkey PRIMARY KEY (id);


--
-- Name: cmdb_relation cmdb_relation_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_relation
    ADD CONSTRAINT cmdb_relation_pkey PRIMARY KEY (id);


--
-- Name: infra_api_access_log infra_api_access_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_api_access_log
    ADD CONSTRAINT infra_api_access_log_pkey PRIMARY KEY (id);


--
-- Name: infra_api_error_log infra_api_error_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_api_error_log
    ADD CONSTRAINT infra_api_error_log_pkey PRIMARY KEY (id);


--
-- Name: infra_approval_rule infra_approval_rule_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_approval_rule
    ADD CONSTRAINT infra_approval_rule_pkey PRIMARY KEY (id);


--
-- Name: infra_codegen_column infra_codegen_column_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_codegen_column
    ADD CONSTRAINT infra_codegen_column_pkey PRIMARY KEY (id);


--
-- Name: infra_codegen_table infra_codegen_table_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_codegen_table
    ADD CONSTRAINT infra_codegen_table_pkey PRIMARY KEY (id);


--
-- Name: infra_config infra_config_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_config
    ADD CONSTRAINT infra_config_pkey PRIMARY KEY (id);


--
-- Name: infra_data_source_config infra_data_source_config_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_data_source_config
    ADD CONSTRAINT infra_data_source_config_pkey PRIMARY KEY (id);


--
-- Name: infra_file_config infra_file_config_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_file_config
    ADD CONSTRAINT infra_file_config_pkey PRIMARY KEY (id);


--
-- Name: infra_file infra_file_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_file
    ADD CONSTRAINT infra_file_pkey PRIMARY KEY (id);


--
-- Name: infra_high_risk_port_rule infra_high_risk_port_rule_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_high_risk_port_rule
    ADD CONSTRAINT infra_high_risk_port_rule_pkey PRIMARY KEY (id);


--
-- Name: infra_inspection_result infra_inspection_result_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_inspection_result
    ADD CONSTRAINT infra_inspection_result_pkey PRIMARY KEY (id);


--
-- Name: infra_inspection_result infra_inspection_result_task_id_ip_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_inspection_result
    ADD CONSTRAINT infra_inspection_result_task_id_ip_key UNIQUE (task_id, ip);


--
-- Name: infra_job_log infra_job_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_job_log
    ADD CONSTRAINT infra_job_log_pkey PRIMARY KEY (id);


--
-- Name: infra_job infra_job_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_job
    ADD CONSTRAINT infra_job_pkey PRIMARY KEY (id);


--
-- Name: infra_network_policy infra_network_policy_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_network_policy
    ADD CONSTRAINT infra_network_policy_pkey PRIMARY KEY (id);


--
-- Name: system_dept pk_system_dept; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_dept
    ADD CONSTRAINT pk_system_dept PRIMARY KEY (id);


--
-- Name: system_dict_data pk_system_dict_data; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_dict_data
    ADD CONSTRAINT pk_system_dict_data PRIMARY KEY (id);


--
-- Name: system_dict_type pk_system_dict_type; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_dict_type
    ADD CONSTRAINT pk_system_dict_type PRIMARY KEY (id);


--
-- Name: system_login_log pk_system_login_log; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_login_log
    ADD CONSTRAINT pk_system_login_log PRIMARY KEY (id);


--
-- Name: system_mail_account pk_system_mail_account; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_mail_account
    ADD CONSTRAINT pk_system_mail_account PRIMARY KEY (id);


--
-- Name: system_mail_log pk_system_mail_log; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_mail_log
    ADD CONSTRAINT pk_system_mail_log PRIMARY KEY (id);


--
-- Name: system_mail_template pk_system_mail_template; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_mail_template
    ADD CONSTRAINT pk_system_mail_template PRIMARY KEY (id);


--
-- Name: system_menu pk_system_menu; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_menu
    ADD CONSTRAINT pk_system_menu PRIMARY KEY (id);


--
-- Name: system_notice pk_system_notice; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_notice
    ADD CONSTRAINT pk_system_notice PRIMARY KEY (id);


--
-- Name: system_notify_message pk_system_notify_message; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_notify_message
    ADD CONSTRAINT pk_system_notify_message PRIMARY KEY (id);


--
-- Name: system_notify_template pk_system_notify_template; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_notify_template
    ADD CONSTRAINT pk_system_notify_template PRIMARY KEY (id);


--
-- Name: system_oauth2_access_token pk_system_oauth2_access_token; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_oauth2_access_token
    ADD CONSTRAINT pk_system_oauth2_access_token PRIMARY KEY (id);


--
-- Name: system_oauth2_approve pk_system_oauth2_approve; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_oauth2_approve
    ADD CONSTRAINT pk_system_oauth2_approve PRIMARY KEY (id);


--
-- Name: system_oauth2_client pk_system_oauth2_client; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_oauth2_client
    ADD CONSTRAINT pk_system_oauth2_client PRIMARY KEY (id);


--
-- Name: system_oauth2_code pk_system_oauth2_code; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_oauth2_code
    ADD CONSTRAINT pk_system_oauth2_code PRIMARY KEY (id);


--
-- Name: system_oauth2_refresh_token pk_system_oauth2_refresh_token; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_oauth2_refresh_token
    ADD CONSTRAINT pk_system_oauth2_refresh_token PRIMARY KEY (id);


--
-- Name: system_operate_log pk_system_operate_log; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_operate_log
    ADD CONSTRAINT pk_system_operate_log PRIMARY KEY (id);


--
-- Name: system_post pk_system_post; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_post
    ADD CONSTRAINT pk_system_post PRIMARY KEY (id);


--
-- Name: system_role pk_system_role; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_role
    ADD CONSTRAINT pk_system_role PRIMARY KEY (id);


--
-- Name: system_role_menu pk_system_role_menu; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_role_menu
    ADD CONSTRAINT pk_system_role_menu PRIMARY KEY (id);


--
-- Name: system_sms_channel pk_system_sms_channel; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_sms_channel
    ADD CONSTRAINT pk_system_sms_channel PRIMARY KEY (id);


--
-- Name: system_sms_code pk_system_sms_code; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_sms_code
    ADD CONSTRAINT pk_system_sms_code PRIMARY KEY (id);


--
-- Name: system_sms_log pk_system_sms_log; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_sms_log
    ADD CONSTRAINT pk_system_sms_log PRIMARY KEY (id);


--
-- Name: system_sms_template pk_system_sms_template; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_sms_template
    ADD CONSTRAINT pk_system_sms_template PRIMARY KEY (id);


--
-- Name: system_social_client pk_system_social_client; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_social_client
    ADD CONSTRAINT pk_system_social_client PRIMARY KEY (id);


--
-- Name: system_social_user pk_system_social_user; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_social_user
    ADD CONSTRAINT pk_system_social_user PRIMARY KEY (id);


--
-- Name: system_social_user_bind pk_system_social_user_bind; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_social_user_bind
    ADD CONSTRAINT pk_system_social_user_bind PRIMARY KEY (id);


--
-- Name: system_tenant pk_system_tenant; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_tenant
    ADD CONSTRAINT pk_system_tenant PRIMARY KEY (id);


--
-- Name: system_tenant_package pk_system_tenant_package; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_tenant_package
    ADD CONSTRAINT pk_system_tenant_package PRIMARY KEY (id);


--
-- Name: system_user_post pk_system_user_post; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_user_post
    ADD CONSTRAINT pk_system_user_post PRIMARY KEY (id);


--
-- Name: system_user_role pk_system_user_role; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_user_role
    ADD CONSTRAINT pk_system_user_role PRIMARY KEY (id);


--
-- Name: system_users pk_system_users; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_users
    ADD CONSTRAINT pk_system_users PRIMARY KEY (id);


--
-- Name: yudao_demo01_contact yudao_demo01_contact_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.yudao_demo01_contact
    ADD CONSTRAINT yudao_demo01_contact_pkey PRIMARY KEY (id);


--
-- Name: yudao_demo02_category yudao_demo02_category_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.yudao_demo02_category
    ADD CONSTRAINT yudao_demo02_category_pkey PRIMARY KEY (id);


--
-- Name: yudao_demo03_course yudao_demo03_course_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.yudao_demo03_course
    ADD CONSTRAINT yudao_demo03_course_pkey PRIMARY KEY (id);


--
-- Name: yudao_demo03_grade yudao_demo03_grade_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.yudao_demo03_grade
    ADD CONSTRAINT yudao_demo03_grade_pkey PRIMARY KEY (id);


--
-- Name: yudao_demo03_student yudao_demo03_student_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.yudao_demo03_student
    ADD CONSTRAINT yudao_demo03_student_pkey PRIMARY KEY (id);


--
-- Name: idx_ai_chat_conversation_knowledge_ids; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_chat_conversation_knowledge_ids ON ai.chat_conversations USING gin (knowledge_ids);


--
-- Name: idx_ai_chat_conversation_user; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_chat_conversation_user ON ai.chat_conversations USING btree (user_id, pinned DESC, update_time DESC);


--
-- Name: idx_ai_chat_message_conversation; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_chat_message_conversation ON ai.chat_messages USING btree (conversation_id, id);


--
-- Name: idx_ai_chat_roles_public; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_chat_roles_public ON ai.chat_roles USING btree (public_status, status, sort, id DESC);


--
-- Name: idx_ai_chat_roles_user; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_chat_roles_user ON ai.chat_roles USING btree (user_id, id DESC);


--
-- Name: idx_ai_images_pending_task; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_images_pending_task ON ai.images USING btree (COALESCE(last_poll_time, (0)::bigint), id) WHERE ((status = 10) AND (task_id IS NOT NULL));


--
-- Name: idx_ai_images_user; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_images_user ON ai.images USING btree (user_id, id DESC);


--
-- Name: idx_ai_knowledge_segment_base; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_knowledge_segment_base ON ai.knowledge_segments USING btree (knowledge_id, status);


--
-- Name: idx_ai_model_catalog_type; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_model_catalog_type ON ai.model_catalog USING btree (platform, type, active);


--
-- Name: idx_ai_model_platform_type; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_model_platform_type ON ai.model_configs USING btree (platform, type, status);


--
-- Name: idx_ai_music_pending; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_music_pending ON ai.music USING btree (status, last_poll_time) WHERE (status = 10);


--
-- Name: idx_ai_music_user_status; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_music_user_status ON ai.music USING btree (user_id, status, id DESC);


--
-- Name: idx_ai_tools_status_name; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_tools_status_name ON ai.tools USING btree (status, name);


--
-- Name: idx_ai_writes_user_time; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_writes_user_time ON ai.writes USING btree (user_id, id DESC);


--
-- Name: cmdb_instance_tenant_id_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX cmdb_instance_tenant_id_key ON public.cmdb_instance USING btree (tenant_id, id);


--
-- Name: cmdb_net_zone_tenant_id_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX cmdb_net_zone_tenant_id_key ON public.cmdb_net_zone USING btree (tenant_id, id);


--
-- Name: idx_app_endpoint_business; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_app_endpoint_business ON public.infra_application_endpoint USING btree (business_application_id) WHERE (deleted = 0);


--
-- Name: idx_asset_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_asset_ip ON public.infra_asset USING btree (ip) WHERE (deleted = 0);


--
-- Name: idx_asset_tenant_ip_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_asset_tenant_ip_unique ON public.infra_asset USING btree (tenant_id, ip) WHERE ((deleted = 0) AND (tenant_id IS NOT NULL));


--
-- Name: idx_business_app_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_business_app_name ON public.infra_business_application USING btree (name) WHERE (deleted = 0);


--
-- Name: idx_cloud_asset_instance; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_asset_instance ON public.infra_cloud_asset USING btree (instance_id) WHERE (deleted = 0);


--
-- Name: idx_cloud_asset_provider; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_asset_provider ON public.infra_cloud_asset USING btree (cloud_provider_config_id) WHERE (deleted = 0);


--
-- Name: idx_cloud_platform_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_platform_zone ON public.infra_cloud_platform USING btree (zone_id) WHERE (deleted = 0);


--
-- Name: idx_cloud_provider_config_platform; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_provider_config_platform ON public.infra_cloud_provider_config USING btree (platform_id) WHERE (deleted = 0);


--
-- Name: idx_cloud_provider_config_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_provider_config_zone ON public.infra_cloud_provider_config USING btree (zone_id) WHERE (deleted = 0);


--
-- Name: idx_cloud_resource_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_resource_customer ON public.infra_cloud_resource USING btree (customer_name) WHERE (deleted = 0);


--
-- Name: idx_cloud_resource_region; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_resource_region ON public.infra_cloud_resource USING btree (cloud_region) WHERE (deleted = 0);


--
-- Name: idx_cloud_zone_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cloud_zone_code ON public.infra_cloud_zone USING btree (zone_code) WHERE (deleted = 0);


--
-- Name: idx_cmdb_attribute_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_attribute_model ON public.cmdb_attribute USING btree (model_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_instance_attributes; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_instance_attributes ON public.cmdb_instance USING gin (attributes jsonb_path_ops);


--
-- Name: idx_cmdb_instance_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_instance_model ON public.cmdb_instance USING btree (model_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_instance_tenant_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_instance_tenant_model ON public.cmdb_instance USING btree (tenant_id, model_id, id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_model_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_model_code ON public.cmdb_model USING btree (code) WHERE (deleted = 0);


--
-- Name: idx_cmdb_net_zone_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_net_zone_parent ON public.cmdb_net_zone USING btree (parent_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_net_zone_tenant_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_net_zone_tenant_parent ON public.cmdb_net_zone USING btree (tenant_id, parent_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_relation_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_relation_source ON public.cmdb_relation USING btree (source_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_relation_target; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_relation_target ON public.cmdb_relation USING btree (target_id) WHERE (deleted = 0);


--
-- Name: idx_cmdb_relation_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cmdb_relation_tenant ON public.cmdb_relation USING btree (tenant_id) WHERE (deleted = 0);


--
-- Name: idx_high_risk_port_rule_identity; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_high_risk_port_rule_identity ON public.infra_high_risk_port_rule USING btree (protocol, port_start, port_end) WHERE (deleted = 0);


--
-- Name: idx_infra_api_access_log_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_api_access_log_time ON public.infra_api_access_log USING btree (create_time DESC) WHERE (deleted = 0);


--
-- Name: idx_infra_api_error_log_status_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_api_error_log_status_time ON public.infra_api_error_log USING btree (process_status, create_time DESC) WHERE (deleted = 0);


--
-- Name: idx_infra_asset_inventory_application; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_asset_inventory_application ON public.infra_asset USING btree (application_name, server_name) WHERE (deleted = 0);


--
-- Name: idx_infra_asset_inventory_org; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_asset_inventory_org ON public.infra_asset USING btree (organization_name, business_department) WHERE (deleted = 0);


--
-- Name: idx_infra_asset_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_asset_tenant ON public.infra_asset USING btree (tenant_id);


--
-- Name: idx_infra_codegen_column_table; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_codegen_column_table ON public.infra_codegen_column USING btree (table_id, ordinal_position, id) WHERE (deleted = 0);


--
-- Name: idx_infra_inspection_baseline_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_inspection_baseline_tenant ON public.infra_inspection_baseline USING btree (tenant_id);


--
-- Name: idx_infra_inspection_result_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_inspection_result_tenant ON public.infra_inspection_result USING btree (tenant_id);


--
-- Name: idx_infra_job_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_job_active ON public.infra_job USING btree (status, id) WHERE (deleted = 0);


--
-- Name: idx_infra_job_log_job_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_job_log_job_time ON public.infra_job_log USING btree (job_id, create_time DESC) WHERE (deleted = 0);


--
-- Name: idx_infra_network_policy_application_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_network_policy_application_date ON public.infra_network_policy USING btree (application_date DESC) WHERE (deleted = 0);


--
-- Name: idx_infra_network_policy_dest_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_network_policy_dest_ip ON public.infra_network_policy USING btree (destination_ip) WHERE (deleted = 0);


--
-- Name: idx_infra_network_policy_endpoints; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_network_policy_endpoints ON public.infra_network_policy USING btree (source_ip, destination_ip) WHERE (deleted = 0);


--
-- Name: idx_infra_network_policy_source_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_network_policy_source_ip ON public.infra_network_policy USING btree (source_ip) WHERE (deleted = 0);


--
-- Name: idx_infra_network_policy_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_network_policy_tenant ON public.infra_network_policy USING btree (tenant_id);


--
-- Name: idx_infra_resource_ticket_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_resource_ticket_tenant ON public.infra_resource_ticket USING btree (tenant_id, id) WHERE (deleted = 0);


--
-- Name: idx_infra_risk_policy_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_risk_policy_source ON public.infra_risk USING btree (tenant_id, source_type, source_id) WHERE (deleted = 0);


--
-- Name: idx_infra_risk_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_risk_tenant ON public.infra_risk USING btree (tenant_id);


--
-- Name: idx_infra_task_expired_lease; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_task_expired_lease ON public.infra_task USING btree (lease_expires_at) WHERE ((deleted = 0) AND ((status)::text = 'running'::text));


--
-- Name: idx_infra_task_tenant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_task_tenant ON public.infra_task USING btree (tenant_id);


--
-- Name: idx_infra_task_tenant_idempotency; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_infra_task_tenant_idempotency ON public.infra_task USING btree (tenant_id, idempotency_key) WHERE ((deleted = 0) AND (idempotency_key IS NOT NULL));


--
-- Name: idx_infra_task_worker_queue; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_infra_task_worker_queue ON public.infra_task USING btree (next_attempt_at, create_time) WHERE ((deleted = 0) AND ((status)::text = ANY ((ARRAY['queued'::character varying, 'retrying'::character varying])::text[])));


--
-- Name: idx_inspection_baseline_tenant_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_inspection_baseline_tenant_ip ON public.infra_inspection_baseline USING btree (tenant_id, ip) NULLS NOT DISTINCT;


--
-- Name: idx_inspection_result_task; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_result_task ON public.infra_inspection_result USING btree (task_id);


--
-- Name: idx_machine_room_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_machine_room_code ON public.infra_machine_room USING btree (room_code) WHERE (deleted = 0);


--
-- Name: idx_machine_room_provider; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_machine_room_provider ON public.infra_machine_room USING btree (provider_id) WHERE (deleted = 0);


--
-- Name: idx_network_zone_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_network_zone_name ON public.infra_network_zone USING btree (name) WHERE (deleted = 0);


--
-- Name: idx_physical_resource_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_physical_resource_customer ON public.infra_physical_resource USING btree (customer_name) WHERE (deleted = 0);


--
-- Name: idx_physical_resource_room; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_physical_resource_room ON public.infra_physical_resource USING btree (machine_room_id) WHERE (deleted = 0);


--
-- Name: idx_resource_ticket_provider; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_resource_ticket_provider ON public.infra_resource_ticket USING btree (provider_id) WHERE (deleted = 0);


--
-- Name: idx_resource_ticket_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_resource_ticket_status ON public.infra_resource_ticket USING btree (ticket_status) WHERE (deleted = 0);


--
-- Name: idx_resource_ticket_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_resource_ticket_type ON public.infra_resource_ticket USING btree (resource_type) WHERE (deleted = 0);


--
-- Name: idx_resource_ticket_workflow; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_resource_ticket_workflow ON public.infra_resource_ticket USING btree (ticket_type, ticket_status) WHERE (deleted = 0);


--
-- Name: idx_risk_asset_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_risk_asset_ip ON public.infra_risk USING btree (asset_ip) WHERE (deleted = 0);


--
-- Name: idx_risk_inspection_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_risk_inspection_key ON public.infra_risk USING btree (tenant_id, inspection_key) WHERE ((deleted = 0) AND (inspection_key IS NOT NULL));


--
-- Name: idx_risk_severity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_risk_severity ON public.infra_risk USING btree (severity) WHERE (deleted = 0);


--
-- Name: idx_risk_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_risk_status ON public.infra_risk USING btree (status) WHERE (deleted = 0);


--
-- Name: idx_security_product_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_product_category ON public.infra_security_product USING btree (category) WHERE (deleted = 0);


--
-- Name: idx_security_product_vendor; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_product_vendor ON public.infra_security_product USING btree (vendor) WHERE (deleted = 0);


--
-- Name: idx_service_provider_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_service_provider_code ON public.infra_service_provider USING btree (provider_code) WHERE (deleted = 0);


--
-- Name: idx_system_login_log_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_login_log_01 ON public.system_login_log USING btree (username);


--
-- Name: idx_system_login_log_02; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_login_log_02 ON public.system_login_log USING btree (create_time);


--
-- Name: idx_system_menu_active_menu; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_menu_active_menu ON public.system_menu USING btree (active_menu_id) WHERE ((deleted = 0) AND (active_menu_id IS NOT NULL));


--
-- Name: idx_system_menu_tree_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_menu_tree_active ON public.system_menu USING btree (parent_id, sort, id) WHERE ((deleted = 0) AND (status = 0));


--
-- Name: idx_system_notify_message_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_notify_message_01 ON public.system_notify_message USING btree (user_id, user_type, read_status);


--
-- Name: idx_system_oauth2_access_token_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_access_token_01 ON public.system_oauth2_access_token USING btree (md5(access_token));


--
-- Name: idx_system_oauth2_access_token_02; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_access_token_02 ON public.system_oauth2_access_token USING btree (refresh_token);


--
-- Name: idx_system_oauth2_access_token_active_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_access_token_active_user ON public.system_oauth2_access_token USING btree (user_id, deleted, expires_time);


--
-- Name: idx_system_oauth2_approve_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_approve_01 ON public.system_oauth2_approve USING btree (user_id, user_type, client_id);


--
-- Name: idx_system_oauth2_client_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_client_01 ON public.system_oauth2_client USING btree (client_id);


--
-- Name: idx_system_oauth2_code_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_code_01 ON public.system_oauth2_code USING btree (code);


--
-- Name: idx_system_oauth2_refresh_token_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_oauth2_refresh_token_01 ON public.system_oauth2_refresh_token USING btree (refresh_token);


--
-- Name: idx_system_operate_log_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_operate_log_01 ON public.system_operate_log USING btree (user_id);


--
-- Name: idx_system_operate_log_02; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_operate_log_02 ON public.system_operate_log USING btree (create_time);


--
-- Name: idx_system_role_menu_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_role_menu_01 ON public.system_role_menu USING btree (role_id);


--
-- Name: idx_system_role_menu_active_menu; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_role_menu_active_menu ON public.system_role_menu USING btree (menu_id, role_id) WHERE (deleted = 0);


--
-- Name: idx_system_role_tenant_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_role_tenant_active ON public.system_role USING btree (tenant_id, status, id) WHERE (deleted = 0);


--
-- Name: idx_system_sms_code_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_sms_code_01 ON public.system_sms_code USING btree (mobile);


--
-- Name: idx_system_social_user_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_social_user_01 ON public.system_social_user USING btree (type, openid);


--
-- Name: idx_system_social_user_02; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_social_user_02 ON public.system_social_user USING btree (type, code, state);


--
-- Name: idx_system_social_user_bind_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_social_user_bind_01 ON public.system_social_user_bind USING btree (user_type, social_user_id);


--
-- Name: idx_system_user_role_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_user_role_01 ON public.system_user_role USING btree (user_id);


--
-- Name: idx_system_user_role_role; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_user_role_role ON public.system_user_role USING btree (role_id, user_id) WHERE (deleted = 0);


--
-- Name: idx_system_users_01; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_users_01 ON public.system_users USING btree (username);


--
-- Name: idx_system_users_02; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_users_02 ON public.system_users USING btree (mobile);


--
-- Name: idx_system_users_03; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_users_03 ON public.system_users USING btree (email);


--
-- Name: idx_system_users_04; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_users_04 ON public.system_users USING btree (dept_id);


--
-- Name: idx_task_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_task_status ON public.infra_task USING btree (status) WHERE (deleted = 0);


--
-- Name: system_users_identity_uuid_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX system_users_identity_uuid_key ON public.system_users USING btree (identity_uuid);


--
-- Name: cmdb_instance cmdb_instance_require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER cmdb_instance_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.cmdb_instance FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: cmdb_instance cmdb_instance_unique_value_sync; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER cmdb_instance_unique_value_sync AFTER INSERT OR DELETE OR UPDATE OF id, tenant_id, model_id, attributes, deleted ON public.cmdb_instance FOR EACH ROW EXECUTE FUNCTION public.cmdb_sync_instance_unique_value();


--
-- Name: cmdb_model cmdb_model_unique_value_sync; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER cmdb_model_unique_value_sync AFTER UPDATE OF unique_key ON public.cmdb_model FOR EACH ROW EXECUTE FUNCTION public.cmdb_rebuild_model_unique_values();


--
-- Name: cmdb_relation cmdb_relation_require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER cmdb_relation_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.cmdb_relation FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_resource_ticket infra_ticket_require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER infra_ticket_require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_resource_ticket FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_asset require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_asset FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_inspection_baseline require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_inspection_baseline FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_inspection_result require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_inspection_result FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_network_policy require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_network_policy FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_risk require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_risk FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: infra_task require_tenant; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_task FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();


--
-- Name: chat_conversations chat_conversations_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_conversations
    ADD CONSTRAINT chat_conversations_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: chat_messages chat_messages_conversation_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_messages
    ADD CONSTRAINT chat_messages_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES ai.chat_conversations(id) ON DELETE CASCADE;


--
-- Name: chat_messages chat_messages_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_messages
    ADD CONSTRAINT chat_messages_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: chat_roles chat_roles_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.chat_roles
    ADD CONSTRAINT chat_roles_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: images images_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.images
    ADD CONSTRAINT images_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: images images_parent_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.images
    ADD CONSTRAINT images_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES ai.images(id) ON DELETE SET NULL;


--
-- Name: knowledge_bases knowledge_bases_embedding_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_bases
    ADD CONSTRAINT knowledge_bases_embedding_model_id_fkey FOREIGN KEY (embedding_model_id) REFERENCES ai.model_configs(id);


--
-- Name: knowledge_documents knowledge_documents_knowledge_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_documents
    ADD CONSTRAINT knowledge_documents_knowledge_id_fkey FOREIGN KEY (knowledge_id) REFERENCES ai.knowledge_bases(id) ON DELETE CASCADE;


--
-- Name: knowledge_segments knowledge_segments_document_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_segments
    ADD CONSTRAINT knowledge_segments_document_id_fkey FOREIGN KEY (document_id) REFERENCES ai.knowledge_documents(id) ON DELETE CASCADE;


--
-- Name: knowledge_segments knowledge_segments_knowledge_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_segments
    ADD CONSTRAINT knowledge_segments_knowledge_id_fkey FOREIGN KEY (knowledge_id) REFERENCES ai.knowledge_bases(id) ON DELETE CASCADE;


--
-- Name: model_catalog model_catalog_platform_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.model_catalog
    ADD CONSTRAINT model_catalog_platform_fkey FOREIGN KEY (platform) REFERENCES ai.model_platforms(platform) ON DELETE CASCADE;


--
-- Name: music music_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.music
    ADD CONSTRAINT music_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: writes writes_model_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.writes
    ADD CONSTRAINT writes_model_id_fkey FOREIGN KEY (model_id) REFERENCES ai.model_configs(id);


--
-- Name: cmdb_instance cmdb_instance_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance
    ADD CONSTRAINT cmdb_instance_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: cmdb_instance_unique_value cmdb_instance_unique_value_instance_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance_unique_value
    ADD CONSTRAINT cmdb_instance_unique_value_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.cmdb_instance(id) ON DELETE CASCADE;


--
-- Name: cmdb_instance_unique_value cmdb_instance_unique_value_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_instance_unique_value
    ADD CONSTRAINT cmdb_instance_unique_value_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.cmdb_model(id);


--
-- Name: cmdb_relation cmdb_relation_source_tenant_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_relation
    ADD CONSTRAINT cmdb_relation_source_tenant_fk FOREIGN KEY (tenant_id, source_id) REFERENCES public.cmdb_instance(tenant_id, id);


--
-- Name: cmdb_relation cmdb_relation_target_tenant_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_relation
    ADD CONSTRAINT cmdb_relation_target_tenant_fk FOREIGN KEY (tenant_id, target_id) REFERENCES public.cmdb_instance(tenant_id, id);


--
-- Name: cmdb_relation cmdb_relation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cmdb_relation
    ADD CONSTRAINT cmdb_relation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_asset infra_asset_net_zone_tenant_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_asset
    ADD CONSTRAINT infra_asset_net_zone_tenant_fk FOREIGN KEY (tenant_id, net_zone_id) REFERENCES public.cmdb_net_zone(tenant_id, id);


--
-- Name: infra_asset infra_asset_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_asset
    ADD CONSTRAINT infra_asset_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_inspection_baseline infra_inspection_baseline_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_inspection_baseline
    ADD CONSTRAINT infra_inspection_baseline_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_inspection_result infra_inspection_result_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_inspection_result
    ADD CONSTRAINT infra_inspection_result_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_network_policy infra_network_policy_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_network_policy
    ADD CONSTRAINT infra_network_policy_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_resource_ticket infra_resource_ticket_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_resource_ticket
    ADD CONSTRAINT infra_resource_ticket_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_risk infra_risk_high_risk_rule_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_risk
    ADD CONSTRAINT infra_risk_high_risk_rule_fk FOREIGN KEY (rule_id) REFERENCES public.infra_high_risk_port_rule(id);


--
-- Name: infra_risk infra_risk_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_risk
    ADD CONSTRAINT infra_risk_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- Name: infra_task infra_task_tenant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.infra_task
    ADD CONSTRAINT infra_task_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.system_tenant(id);


--
-- PostgreSQL database dump complete
--

\unrestrict O11kQ74k2KCb73stt2JlE8yvRaKwWt5cSHy1KaavgPkPrE4pMJF15fbbx63PsTZ

