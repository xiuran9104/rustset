use crate::InfraState;
use chrono::Utc;
use rustset_framework_tenant::TenantContext;
use rustset_framework_tofu::{
    CloudTarget, DEMO_PROVIDER, TofuExecutor, credential_env, provider_source, render_main_tf,
    render_tfvars,
};
use rustset_framework_web::AppError;
use serde_json::{Map, Value, json};
use sqlx::Row;
use tracing::warn;

use super::repository;

pub(super) async fn apply_resource_side_effect(
    s: &InfraState,
    tenant: &TenantContext,
    ticket: &Value,
) {
    use tracing::info;

    let Some(target) = ticket.get("targetResourceId").and_then(Value::as_i64) else {
        warn!("ticket approval without targetResourceId; ledger not updated");
        return;
    };
    let ticket_type = ticket
        .get("ticketType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // 拆表后目标资源按 target_resource_type（或工单资源类型兜底）路由到对应台账。
    let target_type = ticket
        .get("targetResourceType")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| ticket.get("resourceType").and_then(Value::as_str))
        .unwrap_or("cloud");
    let physical = target_type == "physical";
    let ledger_table = if physical {
        "infra_physical_resource"
    } else {
        "infra_cloud_resource"
    };
    let new_status = match ticket_type {
        "stop" if physical => "offline",
        "stop" => "已停止",
        "reclaim" => "retired",
        _ => return,
    };
    let result = sqlx::query(&format!(
        "UPDATE {ledger_table}
         SET ecs_status=$2, update_time=now()
         WHERE id=$1 AND tenant_id=$3 AND deleted=0"
    ))
    .bind(target)
    .bind(new_status)
    .bind(tenant.id())
    .execute(&s.pool)
    .await;
    match result {
        Ok(updated) if updated.rows_affected() > 0 => {
            info!(
                target,
                new_status, "resource ledger updated by ticket approval"
            );
        }
        _ => warn!(
            target,
            "ticket target resource not found; ledger not updated"
        ),
    }
}

pub(super) async fn run(
    s: &InfraState,
    tenant: &TenantContext,
    id: i64,
    operator: &str,
    payload: &Value,
) -> Result<Value, AppError> {
    let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    // Hold a transaction-scoped advisory lock across the external operation.
    // Dropping the transaction releases it even on an early error/cancellation.
    let mut execution_lock = s
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to lock provision"))?;
    let acquired: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("rustset:provision:{id}"))
            .fetch_one(&mut *execution_lock)
            .await
            .map_err(|_| AppError::internal("failed to lock provision"))?;
    if !acquired {
        return Err(AppError::bad_request("该工单正在开通，请勿重复执行"));
    }
    let mut ticket = repository::get(&s.pool, &tenant, id).await?;
    let status = ticket
        .get("ticketStatus")
        .and_then(Value::as_str)
        .unwrap_or("");
    if status != "pending_provision" && status != "approved" {
        return Err(AppError::bad_request("只能配置待配置状态的工单"));
    }

    let saved: Value = ticket
        .get("targetConfig")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| AppError::bad_request("开通参数必须是 JSON 对象"))?
        .unwrap_or(json!({}));
    let config_id = payload
        .get("configId")
        .or_else(|| saved.get("configId"))
        .and_then(Value::as_i64);
    let (target, config_id) = match load_platform_target(s, tenant, &ticket, config_id).await {
        Ok(target) => target,
        Err(error) => {
            record_failure(
                s,
                tenant,
                id,
                "无法加载开通凭据：请检查厂商、平台、状态和区域",
            )
            .await?;
            return Err(error);
        }
    };
    let cloud_category = target.cloud_category.clone();
    let region = target.region.clone();

    let mut spec = Map::new();
    for (ticket_key, spec_key) in [
        ("ecsName", "ecs_name"),
        ("ecsType", "ecs_type"),
        ("ecsOs", "ecs_os"),
        ("cloudRegion", "cloud_region"),
        ("resourceCount", "resource_count"),
        ("cpuCores", "cpu_cores"),
        ("memoryGb", "memory_gb"),
        ("applicationName", "application_name"),
        ("systemDiskSizeGb", "system_disk_size_gb"),
    ] {
        if let Some(value) = ticket.get(ticket_key).filter(|v| !v.is_null()) {
            spec.insert(spec_key.to_string(), value.clone());
        }
    }
    spec.insert("ticket_id".to_string(), json!(id.to_string()));
    merge_provision_parameters(&mut spec, &ticket, payload)?;
    let spec = Value::Object(spec);
    let main_tf = match render_main_tf(&target, &spec) {
        Ok(template) => template,
        Err(reason) => {
            record_failure(s, tenant, id, &reason).await?;
            return Err(AppError::bad_request(reason));
        }
    };
    // Once a workspace has been used, do not switch its provider or account.
    let previous_workspace = ticket
        .get("tofuWorkspace")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !previous_workspace.is_empty()
        && (ticket.get("cloudCategory").and_then(Value::as_str) != Some(cloud_category.as_str())
            || saved.get("configId").and_then(Value::as_i64) != config_id)
    {
        return Err(AppError::bad_request(
            "已有执行工作区，不能更换厂商或凭据配置",
        ));
    }

    let executor = TofuExecutor::from_env();
    if !executor.available().await {
        let reason = "OpenTofu binary not found: install it (opentofu.org) and/or set TOFU_BINARY";
        record_failure(s, tenant, id, reason).await?;
        return Err(AppError::bad_request(reason));
    }

    let workspace_name = format!("ticket-{id}");
    let workspace = executor
        .ensure_workspace(&workspace_name)
        .map_err(AppError::bad_request)?;
    let mut effective = Map::new();
    effective.insert("configId".into(), json!(config_id));
    for (api, key) in [
        ("imageId", "image_id"),
        ("flavor", "ecs_type"),
        ("availabilityZone", "availability_zone"),
        ("subnetId", "subnet_id"),
        ("vpcId", "vpc_id"),
        ("securityGroups", "security_groups"),
    ] {
        if let Some(value) = spec.get(key) {
            effective.insert(api.into(), value.clone());
        }
    }
    sqlx::query("UPDATE infra_resource_ticket SET target_config=$2, cloud_category=$3, cloud_region=$4, tofu_workspace=$5, ecs_type=$6, update_time=now() WHERE id=$1 AND tenant_id=$7 AND deleted=0")
        .bind(id).bind(Value::Object(effective).to_string()).bind(&cloud_category).bind(&region).bind(&workspace_name)
        .bind(spec.get("ecs_type").and_then(Value::as_str).unwrap_or_default())
        .bind(tenant.id()).execute(&s.pool).await.map_err(|_| AppError::internal("failed to save provision parameters"))?;
    ticket["cloudCategory"] = json!(cloud_category);
    ticket["cloudRegion"] = json!(region);
    ticket["ecsType"] = spec.get("ecs_type").cloned().unwrap_or(Value::Null);
    TofuExecutor::write_file(&workspace, "main.tf", &main_tf).map_err(AppError::bad_request)?;
    TofuExecutor::write_file(
        &workspace,
        "terraform.tfvars.json",
        &render_tfvars(&spec, &region),
    )
    .map_err(AppError::bad_request)?;

    let mut log = String::new();
    let init = executor.init(&workspace).await;
    log.push_str(&init.combined_log());
    if !init.success {
        record_failure(s, tenant, id, &log).await?;
        return Err(AppError::bad_request(format!(
            "tofu init failed: {}",
            tail(&init.stderr)
        )));
    }

    let env = credential_env(&target);
    let apply = executor.apply(&env, &workspace).await;
    log.push_str(&apply.combined_log());
    if !apply.success {
        record_failure(s, tenant, id, &log).await?;
        return Err(AppError::bad_request(format!(
            "tofu apply failed: {}",
            tail(&apply.stderr)
        )));
    }

    let outputs = match executor.outputs(&workspace).await {
        Ok(outputs) => outputs,
        Err(reason) => {
            record_failure(s, tenant, id, &reason).await?;
            return Err(AppError::bad_request(reason));
        }
    };
    let tf_outputs = Value::Object(outputs.clone().into_iter().collect());
    if let Err(error) =
        upsert_cmdb_instance(s, tenant, id, &ticket, &cloud_category, &tf_outputs).await
    {
        record_failure(
            s,
            tenant,
            id,
            "云资源已开通，但 CMDB 回写失败；保留工作区后重试",
        )
        .await?;
        return Err(error);
    }
    sqlx::query(
        "UPDATE infra_resource_ticket
         SET ticket_status='pending_delivery', provisioner=$2, provision_time=$3,
             provision_details='OpenTofu apply 完成', apply_status='applied',
             apply_log=$4, tf_outputs=$5, tofu_workspace=$6, update_time=now()
         WHERE id=$1 AND tenant_id=$7 AND deleted=0",
    )
    .bind(id)
    .bind(operator)
    .bind(&now)
    .bind(truncate_log(&log))
    .bind(&tf_outputs)
    .bind(&workspace_name)
    .bind(tenant.id())
    .execute(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?;

    Ok(
        json!({"message": "OpenTofu 配置完成", "id": id, "ticketStatus": "pending_delivery", "applyStatus": "applied", "outputs": tf_outputs}),
    )
}

async fn record_failure(
    s: &InfraState,
    tenant: &TenantContext,
    id: i64,
    log: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE infra_resource_ticket SET apply_status='failed', apply_log=$2, update_time=now()
         WHERE id=$1 AND tenant_id=$3 AND deleted=0",
    )
    .bind(id)
    .bind(truncate_log(log))
    .bind(tenant.id())
    .execute(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed to record provision failure"))?;
    Ok(())
}

/// One enabled credential row for the ticket's cloud platform.
pub(crate) async fn load_platform_target(
    s: &InfraState,
    tenant: &TenantContext,
    ticket: &Value,
    config_id: Option<i64>,
) -> Result<(CloudTarget, Option<i64>), AppError> {
    if ticket.get("cloudCategory").and_then(Value::as_str) == Some(DEMO_PROVIDER)
        && config_id.is_none()
    {
        return Ok((
            CloudTarget {
                cloud_category: DEMO_PROVIDER.into(),
                provider_source: "hashicorp/null".into(),
                region: String::new(),
                access_key_id: String::new(),
                access_key_secret: String::new(),
            },
            None,
        ));
    }
    let platform_id = ticket
        .get("cloudPlatformId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let rows = sqlx::query(
        "SELECT id, provider, region_name, access_key_id, access_key_secret FROM infra_cloud_provider_config
         WHERE platform_id=$1 AND tenant_id=$2 AND deleted=0 AND status IN ('active', 'enabled')
         AND ($3::bigint IS NULL OR id=$3) ORDER BY id LIMIT 2",
    )
    .bind(platform_id)
    .bind(tenant.id())
    .bind(config_id)
    .fetch_all(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed to read cloud credentials"))?;
    if rows.len() != 1 {
        return Err(AppError::bad_request(
            "请选择该云平台下唯一的已启用凭据配置",
        ));
    }
    let row = &rows[0];
    let category: String = row.get("provider");
    let source =
        provider_source(&category).ok_or_else(|| AppError::bad_request("该厂商尚无开通模板"))?;
    let region = ticket
        .get("cloudRegion")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| row.get("region_name"));
    let access_key_id = crate::open_secret(&row.get::<String, _>("access_key_id"));
    let access_key_secret = crate::open_secret(&row.get::<String, _>("access_key_secret"));
    if access_key_id.trim().is_empty() || access_key_secret.trim().is_empty() {
        return Err(AppError::bad_request("开通需要有效的 AK/SK 凭据"));
    }
    Ok((
        CloudTarget {
            cloud_category: category,
            provider_source: source.into(),
            region,
            access_key_id,
            access_key_secret,
        },
        Some(row.get("id")),
    ))
}

fn merge_provision_parameters(
    spec: &mut Map<String, Value>,
    ticket: &Value,
    payload: &Value,
) -> Result<(), AppError> {
    let stored = ticket
        .get("targetConfig")
        .and_then(Value::as_str)
        .unwrap_or("");
    let saved: Value = if stored.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(stored)
            .map_err(|_| AppError::bad_request("开通参数必须是 JSON 对象"))?
    };
    if !saved.is_object() {
        return Err(AppError::bad_request("开通参数必须是 JSON 对象"));
    }
    for (api, key) in [
        ("imageId", "image_id"),
        ("flavor", "ecs_type"),
        ("availabilityZone", "availability_zone"),
        ("subnetId", "subnet_id"),
        ("vpcId", "vpc_id"),
        ("securityGroups", "security_groups"),
    ] {
        if let Some(value) = payload
            .get(api)
            .filter(|v| !v.is_null())
            .or_else(|| saved.get(api))
        {
            spec.insert(key.into(), value.clone());
        }
    }
    Ok(())
}

/// Upsert the provisioned resource into CMDB under model code
/// `cloud_<resource_type>` (created implicitly, key `ticket_id`).
async fn upsert_cmdb_instance(
    s: &InfraState,
    tenant: &TenantContext,
    ticket_id: i64,
    ticket: &Value,
    cloud_category: &str,
    tf_outputs: &Value,
) -> Result<(), AppError> {
    if ticket.get("tenantId").and_then(Value::as_i64) != Some(tenant.id()) {
        return Err(AppError::forbidden(
            "ticket tenant does not match CMDB scope",
        ));
    }
    let resource_type = ticket
        .get("resourceType")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("ecs");
    let model_code = format!("cloud_{resource_type}");
    let mut tx = s
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start CMDB upsert"))?;
    // Match CMDB model creation's transaction lock. ON CONFLICT cannot protect
    // model codes because their current index is not unique.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("cmdb:model:{model_code}"))
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to lock provisioned model code"))?;
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM cmdb_model WHERE code=$1 AND deleted=0 ORDER BY id LIMIT 1",
    )
    .bind(&model_code)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to find provisioned CMDB model"))?;
    let model_id: i64 = match existing {
        Some(id) => id,
        None => sqlx::query_scalar("INSERT INTO cmdb_model (name, code, unique_key, creator, updater) VALUES ($1, $2, 'ticket_id', 'tofu', 'tofu') RETURNING id")
            .bind(format!("{resource_type}（云资源）")).bind(&model_code).fetch_one(&mut *tx).await
            .map_err(|_| AppError::internal("failed to create provisioned CMDB model"))?,
    };
    // Model first, then instance: same lock order as CMDB create/update/import.
    let model =
        sqlx::query("SELECT unique_key FROM cmdb_model WHERE id=$1 AND deleted=0 FOR UPDATE")
            .bind(model_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to lock provisioned CMDB model"))?
            .ok_or_else(|| AppError::not_found("provisioned CMDB model not found"))?;

    let mut attributes = Map::new();
    attributes.insert("ticket_id".to_string(), json!(ticket_id.to_string()));
    for (ticket_key, attr_key) in [
        ("ecsName", "ecs_name"),
        ("ecsType", "ecs_type"),
        ("cloudPlatformName", "cloud_platform"),
        ("cloudRegion", "cloud_region"),
        ("cpuCores", "cpu_cores"),
        ("memoryGb", "memory_gb"),
        ("resourceCount", "resource_count"),
    ] {
        if let Some(value) = ticket.get(ticket_key).filter(|v| !v.is_null()) {
            attributes.insert(attr_key.to_string(), value.clone());
        }
    }
    attributes.insert("cloud_category".to_string(), json!(cloud_category));
    attributes.insert("tf_outputs".to_string(), tf_outputs.clone());
    let unique_key: Option<String> = model.get("unique_key");
    if let Some(key) = unique_key.as_deref().filter(|key| !key.is_empty()) {
        if let Some(value) = attributes.get(key).filter(|value| {
            !value.is_null()
                && !value.as_str().is_some_and(|s| s.trim().is_empty())
                && !value.as_array().is_some_and(|a| a.is_empty())
        }) {
            let duplicate: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM cmdb_instance WHERE model_id=$1 AND deleted=0 AND attributes->>'ticket_id' IS DISTINCT FROM $2 AND attributes->$3 = $4::jsonb AND tenant_id=$5)"
            ).bind(model_id).bind(ticket_id.to_string()).bind(key).bind(value).bind(tenant.id())
                .fetch_one(&mut *tx).await.map_err(|_| AppError::internal("failed to check provisioned CMDB unique key"))?;
            if duplicate {
                return Err(AppError::bad_request(
                    "provisioned resource conflicts with the CMDB unique key",
                ));
            }
        }
    }
    let payload = Value::Object(attributes);

    let updated = sqlx::query(
        "UPDATE cmdb_instance SET attributes = $3, updater='tofu', update_time=now()
         WHERE model_id=$1 AND tenant_id=$4 AND deleted=0 AND attributes->>'ticket_id'=$2",
    )
    .bind(model_id)
    .bind(ticket_id.to_string())
    .bind(&payload)
    .bind(tenant.id())
    .execute(&mut *tx)
    .await
    .map(|result| result.rows_affected())
    .map_err(|_| AppError::internal("failed to update provisioned CMDB instance"))?;
    if updated == 0 {
        sqlx::query(
            "INSERT INTO cmdb_instance (model_id, attributes, creator, updater, tenant_id)
             VALUES ($1, $2, 'tofu', 'tofu', $3)",
        )
        .bind(model_id)
        .bind(&payload)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to insert provisioned CMDB instance"))?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit CMDB upsert"))?;
    Ok(())
}

fn truncate_log(log: &str) -> String {
    const LIMIT: usize = 60 * 1024;
    if log.len() <= LIMIT {
        return log.to_string();
    }
    let mut start = log.len() - LIMIT;
    while !log.is_char_boundary(start) {
        start += 1;
    }
    format!("...\n{}", &log[start..])
}

#[cfg(test)]
mod provision_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
    async fn concurrent_cmdb_upserts_reuse_model_and_instance() {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use std::str::FromStr;

        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
        let admin = sqlx::PgPool::connect(&url).await.unwrap();
        let schema = format!("cmdb_provision_test_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&url)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .unwrap();
        let ddl = include_str!("../../../../../sql/postgresql/0009_cmdb_core.sql")
            .split("-- Menus:")
            .next()
            .unwrap()
            .replace("public.", "");
        sqlx::raw_sql(&ddl).execute(&pool).await.unwrap();
        sqlx::raw_sql("CREATE TABLE system_tenant(id bigint PRIMARY KEY); INSERT INTO system_tenant VALUES (1), (2); CREATE TABLE infra_resource_ticket(id bigint PRIMARY KEY, deleted smallint DEFAULT 0);")
            .execute(&pool).await.unwrap();
        for migration in [
            include_str!("../../../../../sql/postgresql/0025_cmdb_instance_unique_values.sql"),
            include_str!("../../../../../sql/postgresql/0026_cmdb_tenant_isolation.sql"),
        ] {
            sqlx::raw_sql(&migration.replace("public.", ""))
                .execute(&pool)
                .await
                .unwrap();
        }
        let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
        let state = InfraState::new(
            pool.clone(),
            crate::object_storage::ObjectStorage::from_env().unwrap(),
        );
        let ticket = json!({"resourceType": "ecs", "ecsName": "test", "tenantId": 1});
        let outputs = json!({"id": "test-instance"});
        let (first, second) = tokio::join!(
            upsert_cmdb_instance(&state, &tenant, 1, &ticket, "demo", &outputs),
            upsert_cmdb_instance(&state, &tenant, 1, &ticket, "demo", &outputs),
        );
        first.unwrap();
        second.unwrap();
        let models: i64 = sqlx::query_scalar("SELECT count(*) FROM cmdb_model")
            .fetch_one(&pool)
            .await
            .unwrap();
        let instances: i64 = sqlx::query_scalar("SELECT count(*) FROM cmdb_instance")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(models, 1);
        assert_eq!(instances, 1);
        sqlx::query("UPDATE cmdb_model SET unique_key='cloud_category'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            upsert_cmdb_instance(&state, &tenant, 2, &ticket, "demo", &outputs)
                .await
                .is_err()
        );
        let instances: i64 = sqlx::query_scalar("SELECT count(*) FROM cmdb_instance")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            instances, 1,
            "unique-key conflict must roll back the upsert"
        );
        let other_tenant = TenantContext::from_persisted_id(Some(2)).unwrap();
        assert!(
            upsert_cmdb_instance(&state, &other_tenant, 1, &ticket, "demo", &outputs)
                .await
                .is_err()
        );
        let other_ticket = json!({"resourceType": "ecs", "ecsName": "other", "tenantId": 2});
        upsert_cmdb_instance(&state, &other_tenant, 3, &other_ticket, "demo", &outputs)
            .await
            .unwrap();
        let owners: Vec<i64> =
            sqlx::query_scalar("SELECT tenant_id FROM cmdb_instance ORDER BY tenant_id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(owners, vec![1, 2]);
        assert!(repository::get(&pool, &other_tenant, 1).await.is_err());
        pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }

    #[test]
    fn merges_only_allowed_parameters_and_keeps_approved_quantity() {
        let ticket = json!({"targetConfig": r#"{"imageId":"saved-image","subnetId":"saved-subnet","securityGroups":["sg-1"]}"#});
        let mut spec = Map::from_iter([("resource_count".into(), json!(2))]);
        merge_provision_parameters(&mut spec, &ticket, &json!({"imageId":"selected-image", "resourceCount":999, "accessKeySecret":"never-store"})).unwrap();
        assert_eq!(spec["image_id"], "selected-image");
        assert_eq!(spec["subnet_id"], "saved-subnet");
        assert_eq!(spec["security_groups"], json!(["sg-1"]));
        assert_eq!(spec["resource_count"], 2);
        assert!(!spec.contains_key("accessKeySecret"));
    }

    #[test]
    fn rejects_invalid_saved_parameters() {
        for raw in ["not json", "[]", "null", "123"] {
            assert!(
                merge_provision_parameters(
                    &mut Map::new(),
                    &json!({"targetConfig":raw}),
                    &json!({})
                )
                .is_err()
            );
        }
    }

    #[test]
    fn truncates_unicode_provider_logs_without_panicking() {
        let log = format!("{}x", "云".repeat(30_000));
        let truncated = truncate_log(&log);
        assert!(truncated.ends_with('x'));
        assert!(truncated.len() <= 60 * 1024 + 4);
    }
}

fn tail(text: &str) -> String {
    text.lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}
