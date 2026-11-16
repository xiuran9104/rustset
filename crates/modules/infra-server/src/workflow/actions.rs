//! 自动动作注册表：auto 步骤的执行器（match 风格，同 ai tools.rs）。
//!
//! v1 动作：firewall_push / trigger_scan / create_ticket / set_context。
//! firewall_push 走既有自动化框架（开关/熔断/审计）；失败进入退避重试
//! （最多 3 次），重试由 runner 按 retry_at 到期后调 engine::advance 驱动。

use rustset_framework_database::DbPool;
use rustset_framework_tenant::TenantContext;
use serde_json::{Value, json};
use sqlx::Row;

/// 已知动作名（校验用）。
pub(crate) fn is_known_action(action: &str) -> Option<&'static str> {
    const ACTIONS: [&str; 4] = ["firewall_push", "trigger_scan", "create_ticket", "set_context"];
    ACTIONS.iter().find(|a| **a == action).copied()
}

/// 执行一个 auto 步骤：读取实例/步骤 → 分发动作 → 落结果并推进。
/// 成功 → step done + engine::advance；失败 → attempt+1、retry_at 退避，
/// attempt>=3 → failed（engine::advance 收尾实例为 failed）。
pub(crate) async fn execute_step(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    step_id: i64,
) {
    if let Err(error) = execute_step_inner(pool, tenant, instance_id, step_id).await {
        tracing::warn!(instance_id, step_id, %error, "flow auto step failed");
    }
}

async fn execute_step_inner(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    step_id: i64,
) -> Result<(), String> {
    // 先置 running 标记（attempt 在此累加，退避由结果路径写回）。
    let step = sqlx::query(
        "SELECT step_key, name, config, attempt_count, status FROM infra_flow_step
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

    let outcome: Result<Value, String> = match action.as_str() {
        "firewall_push" => firewall_push(pool, tenant, instance_id, &payload, &params).await,
        "trigger_scan" => trigger_scan(pool, tenant, instance_id, &payload, &params).await,
        "create_ticket" => create_ticket(pool, tenant, instance_id, &payload, &params).await,
        "set_context" => set_context(&mut context, &payload, &params),
        other => Err(format!("未知自动动作：{other}")),
    };

    match outcome {
        Ok(result) => {
            // set_context 需要把更新后的 context 写回实例。
            if action == "set_context" {
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
    super::engine::advance(pool, tenant, instance_id).await;
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

/// 创建资源工单：params 即工单字段（camelCase），applicant 默认发起人 system。
async fn create_ticket(
    pool: &DbPool,
    tenant: &TenantContext,
    instance_id: i64,
    _payload: &Value,
    params: &Value,
) -> Result<Value, String> {
    let mut ticket_payload = params.clone();
    if !ticket_payload
        .as_object()
        .is_some_and(|o| o.contains_key("name"))
    {
        if let Some(object) = ticket_payload.as_object_mut() {
            object.insert(
                "name".to_string(),
                json!(format!("流程创建 F{instance_id}")),
            );
        }
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
