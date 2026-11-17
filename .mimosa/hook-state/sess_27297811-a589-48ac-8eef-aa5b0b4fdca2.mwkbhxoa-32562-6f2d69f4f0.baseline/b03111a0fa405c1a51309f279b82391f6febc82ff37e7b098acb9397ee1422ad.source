use std::{collections::BTreeSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// A colon-separated capability, for example `system:user:read`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permission(String);

impl Permission {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidPermission> {
        let value = value.into();
        let valid = value == "*"
            || (!value.is_empty()
                && value.split(':').all(|segment| {
                    segment == "*"
                        || (!segment.is_empty()
                            && segment.chars().all(|character| {
                                character.is_ascii_lowercase()
                                    || character.is_ascii_digit()
                                    || character == '-'
                                    || character == '_'
                            }))
                }));

        valid.then_some(Self(value)).ok_or(InvalidPermission)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for Permission {
    type Err = InvalidPermission;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPermission;

impl fmt::Display for InvalidPermission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("permission must use lowercase colon-separated segments")
    }
}

impl std::error::Error for InvalidPermission {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermissionSet(BTreeSet<Permission>);

impl PermissionSet {
    pub fn new(permissions: impl IntoIterator<Item = Permission>) -> Self {
        Self(permissions.into_iter().collect())
    }

    pub fn allows(&self, required: &Permission) -> bool {
        self.0
            .iter()
            .any(|granted| permission_matches(granted, required))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Permission> {
        self.0.iter()
    }
}

fn permission_matches(granted: &Permission, required: &Permission) -> bool {
    if granted.as_str() == "*" || granted == required {
        return true;
    }

    let granted_segments: Vec<_> = granted.as_str().split(':').collect();
    let required_segments: Vec<_> = required.as_str().split(':').collect();

    granted_segments.len() == required_segments.len()
        && granted_segments
            .iter()
            .zip(required_segments)
            .all(|(granted, required)| *granted == "*" || *granted == required)
}

#[cfg(test)]
mod tests {
    use super::{Permission, PermissionSet};

    fn permission(value: &str) -> Permission {
        Permission::new(value).unwrap()
    }

    #[test]
    fn exact_and_segment_wildcards_are_supported() {
        let permissions =
            PermissionSet::new([permission("system:user:read"), permission("toon:project:*")]);

        assert!(permissions.allows(&permission("system:user:read")));
        assert!(permissions.allows(&permission("toon:project:update")));
        assert!(!permissions.allows(&permission("system:role:read")));
    }

    #[test]
    fn rejects_ambiguous_permission_codes() {
        assert!(Permission::new("System User Read").is_err());
        assert!(Permission::new("system::read").is_err());
    }
}
