use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;

#[derive(Debug, Serialize, JsonSchema)]
pub struct SystemCapability {
    pub module: &'static str,
    pub capabilities: [&'static str; 4],
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(
        default,
        alias = "tenantId",
        deserialize_with = "deserialize_optional_i64"
    )]
    pub tenant_id: Option<i64>,
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;

    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .ok_or_else(|| de::Error::custom("tenantId must be an integer"))
            .map(Some),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                value
                    .parse::<i64>()
                    .map(Some)
                    .map_err(|_| de::Error::custom("tenantId must be an integer string"))
            }
        }
        _ => Err(de::Error::custom("tenantId must be an integer or string")),
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CurrentUserResponse {
    pub user_id: String,
    pub username: String,
    pub tenant_id: Option<String>,
    pub role_codes: Vec<String>,
    pub permissions: Vec<String>,
    pub data_scope: String,
}

#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub status: String,
    pub role_ids: Vec<String>,
    pub role_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
    pub password: String,
    #[serde(default)]
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RoleSummary {
    pub id: String,
    pub code: String,
    pub name: String,
    pub data_scope: String,
    pub is_system: bool,
    pub enabled: bool,
    pub permission_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub data_scope: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: String,
    pub data_scope: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PermissionSummary {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AuditLogSummary {
    pub id: String,
    pub actor_user_id: Option<String>,
    pub actor_username: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub detail: Value,
    pub created_at: String,
}

impl Default for SystemCapability {
    fn default() -> Self {
        Self {
            module: "system",
            capabilities: ["user", "auth-session", "permission", "tenant"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LoginRequest;

    #[test]
    fn login_request_accepts_numeric_tenant_id() {
        let request: LoginRequest =
            serde_json::from_str(r#"{"username":"admin","password":"admin123","tenantId":1}"#)
                .unwrap();

        assert_eq!(request.tenant_id, Some(1));
    }

    #[test]
    fn login_request_accepts_string_tenant_id() {
        let request: LoginRequest =
            serde_json::from_str(r#"{"username":"admin","password":"admin123","tenantId":"1"}"#)
                .unwrap();

        assert_eq!(request.tenant_id, Some(1));
    }

    #[test]
    fn login_request_accepts_empty_tenant_id_as_none() {
        let request: LoginRequest =
            serde_json::from_str(r#"{"username":"admin","password":"admin123","tenantId":""}"#)
                .unwrap();

        assert_eq!(request.tenant_id, None);
    }

    #[test]
    fn login_request_accepts_snake_case_tenant_id() {
        let request: LoginRequest =
            serde_json::from_str(r#"{"username":"admin","password":"admin123","tenant_id":"1"}"#)
                .unwrap();

        assert_eq!(request.tenant_id, Some(1));
    }
}
