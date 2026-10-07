mod provisioning;
mod repository;
pub(crate) use provisioning::load_platform_target;

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
use rustset_framework_web::AppError;
use rustset_infra_api::{
    ApproveResourceTicketRequest, CreateApprovalRuleRequest, CreateResourceTicketRequest,
    DeliverResourceTicketRequest, ProvisionResourceTicketRequest, UpdateApprovalRuleRequest,
    UpdateResourceTicketRequest,
};
use serde_json::{Value, json};
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
        provisioning::apply_resource_side_effect(s, tenant, &ticket).await;
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
            provisioning::run(s, tenant, id, &format!("auto:{rule_name}"), &json!({})).await
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
        provisioning::apply_resource_side_effect(&s, &tenant, &ticket).await;
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
async fn provision(
    State(s): State<InfraState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    user: CurrentUser,
    Json(payload): Json<ProvisionResourceTicketRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let payload = serde_json::to_value(payload)
        .map_err(|_| AppError::bad_request("invalid provision request"))?;
    Ok(Json(ApiResponse::new(
        provisioning::run(&s, &tenant, id, &user.username, &payload).await?,
    )))
}

/// Real provisioning: resolve cloud credentials, render the tofu workspace,
/// run init/apply, record logs/outputs on the ticket, write the result into
/// CMDB, and advance the ticket on success.
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
