use anyhow::{Context, bail};
use tracing::{info, warn};

use crate::SystemState;

pub async fn initialize(state: &SystemState) -> anyhow::Result<()> {
    ensure_identity_uuids(state).await?;
    let administrators = sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM system_users u
         JOIN system_user_role ur ON ur.user_id = u.id AND ur.deleted = 0
         JOIN system_role r ON r.id = ur.role_id AND r.deleted = 0
         WHERE u.deleted = 0 AND u.status = 0 AND r.status = 0
           AND r.code = 'super_admin'",
    )
    .fetch_one(&state.pool)
    .await
    .context("failed to verify RustSet administrator")?;

    if administrators == 0 {
        bootstrap_administrator(state).await?;
        let administrators = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)
             FROM system_users u
             JOIN system_user_role ur ON ur.user_id = u.id AND ur.deleted = 0
             JOIN system_role r ON r.id = ur.role_id AND r.deleted = 0
             WHERE u.deleted = 0 AND u.status = 0 AND r.status = 0
               AND r.code = 'super_admin'",
        )
        .fetch_one(&state.pool)
        .await
        .context("failed to re-verify RustSet administrator")?;
        if administrators == 0 {
            bail!("RustSet has no enabled super administrator");
        }
    } else {
        info!(administrators, "RustSet administrators are ready");
    }

    initialize_admin_password(state).await?;
    if let Err(error) = reseal_legacy_secrets(state).await {
        warn!(
            ?error,
            "failed to re-seal legacy secrets; keeping existing values"
        );
    }

    Ok(())
}

/// Create the initial super administrator from `BOOTSTRAP_ADMIN_USERNAME` /
/// `BOOTSTRAP_ADMIN_PASSWORD` when the database has none at all (for example a
/// baseline without seeded admins). The account is created inside the lowest
/// enabled tenant and linked to the `super_admin` role. An advisory lock keeps
/// concurrent gateway starts from inserting duplicates; `system_users` has no
/// unique constraint on `username`.
async fn bootstrap_administrator(state: &SystemState) -> anyhow::Result<()> {
    let username = std::env::var("BOOTSTRAP_ADMIN_USERNAME")
        .unwrap_or_else(|_| "admin".into())
        .trim()
        .to_string();
    let password = std::env::var("BOOTSTRAP_ADMIN_PASSWORD")
        .unwrap_or_default()
        .trim()
        .to_string();
    if username.is_empty() || password.is_empty() {
        warn!(
            "no enabled super administrator exists and BOOTSTRAP_ADMIN_PASSWORD is not set; \
             cannot bootstrap an administrator"
        );
        return Ok(());
    }

    let mut tx = state
        .pool
        .begin()
        .await
        .context("failed to begin admin bootstrap transaction")?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('rustset-bootstrap-admin'))")
        .execute(&mut *tx)
        .await
        .context("failed to lock admin bootstrap")?;

    let already = sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM system_users u
         JOIN system_user_role ur ON ur.user_id = u.id AND ur.deleted = 0
         JOIN system_role r ON r.id = ur.role_id AND r.deleted = 0
         WHERE u.deleted = 0 AND u.status = 0 AND r.status = 0
           AND r.code = 'super_admin'",
    )
    .fetch_one(&mut *tx)
    .await
    .context("failed to re-check administrators under lock")?;
    if already > 0 {
        tx.commit()
            .await
            .context("failed to commit bootstrap check")?;
        info!("another administrator appeared concurrently; skipping bootstrap");
        return Ok(());
    }

    let tenant_id: Option<i64> = sqlx::query_scalar(
        "SELECT tenant.id
         FROM system_tenant tenant
         LEFT JOIN system_users existing
           ON existing.tenant_id = tenant.id
          AND existing.username = $1
          AND existing.deleted = 0
         WHERE tenant.deleted = 0 AND tenant.status = 0
         ORDER BY (existing.id IS NOT NULL) DESC, tenant.id
         LIMIT 1",
    )
    .bind(&username)
    .fetch_optional(&mut *tx)
    .await
    .context("failed to locate bootstrap tenant")?;
    let Some(tenant_id) = tenant_id else {
        bail!("no enabled system_tenant row to bootstrap an administrator into");
    };

    // A database with no enabled super administrator is already locked out;
    // bootstrap is the recovery path, so accept a disabled super_admin role
    // and re-enable it, and create the role when it is missing entirely.
    let role: Option<(i64, i16)> =
        sqlx::query_as("SELECT id, status FROM system_role WHERE code = 'super_admin' AND tenant_id = $1 AND deleted = 0 ORDER BY id LIMIT 1")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .context("failed to locate super_admin role")?;
    let role_id = match role {
        Some((id, status)) => {
            if status != 0 {
                sqlx::query(
                    "UPDATE system_role SET status = 0, updater = 'bootstrap' WHERE id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .context("failed to re-enable super_admin role")?;
                info!(
                    role_id = id,
                    "re-enabled disabled super_admin role during bootstrap"
                );
            }
            id
        }
        None => {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO system_role (id, name, code, sort, data_scope, status, type, tenant_id, creator, updater)
                 VALUES (nextval('system_role_seq'), '超级管理员', 'super_admin', 1, 1, 0, 1, $1, 'bootstrap', 'bootstrap')
                 RETURNING id",
            )
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await
            .context("failed to create super_admin role")?;
            info!(
                role_id = id,
                "created missing super_admin role during bootstrap"
            );
            id
        }
    };

    let hash = state
        .passwords
        .hash_secret(&password)
        .context("failed to hash bootstrap admin password")?;

    let inserted: Option<i64> = sqlx::query_scalar(
        "INSERT INTO system_users (id, username, password, nickname, status, tenant_id, creator, updater)
         SELECT nextval('system_users_seq'), $1, $2, $3, 0, $4, 'bootstrap', 'bootstrap'
         WHERE NOT EXISTS (
             SELECT 1 FROM system_users WHERE username = $1 AND deleted = 0
         )
         RETURNING id",
    )
    .bind(&username)
    .bind(&hash)
    .bind("Administrator")
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .context("failed to insert bootstrap administrator")?;
    if let Some(id) = inserted {
        sqlx::query("UPDATE system_users SET identity_uuid = $2 WHERE id = $1")
            .bind(id)
            .bind(crate::infrastructure::identity_uuid_for(id))
            .execute(&mut *tx)
            .await
            .context("failed to assign bootstrap identity uuid")?;
    }
    let user_id = match inserted {
        Some(id) => id,
        None => sqlx::query_scalar::<_, i64>(
            "SELECT id FROM system_users WHERE username = $1 AND deleted = 0",
        )
        .bind(&username)
        .fetch_one(&mut *tx)
        .await
        .context("failed to look up existing bootstrap username")?,
    };

    sqlx::query(
        "INSERT INTO system_user_role (id, user_id, role_id, tenant_id, creator, updater)
         SELECT nextval('system_user_role_seq'), $1, $2, $3, 'bootstrap', 'bootstrap'
         WHERE NOT EXISTS (
             SELECT 1 FROM system_user_role
             WHERE user_id = $1 AND role_id = $2 AND deleted = 0
         )",
    )
    .bind(user_id)
    .bind(role_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .context("failed to assign super_admin role")?;

    tx.commit()
        .await
        .context("failed to commit admin bootstrap")?;
    info!(
        username,
        user_id, tenant_id, "bootstrapped initial administrator"
    );
    Ok(())
}

/// Set the initial password from `BOOTSTRAP_ADMIN_PASSWORD` while the account's
/// password column is still empty. The update is conditional on
/// `password = ''` so a password changed through the UI or by a concurrent
/// start is never overwritten, and the flow is idempotent across restarts.
async fn initialize_admin_password(state: &SystemState) -> anyhow::Result<()> {
    let username = std::env::var("BOOTSTRAP_ADMIN_USERNAME")
        .unwrap_or_else(|_| "admin".into())
        .trim()
        .to_string();
    if username.is_empty() {
        return Ok(());
    }

    let password = std::env::var("BOOTSTRAP_ADMIN_PASSWORD")
        .unwrap_or_default()
        .trim()
        .to_string();
    if password.is_empty() {
        // An empty password can only appear on restored snapshots; the normal
        // baseline seeds a real hash, so do not spam the log on every start.
        return Ok(());
    }

    let hash = state
        .passwords
        .hash_secret(&password)
        .context("failed to hash bootstrap admin password")?;

    let updated = sqlx::query(
        "UPDATE system_users SET password = $1, updater = 'bootstrap'
         WHERE username = $2 AND deleted = 0 AND status = 0 AND password = ''",
    )
    .bind(&hash)
    .bind(&username)
    .execute(&state.pool)
    .await
    .context("failed to persist bootstrap admin password")?
    .rows_affected();

    if updated == 1 {
        info!(username, "bootstrap admin password initialized");
    }
    Ok(())
}

/// Backfill `system_users.identity_uuid` (SM3-derived, 国密) for any user the
/// migrations could not cover — the derivation is computed in Rust, so seed
/// literals in migrations only cover the baseline users. Idempotent.
pub async fn ensure_identity_uuids(state: &SystemState) -> anyhow::Result<()> {
    let missing: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM system_users WHERE identity_uuid IS NULL ORDER BY id")
            .fetch_all(&state.pool)
            .await
            .context("failed to list users without identity uuid")?;
    if missing.is_empty() {
        return Ok(());
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .context("failed to begin identity uuid backfill")?;
    for id in missing {
        sqlx::query("UPDATE system_users SET identity_uuid = $2 WHERE id = $1")
            .bind(id)
            .bind(crate::infrastructure::identity_uuid_for(id))
            .execute(&mut *tx)
            .await
            .context("failed to backfill identity uuid")?;
    }
    tx.commit()
        .await
        .context("failed to commit identity uuid backfill")?;
    info!("backfilled SM3 identity uuids for users lacking one");
    Ok(())
}

/// Tables and columns that may hold sealed secrets, with their identifying
/// key columns. Keep in sync with `seal_sensitive_payload` in
/// management/compat.rs and the infra-server `seal_secret` call sites; the
/// gateway `reseal-secrets` subcommand reuses this registry.
pub const SEALED_COLUMNS: &[(&str, &[&str], &[&str])] = &[
    ("system_mail_account", &["id"], &["password"]),
    ("system_sms_channel", &["id"], &["api_key", "api_secret"]),
    ("system_oauth2_client", &["id"], &["secret"]),
    ("system_social_client", &["id"], &["client_secret"]),
    ("infra_data_source_config", &["id"], &["password"]),
    (
        "infra_cloud_provider_config",
        &["id"],
        &["access_key_id", "access_key_secret"],
    ),
    ("infra_automation_credential", &["id", "tenant_id"], &["api_password"]),
];

/// Row key of a sealed value: `id` plus the optional tenant part used by
/// composite primary keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SealedRowKey {
    pub id: i64,
    pub tenant_id: Option<i64>,
}

/// Startup re-seal pass (信创方案 §4.4.2、§7.2):
///
/// * `enc:v1:` legacy XOR rows always convert — to v3 when a master key ring
///   is configured, else to v2 (the historical behavior);
/// * `enc:sm4:v2:` rows convert to v3 **only** while the ring is present, so
///   a database migration and the cipher upgrade never change at once;
/// * undecodable rows stay untouched and are counted once. The pass runs in
///   bounded batches, is idempotent, and never wraps all history in one
///   transaction.
pub async fn reseal_legacy_secrets(state: &SystemState) -> anyhow::Result<()> {
    let provider = rustset_framework_gm::secrets::SealingSecrets::from_env()
        .map_err(|error| anyhow::anyhow!("failed to load sealing secrets: {error}"))?;
    if provider.uses_master_key() {
        info!(
            key_version = provider.active_key_version().unwrap_or(0),
            "sealing master key ring active; legacy sealed values upgrade to v3"
        );
    }
    let (converted, skipped) = reseal_sealed_columns(&state.pool, &provider, 200).await?;
    if converted > 0 || skipped > 0 {
        info!(converted, skipped, "re-sealed legacy secrets");
    }
    Ok(())
}

/// Batch conversion shared with the gateway `reseal-secrets` subcommand.
/// Returns `(converted, skipped)`; failures leave the row untouched so reruns
/// only process the remaining backlog (信创方案 §4.4.2.3).
pub async fn reseal_sealed_columns(
    pool: &rustset_framework_database::DbPool,
    provider: &rustset_framework_gm::secrets::SealingSecrets,
    batch_size: i64,
) -> anyhow::Result<(usize, usize)> {
    use sqlx::Row as _;
    use std::collections::HashSet;

    let include_v2 = provider.uses_master_key();
    let mut converted = 0usize;
    let mut skipped = 0usize;

    for (table, key_columns, columns) in SEALED_COLUMNS {
        let composite = key_columns.len() > 1;
        for column in *columns {
            let predicate = if include_v2 {
                format!("({column} LIKE 'enc:v1:%' OR {column} LIKE 'enc:sm4:v2:%')")
            } else {
                format!("{column} LIKE 'enc:v1:%'")
            };
            // rows that failed to convert once are skipped for this run so a
            // partially stuck backlog cannot loop forever
            let mut stuck: HashSet<SealedRowKey> = HashSet::new();
            loop {
                let select = format!(
                    "SELECT id{tenant_part} , {column} FROM {table} WHERE {predicate} LIMIT {batch_size}",
                    tenant_part = if composite { ", tenant_id" } else { "" },
                );
                let rows = sqlx::query(sqlx::AssertSqlSafe(select))
                    .fetch_all(pool)
                    .await
                    .with_context(|| format!("failed to list legacy secrets in {table}.{column}"))?;
                if rows.is_empty() {
                    break;
                }
                let mut progress = 0usize;
                for row in &rows {
                    let id: i64 = row.get("id");
                    let tenant_id = if composite {
                        row.get::<Option<i64>, _>("tenant_id")
                    } else {
                        None
                    };
                    let key = SealedRowKey { id, tenant_id };
                    if stuck.contains(&key) {
                        continue;
                    }
                    let Some(sealed) = row.get::<Option<String>, _>(column) else {
                        continue;
                    };
                    match provider.upgrade_sealed(&sealed) {
                        Ok(Some(resealed)) => {
                            let update = if composite {
                                format!("UPDATE {table} SET {column} = $3 WHERE id = $1 AND tenant_id = $2")
                            } else {
                                format!("UPDATE {table} SET {column} = $2 WHERE id = $1")
                            };
                            let mut query = sqlx::query(sqlx::AssertSqlSafe(update)).bind(id);
                            if composite {
                                query = query.bind(tenant_id);
                            }
                            query
                                .bind(resealed)
                                .execute(pool)
                                .await
                                .with_context(|| format!("failed to re-seal {table}.{column}"))?;
                            converted += 1;
                            progress += 1;
                        }
                        Ok(None) => {
                            // v2 without a master key: expected no-op, not a failure
                        }
                        Err(_) => {
                            stuck.insert(key);
                            skipped += 1;
                        }
                    }
                }
                if progress == 0 {
                    break;
                }
            }
        }
    }
    Ok((converted, skipped))
}
