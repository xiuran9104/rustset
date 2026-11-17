//! 流程实例 API：启动（快照定义展开步骤）、我的待办、步骤操作
//! （approve/reject/complete/retry）、取消、查询。

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use rustset_framework_database::BindIn;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use super::notify;
use crate::{InfraState, Page, QueryParams};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/flow-instance/page", get(page))
        .api_route("/infra/flow-instance/get", get(get_one))
        .api_route("/infra/flow-instance/create", post(create))
        .api_route("/infra/flow-instance/cancel", post(cancel))
        .api_route("/infra/flow-instance/todo", get(todo))
        .api_route(
            "/infra/flow-instance/{id}/step/{step_key}/approve",
            post(step_approve),
        )
        .api_route(
            "/infra/flow-instance/{id}/step/{step_key}/reject",
            post(step_reject),
        )
        .api_route(
            "/infra/flow-instance/{id}/step/{step_key}/complete",
            post(step_complete),
        )
        .api_route(
            "/infra/flow-instance/{id}/step/{step_key}/retry",
            post(step_retry),
        )
}

#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StartRequest {
    definition_id: i64,
    title: Option<String>,
    payload: Option<Value>,
    biz_type: Option<String>,
    biz_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StepActionRequest {
    comment: Option<String>,
    /// manual 完成时的执行结果（可选 JSON）
    result: Option<Value>,
}

async fn page(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let pn = params.page_no.unwrap_or(1).max(1);
    let ps = params.page_size.unwrap_or(10).clamp(1, 200);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_flow_instance WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count flow instances"))?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(i) || jsonb_build_object(
             'definitionName', d.name,
             'currentStepName', (
                 SELECT s.name FROM infra_flow_step s
                 WHERE s.instance_id=i.id AND s.tenant_id=i.tenant_id
                   AND s.status='active' ORDER BY s.step_index LIMIT 1
             )
         ) - 'tenant_id' - 'deleted' - 'definition_snapshot' - 'context'
         FROM infra_flow_instance i
         LEFT JOIN infra_flow_definition d ON d.id=i.definition_id AND d.tenant_id=i.tenant_id
         WHERE i.tenant_id=$1 AND i.deleted=0
         ORDER BY i.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(tenant.id())
    .bind(ps)
    .bind((pn - 1) * ps)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list flow instances"))?;
    Ok(Json(ApiResponse::new(Page { list, total })))
}

async fn get_one(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let instance = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(i) || jsonb_build_object('definitionName', d.name) - 'tenant_id' - 'deleted'
         FROM infra_flow_instance i
         LEFT JOIN infra_flow_definition d ON d.id=i.definition_id AND d.tenant_id=i.tenant_id
         WHERE i.id=$1 AND i.tenant_id=$2 AND i.deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read flow instance"))?
    .ok_or_else(|| AppError::not_found("flow instance not found"))?;
    let steps = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(s) - 'tenant_id'
         FROM infra_flow_step s
         WHERE s.instance_id=$1 AND s.tenant_id=$2
         ORDER BY s.step_index",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read flow steps"))?;
    Ok(Json(ApiResponse::new(json!({
        "instance": instance,
        "steps": steps,
    }))))
}

/// 我的待办：当前用户命中 assignee 的 active approval/manual 步骤。
async fn todo(
    user: CurrentUser,
    State(state): State<InfraState>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let numeric_id =
        notify::current_user_numeric_id(&state.pool, &user.user_id).await;
    let rows = sqlx::query(
        "SELECT s.assignee, s.id, s.step_key, s.step_index, s.name, s.type, s.config,
                s.timeout_at, i.id AS instance_id, i.title, i.payload
         FROM infra_flow_step s
         JOIN infra_flow_instance i ON i.id=s.instance_id AND i.tenant_id=s.tenant_id
              AND i.status='running' AND i.deleted=0
         WHERE s.tenant_id=$1 AND s.status='active' AND s.type IN ('approval','manual')
         ORDER BY s.step_index, s.id",
    )
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list todo steps"))?;
    let mut list = Vec::new();
    for row in rows {
        let assignee: Value = row.get("assignee");
        if !notify::assignee_matches(&user, numeric_id, &assignee) {
            continue;
        }
        list.push(json!({
            "stepId": row.get::<i64, _>("id"),
            "stepKey": row.get::<String, _>("step_key"),
            "stepIndex": row.get::<i32, _>("step_index"),
            "stepName": row.get::<String, _>("name"),
            "type": row.get::<String, _>("type"),
            "timeoutAt": row.get::<Option<chrono::NaiveDateTime>, _>("timeout_at")
                .map(|t| t.to_string()),
            "instanceId": row.get::<i64, _>("instance_id"),
            "title": row.get::<String, _>("title"),
            "payload": row.get::<Value, _>("payload"),
        }));
    }
    Ok(Json(ApiResponse::new(list)))
}

/// 启动流程实例内核（HTTP handler 与内部钩子共用）。
/// 展开步骤（condition 并入 config，next 存独立列），首轮推进。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_instance(
    state: &InfraState,
    tenant: &TenantContext,
    definition_id: i64,
    definition_name: &str,
    definition_version: i32,
    definition_body: &Value,
    title: Option<String>,
    payload: Value,
    biz_type: &str,
    biz_id: &str,
    starter: &str,
) -> Result<i64, AppError> {
    let title = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| definition_name.to_string());
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to begin transaction"))?;
    let instance_id: i64 = sqlx::query_scalar(
        "INSERT INTO infra_flow_instance
             (definition_id, definition_snapshot, title, biz_type, biz_id, payload, starter, tenant_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(definition_id)
    .bind(json!({ "name": definition_name, "version": definition_version, "steps": definition_body.get("steps") }))
    .bind(&title)
    .bind(biz_type)
    .bind(biz_id)
    .bind(&payload)
    .bind(starter)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::bad_request(format!("创建流程实例失败: {e}")))?;

    let steps = definition_body
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, step) in steps.iter().enumerate() {
        // condition 并入 config 存储（步骤表无独立 condition 列），
        // engine 推进时从 config.condition 求值。
        let mut config = step
            .get("config")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Some(condition) = step.get("condition").filter(|c| !c.is_null()) {
            if let Some(object) = config.as_object_mut() {
                object.insert("condition".to_string(), condition.clone());
            }
        }
        sqlx::query(
            "INSERT INTO infra_flow_step
                 (instance_id, step_key, step_index, name, type, status, config, assignee, next, tenant_id)
             VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8, $9)",
        )
        .bind(instance_id)
        .bind(step.get("key").and_then(Value::as_str).unwrap_or_default())
        .bind(index as i32)
        .bind(step.get("name").and_then(Value::as_str).unwrap_or_default())
        .bind(step.get("type").and_then(Value::as_str).unwrap_or_default())
        .bind(&config)
        .bind(
            step.get("config")
                .and_then(|c| c.get("assignee"))
                .cloned()
                .unwrap_or_else(|| json!({})),
        )
        .bind(step.get("next").cloned().unwrap_or_else(|| json!([])))
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::bad_request(format!("展开流程步骤失败: {e}")))?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit instance"))?;
    Ok(instance_id)
}

async fn create(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<StartRequest>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let definition = sqlx::query(
        "SELECT name, definition, version FROM infra_flow_definition
         WHERE id=$1 AND tenant_id=$2 AND deleted=0 AND status=0",
    )
    .bind(request.definition_id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read flow definition"))?
    .ok_or_else(|| AppError::bad_request("流程定义不存在或已停用"))?;

    let instance_id = start_instance(
        &state,
        &tenant,
        request.definition_id,
        definition.get("name"),
        definition.get("version"),
        &(definition.get("definition")),
        request.title.clone(),
        request.payload.clone().unwrap_or_else(|| json!({})),
        request.biz_type.as_deref().unwrap_or(""),
        request.biz_id.as_deref().unwrap_or(""),
        &user.username,
    )
    .await?;

    // 首轮推进（激活/跳过/执行 auto/notification，直到等待审批）。
    super::engine::advance(&state, &tenant, instance_id).await;
    Ok(Json(ApiResponse::new(instance_id)))
}

async fn cancel(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let result = sqlx::query(
        "UPDATE infra_flow_instance
         SET status='cancelled', finish_time=now(), update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND status='running' AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to cancel flow instance"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::bad_request("实例不存在或已结束"));
    }
    sqlx::query(
        "UPDATE infra_flow_step SET status='cancelled', finished_at=now()
         WHERE instance_id=$1 AND tenant_id=$2 AND status IN ('pending','active')",
    )
    .bind(params.id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to cancel flow steps"))?;
    let _ = user;
    Ok(Json(ApiResponse::new(())))
}

/// 定位 active 步骤并校验操作人命中 assignee。
async fn load_active_step(
    state: &InfraState,
    tenant: &TenantContext,
    user: &CurrentUser,
    instance_id: i64,
    step_key: &str,
) -> Result<(i64, String, Value), AppError> {
    let row = sqlx::query(
        "SELECT id, name, type, config, assignee FROM infra_flow_step
         WHERE instance_id=$1 AND step_key=$2 AND tenant_id=$3 AND status='active'",
    )
    .bind(instance_id)
    .bind(step_key)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read step"))?
    .ok_or_else(|| AppError::bad_request("步骤不存在或不处于待处理状态"))?;
    let step_type: String = row.get("type");
    let config: Value = row.get("config");
    let assignee: Value = row.get("assignee");
    let numeric_id = notify::current_user_numeric_id(&state.pool, &user.user_id).await;
    if !notify::assignee_matches(user, numeric_id, &assignee) {
        return Err(AppError::forbidden("你不是该步骤的处理人"));
    }
    Ok((row.get("id"), step_type, config))
}

async fn step_approve(
    user: CurrentUser,
    State(state): State<InfraState>,
    Path((instance_id, step_key)): Path<(i64, String)>,
    body: Option<Json<StepActionRequest>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let comment = body.map(|Json(b)| b.comment.unwrap_or_default()).unwrap_or_default();
    let (step_id, step_type, config) =
        load_active_step(&state, &tenant, &user, instance_id, &step_key).await?;
    if step_type != "approval" {
        return Err(AppError::bad_request("该步骤不是审批步骤"));
    }
    // 会签：multi=all 时所有 assignee 都通过才 done；否则（any/默认）首人通过即 done。
    let multi_all = config
        .get("assignee")
        .and_then(|a| a.get("multi"))
        .and_then(Value::as_str)
        == Some("all");
    if multi_all {
        let row = sqlx::query(
            "SELECT result, assignee FROM infra_flow_step WHERE id=$1 AND tenant_id=$2",
        )
        .bind(step_id)
        .bind(tenant.id())
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed to read step for countersign"))?;
        let mut result: Value = row.get("result");
        let assignee: Value = row.get("assignee");
        let mut approvers: Vec<String> = result
            .get("approvers")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if !approvers.contains(&user.username) {
            approvers.push(user.username.clone());
        }
        // 期望集合：assignee 解析出的数字 id → 用户名。
        let recipient_ids = notify::resolve_recipients(&state.pool, tenant.id(), &assignee).await;
        let expected_names: Vec<String> = if recipient_ids.is_empty() {
            vec![user.username.clone()] // 兜底：无收件人时操作人自己即全员
        } else {
            let placeholders = rustset_framework_database::expand_in_placeholders(
                &recipient_ids,
                1,
            );
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT username FROM system_users WHERE id IN ({placeholders}) AND deleted=0"
            )))
            .bind_in(&recipient_ids)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default()
        };
        let all_approved = expected_names
            .iter()
            .all(|name| approvers.contains(name));
        result["approvers"] = json!(approvers);
        if all_approved {
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status='done', finished_at=now(), operator=$3, comment=$4, result=$5
                 WHERE id=$1 AND tenant_id=$2",
            )
            .bind(step_id)
            .bind(tenant.id())
            .bind(approvers.join(","))
            .bind(&comment)
            .bind(&result)
            .execute(&state.pool)
            .await
            .map_err(|_| AppError::internal("failed to approve step"))?;
        } else {
            // 未全员通过：记录 approvers，步骤保持 active，不推进。
            sqlx::query(
                "UPDATE infra_flow_step SET result=$3 WHERE id=$1 AND tenant_id=$2",
            )
            .bind(step_id)
            .bind(tenant.id())
            .bind(&result)
            .execute(&state.pool)
            .await
            .map_err(|_| AppError::internal("failed to record approver"))?;
            return Ok(Json(ApiResponse::new(())));
        }
    } else {
        sqlx::query(
            "UPDATE infra_flow_step
             SET status='done', finished_at=now(), operator=$3, comment=$4
             WHERE id=$1 AND tenant_id=$2",
        )
        .bind(step_id)
        .bind(tenant.id())
        .bind(&user.username)
        .bind(&comment)
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed to approve step"))?;
    }
    super::engine::advance(&state, &tenant, instance_id).await;
    Ok(Json(ApiResponse::new(())))
}

async fn step_reject(
    user: CurrentUser,
    State(state): State<InfraState>,
    Path((instance_id, step_key)): Path<(i64, String)>,
    body: Option<Json<StepActionRequest>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let comment = body.map(|Json(b)| b.comment).flatten().unwrap_or_default();
    if comment.trim().is_empty() {
        return Err(AppError::bad_request("拒绝必须填写原因"));
    }
    let (step_id, step_type, _) =
        load_active_step(&state, &tenant, &user, instance_id, &step_key).await?;
    if step_type != "approval" {
        return Err(AppError::bad_request("该步骤不是审批步骤"));
    }
    sqlx::query(
        "UPDATE infra_flow_step
         SET status='failed', finished_at=now(), operator=$3, comment=$4
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(step_id)
    .bind(tenant.id())
    .bind(&user.username)
    .bind(&comment)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to reject step"))?;
    // 拒绝 = 实例失败收尾（engine 把 failed 步骤视为终态失败）。
    sqlx::query(
        "UPDATE infra_flow_step SET status='cancelled', finished_at=now()
         WHERE instance_id=$1 AND tenant_id=$2 AND status IN ('pending','active') AND id<>$3",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .bind(step_id)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to cancel remaining steps"))?;
    sqlx::query(
        "UPDATE infra_flow_instance
         SET status='failed', finish_time=now(), update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND status='running'",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to finish instance"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn step_complete(
    user: CurrentUser,
    State(state): State<InfraState>,
    Path((instance_id, step_key)): Path<(i64, String)>,
    body: Option<Json<StepActionRequest>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let Json(body) = body.unwrap_or(Json(StepActionRequest {
        comment: None,
        result: None,
    }));
    let (step_id, step_type, _) =
        load_active_step(&state, &tenant, &user, instance_id, &step_key).await?;
    if step_type != "manual" && step_type != "approval" {
        return Err(AppError::bad_request("该步骤不支持人工完成"));
    }
    sqlx::query(
        "UPDATE infra_flow_step
         SET status='done', finished_at=now(), operator=$3, comment=$4,
             result=result || $5
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(step_id)
    .bind(tenant.id())
    .bind(&user.username)
    .bind(body.comment.unwrap_or_default())
    .bind(json!({ "manualResult": body.result.unwrap_or(Value::Null) }))
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to complete step"))?;
    super::engine::advance(&state, &tenant, instance_id).await;
    Ok(Json(ApiResponse::new(())))
}

/// 重试失败的 auto 步骤（管理员排障用）：重新激活并立即执行。
async fn step_retry(
    user: CurrentUser,
    State(state): State<InfraState>,
    Path((instance_id, step_key)): Path<(i64, String)>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let row = sqlx::query(
        "SELECT id FROM infra_flow_step
         WHERE instance_id=$1 AND step_key=$2 AND tenant_id=$3 AND status='failed'",
    )
    .bind(instance_id)
    .bind(&step_key)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read step"))?
    .ok_or_else(|| AppError::bad_request("步骤不存在或未处于失败状态"))?;
    // 重开实例 + 步骤，attempt 清零重新执行。
    sqlx::query(
        "UPDATE infra_flow_instance
         SET status='running', finish_time=NULL, update_time=now()
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to reopen instance"))?;
    sqlx::query(
        "UPDATE infra_flow_step
         SET status='active', attempt_count=0, retry_at=now(), finished_at=NULL
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(row.get::<i64, _>("id"))
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to reactivate step"))?;
    super::engine::advance(&state, &tenant, instance_id).await;
    let _ = user;
    Ok(Json(ApiResponse::new(())))
}
