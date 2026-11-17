use serde::{Deserialize, Serialize};

use crate::{Permission, PermissionSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataScope {
    #[default]
    SelfOnly,
    Department,
    Organization,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub user_id: String,
    pub username: String,
    pub tenant_id: Option<String>,
    pub role_codes: Vec<String>,
    pub permissions: PermissionSet,
    pub data_scope: DataScope,
}

impl CurrentUser {
    pub fn can(&self, permission: &Permission) -> bool {
        self.permissions.allows(permission)
    }
}

/// 登录态提取器不参与 OpenAPI 文档建模（鉴权统一由 bearerAuth 安全方案表达）。
impl aide::operation::OperationInput for CurrentUser {}
