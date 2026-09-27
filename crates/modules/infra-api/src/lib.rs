use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
pub struct InfraCapability {
    pub module: &'static str,
    pub capabilities: [&'static str; 14],
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
}
