use super::{ChatProvider, OpenAiCompatibleProvider};
use async_trait::async_trait;
use rustset_ai_api::{ChatRequest, ChatResponse, ModelConfig};
use serde_json::{Value, json};

pub struct AzureOpenAiProvider;
fn adapted(config: &ModelConfig) -> ModelConfig {
    let version = config
        .config
        .get("apiVersion")
        .and_then(Value::as_str)
        .unwrap_or("2024-10-21");
    let mut value = config.clone();
    value.url = format!(
        "{}/openai/deployments/{}",
        config.url.trim_end_matches('/'),
        config.model
    );
    value.config = json!({"textPath":format!("/chat/completions?api-version={version}")});
    value
}
#[async_trait]
impl ChatProvider for AzureOpenAiProvider {
    async fn chat(
        &self,
        config: &ModelConfig,
        request: &ChatRequest,
    ) -> Result<ChatResponse, String> {
        let config = adapted(config);
        OpenAiCompatibleProvider
            .chat_with_header(&config, request, "api-key")
            .await
    }
}
impl AzureOpenAiProvider {
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
        OpenAiCompatibleProvider
            .chat_stream_with_header(&adapted(config), request, on_delta, "api-key")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::adapted;
    use rustset_ai_api::ModelConfig;
    use serde_json::json;
    #[test]
    fn builds_deployment_url() {
        let c = ModelConfig {
            id: 1,
            name: "n".into(),
            key: "k".into(),
            platform: "AzureOpenAI".into(),
            type_: "chat".into(),
            model: "gpt4".into(),
            api_key: "x".into(),
            url: "https://demo.openai.azure.com".into(),
            status: 1,
            config: json!({"apiVersion":"2024-06-01"}),
        };
        let a = adapted(&c);
        assert!(a.url.ends_with("/openai/deployments/gpt4"));
        assert!(
            a.config["textPath"]
                .as_str()
                .unwrap()
                .contains("2024-06-01")
        );
    }
}
