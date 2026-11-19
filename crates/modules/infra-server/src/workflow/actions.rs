//! 自动动作注册表：auto 步骤的执行器（match 风格，同 ai tools.rs）。
//!
//! v1 动作：firewall_push / trigger_scan / create_ticket / set_context。
//! v2 新增：ticket_approve / ticket_provision / ticket_deliver（工单链吸收）。
//! v3 新增：http_request（SSRF 防护）/ start_flow（跨实例调用）。
//! firewall_push 走既有自动化框架（开关/熔断/审计）；失败进入退避重试
//! （最多 3 次），重试由 runner 按 retry_at 到期后调 engine::advance 驱动。

use rustset_framework_database::DbPool;
use rustset_framework_tenant::TenantContext;
use serde_json::{Value, json};
use sqlx::Row;

/// 已知动作名（校验用）。
pub(crate) fn is_known_action(action: &str) -> Option<&'static str> {
    const ACTIONS: [&str; 12] = [
        "firewall_push",
        "trigger_scan",
        "create_ticket",
        "set_context",
        "check_risk_port",
        "register_terminal",
        "update_terminal",
        "ticket_approve",
        "ticket_provision",
        "ticket_deliver",
        "http_request",
        "start_flow",
    ];
    ACTIONS.iter().find(|a| **a == action).copied()
}

/// 执行一个 auto 步骤：读取实例/步骤 → 分发动作 → 落结果并推进。
/// 成功 → step done + engine::advance；失败 → attempt+1、retry_at 退避，
/// attempt>=3 → failed（engine::advance 收尾实例为 failed）。
pub(crate) async fn execute_step(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    step_id: i64,
) {
    if let Err(error) = execute_step_inner(state, tenant, instance_id, step_id).await {
        tracing::warn!(instance_id, step_id, %error, "flow auto step failed");
    }
}

async fn execute_step_inner(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    step_id: i64,
) -> Result<(), String> {
    let pool = &state.pool;
    // 先置 running 标记（attempt 在此累加，退避由结果路径写回）。
    let step = sqlx::query(
        "SELECT step_key, name, type, config, attempt_count, status FROM infra_flow_step
         WHERE id=$1 AND tenant_id=$2 AND instance_id=$3",
    )
    .bind(step_id)
    .bind(tenant.id())
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("step not found")?;
    if step.get::<&str, _>("status") != "active" {
        return Ok(());
    }
    let config: Value = step.get("config");
    let attempt: i32 = step.get("attempt_count");
    let action = config
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let params = config.get("params").cloned().unwrap_or_else(|| json!({}));

    let instance = sqlx::query(
        "SELECT payload, context, title FROM infra_flow_instance
         WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    let payload: Value = instance.get("payload");
    let mut context: Value = instance.get("context");

    // AI 节点（llm/knowledge/agent）：按 step type 分发，不走 action 注册表。
    let step_type: String = step.get("type");
    let outcome: Result<Value, String> = match step_type.as_str() {
        "llm" => {
            let result = super::ai_nodes::execute_llm(pool, &config, &payload, &mut context).await;
            // context 被修改，回写实例
            if result.is_ok() {
                let _ = super::ai_nodes::persist_context(pool, tenant, instance_id, &context).await;
            }
            result
        }
        "knowledge" => {
            let result =
                super::ai_nodes::execute_knowledge(pool, &config, &payload, &mut context).await;
            if result.is_ok() {
                let _ = super::ai_nodes::persist_context(pool, tenant, instance_id, &context).await;
            }
            result
        }
        "agent" => {
            let sys_user = super::ai_nodes::system_user();
            let result =
                super::ai_nodes::execute_agent(pool, &sys_user, &config, &payload, &mut context).await;
            if result.is_ok() {
                let _ = super::ai_nodes::persist_context(pool, tenant, instance_id, &context).await;
            }
            result
        }
        // auto 节点：按 action 分发（原有逻辑）。
        // 参数先做 Value 级变量插值（Windmill 风格：任何动作参数都可引用
        // payload/context，http_request 的 URL/body、start_flow 的 payload 等
        // 全部生效）；SSRF 校验在插值后的 URL 上照常执行。
        _ => {
            let params = super::variables::interpolate_value(&params, &payload, &context);
            let outcome: Result<Value, String> = match action.as_str() {
        "firewall_push" => firewall_push(pool, tenant, instance_id, &payload, &params).await,
        "trigger_scan" => trigger_scan(pool, tenant, instance_id, &payload, &params).await,
        "create_ticket" => create_ticket(pool, tenant, instance_id, &payload, &params).await,
        "set_context" => set_context(&mut context, &payload, &params),
        "check_risk_port" => check_risk_port(pool, &mut context, &params).await,
        "register_terminal" => register_terminal(pool, tenant, instance_id, &payload).await,
        "update_terminal" => update_terminal(pool, tenant, &params).await,
        "ticket_approve" => ticket_approve(state, tenant, instance_id, &params).await,
        "ticket_provision" => ticket_provision(state, tenant, instance_id, &params).await,
        "ticket_deliver" => ticket_deliver(state, tenant, instance_id, &params).await,
        "http_request" => http_request(state, instance_id, &params).await,
        "start_flow" => start_flow(state, tenant, instance_id, step_id, &params).await,
        other => Err(format!("未知自动动作：{other}")),
            };
            outcome
        }
    };

    match outcome {
        Ok(result) => {
            // start_flow wait 模式：步骤保持 active，由 runner 轮询子实例终态。
            if result.get("mode").and_then(Value::as_str) == Some("wait") {
                sqlx::query(
                    "UPDATE infra_flow_step
                     SET result=result || $3, retry_at=now(), attempt_count=attempt_count+1
                     WHERE id=$1 AND tenant_id=$2",
                )
                .bind(step_id)
                .bind(tenant.id())
                .bind(&result)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                super::engine::advance(state, tenant, instance_id).await;
                return Ok(());
            }
            // set_context / check_risk_port 需要把更新后的 context 写回实例。
            if action == "set_context" || action == "check_risk_port" {
                sqlx::query(
                    "UPDATE infra_flow_instance SET context=$3, update_time=now()
                     WHERE id=$1 AND tenant_id=$2",
                )
                .bind(instance_id)
                .bind(tenant.id())
                .bind(&context)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status='done', finished_at=now(), attempt_count=$3,
                     result=result || $4, operator='system'
                 WHERE id=$1 AND tenant_id=$2",
            )
            .bind(step_id)
            .bind(tenant.id())
            .bind(attempt + 1)
            .bind(json!({ "output": result }))
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        Err(message) => {
            // 退避重试：5/10/20 秒（同任务队列节奏），上限 3 次。
            let backoff_secs: i64 = 5 * (1 << attempt.clamp(0, 2));
            let exhausted = attempt + 1 >= 3;
            let next_status = if exhausted { "failed" } else { "active" };
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status=$4, attempt_count=$3, retry_at=$5,
                     finished_at=CASE WHEN $4='failed' THEN now() ELSE finished_at END,
                     result=result || $6
                 WHERE id=$1 AND tenant_id=$2",
            )
            .bind(step_id)
            .bind(tenant.id())
            .bind(attempt + 1)
            .bind(next_status)
            .bind(
                chrono::Utc::now()
                    .checked_add_signed(chrono::Duration::seconds(backoff_secs))
                    .map(|t| t.naive_utc()),
            )
            .bind(json!({ "lastError": message, "attempt": attempt + 1 }))
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    super::engine::advance(state, tenant, instance_id).await;
    Ok(())
}

/// 防火墙策略下发：payload 里直接带 fw* 字段（或 params.ticketField 指定
/// 嵌套对象），经自动化开关/熔断决策后执行，审计 action=flow_auto_push。
async fn firewall_push(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    payload: &Value,
    params: &Value,
) -> Result<Value, String> {
    // 参数源：默认 payload 本身；params.ticketField 指定嵌套对象（如 "ticket"）。
    let source = params
        .get("ticketField")
        .and_then(Value::as_str)
        .and_then(|field| payload.get(field))
        .unwrap_or(payload);
    let field = |key: &str| {
        source
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let request = crate::automation::vendors::PolicyPushRequest {
        // 策略名带 F 前缀区分来源：FLOW-F{实例}-...
        policy_name: format!(
            "FLOW-F{}-{}-{}",
            instance_id,
            field("fwSourceAddress").replace('/', "_"),
            field("fwDestPort").replace('-', "_")
        ),
        source_zone: field("fwSourceZone"),
        source_address: field("fwSourceAddress"),
        dest_zone: field("fwDestZone"),
        dest_address: field("fwDestAddress"),
        service_port: field("fwDestPort"),
        protocol: field("fwProtocol"),
        direction: field("fwDirection"),
        valid_until: {
            let v = field("fwValidUntil");
            if v.is_empty() { None } else { Some(v) }
        },
        ticket_id: instance_id,
    };

    match crate::automation::decide(pool, tenant.id(), "firewall_provision").await {
        crate::automation::AutomationDecision::Auto(config) => {
            let timer = crate::automation::AutoTimer::start();
            let result = crate::automation::firewall::execute_policy(
                pool,
                tenant.id(),
                config.scope_config.as_deref().unwrap_or(""),
                &request,
            )
            .await;
            let elapsed = timer.elapsed_ms();
            if result.success {
                crate::automation::on_success(pool, tenant.id(), "firewall_provision").await;
            } else {
                crate::automation::on_failure(
                    pool,
                    tenant.id(),
                    "firewall_provision",
                    &result.detail,
                )
                .await;
            }
            crate::automation::audit(
                pool,
                tenant.id(),
                "firewall_provision",
                Some(instance_id),
                "flow_auto_push",
                if result.success { "executed" } else { "failed" },
                &result.detail,
                Some(elapsed),
            )
            .await;
            if result.success {
                Ok(json!({ "policyName": request.policy_name, "detail": result.detail }))
            } else {
                Err(format!("防火墙下发失败：{}", result.detail))
            }
        }
        crate::automation::AutomationDecision::Manual
        | crate::automation::AutomationDecision::NotConfigured => {
            // 开关关闭/未配置：不做也不算失败——流程继续（人工路径由 manual 步骤表达）。
            Ok(json!({ "skipped": "automation disabled or not configured" }))
        }
        crate::automation::AutomationDecision::CircuitOpen => {
            Err("防火墙自动化熔断已触发，稍后重试或人工处理".to_string())
        }
    }
}

/// 触发扫描任务：params {name?, target?, taskKind?, ports?}。
async fn trigger_scan(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    payload: &Value,
    params: &Value,
) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("流程触发扫描 F{instance_id}"));
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .or_else(|| payload.get("target").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    if target.trim().is_empty() {
        return Err("trigger_scan 需要目标（target）".to_string());
    }
    // 目标解析为 IP 列表（逗号/空白分隔，非法项报错）。
    let targets: Vec<std::net::IpAddr> = target
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<std::net::IpAddr>()
                .map_err(|_| format!("无效 IP：{s}"))
        })
        .collect::<Result<_, _>>()?;
    let ports: Vec<i32> = params
        .get("ports")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_i64).map(|p| p as i32).collect())
        .unwrap_or_default();
    let task_kind = params
        .get("taskKind")
        .and_then(Value::as_str)
        .unwrap_or("scan");
    let policy = params
        .get("policy")
        .and_then(Value::as_str)
        .unwrap_or("quick");
    let task_id = crate::task::enqueue(
        pool,
        tenant,
        "flow",
        &name,
        &target,
        task_kind,
        policy,
        targets,
        ports,
        Some(&format!("flow:{instance_id}")),
        None,
        None,
        &params.get("options").cloned().unwrap_or_else(|| json!({})),
    )
    .await
    .map_err(|e| e.message().to_string())?;
    Ok(json!({ "taskId": task_id }))
}

/// 创建资源工单：params 即工单字段（camelCase，支持 {{payload.x}} 插值——
/// 插值在 execute_step 分发前统一完成）。流程路径无登录态，DB 必填列
/// （ticket_status/created_by）与工单链默认值在此补齐。
async fn create_ticket(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    payload: &Value,
    params: &Value,
) -> Result<Value, String> {
    let mut ticket_payload = params.clone();
    if let Some(object) = ticket_payload.as_object_mut() {
        // 工单名称默认（工单链的 name 即 ecs_name 之外展示名，流程里用工单名）。
        object
            .entry("ecsName".to_string())
            .or_insert_with(|| json!(format!("流程创建 F{instance_id}")));
        // DB NOT NULL 无默认的列 + 工单链缺省（同 ticket.rs HTTP 路径语义）。
        object
            .entry("ticketStatus".to_string())
            .or_insert_with(|| json!("pending_approval"));
        object
            .entry("createdBy".to_string())
            .or_insert_with(|| json!("flow"));
        object
            .entry("ticketType".to_string())
            .or_insert_with(|| json!("create"));
        object
            .entry("riskLevel".to_string())
            .or_insert_with(|| json!("normal"));
        // 申请人：申报 payload 带 applicantName 时透传（通知/台账归属）。
        if !object.contains_key("applicantName") {
            if let Some(name) = payload.get("applicantName") {
                if !name.is_null() {
                    object.insert("applicantName".to_string(), name.clone());
                }
            }
        }
    } else {
        return Err("create_ticket params 必须是对象".to_string());
    }
    let (response, _) = crate::ticket::repository::create(pool, tenant, ticket_payload)
        .await
        .map_err(|e| e.message().to_string())?;
    let ticket_id = response.0.data;
    // 关联实例业务对象，供 ticket_wait / 前端跳转。
    let _ = sqlx::query(
        "UPDATE infra_flow_instance SET biz_type='ticket', biz_id=$3, update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND (biz_id='' OR biz_id IS NULL)",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .bind(&ticket_id)
    .execute(pool)
    .await;
    Ok(json!({ "ticketId": ticket_id }))
}

/// 写流程上下文：params {key, value} 或 {key, fromPayloadField}。
fn set_context(context: &mut Value, payload: &Value, params: &Value) -> Result<Value, String> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or("set_context 需要 key")?;
    let value = if let Some(field) = params.get("fromPayloadField").and_then(Value::as_str) {
        payload.get(field).cloned().ok_or(format!("payload 缺少字段 {field}"))?
    } else {
        params.get("value").cloned().unwrap_or(Value::Null)
    };
    if let Some(object) = context.as_object_mut() {
        object.insert(key.to_string(), value.clone());
    }
    Ok(json!({ "key": key, "value": value }))
}

// ==================== 高危端口检测（网络策略申请前置） ====================

/// 解析端口描述为区间列表："443"、"8000-8010"、"80,443"（逗号/空白分隔，
/// `a-b` 区间）。空串返回空列表（视为无检测目标）；非法片段报错。
fn parse_port_ranges(spec: &str) -> Result<Vec<(i32, i32)>, String> {
    let mut ranges = Vec::new();
    for token in spec.split([',', ' ', '\t', '，']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (start, end) = if let Some((a, b)) = token.split_once('-') {
            let start: i32 = a.trim().parse().map_err(|_| format!("无效端口：{token}"))?;
            let end: i32 = b.trim().parse().map_err(|_| format!("无效端口：{token}"))?;
            (start, end)
        } else {
            let port: i32 = token.parse().map_err(|_| format!("无效端口：{token}"))?;
            (port, port)
        };
        if !(1..=65535).contains(&start) || !(1..=65535).contains(&end) {
            return Err(format!("端口超出范围（1-65535）：{token}"));
        }
        if start > end {
            return Err(format!("端口区间起点大于终点：{token}"));
        }
        ranges.push((start, end));
    }
    Ok(ranges)
}

/// 端口区间与规则区间是否相交。
fn ranges_overlap(rule: (i32, i32), port: (i32, i32)) -> bool {
    rule.0 <= port.1 && rule.1 >= port.0
}

/// 高危端口检测（通用前置节点）：params {port, sourcePort?, protocol?}——
/// 支持插值（如 "port":"{{payload.fwDestPort}}"）。与
/// infra_high_risk_port_rule（启用规则，协议 any 或与请求一致）求区间交集，
/// 结果写入实例 context，供下游引用：
///   risk_port_hit（bool）/ risk_level（"high"|"normal"）/
///   risk_port_rules（命中规则名逗号串）/ risk_ports（命中端口区间串）
/// 下游用法：安全会签 condition {"field":"risk_port_hit","op":"eq","value":true}；
/// create_ticket 参数 "riskLevel":"{{context.risk_level}}"（类型保型）。
/// 检测口径是 ticket.rs 内联检测的超集：全区间交集（旧口径只查首个端口）。
async fn check_risk_port(
    pool: &DbPool,
    context: &mut Value,
    params: &Value,
) -> Result<Value, String> {
    let port_spec = params.get("port").and_then(Value::as_str).unwrap_or_default();
    let source_spec = params
        .get("sourcePort")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let protocol = params
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("any")
        .to_lowercase();
    // 插值未命中保留的 {{...}} 占位视为未提供（通用动作防御：缺失字段
    // 不应让步骤失败，等同空串）。
    let unresolved = |spec: &str| spec.contains("{{") && spec.contains("}}");
    let port_spec = if unresolved(port_spec) { "" } else { port_spec };
    let source_spec = if unresolved(source_spec) { "" } else { source_spec };
    let protocol = if unresolved(&protocol) { "any".to_string() } else { protocol };

    let mut ranges = parse_port_ranges(port_spec)?;
    if !source_spec.trim().is_empty() {
        ranges.extend(parse_port_ranges(source_spec)?);
    }

    let write_context = |context: &mut Value, hit: bool, rules: &[String], ports: &[String]| {
        let level = if hit { "high" } else { "normal" };
        if let Some(object) = context.as_object_mut() {
            object.insert("risk_port_hit".into(), json!(hit));
            object.insert("risk_level".into(), json!(level));
            object.insert("risk_port_rules".into(), json!(rules.join(", ")));
            object.insert("risk_ports".into(), json!(ports.join(", ")));
        }
    };

    if ranges.is_empty() {
        write_context(context, false, &[], &[]);
        return Ok(json!({ "riskPortHit": false, "riskLevel": "normal", "checkedRanges": 0 }));
    }

    // 规则表为全局基线（同 ticket.rs 检测口径，无租户过滤）。
    let rules: Vec<(String, String, i32, i32)> = sqlx::query_as(
        "SELECT name, protocol, port_start, port_end FROM infra_high_risk_port_rule
         WHERE enabled AND deleted=0 ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut hit_rules: Vec<String> = Vec::new();
    let mut hit_ports: Vec<String> = Vec::new();
    for (name, rule_protocol, rule_start, rule_end) in &rules {
        // 协议匹配：规则 any 或请求 any（不限协议）匹配一切；否则需一致。
        if rule_protocol != "any" && protocol != "any" && *rule_protocol != protocol {
            continue;
        }
        for port_range in &ranges {
            if ranges_overlap((*rule_start, *rule_end), *port_range) {
                if !hit_rules.contains(name) {
                    hit_rules.push(name.clone());
                }
                hit_ports.push(format!(
                    "{}-{}",
                    port_range.0.max(*rule_start),
                    port_range.1.min(*rule_end)
                ));
                break;
            }
        }
    }
    let hit = !hit_rules.is_empty();
    write_context(context, hit, &hit_rules, &hit_ports);
    Ok(json!({
        "riskPortHit": hit,
        "riskLevel": if hit { "high" } else { "normal" },
        "matchedRules": hit_rules,
        "checkedRanges": ranges.len(),
    }))
}

/// 工单审批：params {ticketId 或 ticketIdFrom=instance biz_id, approved, comment}。
/// 复用 ticket::approve_core（含防火墙自动化 + 既有资源回写）。
async fn ticket_approve(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    params: &Value,
) -> Result<Value, String> {
    let ticket_id = resolve_ticket_id(state, tenant, instance_id, params).await?;
    let approved = params.get("approved").and_then(Value::as_bool).unwrap_or(true);
    let comment = params.get("comment").and_then(Value::as_str);
    let operator = format!("flow:F{instance_id}");
    let (new_status, _) = crate::ticket::approve_core(state, tenant, ticket_id, &operator, approved, comment)
        .await
        .map_err(|e| e.message().to_string())?;
    Ok(json!({ "ticketId": ticket_id, "ticketStatus": new_status }))
}

/// 工单供应：params {ticketId, config?}。复用 provisioning::run（tofu）。
async fn ticket_provision(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    params: &Value,
) -> Result<Value, String> {
    let ticket_id = resolve_ticket_id(state, tenant, instance_id, params).await?;
    let config = params.get("config").cloned().unwrap_or_else(|| json!({}));
    let operator = format!("flow:F{instance_id}");
    let result = crate::ticket::provisioning::run(state, tenant, ticket_id, &operator, &config)
        .await
        .map_err(|e| e.message().to_string())?;
    Ok(json!({ "ticketId": ticket_id, "provision": result }))
}

/// 工单交付：params {ticketId, comment?, fileIds?}。复用 ticket::deliver_core。
async fn ticket_deliver(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    params: &Value,
) -> Result<Value, String> {
    let ticket_id = resolve_ticket_id(state, tenant, instance_id, params).await?;
    let comment = params.get("comment").and_then(Value::as_str);
    let file_ids: Option<Vec<i64>> = params
        .get("fileIds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_i64).collect());
    let operator = format!("flow:F{instance_id}");
    crate::ticket::deliver_core(
        state,
        tenant,
        ticket_id,
        &operator,
        comment,
        file_ids.as_deref(),
    )
    .await
    .map_err(|e| e.message().to_string())?;
    Ok(json!({ "ticketId": ticket_id, "delivered": true }))
}

/// 解析工单 id：params.ticketId 直填，或从实例 biz_id 取（create_ticket 落的关联）。
async fn resolve_ticket_id(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    params: &Value,
) -> Result<i64, String> {
    if let Some(id) = params.get("ticketId").and_then(Value::as_i64) {
        return Ok(id);
    }
    let biz_id: Option<String> = sqlx::query_scalar(
        "SELECT biz_id FROM infra_flow_instance WHERE id=$1 AND tenant_id=$2",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?
    .flatten();
    biz_id
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "ticketId 未指定且实例无工单关联（biz_id）".to_string())
}

// ==================== V3: HTTP 通用节点 ====================

/// SSRF 防护：校验 URL 安全性。
/// 仅 http/https；解析 host 后拒绝 loopback/private/link-local/reserved。
/// 返回 Err(原因) 或 Ok(解析后的 URL)。
async fn validate_http_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|e| format!("URL 无效：{e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("不允许的协议 {other}（仅 http/https）")),
    }
    let host = url.host_str().ok_or("URL 缺少主机名")?;
    // 直接 IP 检查
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if is_forbidden_ip(&ip) {
            return Err(format!("禁止访问受保护地址 {host}"));
        }
        return Ok(url);
    }
    // 域名：DNS 解析后逐 IP 检查
    // 注意：TOCTOU（解析后请求时可能换 IP）由 redirect policy none 缓解
    let addrs = tokio::net::lookup_host(format!("{host}:{}", url.port_or_known_default().unwrap_or(80)))
        .await
        .map_err(|e| format!("DNS 解析失败：{e}"))?;
    for addr in addrs {
        if is_forbidden_ip(&addr.ip()) {
            return Err(format!("域名 {host} 解析到受保护地址 {}", addr.ip()));
        }
    }
    Ok(url)
}

fn is_forbidden_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.octets()[0] == 0 // 0.0.0.0/8
                || v4.octets()[0] == 10 // 10.0.0.0/8（is_private 覆盖）
                || v4.octets()[0] == 127 // 127.0.0.0/8（is_loopback 覆盖）
                || (v4.octets()[0] == 172 && (v4.octets()[1] & 0xF0) == 16) // 172.16/12
                || (v4.octets()[0] == 192 && v4.octets()[1] == 168) // 192.168/16
                || (v4.octets()[0] == 169 && v4.octets()[1] == 254) // link-local
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.segments()[0] & 0xFE00 == 0xFC00 // ULA fc00::/7
                || v6.segments()[0] & 0xFFC0 == 0xFE80 // link-local fe80::/10
                || v6.segments()[0] & 0xFF00 == 0xFF00 // multicast ff00::/8
        }
    }
}

/// HTTP 请求节点：params {method, url, headers?, body?, timeoutSeconds?}。
/// SSRF 防护：scheme 校验 + IP 黑名单 + 禁 redirect。
async fn http_request(
    _state: &crate::InfraState,
    _instance_id: i64,
    params: &Value,
) -> Result<Value, String> {
    let url_raw = params
        .get("url")
        .and_then(Value::as_str)
        .ok_or("http_request 需要 url")?;
    let url = validate_http_url(url_raw).await?;
    let method = params
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err(format!("不支持的方法 {method}"));
    }
    let timeout_secs = params
        .get("timeoutSeconds")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .clamp(1, 120);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::none()) // SSRF：禁 redirect
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败：{e}"))?;

    let mut request = client.request(
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "无效方法")?,
        url,
    );
    if let Some(headers) = params.get("headers").and_then(Value::as_object) {
        for (key, value) in headers {
            if let (Some(k), Some(v)) = (Some(key.as_str()), value.as_str()) {
                request = request.header(k, v);
            }
        }
    }
    if let Some(body) = params.get("body") {
        if !body.is_null() {
            request = request.json(body);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("HTTP 请求失败：{e}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body_text = response
        .text()
        .await
        .unwrap_or_default();
    // 截断到 512 字符存结果
    let truncated = if body_text.len() > 512 {
        format!("{}...[截断，共 {} 字符]", &body_text[..512], body_text.len())
    } else {
        body_text
    };
    Ok(json!({
        "status": status,
        "contentType": content_type,
        "body": truncated,
    }))
}

// ==================== V3: 跨实例调用 ====================

/// 启动子流程：params {definitionId 或 definitionName, payload?, wait?}。
/// wait=false（默认）fire-and-forget；wait=true 转轮询等待子实例终态。
async fn start_flow(
    state: &crate::InfraState,
    tenant: &TenantContext,
    instance_id: i64,
    step_id: i64,
    params: &Value,
) -> Result<Value, String> {
    let pool = &state.pool;
    // 定位目标定义
    let definition = if let Some(def_id) = params.get("definitionId").and_then(Value::as_i64) {
        sqlx::query(
            "SELECT id, name, definition, version FROM infra_flow_definition
             WHERE id=$1 AND tenant_id=$2 AND deleted=0 AND status=0",
        )
        .bind(def_id)
        .bind(tenant.id())
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
    } else if let Some(def_name) = params.get("definitionName").and_then(Value::as_str) {
        sqlx::query(
            "SELECT id, name, definition, version FROM infra_flow_definition
             WHERE name=$1 AND tenant_id=$2 AND deleted=0 AND status=0
             ORDER BY id LIMIT 1",
        )
        .bind(def_name)
        .bind(tenant.id())
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        return Err("start_flow 需要 definitionId 或 definitionName".to_string());
    };
    let Some(definition) = definition else {
        return Err("目标流程定义不存在或已停用".to_string());
    };

    let child_payload = params
        .get("payload")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let starter = format!("flow:F{instance_id}");

    let child_id = super::instance::start_instance(
        state,
        tenant,
        definition.get::<i64, _>("id"),
        &definition.get::<String, _>("name"),
        definition.get::<i32, _>("version"),
        &(definition.get::<Value, _>("definition")),
        Some(format!("子流程（由 F{instance_id} 触发）")),
        child_payload.clone(),
        "flow_child",
        &instance_id.to_string(),
        &starter,
    )
    .await
    .map_err(|e| e.message().to_string())?;

    // 推进子实例（激活首个步骤）
    super::engine::advance(state, tenant, child_id).await;

    let wait = params.get("wait").and_then(Value::as_bool).unwrap_or(false);
    if wait {
        // 等待模式：本步骤转轮询，由 runner 检查子实例终态
        sqlx::query(
            "UPDATE infra_flow_step
             SET result=result || $3, retry_at=now()
             WHERE id=$1 AND tenant_id=$2",
        )
        .bind(step_id)
        .bind(tenant.id())
        .bind(json!({ "childInstanceId": child_id, "waiting": true }))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        // 不返回 Ok——让调用方知道我们在等待；步骤保持 active 由 runner 轮询。
        // 但 execute_step 的 Ok 分支会置 done，所以这里返回特殊标记。
        // 实际上等待模式的处理在 runner 的 handle_ticket_waits 扩展中。
        // 这里先返回 child_id，execute_step 会置 done——但这不对。
        // 正确做法：等待时返回 Err 让步骤保持 active？
        // 不，更好的做法：返回一个特殊 Ok 值，在 execute_step 里识别并跳过 done。
        Ok(json!({ "childInstanceId": child_id, "mode": "wait" }))
    } else {
        Ok(json!({ "childInstanceId": child_id, "mode": "fire_and_forget" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_port_single_and_range() {
        assert_eq!(parse_port_ranges("443").unwrap(), vec![(443, 443)]);
        assert_eq!(parse_port_ranges("8000-8010").unwrap(), vec![(8000, 8010)]);
        // 逗号/空白/中文逗号分隔
        assert_eq!(
            parse_port_ranges("80, 443，9000-9002").unwrap(),
            vec![(80, 80), (443, 443), (9000, 9002)]
        );
    }

    #[test]
    fn parse_port_empty_is_no_target() {
        assert!(parse_port_ranges("").unwrap().is_empty());
        assert!(parse_port_ranges("  ").unwrap().is_empty());
        // "任意"/"ANY" 等非数字占位视为非法——调用方传空前应自行过滤
        assert!(parse_port_ranges("任意").is_err());
    }

    #[test]
    fn parse_port_invalid_values() {
        assert!(parse_port_ranges("0").is_err()); // 越界（<1）
        assert!(parse_port_ranges("65536").is_err()); // 越界（>65535）
        assert!(parse_port_ranges("9000-8000").is_err()); // 起点>终点
        assert!(parse_port_ranges("80-abc").is_err()); // 非数字
        assert!(parse_port_ranges("-1").is_err()); // 负数（且非区间语法）
    }

    #[test]
    fn ranges_overlap_semantics() {
        assert!(ranges_overlap((80, 80), (80, 80))); // 点重叠
        assert!(ranges_overlap((1, 100), (50, 150))); // 部分相交
        assert!(ranges_overlap((1, 10), (5, 5))); // 规则包含端口
        assert!(ranges_overlap((8080, 8090), (8000, 9000))); // 端口包含规则
        assert!(!ranges_overlap((80, 443), (444, 65535))); // 相邻不交
        assert!(!ranges_overlap((22, 22), (23, 80))); // 错开
    }
}

// ==================== 终端入账（办公网IP申报闭环） ====================

/// 从文本中提取第一个 IPv4（分配 IP 记录在 assign_ip 步骤备注里）。
fn extract_ipv4(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d{1,3}\.){3}\d{1,3}").ok()?;
    re.find(text).map(|m| m.as_str().to_string())
}

/// 终端入账：办公网IP申报交付确认后把终端（MAC/人员/分配IP）写入
/// infra_terminal 台账（资产中心-终端设备）。幂等：同租户同 MAC 覆盖
/// 更新（应用层 upsert，避免方言分支）。分配 IP 从 assign_ip 步骤的
/// 备注/结果提取；payload.requestedIp 兜底。
async fn register_terminal(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    payload: &Value,
) -> Result<Value, String> {
    let field = |key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mac = field("macAddress");
    if mac.is_empty() {
        return Err("register_terminal：payload 缺少 macAddress".to_string());
    }
    // 分配 IP：优先 assign_ip 步骤备注（人工填写），requestedIp 兜底。
    let assigned: Option<String> = sqlx::query_scalar(
        "SELECT comment FROM infra_flow_step
         WHERE instance_id=$1 AND tenant_id=$2 AND step_key='assign_ip' AND comment IS NOT NULL",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .and_then(|comment: String| extract_ipv4(&comment))
    .or_else(|| {
        let requested = field("requestedIp");
        (!requested.is_empty()).then_some(requested)
    });

    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM infra_terminal WHERE tenant_id=$1 AND mac_address=$2 AND deleted=0",
    )
    .bind(tenant.id())
    .bind(&mac)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let status_active = "active";
    if let Some(id) = existing {
        sqlx::query(
            "UPDATE infra_terminal
             SET hostname=$3, device_type=$4, ip_address=$5, network_zone=$6,
                 access_period=$7, applicant_name=$8, department=$9, employee_id=$10,
                 phone=$11, reason=$12, flow_instance_id=$13, status=$14,
                 updater='flow', update_time=now()
             WHERE id=$1 AND tenant_id=$2",
        )
        .bind(id)
        .bind(tenant.id())
        .bind(field("hostname"))
        .bind(field("deviceType"))
        .bind(assigned.clone().unwrap_or_default())
        .bind(field("networkZone"))
        .bind(field("accessPeriod"))
        .bind(field("applicantName"))
        .bind(field("department"))
        .bind(field("employeeId"))
        .bind(field("phone"))
        .bind(field("reason"))
        .bind(instance_id)
        .bind(status_active)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(json!({ "terminalId": id, "macAddress": mac, "ipAddress": assigned, "mode": "updated" }))
    } else {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO infra_terminal
                 (mac_address, hostname, device_type, ip_address, network_zone,
                  access_period, applicant_name, department, employee_id, phone,
                  reason, flow_instance_id, status, creator, tenant_id)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'flow',$14) RETURNING id",
        )
        .bind(&mac)
        .bind(field("hostname"))
        .bind(field("deviceType"))
        .bind(assigned.clone().unwrap_or_default())
        .bind(field("networkZone"))
        .bind(field("accessPeriod"))
        .bind(field("applicantName"))
        .bind(field("department"))
        .bind(field("employeeId"))
        .bind(field("phone"))
        .bind(field("reason"))
        .bind(instance_id)
        .bind(status_active)
        .bind(tenant.id())
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(json!({ "terminalId": id, "macAddress": mac, "ipAddress": assigned, "mode": "created" }))
    }
}

// ==================== 终端变更（台账只读，变更走申请） ====================

/// 终端变更：按 params.macAddress 定位台账行（参数已插值），operation =
/// retire → status=retired；update → 非空字段覆盖（使用人/部门/IP/期限）。
/// 台账无删除/直改入口，一切变更经「终端变更申请流程」审批后到达这里。
async fn update_terminal(
    pool: &DbPool,
    tenant: &TenantContext,
    params: &Value,
) -> Result<Value, String> {
    let field = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    let mac = field("macAddress");
    // 插值未命中的 {{...}} 占位视为空（同 check_risk_port 防御）。
    let unresolved = |v: &str| v.contains("{{") && v.contains("}}");
    let mac = if unresolved(&mac) { String::new() } else { mac };
    if mac.is_empty() {
        return Err("update_terminal：缺少 macAddress（无法定位终端）".to_string());
    }
    let operation = field("operation");
    let id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM infra_terminal WHERE tenant_id=$1 AND mac_address=$2 AND deleted=0",
    )
    .bind(tenant.id())
    .bind(&mac)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(id) = id else {
        return Err(format!("终端台账中不存在 MAC：{mac}"));
    };

    if operation == "retire" {
        sqlx::query(
            "UPDATE infra_terminal SET status='retired', updater='flow', update_time=now()
             WHERE id=$1 AND tenant_id=$2",
        )
        .bind(id)
        .bind(tenant.id())
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(json!({ "terminalId": id, "macAddress": mac, "operation": "retire", "status": "retired" }));
    }

    // update：非空字段覆盖（信创便携：固定列更新，无动态 SQL 拼接）。
    let (new_applicant, new_department, new_ip, new_period) = (
        field("newApplicantName"),
        field("newDepartment"),
        field("newIpAddress"),
        field("newAccessPeriod"),
    );
    let current: Value = sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM infra_terminal t WHERE id=$1 AND tenant_id=$2",
    )
    .bind(id)
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    // 覆盖值与保留值统一为字符串：Value 直接 bind 到 varchar 列会带
    // JSON 引号/Null 编码落库（E2E 抓获），此处显式 as_str 转换。
    let pick = |json_field: &str, new_value: &str| -> String {
        if !new_value.is_empty() && !unresolved(new_value) {
            new_value.to_string()
        } else {
            current
                .get(json_field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }
    };
    let applicant = pick("applicantName", &new_applicant);
    let department = pick("department", &new_department);
    let ip = pick("ipAddress", &new_ip);
    let period = pick("accessPeriod", &new_period);
    sqlx::query(
        "UPDATE infra_terminal
         SET applicant_name=$3, department=$4, ip_address=$5, access_period=$6,
             status='active', updater='flow', update_time=now()
         WHERE id=$1 AND tenant_id=$2",
    )
    .bind(id)
    .bind(tenant.id())
    .bind(&applicant)
    .bind(&department)
    .bind(&ip)
    .bind(&period)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(json!({
        "terminalId": id, "macAddress": mac, "operation": "update",
        "applicantName": applicant, "department": department, "ipAddress": ip, "accessPeriod": period,
    }))
}
