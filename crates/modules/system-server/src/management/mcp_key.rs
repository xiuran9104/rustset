//! 对外 MCP 服务 API Key 管理：创建后明文仅返回一次，库里只存 SHA-256。

use schemars::JsonSchema;
use std::collections::HashMap;

use aide::axum::ApiRouter;
use axum::{
    Json,
    extract::{Query, State},
};
use rand::Rng;
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    SystemState,
    management::shared::require,
    mcp,
};

pub fn routes() -> ApiRouter<SystemState> {
    ApiRouter::new()
        .api_route(
            "/system/mcp-key/page",
            aide::axum::routing::get(page),
        )
        .api_route(
            "/system/mcp-key/get",
            aide::axum::routing::get(get_one),
        )
        .api_route(
            "/system/mcp-key/create",
            aide::axum::routing::post(create),
        )
        .api_route(
            "/system/mcp-key/update",
            aide::axum::routing::put(update),
        )
        .api_route(
            "/system/mcp-key/delete",
            aide::axum::routing::delete(delete),
        )
}

#[derive(Debug, Serialize, JsonSchema)]
struct Page<T> {
    list: Vec<T>,
    total: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryParams {
    #[serde(default, rename = "pageNo")]
    page_no: Option<i64>,
    #[serde(default, rename = "pageSize")]
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateRequest {
    name: String,
    user_id: i64,
    /// 可选过期时间（yyyy-MM-dd HH:mm:ss，本地时间）
    expires_time: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateRequest {
    id: i64,
    name: Option<String>,
    enabled: Option<i16>,
}

/// 生成 mcp_ + 43 位 base62（≈256bit）密钥明文。
fn generate_api_key() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let body: String = (0..43)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    format!("mcp_{body}")
}

/// 绑定用户必须存在且启用，避免生成出永远无法通过鉴权的密钥；
/// 返回该用户的 tenant_id（密钥归属租户，随插入落库）。
async fn ensure_user_bindable(state: &SystemState, user_id: i64) -> Result<i64, AppError> {
    let row = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT tenant_id FROM system_users WHERE id = $1 AND deleted = 0 AND status = 0",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read bound user"))?;
    row.ok_or_else(|| AppError::bad_request("绑定用户不存在或已停用"))?
        .ok_or_else(|| AppError::bad_request("绑定用户缺少租户归属"))
}

fn parse_expires(raw: Option<&String>) -> Result<Option<chrono::NaiveDateTime>, AppError> {
    match raw.map(String::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(text) => chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
            .map(Some)
            .map_err(|_| AppError::bad_request("expiresTime 格式应为 yyyy-MM-dd HH:mm:ss")),
    }
}

async fn page(
    user: CurrentUser,
    State(state): State<SystemState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Page<Value>>>, AppError> {
    require(&user, "system:mcp-key:query")?;
    let page_no = params.page_no.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (page_no - 1) * page_size;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM system_mcp_api_key WHERE deleted = 0",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count mcp api keys"))?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object(
             'id', k.id,
             'name', k.name,
             'keyPrefix', k.key_prefix,
             'userId', k.user_id,
             'username', u.username,
             'nickname', u.nickname,
             'enabled', k.enabled,
             'expiresTime', k.expires_time,
             'lastUsedTime', k.last_used_time,
             'createTime', k.create_time
         )
         FROM system_mcp_api_key k
         LEFT JOIN system_users u ON u.id = k.user_id
         WHERE k.deleted = 0
         ORDER BY k.id DESC
         LIMIT $1 OFFSET $2",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list mcp api keys"))?;
    Ok(Json(ApiResponse::new(Page { list, total })))
}

async fn get_one(
    user: CurrentUser,
    State(state): State<SystemState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "system:mcp-key:query")?;
    let id = params
        .get("id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let row = sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object(
             'id', k.id,
             'name', k.name,
             'keyPrefix', k.key_prefix,
             'userId', k.user_id,
             'username', u.username,
             'nickname', u.nickname,
             'enabled', k.enabled,
             'expiresTime', k.expires_time,
             'lastUsedTime', k.last_used_time,
             'createTime', k.create_time
         )
         FROM system_mcp_api_key k
         LEFT JOIN system_users u ON u.id = k.user_id
         WHERE k.id = $1 AND k.deleted = 0",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read mcp api key"))?
    .ok_or_else(|| AppError::not_found("mcp api key not found"))?;
    Ok(Json(ApiResponse::new(row)))
}

async fn create(
    user: CurrentUser,
    State(state): State<SystemState>,
    Json(request): Json<CreateRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "system:mcp-key:create")?;
    let name = request.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::bad_request("名称必填且不超过 64 字符"));
    }
    let tenant_id = ensure_user_bindable(&state, request.user_id).await?;
    let expires = parse_expires(request.expires_time.as_ref())?;
    let api_key = generate_api_key();
    let hash = mcp::sha256_hex(&api_key);
    let prefix: String = api_key.chars().take(11).collect();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO system_mcp_api_key
             (name, api_key_hash, key_prefix, user_id, enabled, expires_time, creator, updater, tenant_id)
         VALUES ($1, $2, $3, $4, 1, $5, $6, $6, $7)
         RETURNING id",
    )
    .bind(name)
    .bind(&hash)
    .bind(&prefix)
    .bind(request.user_id)
    .bind(expires)
    .bind(&user.username)
    .bind(tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to create mcp api key"))?;
    // 明文仅此一次返回；此后系统内只有哈希。
    Ok(Json(ApiResponse::new(json!({ "id": id, "apiKey": api_key }))))
}

async fn update(
    user: CurrentUser,
    State(state): State<SystemState>,
    Json(request): Json<UpdateRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "system:mcp-key:update")?;
    if let Some(enabled) = request.enabled {
        if enabled != 0 && enabled != 1 {
            return Err(AppError::bad_request("enabled 仅允许 0/1"));
        }
    }
    if let Some(name) = request.name.as_deref() {
        let name = name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(AppError::bad_request("名称必填且不超过 64 字符"));
        }
    }
    let result = sqlx::query(
        "UPDATE system_mcp_api_key
         SET name = COALESCE($2, name),
             enabled = COALESCE($3, enabled),
             updater = $4,
             update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(request.id)
    .bind(request.name.as_deref().map(str::trim))
    .bind(request.enabled)
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to update mcp api key"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("mcp api key not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete(
    user: CurrentUser,
    State(state): State<SystemState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "system:mcp-key:delete")?;
    let id = params
        .get("id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let result = sqlx::query(
        "UPDATE system_mcp_api_key SET deleted = 1, updater = $2, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(id)
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to delete mcp api key"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("mcp api key not found"));
    }
    Ok(Json(ApiResponse::new(())))
}
