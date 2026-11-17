//! AI 节点执行器：llm / knowledge / agent。
//! 复用 ai-server 的 AiModelFactory（LLM 调用）、knowledge::retrieve（RAG）、
//! tools::execute（Agent 工具执行）。输出统一写入 instance.context。

use rustset_framework_database::DbPool;
use rustset_framework_tenant::TenantContext;
use serde_json::{Value, json};
use sqlx::Row;

use super::variables::{extract_json, interpolate};

/// LLM 节点：提示词（变量插值）→ AiModelFactory.chat → 输出写入 context。
/// config: {modelId, systemPrompt, userPrompt, temperature?, maxTokens?, outputFormat?, outputKey?}
pub(crate) async fn execute_llm(
    pool: &DbPool,
    config: &Value,
    payload: &Value,
    context: &mut Value,
) -> Result<Value, String> {
    let model_id = config
        .get("modelId")
        .and_then(Value::as_i64)
        .ok_or("LLM 节点需要 modelId")?;
    let system_prompt = config
        .get("systemPrompt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let user_prompt = config
        .get("userPrompt")
        .and_then(Value::as_str)
        .ok_or("LLM 节点需要 userPrompt")?;
    let temperature = config.get("temperature").and_then(Value::as_f64);
    let max_tokens = config
        .get("maxTokens")
        .and_then(Value::as_u64)
        .map(|v| v as u32);

    let system = interpolate(&system_prompt, payload, context);
    let user = interpolate(user_prompt, payload, context);

    let messages = vec![
        rustset_ai_api::ChatMessage {
            role: "system".to_string(),
            content: system,
        },
        rustset_ai_api::ChatMessage {
            role: "user".to_string(),
            content: user,
        },
    ];
    let request = rustset_ai_api::ChatRequest {
        model: String::new(),
        messages,
        temperature,
        max_tokens,
    };

    let factory = rustset_ai_server::AiModelFactory::new(pool.clone());
    let response = factory
        .chat(model_id, request)
        .await
        .map_err(|e| format!("LLM 调用失败：{}", e.message()))?;

    let output_format = config.get("outputFormat").and_then(Value::as_str).unwrap_or("text");
    let output = if output_format == "json" {
        extract_json(&response.content)
            .unwrap_or_else(|| Value::String(response.content.clone()))
    } else {
        Value::String(response.content.clone())
    };

    let output_key = config
        .get("outputKey")
        .and_then(Value::as_str)
        .unwrap_or("llm_output");
    if let Some(obj) = context.as_object_mut() {
        obj.insert(output_key.to_string(), output.clone());
    }

    Ok(json!({
        "content": response.content,
        "output": output,
        "outputKey": output_key,
        "usage": response.usage,
    }))
}

/// 知识检索节点：query（变量插值）→ knowledge::retrieve → 格式化写入 context。
/// config: {knowledgeIds, query, topK?, outputKey?}
pub(crate) async fn execute_knowledge(
    pool: &DbPool,
    config: &Value,
    payload: &Value,
    context: &mut Value,
) -> Result<Value, String> {
    let knowledge_ids: Vec<i64> = config
        .get("knowledgeIds")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();
    if knowledge_ids.is_empty() {
        return Err("知识检索节点需要至少一个知识库".to_string());
    }
    let query_template = config
        .get("query")
        .and_then(Value::as_str)
        .ok_or("知识检索节点需要 query")?;
    let query = interpolate(query_template, payload, context);
    if query.trim().is_empty() {
        return Err("知识检索 query 为空".to_string());
    }

    // 构建 AiState（retrieve 需要 factory 做 embedding）
    let tokens = rustset_framework_security::TokenService::new(
        rustset_framework_security::SecurityConfig::new(
            std::env::var("JWT_SECRET").unwrap_or_default(),
            "rustset",
            "rustset-api",
            std::time::Duration::from_secs(900),
        )
        .map_err(|e| format!("SecurityConfig: {e}"))?,
    );
    let ai_state = rustset_ai_server::AiState::new(pool.clone(), tokens);

    let segments = rustset_ai_server::knowledge::retrieve(&ai_state, &knowledge_ids, &query)
        .await
        .map_err(|e| format!("知识检索失败：{}", e.message()))?;

    let formatted = segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let doc = s.get("documentName").and_then(Value::as_str).unwrap_or("未知文档");
            let content = s.get("content").and_then(Value::as_str).unwrap_or_default();
            let similarity = s.get("similarity").and_then(Value::as_f64).unwrap_or(0.0);
            format!("[{}] {}（相似度 {:.2}）\n{}", i + 1, doc, similarity, content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let output_key = config
        .get("outputKey")
        .and_then(Value::as_str)
        .unwrap_or("kb_context");
    if let Some(obj) = context.as_object_mut() {
        obj.insert(output_key.to_string(), json!(formatted));
    }

    Ok(json!({
        "segments": segments.len(),
        "outputKey": output_key,
        "preview": formatted.chars().take(200).collect::<String>(),
    }))
}

/// 构建系统 CurrentUser（super_admin）供 Agent 工具执行使用。
/// 流程引擎在后台运行，没有真实登录会话——用系统身份保证工具权限可用。
pub(crate) fn system_user() -> rustset_framework_security::CurrentUser {
    use rustset_framework_security::{CurrentUser, DataScope};
    CurrentUser {
        user_id: "00000000-0000-0000-0000-000000000001".to_string(),
        username: "system:flow-agent".to_string(),
        tenant_id: None,
        role_codes: vec!["super_admin".to_string()],
        permissions: rustset_framework_security::PermissionSet::new(Vec::new()),
        data_scope: DataScope::All,
    }
}

/// Agent 节点：LLM + 工具循环（chat_tools → tools::execute → 回填，最多 N 轮）。
/// config: {modelId, systemPrompt, userPrompt, toolIds, maxIterations?, outputKey?}
pub(crate) async fn execute_agent(
    pool: &DbPool,
    user: &rustset_framework_security::CurrentUser,
    config: &Value,
    payload: &Value,
    context: &mut Value,
) -> Result<Value, String> {
    // 优先从 Agent 预设加载配置（agentPresetId），回退到内联 config
    let (model_id, system_prompt, tool_ids, _knowledge_ids, _temp, _max_tok) =
        if let Some(preset_id) = config.get("agentPresetId").and_then(Value::as_i64) {
            let row = sqlx::query(
                "SELECT model_id, system_prompt, tool_ids, knowledge_ids, temperature, max_tokens
                 FROM ai_agent_preset WHERE id=$1 AND deleted=0 AND status=0",
            )
            .bind(preset_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("加载 Agent 预设失败：{e}"))?
            .ok_or("Agent 预设不存在或已停用")?;
            let tool_ids: Vec<i64> = row
                .get::<Value, _>("tool_ids")
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            (
                row.get::<i64, _>("model_id"),
                row.get::<String, _>("system_prompt"),
                tool_ids,
                row.get::<Value, _>("knowledge_ids"),
                row.get::<f64, _>("temperature"),
                row.get::<i32, _>("max_tokens"),
            )
        } else {
            // 回退到内联配置
            let model_id = config
                .get("modelId")
                .and_then(Value::as_i64)
                .ok_or("Agent 节点需要 agentPresetId 或 modelId")?;
            let tool_ids: Vec<i64> = config
                .get("toolIds")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            (
                model_id,
                config
                    .get("systemPrompt")
                    .and_then(Value::as_str)
                    .unwrap_or("你是运维助手")
                    .to_string(),
                tool_ids,
                json!([]),
                0.7,
                4096,
            )
        };
    let user_prompt = config
        .get("userPrompt")
        .and_then(Value::as_str)
        .ok_or("Agent 节点需要 userPrompt")?;
    let max_iterations = config
        .get("maxIterations")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .min(10) as usize;
    let _ = user; // Agent 工具执行可能需要 CurrentUser，暂由 tools::execute 内部处理

    let system = interpolate(&system_prompt, payload, context);
    let user_msg = interpolate(user_prompt, payload, context);

    let factory = rustset_ai_server::AiModelFactory::new(pool.clone());

    // 无工具时退化为普通 LLM 调用
    if tool_ids.is_empty() {
        let messages = vec![
            rustset_ai_api::ChatMessage { role: "system".into(), content: system },
            rustset_ai_api::ChatMessage { role: "user".into(), content: user_msg },
        ];
        let request = rustset_ai_api::ChatRequest {
            model: String::new(), messages,
            temperature: Some(0.7), max_tokens: Some(4096),
        };
        let response = factory.chat(model_id, request).await
            .map_err(|e| format!("Agent LLM 调用失败：{}", e.message()))?;
        let output_key = config.get("outputKey").and_then(Value::as_str).unwrap_or("agent_result");
        if let Some(obj) = context.as_object_mut() {
            obj.insert(output_key.to_string(), json!(response.content));
        }
        return Ok(json!({"content": response.content, "iterations": 0, "toolsUsed": []}));
    }

    // 有工具：agentic loop
    let tools = rustset_ai_server::tools::load_definitions(pool, &tool_ids).await
        .map_err(|e| e.message().to_string())?;
    let allowed_names: Vec<String> = tools
        .iter()
        .filter_map(|t| t.pointer("/function/name").and_then(Value::as_str).map(String::from))
        .collect();

    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": user_msg}),
    ];
    let mut tools_used: Vec<String> = Vec::new();
    let mut final_content = String::new();
    let mut rounds_used = 0usize;

    for _round in 0..max_iterations {
        rounds_used = _round + 1;
        let raw = factory
            .chat_tools(model_id, messages.clone(), tools.clone(), Some(0.7), Some(4096))
            .await
            .map_err(|e| format!("Agent chat_tools 失败：{}", e.message()))?;

        let message = raw
            .pointer("/choices/0/message")
            .cloned()
            .ok_or("Agent 响应缺少 message")?;
        let calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if calls.is_empty() {
            final_content = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            break;
        }

        messages.push(message);
        for call in &calls {
            let call_id = call.get("id").and_then(Value::as_str).unwrap_or_default();
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !allowed_names.contains(&name.to_string()) {
                return Err(format!("Agent 请求了未绑定的工具：{name}"));
            }
            let args_str = call
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let args: Value = serde_json::from_str(args_str)
                .map_err(|_| "Agent 工具参数不是合法 JSON".to_string())?;

            let output = rustset_ai_server::tools::execute(pool, user, name, &args)
                .await
                .unwrap_or_else(|e| format!("工具执行失败：{e}"));
            if !tools_used.contains(&name.to_string()) {
                tools_used.push(name.to_string());
            }
            messages.push(json!({"role": "tool", "tool_call_id": call_id, "content": output}));
        }
    }

    if final_content.is_empty() {
        final_content = format!("Agent 在 {} 轮内未产出最终回复", max_iterations);
    }

    let output_key = config
        .get("outputKey")
        .and_then(Value::as_str)
        .unwrap_or("agent_result");
    if let Some(obj) = context.as_object_mut() {
        obj.insert(output_key.to_string(), json!(final_content));
    }

    Ok(json!({
        "content": final_content,
        "iterations": rounds_used,
        "toolsUsed": tools_used,
        "outputKey": output_key,
    }))
}

/// 保存 context 到实例（LLM/knowledge/agent 执行完后调用）。
pub(crate) async fn persist_context(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    context: &Value,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE infra_flow_instance SET context=$3, update_time=now()
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .bind(context)
    .execute(pool)
    .await
    .map_err(|e| format!("context 回写失败：{e}"))?;
    Ok(())
}
