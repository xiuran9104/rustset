//! CMDB attribute type system (veops/cmdb-inspired): the set of attribute
//! types a model may declare, and pure validation of JSON values against
//! them. Choices are `[{ "label": string, "value": string }]`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable envelopes; attribute names remain defined by the owning CMDB model.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceRequest {
    pub model_id: i64,
    pub attributes: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceRequest {
    pub id: i64,
    pub attributes: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdateInstanceRequest {
    pub ids: Vec<i64>,
    pub attributes: serde_json::Map<String, Value>,
}

/// Stable response fields; timestamps preserve the existing local SQL format.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstanceResponse {
    pub id: i64,
    pub model_id: i64,
    pub attributes: Value,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct InstancePageResponse {
    pub list: Vec<InstanceResponse>,
    pub total: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstancePageParams {
    pub model_id: i64,
    pub page_no: Option<i64>,
    pub page_size: Option<i64>,
    pub keyword: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct InstanceImportError {
    pub row: usize,
    pub error: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct InstanceImportResponse {
    pub created: i64,
    pub failed: usize,
    pub errors: Vec<InstanceImportError>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindRelationRequest {
    pub source_id: i64,
    pub target_id: i64,
    pub relation: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAttributeTriggerRequest {
    pub model_id: i64,
    pub name: String,
    pub condition_code: String,
    pub condition_value: Value,
    pub action_code: String,
    pub action_value: Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAttributeTriggerRequest {
    pub id: i64,
    pub name: String,
    pub condition_code: String,
    pub condition_value: Value,
    pub action_code: String,
    pub action_value: Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNetZoneRequest {
    pub name: String,
    #[serde(default)]
    pub parent_id: i64,
    #[serde(default = "default_zone_type")]
    pub zone_type: String,
    pub cidr: Option<String>,
    #[serde(default)]
    pub sort: i32,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateNetZoneRequest {
    pub id: i64,
    pub name: Option<String>,
    pub parent_id: Option<i64>,
    pub zone_type: Option<String>,
    pub cidr: Option<String>,
    pub sort: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveNetZoneRequest {
    pub ip: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentifyAssetsRequest {}

fn default_zone_type() -> String {
    "company".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType {
    Text,
    Textarea,
    Number,
    Float,
    Bool,
    Date,
    Datetime,
    Select,
    MultiSelect,
    Link,
    Json,
    Password,
}

impl AttrType {
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "text" => Self::Text,
            "textarea" => Self::Textarea,
            "number" => Self::Number,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "date" => Self::Date,
            "datetime" => Self::Datetime,
            "select" => Self::Select,
            "multi_select" => Self::MultiSelect,
            "link" => Self::Link,
            "json" => Self::Json,
            "password" => Self::Password,
            _ => return None,
        })
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Textarea => "textarea",
            Self::Number => "number",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::Select => "select",
            Self::MultiSelect => "multi_select",
            Self::Link => "link",
            Self::Json => "json",
            Self::Password => "password",
        }
    }

    /// Validate `value` for this attribute type; `choices` is required for
    /// select/multi_select. Returns a human-readable reason on failure.
    pub fn validate(self, value: &Value, choices: Option<&Value>) -> Result<(), String> {
        match self {
            Self::Text | Self::Textarea | Self::Link | Self::Password => {
                if value.is_string() {
                    Ok(())
                } else {
                    Err("must be a string".into())
                }
            }
            Self::Number => {
                if value.is_i64() || value.is_u64() {
                    Ok(())
                } else {
                    Err("must be an integer".into())
                }
            }
            Self::Float => {
                if value.is_number() {
                    Ok(())
                } else {
                    Err("must be a number".into())
                }
            }
            Self::Bool => {
                if value.is_boolean() {
                    Ok(())
                } else {
                    Err("must be a boolean".into())
                }
            }
            Self::Date => {
                let text = value
                    .as_str()
                    .ok_or_else(|| "must be a date string (YYYY-MM-DD)".to_string())?;
                chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .map(|_| ())
                    .map_err(|_| "must be a date string (YYYY-MM-DD)".into())
            }
            Self::Datetime => {
                let text = value
                    .as_str()
                    .ok_or_else(|| "must be a datetime string".to_string())?;
                let parsed = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
                    .or_else(|_| {
                        chrono::DateTime::parse_from_rfc3339(text).map(|dt| dt.naive_local())
                    });
                parsed
                    .map(|_| ())
                    .map_err(|_| "must be YYYY-MM-DD HH:MM:SS or RFC3339".into())
            }
            Self::Select => {
                let selected = value.as_str().unwrap_or_default();
                choice_values(choices)
                    .iter()
                    .any(|candidate| candidate == selected)
                    .then_some(())
                    .ok_or_else(|| "value is not one of the configured choices".to_string())
            }
            Self::MultiSelect => {
                let items = value
                    .as_array()
                    .ok_or_else(|| "must be an array of choice values".to_string())?;
                let allowed = choice_values(choices);
                for item in items {
                    let text = item.as_str().unwrap_or_default();
                    if !allowed.iter().any(|candidate| candidate == text) {
                        return Err(format!(
                            "value {text:?} is not one of the configured choices"
                        ));
                    }
                }
                Ok(())
            }
            Self::Json => {
                if value.is_object() || value.is_array() {
                    Ok(())
                } else {
                    Err("must be a JSON object or array".into())
                }
            }
        }
    }
}

/// Extract the `value` strings out of a choices definition.
pub fn choice_values(choices: Option<&Value>) -> Vec<String> {
    choices
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("value").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn choices() -> Value {
        json!([
            {"label": "核心", "value": "core"},
            {"label": "边缘", "value": "edge"}
        ])
    }

    #[test]
    fn parses_all_type_codes() {
        for code in [
            "text",
            "textarea",
            "number",
            "float",
            "bool",
            "date",
            "datetime",
            "select",
            "multi_select",
            "link",
            "json",
            "password",
        ] {
            let parsed = AttrType::from_code(code).unwrap_or_else(|| panic!("{code}"));
            assert_eq!(parsed.code(), code);
        }
        assert!(AttrType::from_code("blob").is_none());
    }

    #[test]
    fn validates_scalars_strictly() {
        assert!(AttrType::Number.validate(&json!(3), None).is_ok());
        assert!(AttrType::Number.validate(&json!(3.5), None).is_err());
        assert!(AttrType::Float.validate(&json!(3.5), None).is_ok());
        assert!(AttrType::Bool.validate(&json!(false), None).is_ok());
        assert!(AttrType::Bool.validate(&json!("false"), None).is_err());
        assert!(AttrType::Date.validate(&json!("2026-09-25"), None).is_ok());
        assert!(AttrType::Date.validate(&json!("2026/09/25"), None).is_err());
        assert!(
            AttrType::Datetime
                .validate(&json!("2026-09-25 10:00:00"), None)
                .is_ok()
        );
        assert!(
            AttrType::Datetime
                .validate(&json!("2026-09-25T10:00:00Z"), None)
                .is_ok()
        );
        assert!(AttrType::Json.validate(&json!({"a":1}), None).is_ok());
        assert!(AttrType::Json.validate(&json!(42), None).is_err());
    }

    #[test]
    fn validates_select_against_choices() {
        let choices = choices();
        assert!(
            AttrType::Select
                .validate(&json!("core"), Some(&choices))
                .is_ok()
        );
        assert!(
            AttrType::Select
                .validate(&json!("nope"), Some(&choices))
                .is_err()
        );
        assert!(
            AttrType::MultiSelect
                .validate(&json!(["core", "edge"]), Some(&choices))
                .is_ok()
        );
        assert!(
            AttrType::MultiSelect
                .validate(&json!(["core", "nope"]), Some(&choices))
                .is_err()
        );
        assert!(
            AttrType::MultiSelect
                .validate(&json!("core"), Some(&choices))
                .is_err()
        );
    }

    #[test]
    fn batch_update_request_requires_an_id_array_and_attribute_object() {
        assert!(
            serde_json::from_value::<BatchUpdateInstanceRequest>(
                serde_json::json!({"ids": [1, 2], "attributes": {"status": "online"}})
            )
            .is_ok()
        );
        assert!(
            serde_json::from_value::<BatchUpdateInstanceRequest>(
                serde_json::json!({"ids": "1,2", "attributes": {}})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<BatchUpdateInstanceRequest>(
                serde_json::json!({"ids": [1, 2], "attributes": []})
            )
            .is_err()
        );
    }

    #[test]
    fn net_zone_writes_reject_client_selected_tenants() {
        assert!(
            serde_json::from_value::<CreateNetZoneRequest>(serde_json::json!({
                "name": "core", "zoneType": "segment", "tenantId": 9
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ResolveNetZoneRequest>(serde_json::json!({
                "ip": "10.0.0.1", "tenantId": 9
            }))
            .is_err()
        );
    }
}
