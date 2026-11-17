//! 外部 MCP 服务器登记与工具同步（平台作为 MCP 客户端的管理面）。
//!
//! - CRUD：`/ai/mcp-server/*`；鉴权令牌只写不读（SealingSecrets 封存）。
//! - test-connection：initialize + tools/list，不落库。
//! - sync：把远端工具 upsert 进 `ai.tools`（executor={"kind":"mcp",...}），
//!   远端已下架的工具置 status=0（保留绑定关系），回归再启用。
//! 同步来的工具经既有 tool_ids 机制绑定到聊天角色/对话，由
//! `tools::execute` 的 executor 分支真正调用外部服务器。

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use rustset_framework_database::BindIn;

use crate::{AiState, mcp_client, require};

fn seal_secret(value: &str) -> String {
    // 统一密码服务入口（信创方案 §4.4.2，同 infra-server 模式）：
    // 主密钥就绪时写 v3 认证密文，否则 v2 兼容；已封存/空值原样返回。
    match rustset_framework_gm::secrets::SealingSecrets::from_env() {
        Ok(provider) => provider.seal(value),
        Err(error) => {
            tracing::warn!(%error, "sealing provider unavailable; storing mcp token unsealed");
            value.to_owned()
        }
    }
}

pub(crate) fn open_secret(sealed: &str) -> String {
    match rustset_framework_gm::secrets::SealingSecrets::from_env() {
        Ok(provider) => provider.open(sealed),
        Err(error) => {
            tracing::warn!(%error, "sealing provider unavailable; cannot open mcp token");
            String::new()
        }
    }
}

pub fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/mcp-server/page", get(page))
        .api_route("/ai/mcp-server/get", get(get_one))
        .api_route("/ai/mcp-server/create", post(create))
        .api_route("/ai/mcp-server/update", put(update))
        .api_route("/ai/mcp-server/delete", post(delete).get(delete))
        .api_route("/ai/mcp-server/test-connection", post(test_connection))
        .api_route("/ai/mcp-server/sync", post(sync))
}

#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PageQuery {
    page_no: Option<i64>,
    page_size: Option<i64>,
    name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Save {
    id: Option<i64>,
    name: String,
    url: String,
    /// 鉴权头名，默认 Authorization
    auth_header: Option<String>,
    /// 鉴权头值（如 "Bearer xxx"）；编辑时留空/省略 = 保持不变
    auth_token: Option<String>,
    /// 1=启用 0=停用
    status: Option<i32>,
    remark: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TestRequest {
    url: String,
    auth_header: Option<String>,
    auth_token: Option<String>,
}

/// 行 → 对外 JSON（令牌只回布尔，绝不回显密文）。
fn row_value(row: &sqlx::postgres::PgRow) -> Value {
    let token: String = row.get("auth_token");
    json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "url": row.get::<String, _>("url"),
        "authHeader": row.get::<String, _>("auth_header"),
        "hasToken": !token.is_empty(),
        "status": row.get::<i32, _>("status"),
        "serverInfo": row.get::<Value, _>("server_info"),
        "toolCount": row.get::<i32, _>("tool_count"),
        "lastSyncTime": row.get::<Option<i64>, _>("last_sync_time"),
        "lastError": row.get::<String, _>("last_error"),
        "remark": row.get::<String, _>("remark"),
        "createTime": row.get::<i64, _>("create_time"),
    })
}

async fn load_server(
    state: &AiState,
    id: i64,
) -> Result<sqlx::postgres::PgRow, AppError> {
    sqlx::query("SELECT * FROM ai.mcp_server WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| AppError::internal("读取 MCP 服务器失败"))?
        .ok_or_else(|| AppError::not_found("MCP 服务器不存在"))
}

fn remote_of(row: &sqlx::postgres::PgRow) -> mcp_client::McpRemote {
    let sealed: String = row.get("auth_token");
    mcp_client::McpRemote {
        url: row.get("url"),
        auth_header: row.get("auth_header"),
        auth_token: open_secret(&sealed),
    }
}

/// 服务器名 → 工具名前缀 slug（[a-z0-9_]，最长 24）。
fn slug(name: &str) -> String {
    let mut slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    while slug.starts_with('_') {
        slug.remove(0);
    }
    if slug.is_empty() {
        slug = "srv".to_string();
    }
    let end = slug.len().min(24);
    slug[..end].trim_end_matches('_').to_string()
}

/// 远端工具名 → ai.tools 里的注册名（模型函数名字符集 [A-Za-z0-9_]）。
fn registered_tool_name(server_name: &str, tool: &str) -> String {
    let clean: String = tool
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("mcp_{}_{}", slug(server_name), clean)
}

async fn page(
    user: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:mcp-server:query")?;
    let p = q.page_no.unwrap_or(1).max(1);
    let n = q.page_size.unwrap_or(10).clamp(1, 200);
    let rows = sqlx::query(
        "SELECT * FROM ai.mcp_server WHERE ($1::text IS NULL OR UPPER(name) LIKE UPPER('%'||$1||'%')) ORDER BY id DESC LIMIT $2 OFFSET $3",
    )
    .bind(&q.name)
    .bind(n)
    .bind((p - 1) * n)
    .fetch_all(&s.pool)
    .await
    .map_err(|_| AppError::internal("读取 MCP 服务器失败"))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.mcp_server WHERE ($1::text IS NULL OR UPPER(name) LIKE UPPER('%'||$1||'%'))",
    )
    .bind(&q.name)
    .fetch_one(&s.pool)
    .await
    .map_err(|_| AppError::internal("读取 MCP 服务器失败"))?;
    Ok(Json(ApiResponse::new(json!({
        "list": rows.iter().map(row_value).collect::<Vec<_>>(),
        "total": total
    }))))
}

async fn get_one(
    user: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:mcp-server:query")?;
    let row = load_server(&s, q.id).await?;
    Ok(Json(ApiResponse::new(row_value(&row))))
}

fn validate(v: &Save) -> Result<(), AppError> {
    if v.name.trim().is_empty() || v.name.len() > 64 {
        return Err(AppError::bad_request("名称必填且不超过 64 字符"));
    }
    let url = v.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::bad_request("URL 必须以 http:// 或 https:// 开头"));
    }
    if let Some(header) = v.auth_header.as_deref() {
        if !header.is_empty()
            && !header
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(AppError::bad_request("鉴权头名只能包含字母、数字和连字符"));
        }
    }
    Ok(())
}

async fn create(
    user: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Save>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&user, "ai:mcp-server:create")?;
    validate(&v)?;
    let id = chrono::Utc::now().timestamp_micros();
    let now = chrono::Utc::now().timestamp_millis();
    let token = v.auth_token.unwrap_or_default();
    sqlx::query(
        "INSERT INTO ai.mcp_server(id,name,url,auth_header,auth_token,status,remark,create_time,update_time)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$8)",
    )
    .bind(id)
    .bind(v.name.trim())
    .bind(v.url.trim())
    .bind(v.auth_header.unwrap_or_else(|| "Authorization".to_string()))
    .bind(seal_secret(&token))
    .bind(v.status.unwrap_or(1))
    .bind(v.remark.unwrap_or_default())
    .bind(now)
    .execute(&s.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("uk_ai_mcp_server_name") {
            AppError::bad_request("同名 MCP 服务器已存在")
        } else {
            AppError::bad_request(format!("创建 MCP 服务器失败: {e}"))
        }
    })?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    user: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Save>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&user, "ai:mcp-server:update")?;
    validate(&v)?;
    let id = v.id.ok_or_else(|| AppError::bad_request("id is required"))?;
    // 令牌：填写则封存更新，留空则保持
    let sealed_new = v.auth_token.as_deref().map(seal_secret);
    let now = chrono::Utc::now().timestamp_millis();
    let row = sqlx::query(
        "UPDATE ai.mcp_server
         SET name=$2,url=$3,auth_header=$4,
             auth_token=COALESCE($5,auth_token),
             status=$6,remark=$7,update_time=$8
         WHERE id=$1",
    )
    .bind(id)
    .bind(v.name.trim())
    .bind(v.url.trim())
    .bind(v.auth_header.unwrap_or_else(|| "Authorization".to_string()))
    .bind(sealed_new)
    .bind(v.status.unwrap_or(1))
    .bind(v.remark.unwrap_or_default())
    .bind(now)
    .execute(&s.pool)
    .await
    .map_err(|_| AppError::internal("更新 MCP 服务器失败"))?;
    if row.rows_affected() == 0 {
        return Err(AppError::not_found("MCP 服务器不存在"));
    }
    mcp_client::invalidate_session(id).await;
    Ok(Json(ApiResponse::new(true)))
}

async fn delete(
    user: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&user, "ai:mcp-server:delete")?;
    let server = load_server(&s, q.id).await?;
    // 同步来的工具：被对话绑定则拒绝；否则连同服务器一起删除。
    // executor->> 为 PG-family jsonb 分支（Dameng 适配层会改写）。
    let bound: i64 = sqlx::query_scalar(
        // PG-family array predicate（Dameng 适配层会改写，同 tools.rs remove）
        "SELECT count(*) FROM ai.tools t
         WHERE t.executor->>'kind'='mcp'
           AND (t.executor->>'server_id')::bigint=$1
           AND EXISTS(SELECT 1 FROM ai.chat_conversations c WHERE t.id=ANY(c.tool_ids))",
    )
    .bind(q.id)
    .fetch_one(&s.pool)
    .await
    .map_err(|_| AppError::internal("检查工具引用失败"))?;
    if bound > 0 {
        return Err(AppError::bad_request(
            "该服务器的同步工具正在被对话使用，请先解除绑定",
        ));
    }
    sqlx::query("DELETE FROM ai.tools WHERE executor->>'kind'='mcp' AND (executor->>'server_id')::bigint=$1")
        .bind(q.id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("清理同步工具失败"))?;
    sqlx::query("DELETE FROM ai.mcp_server WHERE id=$1")
        .bind(q.id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("删除 MCP 服务器失败"))?;
    mcp_client::invalidate_session(q.id).await;
    Ok(Json(ApiResponse::new(true)))
}

async fn test_connection(
    user: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<TestRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:mcp-server:query")?;
    let remote = mcp_client::McpRemote {
        url: v.url.trim().to_string(),
        auth_header: v.auth_header.unwrap_or_else(|| "Authorization".to_string()),
        auth_token: v.auth_token.unwrap_or_default(),
    };
    // 临时负 id（不污染会话缓存；与正式 id 冲突概率可忽略——正 id 为 epoch micros > 0）
    let temp_id = -chrono::Utc::now().timestamp_micros();
    let result = mcp_client::list_tools(temp_id, &remote).await;
    mcp_client::invalidate_session(temp_id).await;
    let (server_info, tools) = result.map_err(AppError::bad_request)?;
    Ok(Json(ApiResponse::new(json!({
        "serverInfo": server_info,
        "toolCount": tools.len(),
        "tools": tools.iter().filter_map(|t| t.get("name").and_then(Value::as_str)).collect::<Vec<_>>(),
    }))))
}

/// 同步远端工具进 ai.tools。返回 {added, updated, disabled, toolCount}。
async fn sync(
    user: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:mcp-server:sync")?;
    let server = load_server(&s, q.id).await?;
    if server.get::<i32, _>("status") != 1 {
        return Err(AppError::bad_request("MCP 服务器已停用，请先启用"));
    }
    let name: String = server.get("name");
    let remote = remote_of(&server);
    let now = chrono::Utc::now().timestamp_millis();

    let outcome = mcp_client::list_tools(q.id, &remote).await;
    match outcome {
        Ok((server_info, tools)) => {
            let mut added = 0i64;
            let mut updated = 0i64;
            for tool in &tools {
                let original = match tool.get("name").and_then(Value::as_str) {
                    Some(name) => name.to_string(),
                    None => continue,
                };
                let registered = registered_tool_name(&name, &original);
                let description = format!(
                    "[MCP:{name}] {}",
                    tool.get("description").and_then(Value::as_str).unwrap_or_default()
                );
                let schema = tool.get("inputSchema").cloned().unwrap_or_else(|| {
                    json!({ "type": "object", "properties": {} })
                });
                let executor = json!({"kind":"mcp","server_id":q.id,"tool":original});
                // ai.tools 无 name 唯一约束（避免迁移期数据冲突），用先查后写 upsert；
                // 同步是管理员低频操作，竞态窗口可忽略。
                let existing: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM ai.tools WHERE name=$1",
                )
                .bind(&registered)
                .fetch_optional(&s.pool)
                .await
                .map_err(|_| AppError::internal("同步工具失败"))?;
                if existing.is_some() {
                    sqlx::query(
                        "UPDATE ai.tools SET description=$2,input_schema=$3,executor=$4,status=1,update_time=$5 WHERE name=$1",
                    )
                    .bind(&registered)
                    .bind(&description)
                    .bind(&schema)
                    .bind(&executor)
                    .bind(now)
                    .execute(&s.pool)
                    .await
                    .map_err(|e| AppError::internal(format!("同步工具失败: {e}")))?;
                    updated += 1;
                } else {
                    sqlx::query(
                        "INSERT INTO ai.tools(id,name,description,status,input_schema,executor,create_time,update_time)VALUES($1,$2,$3,1,$4,$5,$6,$6)",
                    )
                    .bind(chrono::Utc::now().timestamp_micros())
                    .bind(&registered)
                    .bind(&description)
                    .bind(&schema)
                    .bind(&executor)
                    .bind(now)
                    .execute(&s.pool)
                    .await
                    .map_err(|e| AppError::internal(format!("同步工具失败: {e}")))?;
                    added += 1;
                }
            }
            // 远端已下架：本服务器名下未出现在本次列表的工具置 status=0。
            // 空列表语义显式处理：远端工具清零时全部下架（IN () 非法，见信创空列表规则）。
            let remote_names: Vec<String> = tools
                .iter()
                .filter_map(|t| {
                    t.get("name")
                        .and_then(Value::as_str)
                        .map(|original| registered_tool_name(&name, original))
                })
                .collect();
            let disabled = if remote_names.is_empty() {
                sqlx::query(
                    "UPDATE ai.tools SET status=0, update_time=$1
                     WHERE executor->>'kind'='mcp' AND (executor->>'server_id')::bigint=$2 AND status=1",
                )
                .bind(now)
                .bind(q.id)
                .execute(&s.pool)
                .await
                .map_err(|_| AppError::internal("下架远端缺失工具失败"))?
                .rows_affected() as i64
            } else {
                let placeholders =
                    // $1=update_time、$2=server_id 已占用，名称列表从 $3 起。
                    rustset_framework_database::expand_in_placeholders(&remote_names, 3);
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE ai.tools SET status=0, update_time=$1
                     WHERE executor->>'kind'='mcp' AND (executor->>'server_id')::bigint=$2
                       AND status=1 AND NOT (name IN ({placeholders}))"
                )))
                .bind(now)
                .bind(q.id)
                .bind_in(&remote_names)
                .execute(&s.pool)
                .await
                .map_err(|_| AppError::internal("下架远端缺失工具失败"))?
                .rows_affected() as i64
            };
            let live_count = tools.len() as i32;
            sqlx::query(
                "UPDATE ai.mcp_server SET server_info=$2, tool_count=$3, last_sync_time=$4, last_error='', update_time=$4 WHERE id=$1",
            )
            .bind(q.id)
            .bind(&server_info)
            .bind(live_count)
            .bind(now)
            .execute(&s.pool)
            .await
            .map_err(|_| AppError::internal("更新同步状态失败"))?;
            Ok(Json(ApiResponse::new(json!({
                "added": added, "updated": updated,
                "disabled": disabled, "toolCount": live_count
            }))))
        }
        Err(error) => {
            let message = error.clone();
            sqlx::query(
                "UPDATE ai.mcp_server SET last_error=$2, update_time=$3 WHERE id=$1",
            )
            .bind(q.id)
            .bind(&message)
            .bind(now)
            .execute(&s.pool)
            .await
            .map_err(|_| AppError::internal("更新同步状态失败"))?;
            Err(AppError::bad_request(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_lowercase_alnum_underscore() {
        assert_eq!(slug("RustSet 平台"), "rustset");
        assert_eq!(slug("GitHub Copilot!"), "github_copilot");
        assert_eq!(slug("___"), "srv");
    }

    #[test]
    fn registered_name_stays_in_function_charset() {
        assert_eq!(
            registered_tool_name("RustSet", "asset_page"),
            "mcp_rustset_asset_page"
        );
        assert_eq!(
            registered_tool_name("Srv", "weird-tool.name"),
            "mcp_srv_weird_tool_name"
        );
    }
}
