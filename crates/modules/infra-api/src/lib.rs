use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
pub struct InfraCapability {
    pub module: &'static str,
    pub capabilities: [&'static str; 14],
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNetworkZoneRequest {
    pub name: String,
    pub cidr: String,
    #[serde(default)]
    pub priority: i32,
    pub cloud_platform_id: Option<i64>,
    pub cloud_platform_name: Option<String>,
    pub machine_room_id: Option<i64>,
    pub machine_room_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateNetworkZoneRequest {
    pub id: String,
    pub name: String,
    pub cidr: String,
    #[serde(default)]
    pub priority: i32,
    pub cloud_platform_id: Option<i64>,
    pub cloud_platform_name: Option<String>,
    pub machine_room_id: Option<i64>,
    pub machine_room_name: Option<String>,
}

impl Default for InfraCapability {
    fn default() -> Self {
        Self {
            module: "infra",
            capabilities: [
                "config", "file", "job", "monitor", "asset", "business", "ticket", "task", "risk",
                "cloud", "provider", "room", "zone", "security",
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateScanTaskRequest {
    pub name: String,
    pub target: String,
    pub port_policy: String,
    #[serde(default)]
    pub domain_brute: bool,
    pub service_detection: Option<bool>,
    #[serde(default)]
    pub os_detection: bool,
    #[serde(default)]
    pub site_identify: bool,
    pub idempotency_key: Option<String>,
    pub max_attempts: Option<i64>,
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScanTaskRequest {
    pub id: String,
    pub name: String,
    pub target: String,
    pub port_policy: String,
    #[serde(default)]
    pub domain_brute: bool,
    pub service_detection: Option<bool>,
    #[serde(default)]
    pub os_detection: bool,
    #[serde(default)]
    pub site_identify: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TaskIdRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TaskIdsQuery {
    pub ids: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TriggerScanRequest {
    pub target_ip: String,
    #[serde(default)]
    pub ports: Vec<i32>,
    pub idempotency_key: Option<String>,
    pub max_attempts: Option<i64>,
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TriggerScanResponse {
    pub task_id: String,
    pub message: String,
    pub target_ip: String,
    pub ports: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanTaskResponse {
    pub id: String,
    pub name: String,
    pub target: String,
    pub status: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub found_assets: i32,
    pub found_risks: i32,
    pub port_policy: String,
    pub domain_brute: bool,
    pub service_detection: bool,
    pub os_detection: bool,
    pub site_identify: bool,
    pub created_by: Option<String>,
    pub task_kind: String,
    pub scan_ports: Vec<i32>,
    pub total_targets: i32,
    pub completed_targets: i32,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub timeout_seconds: i32,
    pub next_attempt_at: String,
    pub cancel_requested: bool,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScanTaskPageResponse {
    pub list: Vec<ScanTaskResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunInspectionRequest {
    pub target_ips: Vec<String>,
    pub ports: Vec<i32>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueuedTaskResponse {
    pub task_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionDifference {
    pub kind: String,
    pub port: Option<i32>,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionRiskResponse {
    pub id: String,
    pub asset_ip: String,
    pub port: i32,
    pub severity: String,
    pub description: String,
    pub solution: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionResultResponse {
    pub id: String,
    pub task_id: String,
    pub ip: String,
    pub registered: bool,
    pub baseline_ports: Option<Vec<i32>>,
    pub open_ports: Vec<i32>,
    pub uncertain_ports: Vec<i32>,
    pub differences: Vec<InspectionDifference>,
    pub risks: Vec<InspectionRiskResponse>,
    pub create_time: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionBaselineResponse {
    pub ip: String,
    pub allowed_ports: Vec<i32>,
    pub reason: String,
    pub updated_by: String,
    pub update_time: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct InspectionResultQuery {
    #[serde(rename = "taskId", alias = "task_id")]
    pub task_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct IpQuery {
    pub ip: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveInspectionBaselineRequest {
    pub ip: String,
    pub allowed_ports: Vec<i32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateResourceTicketRequest {
    pub resource_type: String,
    pub ecs_name: String,
    pub idempotency_key: Option<String>,
    pub ticket_type: Option<String>,
    pub risk_level: Option<String>,
    pub target_resource_id: Option<i64>,
    pub target_resource_type: Option<String>,
    pub target_config: Option<String>,
    pub maintenance_window: Option<String>,
    pub allow_interruption: Option<bool>,
    pub backup_confirmed: Option<bool>,
    pub rollback_plan: Option<String>,
    pub retention_until: Option<String>,
    pub provider_id: Option<i64>,
    pub provider_name: Option<String>,
    pub cloud_platform_id: Option<i64>,
    pub cloud_platform_name: Option<String>,
    pub machine_room_id: Option<i64>,
    pub machine_room_name: Option<String>,
    pub cloud_region: Option<String>,
    pub cloud_category: Option<String>,
    pub zone_name: Option<String>,
    pub zone_cabinet: Option<String>,
    pub rack_units: Option<i32>,
    pub customer_name: Option<String>,
    pub application_name: Option<String>,
    pub application_endpoint_id: Option<i64>,
    pub application_domain: Option<String>,
    pub contract_name: Option<String>,
    pub ecs_type: Option<String>,
    pub ecs_os: Option<String>,
    pub resource_count: Option<i32>,
    pub cpu_cores: Option<i32>,
    pub memory_gb: Option<i32>,
    pub system_disk: Option<String>,
    pub system_disk_size_gb: Option<i32>,
    pub data_disk: Option<String>,
    pub expire_at: Option<String>,
    pub has_security_product: Option<bool>,
    pub security_products: Option<String>,
    pub ip_address: Option<String>,
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateResourceTicketRequest {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecs_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_window: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_interruption: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_confirmed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_cores: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_gb: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_disk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_disk_size_gb: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_disk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_security_product: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_products: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApproveResourceTicketRequest {
    pub approved: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvisionResourceTicketRequest {
    pub details: Option<String>,
    pub config_id: Option<i64>,
    pub image_id: Option<String>,
    pub flavor: Option<String>,
    pub availability_zone: Option<String>,
    pub subnet_id: Option<String>,
    pub vpc_id: Option<String>,
    #[serde(default)]
    pub security_groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliverResourceTicketRequest {
    pub comment: Option<String>,
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scan_requests_use_stable_camel_case_fields() {
        let request: CreateScanTaskRequest = serde_json::from_value(json!({
            "name": "core scan",
            "target": "127.0.0.1",
            "portPolicy": "COMMON",
            "serviceDetection": true,
            "idempotencyKey": "request-1",
            "maxAttempts": 4,
            "timeoutSeconds": 60
        }))
        .unwrap();
        assert_eq!(request.port_policy, "COMMON");
        assert_eq!(request.idempotency_key.as_deref(), Some("request-1"));
        assert_eq!(request.max_attempts, Some(4));
        assert!(request.service_detection.unwrap());
    }

    #[test]
    fn inspection_responses_do_not_expose_database_field_names() {
        let response = InspectionResultResponse {
            id: "result-1".to_owned(),
            task_id: "task-1".to_owned(),
            ip: "127.0.0.1".to_owned(),
            registered: true,
            baseline_ports: Some(vec![22]),
            open_ports: vec![22],
            uncertain_ports: vec![],
            differences: vec![],
            risks: vec![],
            create_time: "2026-09-27T00:00:00Z".to_owned(),
        };
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["taskId"], "task-1");
        assert_eq!(value["baselinePorts"], json!([22]));
        assert!(value.get("task_id").is_none());
        assert!(value.get("baseline_ports").is_none());
    }

    #[test]
    fn ticket_write_contracts_reject_unknown_fields() {
        let request: CreateResourceTicketRequest = serde_json::from_value(json!({
            "resourceType": "cloud",
            "ecsName": "web-01",
            "idempotencyKey": "ticket-1",
            "cpuCores": 4
        }))
        .unwrap();
        assert_eq!(request.resource_type, "cloud");
        assert_eq!(request.idempotency_key.as_deref(), Some("ticket-1"));

        let patch: UpdateResourceTicketRequest = serde_json::from_value(json!({
            "id": 7,
            "cpuCores": 8
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(patch).unwrap(),
            json!({ "id": 7, "cpuCores": 8 })
        );

        assert!(
            serde_json::from_value::<ApproveResourceTicketRequest>(json!({
                "approved": true,
                "unexpected": "field"
            }))
            .is_err()
        );
    }
}
