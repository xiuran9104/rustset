mod repository;

use crate::{
    InfraState, QueryParams, TableSpec, id_param, tenant_soft_delete, tenant_table_create,
    tenant_table_list, tenant_table_page, tenant_table_update,
};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use chrono::Utc;
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_tofu::{
    CloudTarget, DEMO_PROVIDER, TofuExecutor, credential_env, provider_source, render_main_tf,
    render_tfvars,
};
use rustset_framework_web::AppError;
use rustset_infra_api::{
    ApproveResourceTicketRequest, CreateApprovalRuleRequest, CreateResourceTicketRequest,
    DeliverResourceTicketRequest, ProvisionResourceTicketRequest, UpdateApprovalRuleRequest,
    UpdateResourceTicketRequest,
};
use serde_json::{Map, Value, json};
use sqlx::Row;
use std::collections::HashMap;
use tracing::warn;

const TICKET: TableSpec = TableSpec {
    table: "infra_resource_ticket",
    seq: "infra_resource_ticket_seq",
};

const APPROVAL_RULE: TableSpec = TableSpec {
    table: "infra_approval_rule",
    seq: "infra_approval_rule_seq",
};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/resource-ticket/page", get(page))
        .api_route("/infra/resource-ticket/get", get(get_one))
        .api_route("/infra/resource-ticket/create", post(create))
        .api_route("/infra/resource-ticket/update", put(update))
        .api_route("/infra/resource-ticket/delete", delete(delete_one))
        .api_route("/infra/resource-ticket/delete-list", delete(delete_list))
        .api_route("/infra/resource-ticket/{id}/approve", post(approve))
        .api_route("/infra/resource-ticket/{id}/provision", post(provision))
        .api_route("/infra/resource-ticket/{id}/deliver", post(deliver))
        .api_route("/infra/approval-rule/page", get(rule_page))
        .api_route("/infra/approval-rule/list", get(rule_list))
        .api_route("/infra/approval-rule/create", post(rule_create))
        .api_route("/infra/approval-rule/update", put(rule_update))
        .api_route("/infra/approval-rule/delete", delete(rule_delete))
}

async fn page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    repository::page(&s.pool, &TenantContext::from_user(&user)?, p).await
}
async fn get_one(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(Json(ApiResponse::new(
        repository::get(&s.pool, &TenantContext::from_user(&user)?, id_param(&p)?).await?,
    )))
}

async fn create(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(request): Json<CreateResourceTicketRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let mut payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid resource ticket"))?;
    if let Some(obj) = payload.as_object_mut() {
        obj.retain(|_, value| !value.is_null());
        if let Some(Value::Bool(enabled)) = obj.get("hasSecurityProduct").cloned() {
            obj.insert(
                "hasSecurityProduct".to_string(),
                Value::Number((enabled as i32).into()),
            );
        }
        obj.entry("ticketStatus".to_string())
            .or_insert(Value::String("pending_approval".to_string()));
        let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();
        obj.entry("createTime".to_string())
            .or_insert(Value::String(now.clone()));
        obj.entry("updateTime".to_string())
            .or_insert(Value::String(now));
        obj.entry("deliveryStatus".to_string())
            .or_insert(Value::String("未交付".to_string()));
        obj.entry("ticketType".to_string())
            .or_insert(Value::String("create".to_string()));
        obj.entry("riskLevel".to_string())
            .or_insert(Value::String("normal".to_string()));
        obj.entry("approvalStage".to_string())
            .or_insert(Value::Number(1.into()));
        obj.entry("approvalTotal".to_string())
            .or_insert(Value::Number(1.into()));
        obj.entry("currentApprovalRole".to_string())
            .or_insert(Value::String("资源管理员".to_string()));
        obj.entry("createdBy".to_string())
            .or_insert(Value::String(user.username.clone()));
        obj.entry("applicantName".to_string())
            .or_insert(Value::String(user.username));
    }
    let (Json(created), inserted) = repository::create(&s.pool, &tenant, payload).await?;
    let Ok(id) = created.data.parse::<i64>() else {
        return Ok(Json(created));
    };
    if inserted && let Some(rule) = match_approval_rule(&s, &tenant, id).await? {
        apply_auto_approval(&s, &tenant, id, &rule).await?;
    }
    Ok(Json(created))
}

/// The first enabled rule whose resource type matches (empty = any) and
/// whose thresholds all cover the ticket.
async fn match_approval_rule(
    s: &InfraState,
    tenant: &TenantContext,
    ticket_id: i64,
) -> Result<Option<Value>, AppError> {
    let ticket = repository::get(&s.pool, &tenant, ticket_id).await?;
    let rows = sqlx::query(
        "SELECT id, name, resource_type, max_cpu_cores, max_memory_gb, max_resource_count, auto_provision
         FROM infra_approval_rule WHERE tenant_id=$1 AND deleted = 0 AND status = 0 ORDER BY id",
    )
    .bind(tenant.id())
    .fetch_all(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed to read approval rules"))?;
    for row in rows {
        let resource_type: String = row.get("resource_type");
        let ticket_type = ticket
            .get("resourceType")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !resource_type.is_empty() && resource_type != ticket_type {
            continue;
        }
        let within = |ticket_key: &str, column: &str| -> bool {
            let limit: Option<i32> = row.get(column);
            match limit {
                None => true,
                Some(limit) => ticket
                    .get(ticket_key)
                    .and_then(Value::as_i64)
                    .map(|value| value <= limit as i64)
                    .unwrap_or(true),
            }
        };
        if !within("cpuCores", "max_cpu_cores")
            || !within("memoryGb", "max_memory_gb")
            || !within("resourceCount", "max_resource_count")
        {
            continue;
        }
        let auto_provision: bool = row.get("auto_provision");
        return Ok(Some(json!({
            "id": row.get::<i64, _>("id"),
            "name": row.get::<String, _>("name"),
            "autoProvision": auto_provision,
        })));
    }
    Ok(None)
}

async fn apply_auto_approval(
    s: &InfraState,
    tenant: &TenantContext,
    id: i64,
    rule: &Value,
) -> Result<(), AppError> {
    let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let rule_name = rule.get("name").and_then(Value::as_str).unwrap_or("rule");
    let ticket = repository::get(&s.pool, &tenant, id).await?;
    // 新建类工单走开通/交付流水线；针对既有资源的工单（变更/停机/回收）
    // 没有开通环节，审批通过即为执行完成，并立即回写台账状态形成闭环。
    let new_status = approved_next_status(&ticket);
    sqlx::query(
        "UPDATE infra_resource_ticket
         SET ticket_status=$2, approver=$3, approve_time=$4,
             approve_comment=$5, update_time=now()
         WHERE id=$1 AND tenant_id=$6 AND deleted=0",
    )
    .bind(id)
    .bind(new_status)
    .bind(format!("auto:{rule_name}"))
    .bind(&now)
    .bind(format!("命中自动审批规则 {rule_name}"))
    .bind(tenant.id())
    .execute(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed to auto-approve"))?;
    if new_status == "delivered" {
        apply_resource_side_effect(s, tenant, &ticket).await;
        return Ok(());
    }
    if new_status == "pending_provision"
        && rule
            .get("autoProvision")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        // A failed auto-provision must not fail ticket creation; the
        // ticket stays pending_provision with the failure recorded.
        if let Err(error) =
            run_provision(s, tenant, id, &format!("auto:{rule_name}"), &json!({})).await
        {
            warn!(error = ?error, ticket = id, "auto-provision failed");
        }
    }
    Ok(())
}

async fn update(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(request): Json<UpdateResourceTicketRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let mut p = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid resource ticket update"))?;
    if let Some(object) = p.as_object_mut()
        && let Some(Value::Bool(enabled)) = object.get("hasSecurityProduct").cloned()
    {
        object.insert(
            "hasSecurityProduct".to_string(),
            Value::Number((enabled as i32).into()),
        );
    }
    repository::update(&s.pool, &TenantContext::from_user(&user)?, p).await
}
async fn delete_one(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    repository::delete(&s.pool, &TenantContext::from_user(&user)?, &[id_param(&p)?]).await
}
async fn delete_list(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let ids = p
        .get("ids")
        .ok_or_else(|| AppError::bad_request("ids are required"))?
        .split(',')
        .map(|id| id.trim().parse::<i64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::bad_request("invalid ticket IDs"))?;
    repository::delete(&s.pool, &TenantContext::from_user(&user)?, &ids).await
}

async fn approve(
    State(s): State<InfraState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    user: CurrentUser,
    Json(payload): Json<ApproveResourceTicketRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let approved = payload.approved;
    let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let ticket = repository::get(&s.pool, &tenant, id).await?;
    if ticket.get("ticketStatus").and_then(|v| v.as_str()) != Some("pending_approval") {
        return Err(AppError::bad_request("只能审批待审批状态的工单"));
    }
    // 变更/停机/回收类工单审批通过即执行完成，并回写目标资源状态；
    // 云资源新建进入开通（provision），物理资源新建直接进入待交付。
    let new_status = if !approved {
        "rejected"
    } else {
        approved_next_status(&ticket)
    };
    sqlx::query("UPDATE infra_resource_ticket SET ticket_status=$2, approver=$3, approve_time=$4, approve_comment=$5, update_time=now() WHERE id=$1 AND tenant_id=$6 AND deleted=0")
        .bind(id).bind(new_status).bind(&user.username).bind(&now).bind(payload.comment.as_deref())
        .bind(tenant.id()).execute(&s.pool).await.map_err(|_| AppError::internal("failed"))?;
    if approved && new_status == "delivered" {
        apply_resource_side_effect(&s, &tenant, &ticket).await;
    }
    Ok(Json(ApiResponse::new(
        json!({"message": if approved { "工单审批通过" } else { "工单已拒绝" }, "id": id, "ticketStatus": new_status}),
    )))
}

/// 审批通过后的流转去向：
/// - 云资源新建 → pending_provision（走 OpenTofu 开通）
/// - 物理资源新建 → pending_delivery（无开通环节，等待上架交付）
/// - 既有资源工单（变更/停机/回收）→ delivered（执行完成并回写台账）
fn approved_next_status(ticket: &Value) -> &'static str {
    let ticket_type = ticket
        .get("ticketType")
        .and_then(Value::as_str)
        .unwrap_or("create");
    if ticket_type != "create" {
        return "delivered";
    }
    if ticket.get("resourceType").and_then(Value::as_str) == Some("physical") {
        "pending_delivery"
    } else {
        "pending_provision"
    }
}

/// 闭环回写：针对既有资源的工单审批通过后，把台账行的状态推进到位。
/// - 停机：云资源 → 已停止，物理资源 → offline
/// - 回收：两类资源 → retired（已退役）
/// - 变更：只修改配置，不改状态（变更内容记录在工单上）
async fn apply_resource_side_effect(s: &InfraState, tenant: &TenantContext, ticket: &Value) {
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

async fn provision(
    State(s): State<InfraState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    user: CurrentUser,
    Json(payload): Json<ProvisionResourceTicketRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let payload = serde_json::to_value(payload)
        .map_err(|_| AppError::bad_request("invalid provision request"))?;
    run_provision(&s, &tenant, id, &user.username, &payload).await
}

/// Real provisioning: resolve cloud credentials, render the tofu workspace,
/// run init/apply, record logs/outputs on the ticket, write the result into
/// CMDB, and advance the ticket on success.
async fn run_provision(
    s: &InfraState,
    tenant: &TenantContext,
    id: i64,
    operator: &str,
    payload: &Value,
) -> Result<Json<ApiResponse<Value>>, AppError> {
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

    Ok(Json(ApiResponse::new(
        json!({"message": "OpenTofu 配置完成", "id": id, "ticketStatus": "pending_delivery", "applyStatus": "applied", "outputs": tf_outputs}),
    )))
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
        let ddl = include_str!("../../../../sql/postgresql/0009_cmdb_core.sql")
            .split("-- Menus:")
            .next()
            .unwrap()
            .replace("public.", "");
        sqlx::raw_sql(&ddl).execute(&pool).await.unwrap();
        sqlx::raw_sql("CREATE TABLE system_tenant(id bigint PRIMARY KEY); INSERT INTO system_tenant VALUES (1), (2); CREATE TABLE infra_resource_ticket(id bigint PRIMARY KEY, deleted smallint DEFAULT 0);")
            .execute(&pool).await.unwrap();
        for migration in [
            include_str!("../../../../sql/postgresql/0025_cmdb_instance_unique_values.sql"),
            include_str!("../../../../sql/postgresql/0026_cmdb_tenant_isolation.sql"),
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

async fn rule_page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(
        &s.pool,
        TenantContext::from_user(&user)?.id(),
        APPROVAL_RULE,
        p,
    )
    .await
}
async fn rule_list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(
        &s.pool,
        TenantContext::from_user(&user)?.id(),
        APPROVAL_RULE,
    )
    .await
}
async fn rule_create(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(request): Json<CreateApprovalRuleRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    validate_rule(&request)?;
    let mut payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid approval rule"))?;
    let object = payload
        .as_object_mut()
        .expect("request serializes as object");
    object.insert("creator".into(), Value::String(user.username.clone()));
    object.insert("updater".into(), Value::String(user.username.clone()));
    tenant_table_create(
        &s.pool,
        TenantContext::from_user(&user)?.id(),
        APPROVAL_RULE,
        payload,
    )
    .await
}
async fn rule_update(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(request): Json<UpdateApprovalRuleRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    validate_rule(&request.rule)?;
    let mut payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid approval rule"))?;
    payload
        .as_object_mut()
        .expect("request serializes as object")
        .insert("updater".into(), Value::String(user.username.clone()));
    tenant_table_update(
        &s.pool,
        TenantContext::from_user(&user)?.id(),
        APPROVAL_RULE,
        payload,
    )
    .await
}

fn validate_rule(rule: &CreateApprovalRuleRequest) -> Result<(), AppError> {
    if rule.name.trim().is_empty() || rule.name.chars().count() > 128 {
        return Err(AppError::bad_request("规则名称需要 1-128 个字符"));
    }
    if rule.resource_type.chars().count() > 64
        || [
            rule.max_cpu_cores,
            rule.max_memory_gb,
            rule.max_resource_count,
        ]
        .into_iter()
        .flatten()
        .any(|value| value < 0)
    {
        return Err(AppError::bad_request("规则上限必须是非负整数"));
    }
    if !matches!(rule.status, 0 | 1) {
        return Err(AppError::bad_request("规则状态必须是 0 或 1"));
    }
    Ok(())
}
async fn rule_delete(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &s.pool,
        TenantContext::from_user(&user)?.id(),
        APPROVAL_RULE.table,
        &[id_param(&p)?],
    )
    .await
}

async fn deliver(
    State(s): State<InfraState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    user: CurrentUser,
    Json(payload): Json<DeliverResourceTicketRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let ticket = repository::get(&s.pool, &tenant, id).await?;
    if ticket.get("ticketStatus").and_then(|v| v.as_str()) != Some("pending_delivery") {
        return Err(AppError::bad_request("只能交付待交付状态的工单"));
    }
    let mut tx = s
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to begin delivery"))?;
    sqlx::query("UPDATE infra_resource_ticket SET ticket_status='delivered', delivery_status='已交付', deliverer=$2, deliver_time=$3, deliver_comment=$4, update_time=now() WHERE id=$1 AND tenant_id=$5 AND deleted=0")
        .bind(id).bind(&user.username).bind(&now).bind(payload.comment.as_deref())
        .bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed"))?;
    // 闭环：新建类工单交付完成后，自动在业务资源台账落一条对应记录，
    // 与工单状态更新同事务，避免"交付了但台账没有"的半程状态。
    if ticket
        .get("ticketType")
        .and_then(Value::as_str)
        .unwrap_or("create")
        == "create"
    {
        insert_delivered_ledger_row(&mut tx, &tenant, &ticket, &user.username).await?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit delivery"))?;
    Ok(Json(ApiResponse::new(
        json!({"message": "工单交付完成", "id": id, "ticketStatus": "delivered"}),
    )))
}

/// 把一张已交付的新建工单按资源类型写进对应台账
/// （`infra_cloud_resource` 或 `infra_physical_resource`）。
async fn insert_delivered_ledger_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &TenantContext,
    ticket: &Value,
    operator: &str,
) -> Result<(), AppError> {
    let ticket_id = ticket.get("id").and_then(Value::as_i64).unwrap_or_default();
    let str_field = |key: &str| {
        ticket
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
            .to_string()
    };
    let i32_field = |key: &str| -> i32 {
        ticket
            .get(key)
            .and_then(Value::as_i64)
            .map(|value| value as i32)
            .unwrap_or(0)
    };
    let resource_type = {
        let value = str_field("resourceType");
        if value.is_empty() {
            "cloud".to_string()
        } else {
            value
        }
    };
    let ecs_status = if resource_type == "physical" {
        "available"
    } else {
        "运行中"
    };
    // 优先用工单登记的 IP，其次取 tofu 输出里的常见 IP 字段。
    let ip_address = {
        let from_ticket = str_field("ipAddress");
        if !from_ticket.is_empty() {
            from_ticket
        } else {
            ["public_ip", "ip_address", "ip"]
                .iter()
                .find_map(|key| {
                    ticket
                        .get("tfOutputs")
                        .and_then(Value::as_object)
                        .and_then(|outputs| outputs.get(*key))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default()
        }
    };
    let remark = format!("由资源工单 #{ticket_id} 交付生成");
    if resource_type == "physical" {
        sqlx::query(
            "INSERT INTO infra_physical_resource
             (id, ecs_name, ecs_status, cloud_region, cloud_category, customer_name,
              management_ip, cpu_cores, memory_gb, deployment_type,
              application_status, delivery_status, remarks, creator, updater, tenant_id)
             VALUES (nextval('infra_physical_resource_seq'), $1, $2, $3, $4, $5,
                     $6, $7, $8, 'standalone', '已批准', '已交付', $9, $10, $10, $11)",
        )
        .bind(str_field("ecsName"))
        .bind(ecs_status)
        .bind(str_field("cloudRegion"))
        .bind(str_field("cloudCategory"))
        .bind(str_field("customerName"))
        .bind(ip_address)
        .bind(i32_field("cpuCores"))
        .bind(i32_field("memoryGb"))
        .bind(&remark)
        .bind(operator)
        .bind(tenant.id())
        .execute(&mut **tx)
        .await
        .map_err(|_| {
            AppError::internal("failed to create physical ledger row for delivered ticket")
        })?;
    } else {
        sqlx::query(
            "INSERT INTO infra_cloud_resource
             (id, ecs_name, ecs_status, resource_id, cloud_region, cloud_category,
              customer_name, instance_id, ecs_type, ecs_os, cpu_cores, memory_gb,
              system_disk, system_disk_size_gb, ip_address,
              application_status, delivery_status, remarks, creator, updater, tenant_id)
             VALUES (nextval('infra_cloud_resource_seq'), $1, $2, $3, $4, $5,
                     $6, $7, $8, $9, $10, $11, $12, $13, $14,
                     '已批准', '已交付', $15, $16, $16, $17)",
        )
        .bind(str_field("ecsName"))
        .bind(ecs_status)
        .bind({
            let instance = str_field("instanceId");
            if instance.is_empty() {
                ticket_id.to_string()
            } else {
                instance
            }
        })
        .bind(str_field("cloudRegion"))
        .bind(str_field("cloudCategory"))
        .bind(str_field("customerName"))
        .bind(str_field("instanceId"))
        .bind(str_field("ecsType"))
        .bind(str_field("ecsOs"))
        .bind(i32_field("cpuCores"))
        .bind(i32_field("memoryGb"))
        .bind(str_field("systemDisk"))
        .bind(i32_field("systemDiskSizeGb"))
        .bind(ip_address)
        .bind(&remark)
        .bind(operator)
        .bind(tenant.id())
        .execute(&mut **tx)
        .await
        .map_err(|_| {
            AppError::internal("failed to create cloud ledger row for delivered ticket")
        })?;
    }
    tracing::info!(
        ticket_id,
        "delivered ticket recorded in business resource ledger"
    );
    Ok(())
}
