//! AI Agent 预设 CRUD：可复用的 AI 代理配置（模型+提示词+工具+知识库）。

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post, put};
use axum::{Json, extract::{Query, State}};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::{AiState, require};

pub fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/agent-preset/page", get(page))
        .api_route("/ai/agent-preset/get", get(get_one))
        .api_route("/ai/agent-preset/simple-list", get(simple_list))
        .api_route("/ai/agent-preset/create", post(create))
        .api_route("/ai/agent-preset/update", put(update))
        .api_route("/ai/agent-preset/delete", get(delete))
}

#[derive(Deserialize, JsonSchema)]
struct Id { id: i64 }

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PageQuery {
    page_no: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Save {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    model_id: i64,
    system_prompt: Option<String>,
    tool_ids: Option<Vec<i64>>,
    knowledge_ids: Option<Vec<i64>>,
    temperature: Option<f64>,
    max_tokens: Option<i32>,
    status: Option<i16>,
}

fn row_value(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "description": row.get::<String, _>("description"),
        "modelId": row.get::<i64, _>("model_id"),
        "systemPrompt": row.get::<String, _>("system_prompt"),
        "toolIds": row.get::<Value, _>("tool_ids"),
        "knowledgeIds": row.get::<Value, _>("knowledge_ids"),
        "temperature": row.get::<f64, _>("temperature"),
        "maxTokens": row.get::<i32, _>("max_tokens"),
        "status": row.get::<i16, _>("status"),
    })
}

const SELECT_COLS: &str = "id,name,description,model_id,system_prompt,tool_ids,knowledge_ids,temperature,max_tokens,status";

async fn page(
    user: CurrentUser, State(s): State<AiState>, Query(q): Query<PageQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:agent-preset:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let pn = q.page_no.unwrap_or(1).max(1);
    let ps = q.page_size.unwrap_or(10).clamp(1, 200);
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM ai_agent_preset WHERE tenant_id=$1 AND deleted=0 ORDER BY id DESC LIMIT $2 OFFSET $3"
    )))
    .bind(tenant.id()).bind(ps).bind((pn-1)*ps)
    .fetch_all(&s.pool).await
    .map_err(|_| AppError::internal("查询 Agent 预设失败"))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_agent_preset WHERE tenant_id=$1 AND deleted=0")
    .bind(tenant.id()).fetch_one(&s.pool).await
    .map_err(|_| AppError::internal("查询 Agent 预设失败"))?;
    Ok(Json(ApiResponse::new(json!({"list": rows.iter().map(row_value).collect::<Vec<_>>(), "total": total}))))
}

async fn simple_list(
    user: CurrentUser, State(s): State<AiState>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "ai:agent-preset:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM ai_agent_preset WHERE tenant_id=$1 AND deleted=0 AND status=0 ORDER BY id"
    )))
    .bind(tenant.id()).fetch_all(&s.pool).await
    .map_err(|_| AppError::internal("查询 Agent 预设失败"))?;
    Ok(Json(ApiResponse::new(rows.iter().map(row_value).collect())))
}

async fn get_one(
    user: CurrentUser, State(s): State<AiState>, Query(q): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:agent-preset:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM ai_agent_preset WHERE id=$1 AND tenant_id=$2 AND deleted=0"
    )))
    .bind(q.id).bind(tenant.id())
    .fetch_optional(&s.pool).await
    .map_err(|_| AppError::internal("查询 Agent 预设失败"))?
    .ok_or_else(|| AppError::not_found("Agent 预设不存在"))?;
    Ok(Json(ApiResponse::new(row_value(&row))))
}

async fn create(
    user: CurrentUser, State(s): State<AiState>, Json(v): Json<Save>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&user, "ai:agent-preset:create")?;
    let tenant = TenantContext::from_user(&user)?;
    let name = v.name.trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("名称必填且不超过 128 字符"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO ai_agent_preset (name,description,model_id,system_prompt,tool_ids,knowledge_ids,temperature,max_tokens,status,creator,updater,tenant_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10,$11) RETURNING id")
    .bind(name).bind(v.description.unwrap_or_default())
    .bind(v.model_id).bind(v.system_prompt.unwrap_or_default())
    .bind(json!(v.tool_ids.unwrap_or_default()))
    .bind(json!(v.knowledge_ids.unwrap_or_default()))
    .bind(v.temperature.unwrap_or(0.7))
    .bind(v.max_tokens.unwrap_or(4096))
    .bind(v.status.unwrap_or(0))
    .bind(&user.username).bind(tenant.id())
    .fetch_one(&s.pool).await
    .map_err(|e| AppError::bad_request(format!("创建 Agent 预设失败: {e}")))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    user: CurrentUser, State(s): State<AiState>, Json(v): Json<Save>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "ai:agent-preset:create")?;
    let tenant = TenantContext::from_user(&user)?;
    let id = v.id.ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = v.name.trim();
    if name.is_empty() { return Err(AppError::bad_request("名称不能为空")); }
    let r = sqlx::query(
        "UPDATE ai_agent_preset SET name=$2,description=$3,model_id=$4,system_prompt=$5,
         tool_ids=$6,knowledge_ids=$7,temperature=$8,max_tokens=$9,status=$10,updater=$11,update_time=now()
         WHERE id=$1 AND tenant_id=$12 AND deleted=0")
    .bind(id).bind(name).bind(v.description.unwrap_or_default())
    .bind(v.model_id).bind(v.system_prompt.unwrap_or_default())
    .bind(json!(v.tool_ids.unwrap_or_default()))
    .bind(json!(v.knowledge_ids.unwrap_or_default()))
    .bind(v.temperature.unwrap_or(0.7))
    .bind(v.max_tokens.unwrap_or(4096))
    .bind(v.status.unwrap_or(0))
    .bind(&user.username).bind(tenant.id())
    .execute(&s.pool).await
    .map_err(|_| AppError::internal("更新 Agent 预设失败"))?;
    if r.rows_affected() == 0 { return Err(AppError::not_found("Agent 预设不存在")); }
    Ok(Json(ApiResponse::new(())))
}

async fn delete(
    user: CurrentUser, State(s): State<AiState>, Query(q): Query<Id>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "ai:agent-preset:create")?;
    let tenant = TenantContext::from_user(&user)?;
    let r = sqlx::query(
        "UPDATE ai_agent_preset SET deleted=1,updater=$3,update_time=now() WHERE id=$1 AND tenant_id=$2 AND deleted=0")
    .bind(q.id).bind(tenant.id()).bind(&user.username)
    .execute(&s.pool).await
    .map_err(|_| AppError::internal("删除 Agent 预设失败"))?;
    if r.rows_affected() == 0 { return Err(AppError::not_found("Agent 预设不存在")); }
    Ok(Json(ApiResponse::new(())))
}
