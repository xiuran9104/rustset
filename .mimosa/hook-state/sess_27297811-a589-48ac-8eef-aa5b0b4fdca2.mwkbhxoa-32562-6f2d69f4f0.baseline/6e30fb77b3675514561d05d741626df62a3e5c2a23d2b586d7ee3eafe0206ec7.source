use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use rustset_framework_database::DbPool;
use rustset_framework_security::{CurrentUser, DataScope, Permission, PermissionSet};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserAccount {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub tenant_id: Option<String>,
    pub status: String,
    pub failed_login_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
}

/// SM3-derived identity UUID for a numeric user id (国密 replacement for the
/// retired md5 mapping). Stored in `system_users.identity_uuid`; all API
/// surfaces keep presenting this UUID as the user id.
pub fn identity_uuid_for(user_id: i64) -> Uuid {
    Uuid::from_bytes(rustset_framework_gm::sm3_uuid_bytes(&format!(
        "{}{user_id}",
        rustset_framework_gm::IDENTITY_DOMAIN
    )))
}

pub async fn find_account_by_username(
    pool: &DbPool,
    username: &str,
    tenant_id: Option<i64>,
) -> anyhow::Result<Option<UserAccount>> {
    sqlx::query_as::<_, UserAccount>(
        "SELECT source.identity_uuid AS id,
                source.username, source.password AS password_hash,
                source.tenant_id::text AS tenant_id,
                CASE source.status WHEN 0 THEN 'active' ELSE 'disabled' END AS status,
                0 AS failed_login_attempts,
                NULL::timestamptz AS locked_until
         FROM system_users source
         WHERE lower(source.username) = lower($1)
           AND (
               source.tenant_id = $2
               OR (
                   $2::bigint IS NULL
                   AND 1 = (SELECT count(*) FROM system_users duplicate
                            WHERE lower(duplicate.username) = lower($1)
                              AND duplicate.deleted = 0)
               )
           )
           AND source.deleted = 0
         ORDER BY source.id
         LIMIT 1",
    )
    .bind(username)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .context("failed to query user account")
}

pub async fn find_account_by_id(
    pool: &DbPool,
    user_id: Uuid,
) -> anyhow::Result<Option<UserAccount>> {
    sqlx::query_as::<_, UserAccount>(
        "SELECT source.identity_uuid AS id,
                source.username, source.password AS password_hash,
                source.tenant_id::text AS tenant_id,
                CASE source.status WHEN 0 THEN 'active' ELSE 'disabled' END AS status,
                0 AS failed_login_attempts,
                NULL::timestamptz AS locked_until
         FROM system_users source
         WHERE source.identity_uuid = $1
           AND source.deleted = 0
         ORDER BY source.id
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("failed to query user account")
}

pub async fn record_failed_login(
    pool: &DbPool,
    user_id: Uuid,
    failed_attempts: i32,
    lock_until: Option<DateTime<Utc>>,
) -> anyhow::Result<()> {
    let status = if lock_until.is_some() {
        "locked"
    } else {
        "active"
    };
    let _ = (pool, user_id, failed_attempts, lock_until, status);
    Ok(())
}

pub async fn record_successful_login(pool: &DbPool, user_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE system_users source
         SET login_date = now(), update_time = now()
         WHERE source.identity_uuid = $1",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .context("failed to record successful login")?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct RolePermissionRow {
    role_code: String,
    data_scope: String,
    permission_code: Option<String>,
}

pub async fn load_current_user(
    pool: &DbPool,
    account: &UserAccount,
) -> anyhow::Result<CurrentUser> {
    let rows = sqlx::query_as::<_, RolePermissionRow>(
        "SELECT r.code AS role_code,
                CASE r.data_scope
                    WHEN 1 THEN 'all'
                    WHEN 2 THEN 'organization'
                    WHEN 3 THEN 'department'
                    WHEN 4 THEN 'department'
                    ELSE 'self_only'
                END AS data_scope,
                m.permission AS permission_code
         FROM system_users u
         JOIN system_tenant tenant
           ON tenant.id = u.tenant_id AND tenant.deleted = 0 AND tenant.status = 0
         LEFT JOIN system_tenant_package package
           ON package.id = tenant.package_id AND package.deleted = 0 AND package.status = 0
         JOIN system_user_role ur ON ur.user_id = u.id AND ur.deleted = 0
         JOIN system_role r ON r.id = ur.role_id AND r.deleted = 0 AND r.status = 0
         LEFT JOIN system_role_menu rm ON rm.role_id = r.id AND rm.deleted = 0
         LEFT JOIN system_menu m ON m.deleted = 0 AND m.status = 0
             AND m.permission <> ''
             AND (r.code = 'super_admin' OR m.id = rm.menu_id)
             AND (
                 tenant.package_id = 0
                 OR EXISTS (
                     SELECT 1
                     FROM jsonb_array_elements_text(package.menu_ids::jsonb) allowed(menu_id)
                     WHERE allowed.menu_id::bigint = m.id
                 )
             )
         WHERE u.identity_uuid = $1
           AND u.deleted = 0 AND u.status = 0
         ORDER BY r.code, m.permission",
    )
    .bind(account.id)
    .fetch_all(pool)
    .await
    .context("failed to load effective permissions")?;

    let mut roles = Vec::new();
    let mut permissions = Vec::new();
    let mut data_scope = DataScope::SelfOnly;
    for row in rows {
        if !roles.contains(&row.role_code) {
            roles.push(row.role_code);
        }
        data_scope = widest_scope(data_scope, parse_scope(&row.data_scope)?);
        if let Some(code) = row.permission_code {
            permissions.push(
                Permission::new(code)
                    .map_err(|error| anyhow!("invalid permission in database: {error}"))?,
            );
        }
    }
    Ok(CurrentUser {
        user_id: account.id.to_string(),
        username: account.username.clone(),
        tenant_id: account.tenant_id.clone(),
        role_codes: roles,
        permissions: PermissionSet::new(permissions),
        data_scope,
    })
}

fn parse_scope(scope: &str) -> anyhow::Result<DataScope> {
    match scope {
        "self_only" => Ok(DataScope::SelfOnly),
        "department" => Ok(DataScope::Department),
        "organization" => Ok(DataScope::Organization),
        "all" => Ok(DataScope::All),
        _ => Err(anyhow!("invalid data scope in database: {scope}")),
    }
}

fn widest_scope(left: DataScope, right: DataScope) -> DataScope {
    fn rank(scope: DataScope) -> u8 {
        match scope {
            DataScope::SelfOnly => 0,
            DataScope::Department => 1,
            DataScope::Organization => 2,
            DataScope::All => 3,
        }
    }

    if rank(left) >= rank(right) {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    use rustset_framework_security::DataScope;

    use super::widest_scope;

    #[test]
    fn combines_multiple_roles_using_the_widest_data_scope() {
        assert_eq!(
            widest_scope(DataScope::Department, DataScope::Organization),
            DataScope::Organization
        );
    }
}
