//! 外部 MCP 服务器客户端（Streamable HTTP，JSON-RPC 2.0）。
//!
//! 平台作为 MCP 客户端：调用登记在 `ai.mcp_server` 的外部服务器。
//! 每服务器惰性建会话（initialize 握手缓存协议版本与 Mcp-Session-Id），
//! 会话失效（404/400）自动重建；无状态服务器同样工作。

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

const PROTOCOL_VERSION: &str = "2025-03-26";
const TIMEOUT_SECONDS: u64 = 30;
const MAX_SESSION_IDLE_MS: i64 = 10 * 60 * 1000;

/// 一次外部调用的连接参数（令牌在调用前即时解封，不落日志）。
#[derive(Clone)]
pub struct McpRemote {
    pub url: String,
    pub auth_header: String,
    pub auth_token: String,
}

/// 已握手会话：协商的协议版本 + 服务器分配的会话 id（None = 无状态服务器）。
#[derive(Clone)]
struct Session {
    protocol_version: String,
    session_id: Option<String>,
    created_at: i64,
}

static SESSIONS: OnceLock<Mutex<HashMap<i64, Session>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<i64, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECONDS))
        .build()
        .expect("reqwest client")
}

fn request_id() -> Value {
    json!(chrono::Utc::now().timestamp_micros())
}

fn initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": request_id(),
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "rustset-ai", "version": env!("CARGO_PKG_VERSION") }
        }
    })
}

async fn post(
    remote: &McpRemote,
    session: Option<&Session>,
    body: Value,
) -> Result<reqwest::Response, String> {
    let mut request = client()
        .post(&remote.url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&body);
    if let Some(session) = session {
        request = request.header("mcp-protocol-version", &session.protocol_version);
        if let Some(id) = session.session_id.as_ref() {
            request = request.header("mcp-session-id", id);
        }
    }
    if !remote.auth_header.is_empty() && !remote.auth_token.is_empty() {
        request = request.header(&remote.auth_header, &remote.auth_token);
    }
    request
        .send()
        .await
        .map_err(|error| format!("连接 MCP 服务器失败: {error}"))
}

/// 解析响应：application/json 直接取；text/event-stream 取首个 data 帧。
async fn read_json(response: reqwest::Response) -> Result<Value, String> {
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    if content_type.contains("text/event-stream") {
        let text = response
            .text()
            .await
            .map_err(|error| format!("读取 MCP 响应失败: {error}"))?;
        for line in text.lines() {
            if let Some(payload) = line.strip_prefix("data:") {
                let payload = payload.trim();
                if payload.is_empty() || payload == "[DONE]" {
                    continue;
                }
                return serde_json::from_str(payload)
                    .map_err(|_| "MCP SSE 帧不是合法 JSON".to_string());
            }
        }
        return Err("MCP SSE 流中没有数据帧".to_string());
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| format!("读取 MCP 响应失败: {error}"))
}

/// 从 JSON-RPC 响应取 result；error 对象转 Err。
fn unwrap_result(payload: Value) -> Result<Value, String> {
    if let Some(error) = payload.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("MCP 服务器错误: {message}"));
    }
    Ok(payload.get("result").cloned().unwrap_or_else(|| json!({})))
}

/// initialize 握手：返回 (会话, 完整 result)。服务器若分配了会话 id，透传回缓存。
async fn initialize(server_id: i64, remote: &McpRemote) -> Result<(Session, Value), String> {
    let response = post(remote, None, initialize_body()).await?;
    if !response.status().is_success() {
        return Err(format!(
            "MCP 服务器返回 HTTP {}（检查 URL 与鉴权头）",
            response.status().as_u16()
        ));
    }
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let result = unwrap_result(read_json(response).await?)?;
    let protocol_version = result
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION)
        .to_string();
    let session = Session {
        protocol_version,
        session_id,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    sessions().lock().await.insert(server_id, session.clone());
    // initialize 完成通知（规范要求；202 无响应体，失败不阻断）。
    let _ = post(
        remote,
        Some(&session),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    Ok((session, result))
}

async fn session_or_init(server_id: i64, remote: &McpRemote) -> Result<Session, String> {
    let cached = {
        let map = sessions().lock().await;
        map.get(&server_id)
            .filter(|session| {
                chrono::Utc::now().timestamp_millis() - session.created_at < MAX_SESSION_IDLE_MS
            })
            .cloned()
    };
    match cached {
        Some(session) => Ok(session),
        None => initialize(server_id, remote).await.map(|(session, _)| session),
    }
}

/// 带会话的 JSON-RPC 请求；404/400（会话失效）重建会话重试一次。
async fn rpc(server_id: i64, remote: &McpRemote, body: Value) -> Result<Value, String> {
    let session = session_or_init(server_id, remote).await?;
    let mut response = post(remote, Some(&session), body.clone()).await?;
    let status = response.status().as_u16();
    if status == 404 || status == 400 {
        sessions().lock().await.remove(&server_id);
        let (fresh, _) = initialize(server_id, remote).await?;
        response = post(remote, Some(&fresh), body).await?;
    }
    if !response.status().is_success() {
        return Err(format!("MCP 服务器返回 HTTP {}", response.status().as_u16()));
    }
    unwrap_result(read_json(response).await?)
}

/// initialize + tools/list：返回 (serverInfo, 工具数组)。
/// serverInfo 来自握手；会话复用时不重复展示成本可忽略（握手一次即缓存）。
pub async fn list_tools(
    server_id: i64,
    remote: &McpRemote,
) -> Result<(Value, Vec<Value>), String> {
    let (_, init_result) = initialize(server_id, remote).await?;
    let server_info = init_result.get("serverInfo").cloned().unwrap_or(json!({}));
    let result = rpc(
        server_id,
        remote,
        json!({ "jsonrpc": "2.0", "id": request_id(), "method": "tools/list" }),
    )
    .await?;
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok((server_info, tools))
}

/// tools/call：拼接全部 text 块返回；远端 isError 转为 Err。
pub async fn call_tool(
    server_id: i64,
    remote: &McpRemote,
    tool: &str,
    arguments: &Value,
) -> Result<String, String> {
    let result = rpc(
        server_id,
        remote,
        json!({
            "jsonrpc": "2.0",
            "id": request_id(),
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }),
    )
    .await?;
    let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
    let text: Vec<String> = result
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let joined = if text.is_empty() {
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    } else {
        text.join("\n")
    };
    if is_error {
        Err(format!("工具 {tool} 执行失败: {joined}"))
    } else {
        Ok(joined)
    }
}

/// 服务器配置变更/删除后清理缓存会话。
pub async fn invalidate_session(server_id: i64) {
    sessions().lock().await.remove(&server_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_frame_format_is_pinned() {
        // read_json 的 SSE 分支约定：取首个非 [DONE] 的 data 帧。
        let frame = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data:"))
            .unwrap()
            .trim();
        let payload: Value = serde_json::from_str(data).unwrap();
        assert!(payload["result"]["tools"].is_array());
    }

    #[test]
    fn unwrap_result_maps_jsonrpc_error() {
        let error = unwrap_result(json!({
            "jsonrpc": "2.0", "id": 1,
            "error": { "code": -32601, "message": "no such method" }
        }));
        assert!(error.unwrap_err().contains("no such method"));
        assert_eq!(unwrap_result(json!({"result": {"ok": 1}})).unwrap()["ok"], 1);
    }
}
