use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use rustset_ai_api::{
    ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse, ImageRequest, MediaResponse,
    ModelConfig, SpeechRequest,
};
use serde_json::{Value, json};

mod anthropic;
mod azure;
mod doubao;
mod gemini;
pub use anthropic::AnthropicProvider;
pub use azure::AzureOpenAiProvider;
pub use doubao::DouBaoMediaProvider;
pub use gemini::GeminiProvider;

fn value_as_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|v| v.to_string()))
}
fn task_id_from(value: &Value) -> Option<String> {
    [
        "/result",
        "/id",
        "/taskId",
        "/task_id",
        "/data/id",
        "/data/taskId",
    ]
    .iter()
    .find_map(|path| value.pointer(path).and_then(value_as_id))
}
fn media_url_from(value: &Value) -> Option<String> {
    [
        "/imageUrl",
        "/image_url",
        "/url",
        "/data/imageUrl",
        "/data/image_url",
        "/data/url",
    ]
    .iter()
    .find_map(|path| {
        value
            .pointer(path)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
    ) -> Result<ChatResponse, String>;
}

pub struct OpenAiCompatibleProvider;

impl OpenAiCompatibleProvider {
    pub async fn raw_chat_with_header(
        &self,
        config: &ModelConfig,
        body: Value,
        auth_header: &str,
    ) -> Result<Value, String> {
        let path = config
            .config
            .get("textPath")
            .and_then(Value::as_str)
            .unwrap_or("/chat/completions");
        let mut builder = reqwest::Client::new()
            .post(format!("{}{}", config.url.trim_end_matches('/'), path))
            .json(&body);
        if !config.api_key.is_empty() {
            builder = if auth_header.eq_ignore_ascii_case("authorization") {
                builder.bearer_auth(config.api_key.trim_start_matches("Bearer "))
            } else {
                builder.header(auth_header, &config.api_key)
            }
        }
        let response = builder.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "模型工具请求失败"));
        }
        Ok(value)
    }
    pub async fn chat_with_header(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
        auth_header: &str,
    ) -> Result<ChatResponse, String> {
        let path = config
            .config
            .get("textPath")
            .and_then(Value::as_str)
            .unwrap_or("/chat/completions");
        let mut body = json!({"model":request.model,"messages":request.messages,"temperature":request.temperature.unwrap_or(0.7)});
        if let Some(limit) = request.max_tokens {
            body["max_tokens"] = json!(limit);
        }
        let mut builder = reqwest::Client::new()
            .post(format!("{}{}", config.url.trim_end_matches('/'), path))
            .json(&body);
        if !config.api_key.is_empty() {
            builder = if auth_header.eq_ignore_ascii_case("authorization") {
                builder.bearer_auth(config.api_key.trim_start_matches("Bearer "))
            } else {
                builder.header(auth_header, &config.api_key)
            };
        }
        let response = builder.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "模型请求失败"));
        }
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| "模型未返回文本".to_string())?
            .to_string();
        Ok(ChatResponse {
            content,
            reasoning: value
                .pointer("/choices/0/message/reasoning_content")
                .and_then(Value::as_str)
                .map(str::to_string),
            usage: value.get("usage").cloned().unwrap_or_else(|| json!({})),
        })
    }
    pub async fn chat_stream_with_header<F, Fut>(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
        mut on_delta: F,
        auth_header: &str,
    ) -> Result<ChatResponse, String>
    where
        F: FnMut(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let path = config
            .config
            .get("textPath")
            .and_then(Value::as_str)
            .unwrap_or("/chat/completions");
        let mut body = json!({"model":request.model,"messages":request.messages,"temperature":request.temperature.unwrap_or(0.7),"stream":true});
        if let Some(limit) = request.max_tokens {
            body["max_tokens"] = json!(limit)
        }
        let mut builder = reqwest::Client::new()
            .post(format!("{}{}", config.url.trim_end_matches('/'), path))
            .json(&body);
        if !config.api_key.is_empty() {
            builder = if auth_header.eq_ignore_ascii_case("authorization") {
                builder.bearer_auth(config.api_key.trim_start_matches("Bearer "))
            } else {
                builder.header(auth_header, &config.api_key)
            }
        }
        let response = builder.send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(response
                .text()
                .await
                .unwrap_or_else(|_| "模型请求失败".into()));
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
                if data == "[DONE]" {
                    continue;
                }
                let value: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
                if let Some(delta) = value
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                {
                    content.push_str(delta);
                    on_delta(delta.into()).await?
                }
            }
        }
        if content.is_empty() {
            return Err("模型未返回流式文本".into());
        }
        Ok(ChatResponse {
            content,
            reasoning: None,
            usage: json!({}),
        })
    }
    fn request(&self, config: &ModelConfig, path: &str) -> reqwest::RequestBuilder {
        let mut request =
            reqwest::Client::new().post(format!("{}{}", config.url.trim_end_matches('/'), path));
        if !config.api_key.is_empty() {
            request = request.bearer_auth(config.api_key.trim_start_matches("Bearer "));
        }
        request
    }
    pub async fn image(
        &self,
        config: &ModelConfig,
        request: &ImageRequest,
    ) -> Result<MediaResponse, String> {
        let path = config
            .config
            .get("imageGeneratePath")
            .and_then(Value::as_str)
            .unwrap_or("/images/generations");
        let size = doubao::normalize_seedream_size(&config.model, &request.size);
        let mut body = json!({"model":config.model,"prompt":request.prompt,"size":size,"n":1});
        let path = if request.references.is_empty() {
            path
        } else if config.platform == rustset_ai_api::AiPlatform::DouBao.code() {
            body["image"] = json!(request.references);
            path
        } else {
            body["images"] = json!(
                request
                    .references
                    .iter()
                    .map(|url| json!({"image_url":url}))
                    .collect::<Vec<_>>()
            );
            config
                .config
                .get("imageEditPath")
                .and_then(Value::as_str)
                .unwrap_or("/images/edits")
        };
        // Ark occasionally resets negotiated HTTP/2 POST streams while the same endpoint remains
        // reachable over HTTP/1.1. Image jobs are long-lived and benefit from the conservative
        // transport; chat and video clients keep their existing negotiation behavior.
        let image_client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| format!("创建图片 HTTP 客户端失败: {error:?}"))?;
        let mut image_request =
            image_client.post(format!("{}{}", config.url.trim_end_matches('/'), path));
        if !config.api_key.is_empty() {
            image_request = image_request.bearer_auth(config.api_key.trim_start_matches("Bearer "));
        }
        let response = image_request
            .json(&body)
            .send()
            .await
            .map_err(|error| format!("图片服务连接失败: {error:?}"))?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "图片生成失败"));
        }
        let url = value
            .pointer("/data/0/url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                value
                    .pointer("/data/0/b64_json")
                    .and_then(Value::as_str)
                    .map(|data| format!("data:image/png;base64,{data}"))
            })
            .ok_or_else(|| "图片响应缺少 URL 或 Base64".to_string())?;
        Ok(MediaResponse {
            url,
            task_id: value.get("id").and_then(Value::as_str).map(str::to_string),
            raw: value,
        })
    }
    async fn midjourney_submit(
        &self,
        config: &ModelConfig,
        config_key: &str,
        default_path: &str,
        payload: Value,
        operation: &str,
    ) -> Result<MediaResponse, String> {
        let path = config
            .config
            .get(config_key)
            .and_then(Value::as_str)
            .unwrap_or(default_path);
        let value = self
            .raw_media_request(
                config,
                reqwest::Method::POST,
                path,
                Some(payload),
                operation,
            )
            .await?;
        let task_id = task_id_from(&value);
        let url = media_url_from(&value).unwrap_or_default();
        if task_id.is_none() && url.is_empty() {
            return Err(format!("{operation}响应缺少任务 ID 或图片 URL"));
        }
        Ok(MediaResponse {
            url,
            task_id,
            raw: value,
        })
    }
    pub async fn midjourney_imagine(
        &self,
        config: &ModelConfig,
        mut payload: Value,
    ) -> Result<MediaResponse, String> {
        payload["model"] = json!(config.model);
        self.midjourney_submit(
            config,
            "midjourneyImaginePath",
            "/mj/submit/imagine",
            payload,
            "Midjourney Imagine",
        )
        .await
    }
    pub async fn midjourney_action(
        &self,
        config: &ModelConfig,
        payload: Value,
    ) -> Result<MediaResponse, String> {
        self.midjourney_submit(
            config,
            "midjourneyActionPath",
            "/mj/submit/action",
            payload,
            "Midjourney Action",
        )
        .await
    }
    pub async fn poll_midjourney(
        &self,
        config: &ModelConfig,
        task_id: &str,
    ) -> Result<MediaResponse, String> {
        let template = config
            .config
            .get("midjourneyTaskPath")
            .and_then(Value::as_str)
            .unwrap_or("/mj/task/{taskId}/fetch");
        let path = template.replace("{taskId}", task_id);
        let value = self
            .raw_media_request(
                config,
                reqwest::Method::GET,
                &path,
                None,
                "Midjourney 任务查询",
            )
            .await?;
        Ok(MediaResponse {
            url: media_url_from(&value).unwrap_or_default(),
            task_id: Some(task_id.into()),
            raw: value,
        })
    }
    async fn raw_media_request(
        &self,
        config: &ModelConfig,
        method: reqwest::Method,
        path: &str,
        payload: Option<Value>,
        operation: &str,
    ) -> Result<Value, String> {
        let mut request = reqwest::Client::new().request(
            method,
            format!("{}{}", config.url.trim_end_matches('/'), path),
        );
        if !config.api_key.is_empty() {
            let header = config
                .config
                .get("authHeader")
                .and_then(Value::as_str)
                .unwrap_or("Authorization");
            request = if header.eq_ignore_ascii_case("authorization") {
                request.bearer_auth(config.api_key.trim_start_matches("Bearer "))
            } else {
                request.header(header, &config.api_key)
            };
        }
        if let Some(body) = payload {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, operation));
        }
        Ok(value)
    }
    pub async fn video(
        &self,
        config: &ModelConfig,
        mut payload: Value,
    ) -> Result<MediaResponse, String> {
        let path = config
            .config
            .get("videoGeneratePath")
            .and_then(Value::as_str)
            .unwrap_or("/videos/generations");
        payload["model"] = json!(config.model);
        let response = self
            .request(config, path)
            .json(&payload)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "视频生成失败"));
        }
        let url = value
            .get("url")
            .or_else(|| value.pointer("/data/0/url"))
            .or_else(|| value.get("filePath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let task_id = value
            .get("id")
            .or_else(|| value.get("task_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if url.is_empty() && task_id.is_none() {
            return Err("视频响应缺少 URL 或任务 ID".into());
        }
        Ok(MediaResponse {
            url,
            task_id,
            raw: value,
        })
    }
    pub async fn music(
        &self,
        config: &ModelConfig,
        mut payload: Value,
    ) -> Result<MediaResponse, String> {
        let path = config
            .config
            .get("musicGeneratePath")
            .and_then(Value::as_str)
            .unwrap_or("/music/generations");
        payload["model"] = json!(config.model);
        let response = self
            .request(config, path)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "音乐生成失败"));
        }
        let url = value
            .get("audio_url")
            .or_else(|| value.get("audioUrl"))
            .or_else(|| value.pointer("/data/0/audio_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let task_id = value
            .get("id")
            .or_else(|| value.get("task_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if url.is_empty() && task_id.is_none() {
            return Err("音乐响应缺少音频 URL 或任务 ID".into());
        }
        Ok(MediaResponse {
            url,
            task_id,
            raw: value,
        })
    }
    pub async fn poll_music(
        &self,
        config: &ModelConfig,
        task_id: &str,
    ) -> Result<MediaResponse, String> {
        let template = config
            .config
            .get("musicTaskPath")
            .and_then(Value::as_str)
            .unwrap_or("/music/tasks/{taskId}");
        let path = template.replace("{taskId}", task_id);
        let mut request =
            reqwest::Client::new().get(format!("{}{}", config.url.trim_end_matches('/'), path));
        if !config.api_key.is_empty() {
            let header = config
                .config
                .get("authHeader")
                .and_then(Value::as_str)
                .unwrap_or("Authorization");
            request = if header.eq_ignore_ascii_case("authorization") {
                request.bearer_auth(config.api_key.trim_start_matches("Bearer "))
            } else {
                request.header(header, &config.api_key)
            }
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "音乐任务查询失败"));
        }
        let url = value
            .get("audio_url")
            .or_else(|| value.get("audioUrl"))
            .or_else(|| value.pointer("/data/audio_url"))
            .or_else(|| value.pointer("/data/audioUrl"))
            .or_else(|| value.pointer("/data/0/audio_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(MediaResponse {
            url,
            task_id: Some(task_id.into()),
            raw: value,
        })
    }
    pub async fn speech(
        &self,
        config: &ModelConfig,
        request: &SpeechRequest,
    ) -> Result<MediaResponse, String> {
        let path = config
            .config
            .get("speechPath")
            .and_then(Value::as_str)
            .unwrap_or("/audio/speech");
        let response=self.request(config,path).json(&json!({"model":config.model,"input":request.input,"voice":request.voice,"response_format":request.format})).send().await.map_err(|error|error.to_string())?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("audio/mpeg")
            .to_string();
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(String::from_utf8_lossy(&bytes).into_owned());
        }
        Ok(MediaResponse {
            url: format!("data:{content_type};base64,{}", STANDARD.encode(bytes)),
            task_id: None,
            raw: json!({}),
        })
    }
    pub async fn embedding(
        &self,
        config: &ModelConfig,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse, String> {
        let path = config
            .config
            .get("embeddingPath")
            .and_then(Value::as_str)
            .unwrap_or("/embeddings");
        let response = self
            .request(config, path)
            .json(&json!({"model":config.model,"input":request.inputs}))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(api_error(&value, "Embedding 请求失败"));
        }
        let embeddings = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "Embedding 响应缺少 data".to_string())?
            .iter()
            .map(|item| {
                item.get("embedding")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "Embedding 数据无效".to_string())?
                    .iter()
                    .map(|number| {
                        number
                            .as_f64()
                            .map(|value| value as f32)
                            .ok_or_else(|| "Embedding 数值无效".to_string())
                    })
                    .collect()
            })
            .collect::<Result<Vec<Vec<f32>>, String>>()?;
        Ok(EmbeddingResponse {
            embeddings,
            usage: value.get("usage").cloned().unwrap_or_else(|| json!({})),
        })
    }
    pub async fn chat_stream<F, Fut>(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
        on_delta: F,
    ) -> Result<ChatResponse, String>
    where
        F: FnMut(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        self.chat_stream_with_header(config, request, on_delta, "authorization")
            .await
    }
}
fn api_error(value: &Value, fallback: &str) -> String {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    async fn chat(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
    ) -> Result<ChatResponse, String> {
        self.chat_with_header(config, request, "authorization")
            .await
    }
}
