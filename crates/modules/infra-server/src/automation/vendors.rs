//! 防火墙厂商适配层：抽象接口 + 内置适配器。
//!
//! 架构：
//! - `FirewallVendor` trait 定义统一操作（推送/撤除/健康检查）
//! - `DryRunAdapter` 记录日志不实际调用（测试/首次部署验证）
//! - `RestAdapter` 通用 REST 模板（对接 Hillstone/Fortinet/Huawei USG 等提供 REST API 的防火墙）
//! - 厂商类型 + 端点 + 策略组存在 `scope_config` JSON 中
//! - 凭据独立存储于 `infra_automation_credential`，不与云平台凭据共用

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{info, warn};

/// 防火墙连接配置（从 scope_config JSON 解析）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallConnection {
    /// 厂商类型：dry_run | hillstone | fortinet | huawei_usg | generic_rest
    pub vendor: String,
    /// API 端点，如 https://10.0.255.1:443/api
    pub endpoint: String,
    /// 专属策略组名称（自动化创建的策略只放在这个组里）
    pub policy_group: String,
}

/// 策略推送请求。
#[derive(Debug, Clone)]
pub struct PolicyPushRequest {
    pub policy_name: String,
    pub source_zone: String,
    pub source_address: String,
    pub dest_zone: String,
    pub dest_address: String,
    pub service_port: String,
    pub protocol: String,
    pub direction: String,
    pub valid_until: Option<String>,
    pub ticket_id: i64,
}

/// 策略操作结果。
#[derive(Debug)]
pub struct VendorResult {
    pub success: bool,
    pub policy_id: Option<String>,
    pub detail: String,
}

/// 防火墙厂商统一接口。
#[async_trait::async_trait]
pub trait FirewallVendor: Send + Sync {
    /// 健康检查：验证 API 可达、凭据有效。
    async fn health_check(&self) -> VendorResult;

    /// 推送放行策略到专属策略组。
    async fn push_policy(&self, req: &PolicyPushRequest) -> VendorResult;

    /// 从策略组撤除指定策略。
    async fn remove_policy(&self, policy_name: &str) -> VendorResult;

    /// 列出策略组下的策略（对账用）。
    async fn list_policies(&self) -> Result<Vec<Value>, String>;
}

/// DryRun 适配器：只记录日志，不实际调用任何设备。
/// 适用于首次部署验证流程、测试环境、以及厂商 API 尚未开通时。
pub struct DryRunAdapter {
    pub connection: FirewallConnection,
}

#[async_trait::async_trait]
impl FirewallVendor for DryRunAdapter {
    async fn health_check(&self) -> VendorResult {
        info!(vendor = "dry_run", endpoint = %self.connection.endpoint,
            "防火墙 dry-run 健康检查通过（不实际连接设备）");
        VendorResult {
            success: true,
            policy_id: None,
            detail: "dry-run 模式：健康检查通过".to_string(),
        }
    }

    async fn push_policy(&self, req: &PolicyPushRequest) -> VendorResult {
        info!(vendor = "dry_run", policy = %req.policy_name,
            source = %req.source_address, dest = %req.dest_address,
            port = %req.service_port,
            "防火墙 dry-run：记录策略推送（不实际下发设备）");
        VendorResult {
            success: true,
            policy_id: Some(format!("dry-run-{}", req.ticket_id)),
            detail: format!(
                "dry-run 模式：策略 {} 已记录（源 {} → 目的 {}:{}），实际下发需切换到厂商适配器",
                req.policy_name, req.source_address, req.dest_address, req.service_port
            ),
        }
    }

    async fn remove_policy(&self, policy_name: &str) -> VendorResult {
        info!(vendor = "dry_run", policy = policy_name,
            "防火墙 dry-run：记录策略撤除");
        VendorResult {
            success: true,
            policy_id: None,
            detail: format!("dry-run 模式：策略 {policy_name} 撤除已记录"),
        }
    }

    async fn list_policies(&self) -> Result<Vec<Value>, String> {
        Ok(vec![])
    }
}

/// 通用 REST 适配器模板：覆盖提供标准 REST API 的防火墙。
/// 实际对接时按厂商微调认证方式和请求体格式。
pub struct RestAdapter {
    pub connection: FirewallConnection,
    pub username: String,
    pub password: String,
    pub http: reqwest::Client,
}

impl RestAdapter {
    pub fn new(connection: FirewallConnection, username: String, password: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true) // 防火墙通常用自签证书
            .build()
            .unwrap_or_default();
        Self { connection, username, password, http }
    }
}

#[async_trait::async_trait]
impl FirewallVendor for RestAdapter {
    async fn health_check(&self) -> VendorResult {
        let url = format!("{}/status", self.connection.endpoint);
        match self.http.get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await
        {
            Ok(resp) if resp.status().is_success() => VendorResult {
                success: true,
                policy_id: None,
                detail: format!("防火墙 API 可达（{}）", resp.status()),
            },
            Ok(resp) => VendorResult {
                success: false,
                policy_id: None,
                detail: format!("防火墙 API 响应异常：HTTP {}", resp.status()),
            },
            Err(e) => VendorResult {
                success: false,
                policy_id: None,
                detail: format!("防火墙 API 不可达：{e}"),
            },
        }
    }

    async fn push_policy(&self, req: &PolicyPushRequest) -> VendorResult {
        // 通用 REST 策略推送（各厂商可能需要微调字段名）
        let url = format!("{}/policies", self.connection.endpoint);
        let body = serde_json::json!({
            "name": req.policy_name,
            "group": self.connection.policy_group,
            "action": "allow",
            "source_zone": req.source_zone,
            "source_address": req.source_address,
            "destination_zone": req.dest_zone,
            "destination_address": req.dest_address,
            "service_port": req.service_port,
            "protocol": req.protocol,
            "direction": req.direction,
            "valid_until": req.valid_until,
        });
        match self.http.post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&body)
            .send().await
        {
            Ok(resp) if resp.status().is_success() => {
                let policy_id = resp.json::<Value>().await
                    .ok()
                    .and_then(|v| v.get("id").and_then(Value::as_str).map(String::from));
                VendorResult {
                    success: true,
                    policy_id,
                    detail: format!("策略 {} 已推送到 {}", req.policy_name, self.connection.policy_group),
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(vendor = %self.connection.vendor, status = %status,
                    "防火墙策略推送失败：{body}");
                VendorResult {
                    success: false,
                    policy_id: None,
                    detail: format!("推送失败：HTTP {status} — {body}"),
                }
            }
            Err(e) => VendorResult {
                success: false,
                policy_id: None,
                detail: format!("推送失败：网络错误 {e}"),
            },
        }
    }

    async fn remove_policy(&self, policy_name: &str) -> VendorResult {
        let url = format!("{}/policies/{}", self.connection.endpoint, policy_name);
        match self.http.delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 404 => {
                VendorResult {
                    success: true,
                    policy_id: None,
                    detail: format!("策略 {policy_name} 已撤除"),
                }
            }
            Ok(resp) => VendorResult {
                success: false,
                policy_id: None,
                detail: format!("撤除失败：HTTP {}", resp.status()),
            },
            Err(e) => VendorResult {
                success: false,
                policy_id: None,
                detail: format!("撤除失败：网络错误 {e}"),
            },
        }
    }

    async fn list_policies(&self) -> Result<Vec<Value>, String> {
        let url = format!("{}/policies?group={}", self.connection.endpoint, self.connection.policy_group);
        match self.http.get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await
        {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<Vec<Value>>().await.map_err(|e| e.to_string())
            }
            Ok(resp) => Err(format!("列表失败：HTTP {}", resp.status())),
            Err(e) => Err(format!("列表失败：{e}")),
        }
    }
}

/// 根据配置构建对应的适配器实例。
pub fn create_adapter(
    connection: FirewallConnection,
    username: String,
    password: String,
) -> Box<dyn FirewallVendor> {
    match connection.vendor.as_str() {
        "dry_run" | "" => Box::new(DryRunAdapter { connection }),
        _ => Box::new(RestAdapter::new(connection, username, password)),
    }
}
