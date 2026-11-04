//! 自动化执行框架：云资源供给与防火墙策略下发共享的配置读取、开关检查、
//! 审计写入和熔断逻辑。两套执行器（tofu / firewall）各自实现具体调用，
//! 但都必须经过这里的前置检查和后置审计。

pub(crate) mod firewall;
pub(crate) mod vendors;

use serde_json::Value;
use sqlx::PgPool;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct AutomationConfig {
    pub automation_type: String,
    pub enabled: bool,
    pub credential_ref: Option<i64>,
    pub scope_config: Option<String>,
    pub circuit_breaker_threshold: i32,
    pub circuit_breaker_count: i32,
    pub circuit_breaker_tripped: bool,
}

#[derive(Debug, Clone)]
pub enum AutomationDecision {
    /// 开关关闭，工单走人工执行
    Manual,
    /// 开关开启且未熔断，可以自动执行
    Auto(AutomationConfig),
    /// 熔断已触发，拒绝自动执行
    CircuitOpen,
    /// 配置缺失（表里没有这条记录）
    NotConfigured,
}

/// 读取指定类型的自动化配置（租户隔离）。
pub async fn load_config(
    pool: &PgPool,
    tenant_id: i64,
    automation_type: &str,
) -> Option<AutomationConfig> {
    let row = sqlx::query_as::<_, (i16, Option<i64>, Option<String>, i32, i32, i16)>(
        "SELECT enabled, credential_ref, scope_config,
                circuit_breaker_threshold, circuit_breaker_count, circuit_breaker_tripped
         FROM infra_automation_config
         WHERE tenant_id=$1 AND automation_type=$2 AND deleted=0",
    )
    .bind(tenant_id)
    .bind(automation_type)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;

    let (enabled, credential_ref, scope_config, threshold, count, tripped) = row;
    Some(AutomationConfig {
        automation_type: automation_type.to_string(),
        enabled: enabled != 0,
        credential_ref,
        scope_config,
        circuit_breaker_threshold: threshold,
        circuit_breaker_count: count,
        circuit_breaker_tripped: tripped != 0,
    })
}

/// 决定一张已审批工单是自动执行还是走人工：
/// 1. 配置不存在 → 人工（安全默认）
/// 2. 开关关闭   → 人工
/// 3. 熔断触发   → 拒绝自动
/// 4. 其余       → 自动
pub async fn decide(
    pool: &PgPool,
    tenant_id: i64,
    automation_type: &str,
) -> AutomationDecision {
    match load_config(pool, tenant_id, automation_type).await {
        None => AutomationDecision::NotConfigured,
        Some(config) => {
            if !config.enabled {
                AutomationDecision::Manual
            } else if config.circuit_breaker_tripped {
                AutomationDecision::CircuitOpen
            } else {
                AutomationDecision::Auto(config)
            }
        }
    }
}

/// 写审计记录：每次决策（执行/跳过/失败/熔断）都留痕。
pub async fn audit(
    pool: &PgPool,
    tenant_id: i64,
    automation_type: &str,
    ticket_id: Option<i64>,
    action: &str,
    result: &str,
    detail: &str,
    duration: Option<u128>,
) {
    let _ = sqlx::query(
        "INSERT INTO infra_automation_audit
         (id, automation_type, ticket_id, action, result, detail, duration_ms, executor, tenant_id)
         VALUES (nextval('infra_automation_audit_seq'), $1, $2, $3, $4, $5, $6, 'system', $7)",
    )
    .bind(automation_type)
    .bind(ticket_id)
    .bind(action)
    .bind(result)
    .bind(detail)
    .bind(duration.map(|d| d as i32).unwrap_or(0))
    .bind(tenant_id)
    .execute(pool)
    .await;
}

/// 成功后重置熔断计数。
pub async fn on_success(pool: &PgPool, tenant_id: i64, automation_type: &str) {
    let _ = sqlx::query(
        "UPDATE infra_automation_config
         SET circuit_breaker_count=0, circuit_breaker_tripped=0,
             last_executed_at=now(), last_result='success'
         WHERE tenant_id=$1 AND automation_type=$2 AND deleted=0",
    )
    .bind(tenant_id)
    .bind(automation_type)
    .execute(pool)
    .await;
}

/// 失败后累加计数，达到阈值自动熔断。
pub async fn on_failure(pool: &PgPool, tenant_id: i64, automation_type: &str, error: &str) {
    let _ = sqlx::query(
        "UPDATE infra_automation_config
         SET circuit_breaker_count = circuit_breaker_count + 1,
             circuit_breaker_tripped = CASE
                 WHEN circuit_breaker_count + 1 >= circuit_breaker_threshold THEN 1
                 ELSE circuit_breaker_tripped
             END,
             last_executed_at = now(),
             last_result = $3
         WHERE tenant_id=$1 AND automation_type=$2 AND deleted=0",
    )
    .bind(tenant_id)
    .bind(automation_type)
    .bind(format!("failed: {error}"))
    .execute(pool)
    .await;
}

/// 计时辅助。
pub struct AutoTimer {
    start: Instant,
}

impl AutoTimer {
    pub fn start() -> Self {
        Self { start: Instant::now() }
    }
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}
