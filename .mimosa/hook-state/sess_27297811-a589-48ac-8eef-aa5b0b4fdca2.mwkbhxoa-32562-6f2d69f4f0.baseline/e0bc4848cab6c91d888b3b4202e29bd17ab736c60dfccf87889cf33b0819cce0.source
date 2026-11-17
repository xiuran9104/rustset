//! 极小 JSON-RPC 2.0 层（MCP Streamable HTTP 所需子集）。
//! 单消息请求；批量请求返回 -32600；notification（无 id）由调用方返回 202。

use serde_json::{Value, json};

/// 协议版本：客户端携带已知版本则回显，否则回落到 2025-03-26。
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
const KNOWN_PROTOCOL_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

#[derive(Debug, Clone)]
pub struct Incoming {
    /// None 表示 notification。
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

/// 解析单条 JSON-RPC 消息。错误返回 (code, message)。
pub fn parse(body: &Value) -> Result<Incoming, (i64, String)> {
    let object = body
        .as_object()
        .ok_or_else(|| (INVALID_REQUEST, "request must be an object".to_string()))?;
    if object.is_empty() {
        return Err((INVALID_REQUEST, "empty request".to_string()));
    }
    // MCP 2025-06-18 起允许批量；本实现先拒绝，保持无状态简单性。
    if object.contains_key("0") || body.as_array().is_some() {
        return Err((INVALID_REQUEST, "batch requests are not supported".to_string()));
    }
    if body.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err((INVALID_REQUEST, "jsonrpc must be \"2.0\"".to_string()));
    }
    let method = body
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| (INVALID_REQUEST, "method is required".to_string()))?
        .to_string();
    let id = body.get("id").cloned();
    // id 只允许 string / number（notification 没有 id；null id 按 spec 属于错误请求，
    // 但宽容处理为响应可寻址的 null）。
    if let Some(id_value) = &id {
        let ok = id_value.is_string() || id_value.is_number() || id_value.is_null();
        if !ok {
            return Err((INVALID_REQUEST, "id must be a string or number".to_string()));
        }
    }
    let params = body.get("params").cloned().unwrap_or_else(|| json!({}));
    Ok(Incoming { id, method, params })
}

pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

pub fn is_notification(message: &Incoming) -> bool {
    message.id.is_none()
}

/// initialize 结果：回显支持的协议版本 + 仅声明 tools 能力。
pub fn initialize_result(client_version: Option<&str>) -> Value {
    let version = client_version
        .filter(|v| KNOWN_PROTOCOL_VERSIONS.contains(v))
        .unwrap_or(DEFAULT_PROTOCOL_VERSION)
        .to_string();
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": { "listChanged": false },
            "logging": {}
        },
        "serverInfo": {
            "name": "rustset-mcp",
            "version": env!("CARGO_PKG_VERSION"),
            "title": "RustSet 平台 MCP 服务"
        },
        "instructions": "RustSet 资产、CMDB、风险、扫描任务、资源工单与 AI 对话工具。所有查询按 API Key 绑定用户的权限与租户隔离。"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_with_params() {
        let message = parse(&json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/list", "params": {"a": 1}
        }))
        .unwrap();
        assert_eq!(message.id, Some(json!(7)));
        assert_eq!(message.method, "tools/list");
        assert_eq!(message.params["a"], 1);
    }

    #[test]
    fn notification_has_no_id() {
        let message =
            parse(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).unwrap();
        assert!(is_notification(&message));
    }

    #[test]
    fn rejects_bad_version_and_batch() {
        assert!(parse(&json!({ "jsonrpc": "1.0", "id": 1, "method": "x" })).is_err());
        assert!(parse(&json!([{ "jsonrpc": "2.0", "id": 1, "method": "x" }])).is_err());
    }

    #[test]
    fn initialize_echoes_known_version() {
        assert_eq!(
            initialize_result(Some("2024-11-05"))["protocolVersion"], "2024-11-05"
        );
        assert_eq!(
            initialize_result(Some("2099-01-01"))["protocolVersion"],
            DEFAULT_PROTOCOL_VERSION
        );
        assert_eq!(initialize_result(None)["protocolVersion"], DEFAULT_PROTOCOL_VERSION);
    }
}
