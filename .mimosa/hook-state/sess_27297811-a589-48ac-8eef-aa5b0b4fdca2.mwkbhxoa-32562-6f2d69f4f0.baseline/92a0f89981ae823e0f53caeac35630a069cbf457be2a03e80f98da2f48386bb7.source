use std::fmt;

use crate::{CurrentUser, Permission};

pub fn authorize(user: &CurrentUser, required: &Permission) -> Result<(), AccessDenied> {
    user.can(required)
        .then_some(())
        .ok_or_else(|| AccessDenied {
            required: required.clone(),
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDenied {
    required: Permission,
}

impl AccessDenied {
    pub fn required(&self) -> &Permission {
        &self.required
    }
}

impl fmt::Display for AccessDenied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "permission denied: {} is required",
            self.required
        )
    }
}

impl std::error::Error for AccessDenied {}

#[cfg(test)]
mod tests {
    use crate::{CurrentUser, DataScope, Permission, PermissionSet, authorize};

    #[test]
    fn denies_a_user_without_the_required_permission() {
        let user = CurrentUser {
            user_id: "user-1".into(),
            username: "reader".into(),
            tenant_id: None,
            role_codes: vec!["viewer".into()],
            permissions: PermissionSet::new([Permission::new("system:user:read").unwrap()]),
            data_scope: DataScope::SelfOnly,
        };
        let required = Permission::new("system:user:delete").unwrap();

        let error = authorize(&user, &required).unwrap_err();
        assert_eq!(error.required(), &required);
    }
}
