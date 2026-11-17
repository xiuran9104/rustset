use async_trait::async_trait;
use futures_util::StreamExt;
use rustset_ai_api::{ChatRequest, ChatResponse, ModelConfig};
use serde_json::{Value, json};

use super::ChatProvider;

pub struct AnthropicProvider;

fn body(request: &ChatRequest, stream: bool) -> Value {
    let system = request
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let messages = request
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| json!({"role":m.role,"content":m.content}))
        .collect::<Vec<_>>();
    json!({"model":request.model,"system":system,"messages":messages,"temperature":request.temperature.unwrap_or(0.7),"max_tokens":request.max_tokens.unwrap_or(4096),"stream":stream})
}

fn builder(config: &ModelConfig, request: &ChatRequest, stream: bool) -> reqwest::RequestBuilder {
    let path = config
        .config
        .get("textPath")
        .and_then(Value::as_str)
        .unwrap_or("/v1/messages");
    reqwest::Client::new()
        .post(format!("{}{}", config.url.trim_end_matches('/'), path))
        .header("x-api-key", &config.api_key)
        .header(
            "anthropic-version",
            config
                .config
                .get("anthropicVersion")
                .and_then(Value::as_str)
                .unwrap_or("2023-06-01"),
        )
        .json(&body(request, stream))
}

#[async_trait]
impl ChatProvider for AnthropicProvider {
    async fn chat(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
    ) -> Result<ChatResponse, String> {
        let response = builder(config, request, false)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Anthropic 请求失败")
                .into());
        }
        let content = value
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.get("text").and_then(Value::as_str))
            .collect::<String>();
        if content.is_empty() {
            return Err("Anthropic 未返回文本".into());
        }
        Ok(ChatResponse {
            content,
            reasoning: None,
            usage: value.get("usage").cloned().unwrap_or_else(|| json!({})),
        })
    }
}

impl AnthropicProvider {
    pub async fn chat_stream<F, Fut>(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
        mut on_delta: F,
    ) -> Result<ChatResponse, String>
    where
        F: FnMut(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let response = builder(config, request, true)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(response
                .text()
                .await
                .unwrap_or_else(|_| "Anthropic 请求失败".into()));
        }
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        while let Some(chunk) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk.map_err(|e| e.to_string())?));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);
                let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                    continue;
                };
                let value: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
                if let Some(delta) = value.pointer("/delta/text").and_then(Value::as_str) {
                    content.push_str(delta);
                    on_delta(delta.into()).await?;
                }
            }
        }
        if content.is_empty() {
            return Err("Anthropic 未返回流式文本".into());
        }
        Ok(ChatResponse {
            content,
            reasoning: None,
            usage: json!({}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::body;
    use rustset_ai_api::{ChatMessage, ChatRequest};
    #[test]
    fn separates_system_message() {
        let v = body(
            &ChatRequest {
                model: "claude".into(),
                messages: vec![
                    ChatMessage {
                        role: "system".into(),
                        content: "rules".into(),
                    },
                    ChatMessage {
                        role: "user".into(),
                        content: "hi".into(),
                    },
                ],
                temperature: None,
                max_tokens: None,
            },
            false,
        );
        assert_eq!(v["system"], "rules");
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    }
}
