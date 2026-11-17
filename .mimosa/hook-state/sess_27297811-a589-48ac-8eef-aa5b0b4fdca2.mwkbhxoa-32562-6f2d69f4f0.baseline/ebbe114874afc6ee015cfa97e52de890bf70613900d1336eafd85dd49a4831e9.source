use super::ChatProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use rustset_ai_api::{ChatRequest, ChatResponse, ModelConfig};
use serde_json::{Value, json};

pub struct GeminiProvider;

fn body(request: &ChatRequest) -> Value {
    let contents=request.messages.iter().filter(|m|m.role!="system").map(|m|json!({"role":if m.role=="assistant"{"model"}else{"user"},"parts":[{"text":m.content}]})).collect::<Vec<_>>();
    let system = request
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = json!({"contents":contents,"generationConfig":{"temperature":request.temperature.unwrap_or(0.7),"maxOutputTokens":request.max_tokens.unwrap_or(4096)}});
    if !system.is_empty() {
        value["systemInstruction"] = json!({"parts":[{"text":system}]});
    }
    value
}
fn url(config: &ModelConfig, stream: bool) -> String {
    let action = if stream {
        "streamGenerateContent?alt=sse"
    } else {
        "generateContent"
    };
    format!(
        "{}/v1beta/models/{}:{}&key={}",
        config.url.trim_end_matches('/'),
        config.model,
        action,
        config.api_key
    )
    .replace("generateContent&key", "generateContent?key")
}
fn extract(value: &Value) -> String {
    value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect()
}

#[async_trait]
impl ChatProvider for GeminiProvider {
    async fn chat(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
    ) -> Result<ChatResponse, String> {
        let response = reqwest::Client::new()
            .post(url(config, false))
            .json(&body(request))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Gemini 请求失败")
                .into());
        }
        let content = extract(&value);
        if content.is_empty() {
            return Err("Gemini 未返回文本".into());
        }
        Ok(ChatResponse {
            content,
            reasoning: None,
            usage: value
                .get("usageMetadata")
                .cloned()
                .unwrap_or_else(|| json!({})),
        })
    }
}
impl GeminiProvider {
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
        let response = reqwest::Client::new()
            .post(url(config, true))
            .json(&body(request))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(response
                .text()
                .await
                .unwrap_or_else(|_| "Gemini 请求失败".into()));
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
                let delta = extract(&value);
                if !delta.is_empty() {
                    content.push_str(&delta);
                    on_delta(delta).await?;
                }
            }
        }
        if content.is_empty() {
            return Err("Gemini 未返回流式文本".into());
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
    fn maps_assistant_to_model() {
        let v = body(&ChatRequest {
            model: "x".into(),
            messages: vec![ChatMessage {
                role: "assistant".into(),
                content: "hi".into(),
            }],
            temperature: None,
            max_tokens: None,
        });
        assert_eq!(v["contents"][0]["role"], "model");
    }
}
