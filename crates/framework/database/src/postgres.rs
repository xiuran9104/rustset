use sha2::Digest;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::DatabaseConfig;

// SQLx embeds the migration directory at compile time.
// 0001_initial.sql is the consolidated baseline. Add new migrations after it
// and never edit a migration after it has been released.
// Recompile this crate whenever the migration catalog changes.
//
// Migrations are the sole source of truth for database schema and baseline data.
// sql/bootstrap/current.sql is a reference-only pg_dump snapshot kept for
// documentation and manual inspection — it is NOT loaded by the application.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../../sql/postgresql");

pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .connect(&config.url)
        .await
}

pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    if reconcile_migrations(pool).await? {
        MIGRATOR.run(pool).await?;
    }
    Ok(())
}

pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

/// Reconciles the sqlx migration history table with the current database state.
///
/// Returns `true` if the caller should run the sqlx migrator (empty or
/// migration-managed database).  Returns `false` when the database was
/// pre-populated (e.g. from a pg_dump) and migration history has been
/// seeded — the caller should skip `MIGRATOR.run()`.
async fn reconcile_migrations(pool: &PgPool) -> anyhow::Result<bool> {
    let has_migration_history: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if has_migration_history {
        return Ok(true);
    }

    let existing_application_tables: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM pg_tables
         WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
           AND NOT (schemaname = 'public' AND tablename = 'spatial_ref_sys')",
    )
    .fetch_one(pool)
    .await?;

    if existing_application_tables == 0 {
        tracing::info!("empty database detected; running all migrations from scratch");
        return Ok(true);
    }

    // Pre-existing tables without migration history — the database was likely
    // restored from a pg_dump snapshot.  Create the history table and record
    // every migration as already applied so sqlx never runs them again.
    tracing::info!(
        existing_application_tables,
        "pre-populated database with no migration history; seeding _sqlx_migrations"
    );

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS public._sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
            success BOOLEAN NOT NULL,
            checksum BYTEA NOT NULL,
            execution_time BIGINT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    for migration in MIGRATOR.iter() {
        // Compute SHA-256 checksum matching sqlx's internal format
        let checksum = sha2::Sha256::digest(migration.sql.as_str().as_bytes());
        sqlx::query(
            "INSERT INTO public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
             VALUES ($1, $2, now(), true, $3, 0)
             ON CONFLICT (version) DO UPDATE SET checksum = EXCLUDED.checksum",
        )
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(checksum.as_slice())
        .execute(pool)
        .await?;
    }

    Ok(false)
}
