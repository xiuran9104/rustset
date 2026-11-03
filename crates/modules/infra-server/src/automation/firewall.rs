//! 防火墙策略执行器：工单审批通过后，通过受限 API 账号在专属策略组下
//! 自动放行策略。与云资源执行器（tofu）完全独立——独立凭据、独立开关、
//! 独立熔断。
//!
//! 安全设计：
//! - 只操作指定策略组（scope_config 限定），不动基线规则
//! - 凭据独立于云平台（credential_ref 引用防火墙自己的服务账号）
//! - 每次操作写审计记录
//! - 连续失败 N 次自动熔断，后续工单降级为人工执行

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 防火墙策略下发请求（从工单字段映射）。
#[derive(Debug, Serialize, Deserialize)]
pub struct FirewallPolicyRequest {
    pub ticket_id: i64,
    pub application_name: String,
    pub source_zone: String,
    pub source_address: String,
    pub dest_zone: String,
    pub dest_address: String,
    pub service_port: String,
    pub protocol: String,
    pub direction: String,
    pub valid_until: Option<String>,
}

/// 执行结果。
#[derive(Debug)]
pub struct FirewallExecution {
    pub success: bool,
    pub detail: String,
    pub policy_name: Option<String>,
}

/// 从工单 JSON 提取防火墙策略请求。
pub fn extract_policy_request(ticket: &Value) -> Option<FirewallPolicyRequest> {
    let str_field = |key: &str| {
        ticket
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(FirewallPolicyRequest {
        ticket_id: ticket.get("id").and_then(Value::as_i64)?,
        application_name: str_field("applicationName"),
        source_zone: str_field("fwSourceZone"),
        source_address: str_field("fwSourceAddress"),
        dest_zone: str_field("fwDestZone"),
        dest_address: str_field("fwDestAddress"),
        service_port: str_field("fwDestPort"),
        protocol: str_field("fwProtocol"),
        direction: str_field("fwDirection"),
        valid_until: Some(str_field("fwValidUntil")).filter(|v| !v.is_empty()),
    })
}

/// 生成防火墙策略名称（可审计、可回溯）。
pub fn generate_policy_name(req: &FirewallPolicyRequest) -> String {
    format!(
        "AUTO-T{}-{}-{}",
        req.ticket_id,
        req.source_address.replace('/', "_"),
        req.service_port.replace('-', "_")
    )
}

/// 模拟下发（实际对接防火墙厂商 API 时替换此函数）。
/// 真正的实现会：
/// 1. 用 credential_ref 获取防火墙 API 凭据
/// 2. 用 scope_config 限定的策略组
/// 3. 调用厂商 REST/SSH API 下发策略
/// 4. 校验策略生效
pub async fn execute_policy(
    _pool: &sqlx::PgPool,
    _config: &super::AutomationConfig,
    _request: &FirewallPolicyRequest,
) -> FirewallExecution {
    let policy_name = generate_policy_name(_request);
    // TODO: 实际对接防火墙厂商 API
    // 当前返回"跳过"——开关默认关闭，此函数仅在开关开启时被调用
    FirewallExecution {
        success: false,
        detail: format!(
            "防火墙 API 对接尚未配置；策略 {policy_name} 需人工下发"
        ),
        policy_name: Some(policy_name),
    }
}

/// 策略到期自动停用/删除（生命周期联动）。
pub async fn revoke_policy(
    _pool: &sqlx::PgPool,
    _tenant_id: i64,
    _ticket_id: i64,
) -> Result<(), String> {
    // TODO: 实际对接防火墙厂商 API，按策略名定位并删除
    Ok(())
}
