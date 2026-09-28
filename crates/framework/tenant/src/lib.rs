//! Explicit tenant scope for tenant-owned data. Never derive it from request
//! headers, query parameters, JSON bodies, or a model-generated tool argument.

use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TenantContext {
    tenant_id: i64,
}

impl TenantContext {
    pub fn from_user(user: &CurrentUser) -> Result<Self, AppError> {
        let id = user.tenant_id.as_deref().and_then(|id| id.parse().ok());
        Self::from_persisted_id(id)
    }

    /// For background work, use an already authorized record's stored owner.
    /// NULL represents historical data awaiting explicit ownership assignment.
    pub fn from_persisted_id(id: Option<i64>) -> Result<Self, AppError> {
        match id {
            Some(tenant_id) if tenant_id > 0 => Ok(Self { tenant_id }),
            _ => Err(AppError::forbidden("tenant context is required")),
        }
    }

    pub fn id(self) -> i64 {
        self.tenant_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustset_framework_security::{DataScope, PermissionSet};
    #[test]
    fn roles_do_not_bypass_missing_or_invalid_tenant() {
        let mut user = CurrentUser {
            user_id: "u".into(),
            username: "u".into(),
            tenant_id: None,
            role_codes: vec!["super_admin".into()],
            permissions: PermissionSet::default(),
            data_scope: DataScope::All,
        };
        for id in [None, Some(""), Some("0"), Some("-1"), Some("other")] {
            user.tenant_id = id.map(str::to_owned);
            assert!(TenantContext::from_user(&user).is_err());
        }
        user.tenant_id = Some("2".into());
        assert_eq!(TenantContext::from_user(&user).unwrap().id(), 2);
    }
}
