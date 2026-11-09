//! 对外 MCP（Model Context Protocol）服务：Streamable HTTP 传输。
//!
//! - `POST /mcp`：单条 JSON-RPC 2.0；响应纯 JSON；notification 返回 202。
//! - `GET /mcp`：405（无服务端推送流；工具均为请求-响应型）。
//! - 无状态实现：不分配 Mcp-Session-Id。
//! - 鉴权：`Authorization: Bearer mcp_...`（兼容 `X-Api-Key`），所有请求都要求
//!   有效密钥；权限与租户隔离继承密钥绑定用户（见 system-server::mcp）。

mod jsonrpc;
mod tools;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, post},
    Router,
};
use rustset_ai_server::AiState;
use rustset_framework_database::DbPool;
use rustset_framework_security::CurrentUser;
use rustset_system_server::SystemState;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct McpState {
    pub pool: DbPool,
    system: SystemState,
    ai: AiState,
}

impl McpState {
    pub fn new(pool: DbPool, system: SystemState, ai: AiState) -> Self {
        Self { pool, system, ai }
    }
}

pub fn routes(state: McpState) -> Router {
    Router::new()
        .route("/mcp", post(handle_post).get(method_not_allowed).delete(method_not_allowed))
        .route("/mcp/", any(method_not_allowed))
        .with_state(state)
}

async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST")],
        Json(json!({
            "jsonrpc": "2.0", "id": null,
            "error": { "code": jsonrpc::INVALID_REQUEST, "message": "method not allowed; MCP streamable HTTP endpoint accepts POST only" }
        })),
    )
        .into_response()
}

/// 从 Authorization: Bearer 或 X-Api-Key 提取 MCP 密钥。
fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(authorization) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = authorization.strip_prefix("Bearer ") {
            if !token.trim().is_empty() {
                return Some(token.trim().to_string());
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer realm=\"rustset-mcp\"")],
        Json(json!({
            "jsonrpc": "2.0", "id": null,
            "error": { "code": jsonrpc::INTERNAL_ERROR, "message": "unauthorized: missing or invalid MCP API key" }
        })),
    )
        .into_response()
}

async fn handle_post(
    State(state): State<McpState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let Some(api_key) = extract_api_key(&headers) else {
        return unauthorized_response();
    };
    let user = match rustset_system_server::authenticate_mcp_key(&state.system, &api_key).await {
        Ok(user) => user,
        Err(_) => return unauthorized_response(),
    };
    let Some(Json(body)) = body else {
        return json_error_response(
            StatusCode::BAD_REQUEST,
            Value::Null,
            jsonrpc::PARSE_ERROR,
            "empty request body",
        );
    };
    dispatch(&state, &user, body).await
}

/// 核心分发：鉴权后可独立测试。
async fn dispatch(state: &McpState, user: &CurrentUser, body: Value) -> Response {
    let message = match jsonrpc::parse(&body) {
        Ok(message) => message,
        Err((code, message)) => {
            return json_error_response(StatusCode::BAD_REQUEST, Value::Null, code, &message)
        }
    };
    if jsonrpc::is_notification(&message) {
        // notifications/* 与其它无 id 消息：按 spec 返回 202，无响应体。
        return StatusCode::ACCEPTED.into_response();
    }
    let id = message.id.clone().unwrap_or(Value::Null);
    let response = match message.method.as_str() {
        "initialize" => jsonrpc::success(
            id,
            jsonrpc::initialize_result(
                message
                    .params
                    .get("protocolVersion")
                    .and_then(Value::as_str),
            ),
        ),
        "ping" => jsonrpc::success(id, json!({})),
        "tools/list" => jsonrpc::success(id, tools::descriptors()),
        "resources/list" => jsonrpc::success(id, json!({ "resources": [] })),
        "prompts/list" => jsonrpc::success(id, json!({ "prompts": [] })),
        "tools/call" => {
            let Some(name) = message
                .params
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                return json_error_response(
                    StatusCode::OK,
                    id,
                    jsonrpc::INVALID_PARAMS,
                    "tools/call requires name",
                );
            };
            let arguments = message
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let ctx = tools::ToolCtx {
                pool: &state.pool,
                ai: &state.ai,
                user,
            };
            match tools::dispatch(&ctx, &name, &arguments).await {
                Ok(result) => jsonrpc::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string(&result)
                                .unwrap_or_else(|_| "{}".to_string())
                        }],
                        "isError": false
                    }),
                ),
                Err(error) => {
                    // 业务/权限错误以 isError:true 的工具结果返回（MCP 惯例）；
                    // 权限拒绝额外用 HTTP 403 标注，便于客户端定位。
                    let text = error.message().to_string();
                    let forbidden = error.status_code() == StatusCode::FORBIDDEN;
                    jsonrpc::success(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": true,
                            "forbidden": forbidden
                        }),
                    )
                }
            }
        }
        other => jsonrpc::error(
            id,
            jsonrpc::METHOD_NOT_FOUND,
            &format!("method not found: {other}"),
        ),
    };
    Json(response).into_response()
}

fn json_error_response(
    status: StatusCode,
    id: Value,
    code: i64,
    message: &str,
) -> Response {
    (status, Json(jsonrpc::error(id, code, message))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use rustset_framework_security::{DataScope, PermissionSet, SecurityConfig, TokenService};
    use std::sync::OnceLock;
    use tower::ServiceExt;

    fn tokens() -> &'static TokenService {
        static TOKENS: OnceLock<TokenService> = OnceLock::new();
        TOKENS.get_or_init(|| {
            TokenService::new(
                SecurityConfig::new(
                    "test-jwt-secret-at-least-32-bytes!!",
                    "rustset",
                    "rustset-api",
                    std::time::Duration::from_secs(900),
                )
                .expect("test security config"),
            )
        })
    }

    fn test_pool() -> DbPool {
        sqlx::Pool::connect_lazy(
            &std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
                "postgres://rustset:rustset@127.0.0.1:5432/rustset".to_string()
            }),
        )
        .expect("test pool must be lazy-connectable")
    }

    fn test_state() -> McpState {
        let pool = test_pool();
        McpState::new(
            pool.clone(),
            SystemState::new(pool.clone(), tokens().clone()),
            AiState::new(pool, tokens().clone()),
        )
    }

    fn test_user() -> CurrentUser {
        CurrentUser {
            user_id: "00000000-0000-0000-0000-000000000001".to_string(),
            username: "mcp-test".to_string(),
            tenant_id: Some("1".to_string()),
            role_codes: vec!["super_admin".to_string()],
            permissions: PermissionSet::new(Vec::new()),
            data_scope: DataScope::All,
        }
    }

    fn request(body: Value) -> Request<Body> {
        Request::post("/mcp")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body_json(response: Response) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn dispatch_initialize_echoes_known_protocol_version() {
        let state = test_state();
        let response = dispatch(
            &state,
            &test_user(),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2024-11-05" } }),
        )
        .await;
        let body = body_json(response).await;
        assert_eq!(body["id"], 1);
        assert_eq!(body["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(body["result"]["serverInfo"]["name"], "rustset-mcp");
        assert!(body["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn dispatch_tools_list_lists_all_tools() {
        let state = test_state();
        let response = dispatch(
            &state,
            &test_user(),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        )
        .await;
        let body = body_json(response).await;
        let tools = body["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), crate::tools::TOOLS.len());
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"asset_page"));
        assert!(names.contains(&"ai_chat_send"));
    }

    #[tokio::test]
    async fn dispatch_unknown_method_returns_32601() {
        let state = test_state();
        let response = dispatch(
            &state,
            &test_user(),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "no/such/method" }),
        )
        .await;
        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn dispatch_notification_returns_202() {
        let state = test_state();
        let response = dispatch(
            &state,
            &test_user(),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn post_without_key_is_401() {
        let router = routes(test_state());
        let response = router
            .oneshot(request(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
