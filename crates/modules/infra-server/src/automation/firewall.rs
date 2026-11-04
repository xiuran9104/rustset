//! 防火墙策略执行：从工单字段提取策略请求，通过厂商适配器下发。

use serde_json::Value;
use tracing::info;

use super::vendors::{create_adapter, FirewallConnection, PolicyPushRequest};

/// 从工单 JSON 提取防火墙策略请求。
pub fn extract_policy_request(ticket: &Value) -> Option<PolicyPushRequest> {
    let str_field = |key: &str| {
        ticket.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    let ticket_id = ticket.get("id").and_then(Value::as_i64)?;
    Some(PolicyPushRequest {
        policy_name: generate_policy_name(ticket_id, &str_field("fwSourceAddress"), &str_field("fwDestPort")),
        source_zone: str_field("fwSourceZone"),
        source_address: str_field("fwSourceAddress"),
        dest_zone: str_field("fwDestZone"),
        dest_address: str_field("fwDestAddress"),
        service_port: str_field("fwDestPort"),
        protocol: str_field("fwProtocol"),
        direction: str_field("fwDirection"),
        valid_until: Some(str_field("fwValidUntil")).filter(|v| !v.is_empty()),
        ticket_id,
    })
}

/// 生成防火墙策略名称（可审计、可回溯）。
pub fn generate_policy_name(ticket_id: i64, source: &str, port: &str) -> String {
    format!(
        "AUTO-T{}-{}-{}",
        ticket_id,
        source.replace('/', "_"),
        port.replace('-', "_")
    )
}

/// 执行策略推送：读取配置 → 构建适配器 → 调用 → 返回结果。
pub async fn execute_policy(
    pool: &sqlx::PgPool,
    tenant_id: i64,
    scope_config: &str,
    request: &PolicyPushRequest,
) -> super::vendors::VendorResult {
    // 解析连接配置
    let connection: FirewallConnection = match serde_json::from_str(scope_config) {
        Ok(conn) => conn,
        Err(e) => {
            return super::vendors::VendorResult {
                success: false,
                policy_id: None,
                detail: format!("scope_config JSON 解析失败：{e}；请配置 vendor/endpoint/policy_group"),
            };
        }
    };

    // 读取凭据
    let credential: Option<(String, String)> = sqlx::query_as(
        "SELECT api_username, api_password FROM infra_automation_credential
         WHERE tenant_id=$1 AND automation_type='firewall_provision' AND deleted=0
         ORDER BY id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (username, password) = match credential {
        Some((u, p)) => (u, p),
        None => {
            return super::vendors::VendorResult {
                success: false,
                policy_id: None,
                detail: "防火墙凭据未配置；请在自动化配置中设置 API 用户名和密码".to_string(),
            };
        }
    };

    let adapter = create_adapter(connection, username, password);
    info!(policy = %request.policy_name, "防火墙策略推送开始");
    adapter.push_policy(request).await
}

/// 撤除策略（回收/到期联动）。
pub async fn revoke_policy(
    pool: &sqlx::PgPool,
    tenant_id: i64,
    scope_config: &str,
    policy_name: &str,
) -> super::vendors::VendorResult {
    let connection: FirewallConnection = match serde_json::from_str(scope_config) {
        Ok(conn) => conn,
        Err(e) => {
            return super::vendors::VendorResult {
                success: false,
                policy_id: None,
                detail: format!("scope_config JSON 解析失败：{e}"),
            };
        }
    };
    let credential: Option<(String, String)> = sqlx::query_as(
        "SELECT api_username, api_password FROM infra_automation_credential
         WHERE tenant_id=$1 AND automation_type='firewall_provision' AND deleted=0",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (username, password) = match credential {
        Some((u, p)) => (u, p),
        None => {
            return super::vendors::VendorResult {
                success: false,
                policy_id: None,
                detail: "防火墙凭据未配置".to_string(),
            };
        }
    };

    let adapter = create_adapter(connection, username, password);
    adapter.remove_policy(policy_name).await
}

/// 连接测试。
pub async fn test_connection(
    pool: &sqlx::PgPool,
    tenant_id: i64,
    scope_config: &str,
) -> super::vendors::VendorResult {
    let connection: FirewallConnection = match serde_json::from_str(scope_config) {
        Ok(conn) => conn,
        Err(e) => {
            return super::vendors::VendorResult {
                success: false,
                policy_id: None,
                detail: format!("scope_config 解析失败：{e}"),
            };
        }
    };
    let credential: Option<(String, String)> = sqlx::query_as(
        "SELECT api_username, api_password FROM infra_automation_credential
         WHERE tenant_id=$1 AND automation_type='firewall_provision' AND deleted=0",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (username, password) = match credential {
        Some((u, p)) => (u, p),
        None => ("test".to_string(), "test".to_string()),
    };

    let adapter = create_adapter(connection, username, password);
    adapter.health_check().await
}
