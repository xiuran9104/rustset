use sha2::Digest;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::flavor::DatabaseFlavor;
use crate::DatabaseConfig;

// SQLx embeds the migration directory at compile time.
// 0001_initial.sql is the consolidated baseline. Add new migrations after it
// and never edit a migration after it has been released.
// Recompile this crate whenever the migration catalog changes.
//
// Migrations are the sole source of truth for database schema and baseline data.
// sql/bootstrap/current.sql is a reference-only pg_dump snapshot kept for
// documentation and manual inspection — it is NOT loaded by the application.
//
// openGauss 链由 `script/sql-translate --target opengauss` 产出（§4.1.8：
// 生成 + 评审入库，一致性测试把关），这里编译期内嵌后按 flavor 选用。
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../../sql/postgresql");
static MIGRATOR_OPENGAUSS: sqlx::migrate::Migrator = sqlx::migrate!("../../../sql/opengauss");
// 金仓链：仅 NULLS NOT DISTINCT 翻译（实测金仓 PG 模式其余原生兼容）。
static MIGRATOR_KINGBASE: sqlx::migrate::Migrator = sqlx::migrate!("../../../sql/kingbase");

pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .connect(&config.url)
        .await
}

/// 按 `DB_FLAVOR` / `DATABASE_URL` 探测的方言执行迁移（测试与工具入口；
/// 网关主路径显式传 flavor）。
pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    migrate_with_flavor(pool, DatabaseFlavor::detect()).await
}

/// 按方言执行迁移（§4.1.8）。金仓以 PG 兼容模式直连，沿用
/// `sql/postgresql` 链；openGauss 使用翻译链 `sql/opengauss`（实测：
/// 内核无 `ON CONFLICT`/`NULLS NOT DISTINCT`，翻译规则见
/// `script/sql-translate`）；达梦显式拒绝——ODBC 适配器与 `sql/dm` 链
/// 未实装（M3），不允许静默落到 PostgreSQL。
pub async fn migrate_with_flavor(pool: &PgPool, flavor: DatabaseFlavor) -> anyhow::Result<()> {
    match flavor {
        DatabaseFlavor::Dameng => anyhow::bail!(
            "Dameng adapter is not implemented yet (M3): requires the ODBC pool \
             and the translated sql/dm chain"
        ),
        DatabaseFlavor::OpenGauss => {
            if MIGRATOR_OPENGAUSS.iter().count() == 0 {
                anyhow::bail!(
                    "sql/opengauss is empty; generate the flavor chain with \
                     `cargo run -p sql-translate -- --target opengauss --output sql/opengauss`"
                );
            }
            run_migrations(pool, &MIGRATOR_OPENGAUSS).await
        }
        DatabaseFlavor::Kingbase => {
            if MIGRATOR_KINGBASE.iter().count() == 0 {
                anyhow::bail!(
                    "sql/kingbase is empty; generate the flavor chain with \
                     `cargo run -p sql-translate -- --target kingbase --output sql/kingbase`"
                );
            }
            run_migrations(pool, &MIGRATOR_KINGBASE).await
        }
        DatabaseFlavor::Postgres => run_migrations(pool, &MIGRATOR).await,
    }
}

async fn run_migrations(
    pool: &PgPool,
    migrator: &'static sqlx::migrate::Migrator,
) -> anyhow::Result<()> {
    if reconcile_migrations(pool, migrator).await? {
        migrator.run(pool).await?;
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
async fn reconcile_migrations(
    pool: &PgPool,
    migrator: &'static sqlx::migrate::Migrator,
) -> anyhow::Result<bool> {
    // openGauss 没有 to_regclass（其内核源自 PG 9.2），改用 pg_class/pg_namespace
    // 可移植查询（§4.1.2）。
    let has_migration_history: bool = sqlx::query_scalar(
        "SELECT count(*) > 0 FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public' AND c.relname = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if has_migration_history {
        return Ok(true);
    }

    let existing_application_tables: i64 = sqlx::query_scalar(
        // 业务表只出现在 public/ai/infra 三个 schema；openGauss 有大量
        // 自有系统 schema（cstore/db4ai/dbe_perf/…），不能用
        // "非 pg_catalog 即业务表"的判定（实测会误判为预灌库）。
        "SELECT count(*)
         FROM pg_tables
         WHERE schemaname IN ('public', 'ai', 'infra')
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

    // openGauss 内核无 ON CONFLICT（实测），upsert 走 ON DUPLICATE KEY UPDATE
    // （EXCLUDED 可用）；PG 家族其余成员维持原语法。
    let upsert = if DatabaseFlavor::detect() == DatabaseFlavor::OpenGauss {
        "INSERT INTO public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
             VALUES ($1, $2, now(), true, $3, 0)
             ON DUPLICATE KEY UPDATE checksum = EXCLUDED.checksum"
    } else {
        "INSERT INTO public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
             VALUES ($1, $2, now(), true, $3, 0)
             ON CONFLICT (version) DO UPDATE SET checksum = EXCLUDED.checksum"
    };
    for migration in migrator.iter() {
        // Compute SHA-256 checksum matching sqlx's internal format
        let checksum = sha2::Sha256::digest(migration.sql.as_str().as_bytes());
        sqlx::query(upsert)
        .bind(migration.version)
        .bind(migration.description.as_ref())
        .bind(checksum.as_slice())
        .execute(pool)
        .await?;
    }

    Ok(false)
}
