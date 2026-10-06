use rustset_framework_database::{DatabaseConfig, connect, migrate};
use sqlx::Row;

#[tokio::test]
#[ignore = "run with script/test-database-migrations.sh"]
async fn applies_all_migrations_to_empty_postgres() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let config = DatabaseConfig::new(url, 1, 5, std::time::Duration::from_secs(10))
        .expect("valid test database config");
    let pool = connect(&config).await.expect("connect test database");
    migrate(&pool).await.expect("apply complete migration set");

    // New migrations are deliberately rerunnable because operators may need
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&pool)
        .await
        .expect("read migration history");
    assert_eq!(applied, 40);
    let high_risk_rules: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_high_risk_port_rule WHERE enabled AND deleted = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("read high-risk port baseline");
    assert!(high_risk_rules >= 18);
    let risk_source_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema='public' AND table_name='infra_risk'
           AND column_name IN ('source_type','source_id','rule_id')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect network-policy risk linkage");
    assert_eq!(risk_source_columns, 3);
    let unique_constraint: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'cmdb_instance_unique_value'::regclass AND conname = 'cmdb_instance_unique_value_key' AND contype = 'u')"
    ).fetch_one(&pool).await.expect("CMDB unique-value constraint");
    assert!(unique_constraint);
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0026_cmdb_tenant_isolation.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun tenant isolation migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0027_asset_policy_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun asset and policy tenant migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0028_network_policy_port_risk.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun network-policy risk migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0029_asset_tenant_unique_ip.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun tenant asset uniqueness migration");
    let asset_unique_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes
         WHERE schemaname='public' AND indexname='idx_asset_tenant_ip_unique'",
    )
    .fetch_one(&pool)
    .await
    .expect("read tenant asset unique index");
    assert!(asset_unique_index.contains("tenant_id, ip"));
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0030_recoverable_scan_tasks.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun recoverable scan-task migration");
    let task_queue_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema='public' AND table_name='infra_task'
           AND column_name IN ('payload','idempotency_key','attempt_count','max_attempts',
             'timeout_seconds','next_attempt_at','lease_owner','lease_expires_at',
             'heartbeat_at','cancel_requested')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect durable task queue");
    assert_eq!(task_queue_columns, 10);

    for table in ["cmdb_instance", "cmdb_relation", "infra_resource_ticket"] {
        let nullable: String = sqlx::query_scalar("SELECT is_nullable FROM information_schema.columns WHERE table_schema='public' AND table_name=$1 AND column_name='tenant_id'")
            .bind(table).fetch_one(&pool).await.unwrap();
        assert_eq!(nullable, "YES", "historical ownership must not be guessed");
    }
    let definition: String = sqlx::query_scalar("SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid='cmdb_instance_unique_value'::regclass AND conname='cmdb_instance_unique_value_key'")
        .fetch_one(&pool).await.unwrap();
    assert!(definition.contains("tenant_id, model_id, value"));

    let computed_expression_column: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'public' AND table_name = 'cmdb_attribute'
             AND column_name = 'expression' AND data_type = 'text'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect computed attribute expression column");
    assert!(computed_expression_column);
    let attribute_color_column: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'public' AND table_name = 'cmdb_attribute'
             AND column_name = 'color' AND character_maximum_length = 16
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect attribute color column");
    assert!(attribute_color_column);
    let trigger_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public.cmdb_attribute_trigger') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect CMDB attribute trigger table");
    assert!(trigger_table);
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0031_cmdb_computed_attributes.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun computed attributes migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0032_cmdb_attribute_color.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun attribute color migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0033_cmdb_attribute_triggers.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun attribute trigger migration");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0034_provider_room_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun provider and room tenant migration");
    for table in ["infra_service_provider", "infra_machine_room"] {
        let nullable: String = sqlx::query_scalar(
            "SELECT is_nullable FROM information_schema.columns
             WHERE table_schema='public' AND table_name=$1 AND column_name='tenant_id'",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("inspect provider inventory tenant column");
        assert_eq!(
            nullable, "YES",
            "historical ownership must remain unassigned"
        );
    }
    let room_provider_tenant_fk: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint
         WHERE conrelid='infra_machine_room'::regclass
           AND conname='infra_machine_room_provider_tenant_fk')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect room/provider tenant foreign key");
    assert!(room_provider_tenant_fk);
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0035_cloud_security_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun cloud and security tenant migration");
    for table in [
        "infra_cloud_zone",
        "infra_cloud_platform",
        "infra_cloud_provider_config",
        "infra_security_product",
    ] {
        let nullable: String = sqlx::query_scalar(
            "SELECT is_nullable FROM information_schema.columns
             WHERE table_schema='public' AND table_name=$1 AND column_name='tenant_id'",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("inspect cloud inventory tenant column");
        assert_eq!(
            nullable, "YES",
            "historical ownership must remain unassigned"
        );
    }
    let tenant_reference_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint
         WHERE conname IN ('infra_cloud_platform_zone_tenant_fk',
            'infra_cloud_config_zone_tenant_fk', 'infra_cloud_config_platform_tenant_fk',
            'infra_security_cloud_platform_tenant_fk',
            'infra_security_machine_room_tenant_fk', 'infra_security_provider_tenant_fk')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect cloud/security tenant foreign keys");
    assert_eq!(tenant_reference_count, 6);
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0036_business_application_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun business application tenant migration");
    let endpoint_tenant_fk: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint
         WHERE conrelid='infra_application_endpoint'::regclass
           AND conname='infra_endpoint_application_tenant_fk')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect endpoint/application tenant foreign key");
    assert!(endpoint_tenant_fk);
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0037_resource_ledger_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun resource ledger tenant migration");
    let ledger_tenant_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema='public' AND column_name='tenant_id'
           AND table_name IN ('infra_cloud_asset','infra_cloud_resource','infra_physical_resource')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect resource ledger tenant columns");
    assert_eq!(ledger_tenant_columns, 3);
    let discovery_unique_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes WHERE schemaname='public'
         AND indexname='idx_cloud_asset_tenant_instance_config'",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect tenant cloud discovery uniqueness");
    assert!(discovery_unique_index.contains("NULLS NOT DISTINCT"));
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0038_approval_rule_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun approval rule tenant migration");
    let approval_rule_tenant_nullable: String = sqlx::query_scalar(
        "SELECT is_nullable FROM information_schema.columns
         WHERE table_schema='public' AND table_name='infra_approval_rule'
           AND column_name='tenant_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect approval rule tenant column");
    assert_eq!(approval_rule_tenant_nullable, "YES");
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0039_ticket_idempotency.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun ticket idempotency migration");
    let ticket_idempotency_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes WHERE schemaname='public'
         AND indexname='idx_resource_ticket_tenant_idempotency'",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect ticket idempotency index");
    assert!(ticket_idempotency_index.contains("tenant_id, idempotency_key"));
    sqlx::raw_sql(include_str!(
        "../../../../sql/postgresql/0040_network_zone_tenants.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun network zone tenant migration");
    let network_zone_tenant_nullable: String = sqlx::query_scalar(
        "SELECT is_nullable FROM information_schema.columns
         WHERE table_schema='public' AND table_name='infra_network_zone'
           AND column_name='tenant_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect network zone tenant column");
    assert_eq!(network_zone_tenant_nullable, "YES");
    let network_zone_tenant_references: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint WHERE conname IN
         ('infra_network_zone_cloud_platform_tenant_fk',
          'infra_network_zone_machine_room_tenant_fk')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect network zone tenant references");
    assert_eq!(network_zone_tenant_references, 2);

    // 0024 renames the API documentation page from swagger to api-docs.
    let swagger_paths: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND (path = 'swagger' OR component = 'infra/swagger/index')",
    )
    .fetch_one(&pool)
    .await
    .expect("read legacy swagger menu paths");
    assert_eq!(swagger_paths, 0, "swagger path must be renamed to api-docs");
    let api_docs_menu: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND path = 'api-docs' AND component = 'infra/api-docs/index'",
    )
    .fetch_one(&pool)
    .await
    .expect("read api-docs menu");
    assert_eq!(api_docs_menu, 1);

    // 0023 retires the upstream codegen demo showcase; nothing under
    // 代码生成案例 may stay visible or authorized.
    let demo_menus: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0
           AND (component LIKE 'infra/demo%'
                OR component LIKE 'infra/testDemo%'
                OR (parent_id = 2 AND name = '代码生成案例' AND path = 'demo'))",
    )
    .fetch_one(&pool)
    .await
    .expect("read demo showcase menus");
    assert_eq!(demo_menus, 0, "demo showcase menus must be retired");

    // 0022 asset inspection: approved TCP baselines, per-target results and a
    // dedicated page that reuses scan/risk/asset permissions.
    for inspection_table in ["infra_inspection_baseline", "infra_inspection_result"] {
        let exists: bool = sqlx::query_scalar(&format!(
            "SELECT to_regclass('public.{inspection_table}') IS NOT NULL"
        ))
        .fetch_one(&pool)
        .await
        .expect("inspect inspection table");
        assert!(exists, "expected table {inspection_table}");
    }
    let inspection_page: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND component = 'asset-ops/inspection/index' AND type = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("read inspection page");
    assert_eq!(inspection_page, 1);
    let inspection_page_grants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_role_menu grant_page
         JOIN system_menu page ON page.id = grant_page.menu_id
              AND page.component = 'asset-ops/inspection/index' AND page.deleted = 0
         WHERE grant_page.deleted = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("read inspection page grants");
    let scan_page_grants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_role_menu grant_page
         JOIN system_menu page ON page.id = grant_page.menu_id
              AND page.component = 'asset-ops/task/index' AND page.deleted = 0
         WHERE grant_page.deleted = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("read scan task page grants");
    assert!(
        inspection_page_grants >= scan_page_grants,
        "every role that can run scans must also see the inspection page"
    );

    // 0016 removes unsupported BPM and scopes network zones by tenant;
    // 0017 removes user-visible upstream branding from baseline data;
    // 0018 moves internals to 国密 (SM3 identity UUIDs, SM3 password hash,
    // SM4-seeded secrets) and clears the last naming residue.
    let bpm_menus: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND (path = '/bpm' OR permission LIKE 'bpm:%')",
    )
    .fetch_one(&pool)
    .await
    .expect("read BPM menus");
    assert_eq!(bpm_menus, 0, "BPM must not remain visible or authorized");
    // 0021 splits the mixed ledger table into typed cloud/physical tables.
    for split_table in ["infra_cloud_resource", "infra_physical_resource"] {
        let exists: bool = sqlx::query_scalar(&format!(
            "SELECT to_regclass('public.{split_table}') IS NOT NULL"
        ))
        .fetch_one(&pool)
        .await
        .expect("inspect split resource table");
        assert!(exists, "expected table {split_table}");
    }
    let old_ledger_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.infra_business_resource') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect retired ledger table");
    assert!(
        !old_ledger_exists,
        "infra_business_resource must be retired after the 0021 split"
    );

    // 0020 splits 业务资源 into 云资源 + 物理资源 pages.
    let split_resource_menus: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND parent_id = 30343
           AND path IN ('cloud-resource', 'physical-resource')
           AND component IN (
               'asset-ops/business-resource/cloud/index',
               'asset-ops/business-resource/physical/index')",
    )
    .fetch_one(&pool)
    .await
    .expect("read split business-resource menus");
    assert_eq!(
        split_resource_menus, 2,
        "业务管理 must expose separate 云资源 and 物理资源 pages"
    );

    let demo_analytics_menu: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND path = '/analytics'",
    )
    .fetch_one(&pool)
    .await
    .expect("read demo analytics menu");
    assert_eq!(
        demo_analytics_menu, 0,
        "the Vben playground demo analytics page must not stay visible"
    );

    let net_zone_page: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND component = 'cmdb/net-zone/index'",
    )
    .fetch_one(&pool)
    .await
    .expect("read standalone network-zone page");
    assert_eq!(
        net_zone_page, 0,
        "network zones live inside tenant management"
    );
    let tenant_scoped_net_zones: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'public'
             AND table_name = 'cmdb_net_zone'
             AND column_name = 'tenant_id'
             AND is_nullable = 'NO'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect tenant-scoped network zones");
    assert!(tenant_scoped_net_zones);

    // 0015 splits 资产运营 into five top-level modules; components and
    // grants are unchanged, only the menu tree moves.
    let old_root: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND parent_id = 0 AND path = '/asset-ops'",
    )
    .fetch_one(&pool)
    .await
    .expect("read old asset-ops root");
    assert_eq!(old_root, 0, "the old 资产运营 root must be retired");
    let new_roots: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND parent_id = 0
           AND path IN ('/asset-center','/cloud-center','/infra-center','/biz-center','/ops-center')",
    )
    .fetch_one(&pool)
    .await
    .expect("read new module roots");
    assert_eq!(new_roots, 5);
    let orphaned_pages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu page
         WHERE page.deleted = 0 AND page.type = 2
           AND page.component LIKE 'asset-ops/%'
           AND NOT EXISTS (
               SELECT 1 FROM system_menu parent
               WHERE parent.id = page.parent_id AND parent.deleted = 0 AND parent.type = 1
           )",
    )
    .fetch_one(&pool)
    .await
    .expect("check reparented pages");
    assert_eq!(
        orphaned_pages, 0,
        "every moved page must have a live parent"
    );

    // 0014 repaired the truncated cmdb:* codes seeded by 0009.
    let truncated: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND type = 3
           AND permission IN ('cmdb:query','cmdb:create','cmdb:update','cmdb:delete')",
    )
    .fetch_one(&pool)
    .await
    .expect("read truncated cmdb codes");
    assert_eq!(
        truncated, 0,
        "truncated cmdb permission codes must be renamed"
    );
    let model_query: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu WHERE deleted = 0 AND permission = 'cmdb:model:query'",
    )
    .fetch_one(&pool)
    .await
    .expect("read cmdb:model:query");
    assert_eq!(model_query, 1);

    // 0013 CMDB default roles + organization net-zone tree.
    for role_code in ["cmdb_admin", "cmdb_user"] {
        let exists: i64 =
            sqlx::query_scalar("SELECT count(*) FROM system_role WHERE code = $1 AND deleted = 0")
                .bind(role_code)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|_| panic!("read role {role_code}"));
        assert_eq!(exists, 1, "role {role_code} must exist");
    }
    let cmdb_admin_grants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_role_menu rm
         JOIN system_role r ON r.id = rm.role_id AND r.code = 'cmdb_admin' AND r.deleted = 0
         JOIN system_menu m ON m.id = rm.menu_id AND m.deleted = 0
         WHERE m.permission LIKE 'cmdb:%'",
    )
    .fetch_one(&pool)
    .await
    .expect("read cmdb_admin grants");
    let cmdb_total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu WHERE deleted = 0 AND permission LIKE 'cmdb:%' AND type = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("count cmdb permissions");
    assert_eq!(
        cmdb_admin_grants, cmdb_total,
        "cmdb_admin must hold every cmdb permission"
    );
    let net_zone_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public.cmdb_net_zone') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect net zone table");
    assert!(net_zone_table);

    // 0012 approval-rule management page.
    let approval_rule_pages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND component = 'asset-ops/approval-rule/index' AND type = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("read approval rule page");
    assert_eq!(approval_rule_pages, 1);
    let approval_rule_perms: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND permission LIKE 'infra:approval-rule:%' AND type = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("read approval rule permissions");
    assert_eq!(approval_rule_perms, 4);

    // 0011 ops agent: five Rust-executed tools and the preset 运维助理 role.
    let ops_tools: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.tools WHERE name IN
            ('cmdb_model_list','cmdb_instance_query','asset_query','ticket_query','ticket_create')",
    )
    .fetch_one(&pool)
    .await
    .expect("read ops agent tools");
    assert_eq!(ops_tools, 5);
    let chat_models: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.model_configs WHERE type='chat'")
            .fetch_one(&pool)
            .await
            .expect("read chat models");
    let agent_role: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.chat_roles
         WHERE name='运维助理' AND public_status = true AND array_length(tool_ids, 1) = 5",
    )
    .fetch_one(&pool)
    .await
    .expect("read ops agent role");
    assert_eq!(
        agent_role,
        if chat_models > 0 { 1 } else { 0 },
        "agent role is preset iff a chat model row exists"
    );

    // 0010 OpenTofu provision tracking + auto-approval rules.
    let apply_status_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM information_schema.columns
           WHERE table_schema='public'
             AND table_name='infra_resource_ticket'
             AND column_name='apply_status'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect ticket apply tracking");
    assert!(apply_status_exists);
    let approval_rules_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public.infra_approval_rule') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect approval rule table");
    assert!(approval_rules_table);

    // 0009 CMDB core: dynamic models, attributes, JSONB instances, relations.
    for cmdb_table in [
        "public.cmdb_model",
        "public.cmdb_attribute",
        "public.cmdb_instance",
        "public.cmdb_relation",
    ] {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(cmdb_table)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| panic!("inspect {cmdb_table}"));
        assert!(exists, "expected table {cmdb_table}");
    }
    let cmdb_permissions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu WHERE deleted = 0 AND permission LIKE 'cmdb:%' AND type = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("read cmdb permission menus");
    assert_eq!(
        cmdb_permissions, 15,
        "cmdb permissions = 0009 set + net-zone (0013)"
    );

    // 0008 drops the whole Toonflow media business.
    for removed_schema in ["toonflow", "toon", "media"] {
        let schema_exists: bool = sqlx::query_scalar("SELECT to_regnamespace($1) IS NOT NULL")
            .bind(removed_schema)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| panic!("inspect removed schema {removed_schema}"));
        assert!(!schema_exists, "schema {removed_schema} must be dropped");
    }
    let toon_menus: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND (permission LIKE 'toon:%' OR path = '/toonflow')",
    )
    .fetch_one(&pool)
    .await
    .expect("read toonflow menus");
    assert_eq!(toon_menus, 0, "0008 must remove the 短剧工厂 menu subtree");

    let network_policy_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.infra_network_policy') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect network policy table");
    assert!(
        network_policy_exists,
        "0007 must create infra_network_policy"
    );

    let asset_inventory_column_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM information_schema.columns
           WHERE table_schema='public'
             AND table_name='infra_asset'
             AND column_name='classified_protection_level'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect asset inventory columns");
    assert!(
        asset_inventory_column_exists,
        "0007 must extend infra_asset with inventory columns"
    );

    let network_policy_permissions: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM system_menu
         WHERE deleted = 0
           AND permission LIKE 'infra:network-policy:%'
           AND type = 3",
    )
    .fetch_one(&pool)
    .await
    .expect("read network policy permission menus");
    assert_eq!(
        network_policy_permissions, 4,
        "0007 must seed query/create/update/delete permissions for network policies"
    );

    let ticket_type_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM information_schema.columns
           WHERE table_schema='public'
             AND table_name='infra_resource_ticket'
             AND column_name='ticket_type'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect Kairos ticket workflow fields");
    assert!(ticket_type_exists);

    for table in [
        "ai.model_configs",
        "ai.chat_roles",
        "ai.knowledge_segments",
        "ai.images",
        "ai.music",
        "system_users",
        "system_role",
        "system_menu",
        "system_oauth2_access_token",
        "infra_config",
        "infra_job",
        "infra_job_log",
        "infra_api_access_log",
        "infra_api_error_log",
        "infra_codegen_table",
        "infra_codegen_column",
        "yudao_demo01_contact",
        "yudao_demo02_category",
        "yudao_demo03_student",
        "yudao_demo03_course",
        "yudao_demo03_grade",
    ] {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("inspect expected table");
        assert!(exists, "expected table {table}");
    }
    for removed in ["toonflow.vendor_configs", "toonflow.model_prompts"] {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(removed)
            .fetch_one(&pool)
            .await
            .expect("inspect removed table");
        assert!(!exists, "legacy table {removed} must be removed");
    }
    let menu = sqlx::query("SELECT component,deleted FROM system_menu WHERE id=30006")
        .fetch_one(&pool)
        .await
        .expect("AI model menu exists");
    assert_eq!(menu.get::<String, _>("component"), "ai/model/model/index");
    assert_eq!(menu.get::<i16, _>("deleted"), 1);

    let duplicate_route_names: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM (
             SELECT CASE
                    WHEN coalesce(component_name, '') <> '' THEN component_name
                    ELSE name
                    END AS route_name
             FROM system_menu
             WHERE deleted = 0 AND status = 0 AND type <> 3
             GROUP BY route_name
             HAVING count(*) > 1
         ) duplicate_routes",
    )
    .fetch_one(&pool)
    .await
    .expect("read duplicate route names");
    assert_eq!(
        duplicate_route_names, 0,
        "active route menus must not generate duplicate frontend route names"
    );

    let asset_ops_pages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND component LIKE 'asset-ops/%' AND type = 2",
    )
    .fetch_one(&pool)
    .await
    .expect("read Kairos asset operations pages");
    // 16 pages after 0021 + the 资产核查 page from 0022.
    assert_eq!(asset_ops_pages, 17);

    let asset_ops_permissions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND permission LIKE 'infra:%' AND parent_id IN (
             SELECT id FROM system_menu
             WHERE deleted = 0 AND component LIKE 'asset-ops/%' AND type = 2
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("read Kairos asset operations permissions");
    // 0021 adds the four infra:physical-resource:* buttons.
    assert_eq!(asset_ops_permissions, 67);

    let missing_super_admin_asset_links: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM system_role role
         CROSS JOIN system_menu menu
         WHERE role.deleted = 0 AND role.status = 0 AND role.code = 'super_admin'
           AND menu.deleted = 0
           AND (
             menu.path = '/asset-ops'
             OR menu.component LIKE 'asset-ops/%'
             OR menu.parent_id IN (
               SELECT id FROM system_menu
               WHERE deleted = 0 AND component LIKE 'asset-ops/%'
             )
           )
           AND NOT EXISTS (
             SELECT 1 FROM system_role_menu link
             WHERE link.deleted = 0
               AND link.role_id = role.id
               AND link.menu_id = menu.id
           )",
    )
    .fetch_one(&pool)
    .await
    .expect("verify super administrator asset menu grants");
    assert_eq!(missing_super_admin_asset_links, 0);

    let active_menu_links: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_menu
         WHERE deleted = 0 AND visible = false AND active_menu_id IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("read hidden-page business menu links");
    assert!(active_menu_links >= 8);

    let administrators: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM system_users u
         JOIN system_user_role ur ON ur.user_id = u.id AND ur.deleted = 0
         JOIN system_role r ON r.id = ur.role_id AND r.deleted = 0
         WHERE u.deleted = 0 AND u.status = 0 AND r.status = 0
           AND r.code = 'super_admin'",
    )
    .fetch_one(&pool)
    .await
    .expect("read seeded administrators");
    assert!(administrators > 0);

    let baseline_tenants: i64 =
        sqlx::query_scalar("SELECT count(*) FROM system_tenant WHERE deleted = 0")
            .fetch_one(&pool)
            .await
            .expect("read baseline tenants");
    assert_eq!(
        baseline_tenants, 3,
        "fresh migration bootstrap must restore the current baseline tenants, not synthesize a default company"
    );

    let current_baseline_tenant_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM system_tenant
            WHERE id = 1 AND name = 'RustSet' AND deleted = 0
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect current baseline tenant");
    assert!(
        current_baseline_tenant_exists,
        "fresh migration bootstrap must expose the RustSet baseline tenant"
    );

    let visible_upstream_branding: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT count(*) FROM system_tenant
             WHERE deleted = 0 AND (name ILIKE '%芋道%' OR contact_name ILIKE '%芋道%'))
          + (SELECT count(*) FROM system_notice
             WHERE deleted = 0 AND (title ILIKE '%芋道%' OR content ILIKE '%yudao.iocoder.cn%'))
          + (SELECT count(*) FROM system_oauth2_client
             WHERE deleted = 0 AND (name ILIKE '%芋道%' OR logo ILIKE '%yudao.iocoder.cn%'))",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect user-visible baseline branding");
    assert_eq!(visible_upstream_branding, 0);

    let legacy_schema_exists: bool =
        sqlx::query_scalar("SELECT to_regnamespace('system') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("inspect legacy schema");
    assert!(!legacy_schema_exists);

    for runtime_table in [
        "system_oauth2_access_token",
        "system_oauth2_refresh_token",
        "system_login_log",
        "system_operate_log",
        "system_notify_message",
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {runtime_table}"))
            .fetch_one(&pool)
            .await
            .expect("read runtime table");
        assert_eq!(rows, 0, "{runtime_table} must start empty");
    }

    let codex_test_users: i64 =
        sqlx::query_scalar("SELECT count(*) FROM system_users WHERE username='codex_excel_user'")
            .fetch_one(&pool)
            .await
            .expect("read test users");
    assert_eq!(
        codex_test_users, 0,
        "transient test users must not be seeded"
    );

    let users_with_login_traces: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_users WHERE login_ip <> '' OR login_date IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("read user login traces");
    assert_eq!(
        users_with_login_traces, 0,
        "seed users must not carry login traces"
    );

    let users_with_remote_yudao_avatar: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_users
         WHERE avatar LIKE 'http://test.yudao.iocoder.cn/%'
            OR avatar LIKE 'https://test.yudao.iocoder.cn/%'",
    )
    .fetch_one(&pool)
    .await
    .expect("read user avatars");
    assert_eq!(
        users_with_remote_yudao_avatar, 0,
        "seed users must not depend on remote Yudao avatar assets"
    );

    let plain_mail_passwords: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_mail_account
         WHERE password IS NOT NULL AND password <> '' AND password NOT LIKE 'enc:sm4:v2:%'",
    )
    .fetch_one(&pool)
    .await
    .expect("read mail secrets");
    assert_eq!(
        plain_mail_passwords, 0,
        "mail account passwords must be SM4-sealed (国密), not the retired XOR format"
    );

    let plain_sms_secrets: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_sms_channel
         WHERE api_key NOT LIKE 'enc:sm4:v2:%'
            OR (api_secret IS NOT NULL AND api_secret <> '' AND api_secret NOT LIKE 'enc:sm4:v2:%')",
    )
    .fetch_one(&pool)
    .await
    .expect("read sms secrets");
    assert_eq!(
        plain_sms_secrets, 0,
        "sms channel secrets must be SM4-sealed (国密), not the retired XOR format"
    );

    let users_without_identity_uuid: i64 =
        sqlx::query_scalar("SELECT count(*) FROM system_users WHERE identity_uuid IS NULL")
            .fetch_one(&pool)
            .await
            .expect("read user identity uuids");
    assert_eq!(
        users_without_identity_uuid, 0,
        "every user must carry an SM3-derived identity_uuid"
    );

    let legacy_identity_duplicates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT identity_uuid FROM system_users
            WHERE identity_uuid IS NOT NULL
            GROUP BY identity_uuid HAVING count(*) > 1
         ) duplicates",
    )
    .fetch_one(&pool)
    .await
    .expect("read duplicate identity uuids");
    assert_eq!(legacy_identity_duplicates, 0);

    let admin_uses_gm_password: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM system_users
            WHERE id = 1 AND username = 'admin' AND password LIKE '$sm3$%'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("read admin password format");
    assert!(
        admin_uses_gm_password,
        "the seeded admin must verify via PBKDF2-HMAC-SM3 after 0018"
    );

    let user_level_branding_residue: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT count(*) FROM system_users
             WHERE deleted = 0
               AND (username = 'yudao' OR nickname IN ('芋艿', '芋道', '芋道1', '源码')
                    OR email IN ('yudao@iocoder.cn', 'yuanma@iocoder.cn')))
          + (SELECT count(*) FROM system_dept
             WHERE deleted = 0 AND email = 'ry@qq.com')",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect user-level naming residue");
    assert_eq!(user_level_branding_residue, 0);
}
