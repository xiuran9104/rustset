//! 变量插值引擎：`{{source.key.subkey}}` 在任意模板字符串中替换为实际值。
//!
//! source: "payload" → 实例 payload（不可变输入）
//!         "context" → 实例 context（含上游 LLM/知识/Agent 节点的输出）
//!         "steps"   → 按 steps.{key}.output.{field} 查找 context（等价于 context.{outputKey}.{field}）
//!
//! 安全规则：只解析已知前缀；未命中保留原文（`{{xxx}}`），不报错——
//! LLM prompt 里的 JSON 示例不应被解析破坏。

use serde_json::Value;

/// 把 template 中的 `{{source.key.path}}` 替换为实际值。
/// 纯函数，无 IO，单测覆盖。
pub(crate) fn interpolate(template: &str, payload: &Value, context: &Value) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end_rel) = after.find("}}") else {
            // 没有闭合，剩余全是原文
            result.push_str(&rest[start..]);
            return result;
        };
        let var_path = &after[..end_rel];
        let resolved = resolve_var(var_path, payload, context);
        result.push_str(&resolved);
        rest = &after[end_rel + 2..];
    }
    result.push_str(rest);
    result
}

fn resolve_var(path: &str, payload: &Value, context: &Value) -> String {
    let path = path.trim();
    if path.is_empty() {
        return format!("{{{{{}}}}}", path);
    }

    // steps.{key}.output.{field} → 直接在 context 里查 {key}.{field}
    // （执行器把输出写入 context.{outputKey}，steps 是语义别名）
    if let Some(stripped) = path.strip_prefix("steps.") {
        // steps.key.output.field → context.key.field
        if let Some(pos) = stripped.find(".output.") {
            let key = &stripped[..pos];
            let field = &stripped[pos + ".output.".len()..];
            return lookup_in_context(context, key, field, path);
        }
        // steps.key.output → context.key
        if let Some(key) = stripped.strip_suffix(".output") {
            return lookup_in_context(context, key, "", path);
        }
        // steps.key.field（无 output 段，直接当 context key 用）
        let (key, field) = stripped.split_once('.').unwrap_or((stripped, ""));
        return lookup_in_context(context, key, field, path);
    }

    let (source, field_path) = path.split_once('.').unwrap_or((path, ""));
    let root = match source {
        "payload" => payload,
        "context" => context,
        _ => return format!("{{{{{}}}}}", path), // 未知 source 保留原文
    };

    if field_path.is_empty() {
        return value_to_string(root);
    }

    let mut current = root;
    for seg in field_path.split('.') {
        current = match current.get(seg) {
            Some(v) => v,
            None => return format!("{{{{{}}}}}", path), // 未命中保留原文
        };
    }
    value_to_string(current)
}

/// 在 context 中按 key.field 查找（steps 别名解析用）。
fn lookup_in_context(context: &Value, key: &str, field: &str, original: &str) -> String {
    let node = match context.get(key) {
        Some(v) => v,
        None => return format!("{{{{{}}}}}", original),
    };
    if field.is_empty() {
        return value_to_string(node);
    }
    let mut current = node;
    for seg in field.split('.') {
        current = match current.get(seg) {
            Some(v) => v,
            None => return format!("{{{{{}}}}}", original),
        };
    }
    value_to_string(current)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 从 AI 回复文本中提取 JSON：
/// 1. 优先 ```json ... ``` 代码块
/// 2. 回退：第一个 { 到最后一个 }
/// 3. serde_json 解析
pub(crate) fn extract_json(text: &str) -> Option<Value> {
    // 尝试 ```json ... ``` 代码块
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            if let Ok(v) = serde_json::from_str(after[..end].trim()) {
                return Some(v);
            }
        }
    }
    // 尝试 ``` ... ``` 代码块（无语言标记）
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        // 跳过可能的语言标记行
        let after = after.split_once('\n').map(|(_, r)| r).unwrap_or(after);
        if let Some(end) = after.find("```") {
            if let Ok(v) = serde_json::from_str(after[..end].trim()) {
                return Some(v);
            }
        }
    }
    // 回退：第一个 { 或 [ 到最后一个 } 或 ]
    let obj_start = text.find('{');
    let arr_start = text.find('[');
    let (start_ch, start, end_ch) = match (obj_start, arr_start) {
        (Some(o), Some(a)) if o < a => ('{', o, '}'),
        (Some(o), None) => ('{', o, '}'),
        (_, Some(a)) => ('[', a, ']'),
        (None, None) => return None,
    };
    let end = text.rfind(end_ch)?;
    if end <= start {
        return None;
    }
    let candidate = &text[start..=end];
    // 确保首尾匹配
    if !candidate.starts_with(start_ch) || !candidate.ends_with(end_ch) {
        return None;
    }
    serde_json::from_str(candidate).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn interpolate_basic_payload() {
        let payload = json!({"name": "web-server", "cpu": 4, "type": "cloud"});
        let ctx = json!({});
        assert_eq!(interpolate("Hello {{payload.name}}", &payload, &ctx), "Hello web-server");
        assert_eq!(interpolate("CPU={{payload.cpu}}", &payload, &ctx), "CPU=4");
        assert_eq!(interpolate("{{payload.type}}", &payload, &ctx), "cloud");
    }

    #[test]
    fn interpolate_context_and_nested() {
        let payload = json!({"ecsName": "vm-01"});
        let ctx = json!({
            "risk_analysis": {"level": "high", "reason": "开放高危端口"},
            "kb_context": "安全策略：22端口需要审批"
        });
        assert_eq!(
            interpolate("风险={{context.risk_analysis.level}}", &payload, &ctx),
            "风险=high"
        );
        assert_eq!(
            interpolate("{{context.kb_context}}", &payload, &ctx),
            "安全策略：22端口需要审批"
        );
    }

    #[test]
    fn interpolate_steps_alias() {
        let payload = json!({});
        let ctx = json!({"risk_analysis": {"level": "medium"}});
        // steps.{key}.output.{field} → context.{key}.{field}
        assert_eq!(
            interpolate("{{steps.risk_analysis.output.level}}", &payload, &ctx),
            "medium"
        );
        // steps.{key}.output → context.{key}（对象转 JSON 字符串）
        assert_eq!(
            interpolate("{{steps.risk_analysis.output}}", &payload, &ctx),
            r#"{"level":"medium"}"#
        );
    }

    #[test]
    fn interpolate_unknown_source_kept_as_is() {
        let payload = json!({});
        let ctx = json!({});
        assert_eq!(
            interpolate("{{unknown.var}}", &payload, &ctx),
            "{{unknown.var}}"
        );
        assert_eq!(
            interpolate("{{random}}", &payload, &ctx),
            "{{random}}"
        );
    }

    #[test]
    fn interpolate_missing_field_kept_as_is() {
        let payload = json!({"name": "x"});
        let ctx = json!({});
        assert_eq!(
            interpolate("{{payload.nonexistent}}", &payload, &ctx),
            "{{payload.nonexistent}}"
        );
        assert_eq!(
            interpolate("{{payload.a.b.c}}", &payload, &ctx),
            "{{payload.a.b.c}}"
        );
    }

    #[test]
    fn interpolate_no_vars_passthrough() {
        let payload = json!({});
        let ctx = json!({});
        assert_eq!(interpolate("no vars here", &payload, &ctx), "no vars here");
        assert_eq!(interpolate("", &payload, &ctx), "");
    }

    #[test]
    fn interpolate_json_example_in_prompt_preserved() {
        // LLM prompt 里的 JSON 示例不应被破坏（{{ 不是变量语法时）
        let payload = json!({"cpu": 4});
        let ctx = json!({});
        let template = r#"返回 {"level": "high"} 格式，不要输出 {{unknown}}"#;
        let result = interpolate(template, &payload, &ctx);
        assert!(result.contains(r#""level": "high""#));
        assert!(result.contains("{{unknown}}")); // 未知 source 保留
    }

    #[test]
    fn interpolate_multiple_vars_in_one_template() {
        let payload = json!({"name": "vm-01", "type": "cloud"});
        let ctx = json!({"risk": {"level": "high"}});
        let result = interpolate(
            "工单 {{payload.name}}（{{payload.type}}）风险={{context.risk.level}}",
            &payload, &ctx,
        );
        assert_eq!(result, "工单 vm-01（cloud）风险=high");
    }

    #[test]
    fn interpolate_unclosed_brace() {
        let payload = json!({});
        let ctx = json!({});
        assert_eq!(interpolate("hello {{payload", &payload, &ctx), "hello {{payload");
    }

    // ===== extract_json tests =====

    #[test]
    fn extract_json_code_block() {
        let text = "分析结果：\n```json\n{\"level\": \"high\", \"reason\": \"高危端口\"}\n```\n请参考";
        let v = extract_json(text).unwrap();
        assert_eq!(v["level"], "high");
    }

    #[test]
    fn extract_json_bare_code_block() {
        let text = "```\n{\"level\": \"low\"}\n```";
        let v = extract_json(text).unwrap();
        assert_eq!(v["level"], "low");
    }

    #[test]
    fn extract_json_inline() {
        let text = r#"根据分析 {"level":"medium","ports":[22,3389]} 请处理"#;
        let v = extract_json(text).unwrap();
        assert_eq!(v["level"], "medium");
        assert_eq!(v["ports"][1], 3389);
    }

    #[test]
    fn extract_json_no_json() {
        assert!(extract_json("这是普通文本，没有 JSON").is_none());
        assert!(extract_json("").is_none());
    }

    #[test]
    fn extract_json_array() {
        let text = r#"结果：[{"name":"a"},{"name":"b"}]"#;
        let v = extract_json(text).unwrap();
        assert!(v.is_array());
        assert_eq!(v[1]["name"], "b");
    }
}
