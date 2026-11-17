use std::sync::OnceLock;

use aide::openapi::OpenApi;
use axum::Json;
use serde_json::Value;

static DOCUMENT: OnceLock<Value> = OnceLock::new();

/// finish_api 之后发布推导生成的完整文档；打标签与安全方案在 main 中完成。
pub fn publish(api: OpenApi) {
    let value = serde_json::to_value(&api).expect("serialized OpenApi document");
    let _ = DOCUMENT.set(value);
}

pub async fn document() -> Json<Value> {
    Json(DOCUMENT.get().cloned().unwrap_or(Value::Null))
}
