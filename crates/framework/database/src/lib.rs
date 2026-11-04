//! PostgreSQL connection pool, migrations, and readiness primitives.
//! 信创适配：通过 `DatabaseFlavor` 枚举和可移植化工具支持多数据库。

mod config;
mod flavor;
mod postgres;

pub use config::{DatabaseConfig, DatabaseConfigError};
pub use flavor::{
    DatabaseFlavor, expand_in_placeholders, next_placeholder_index, portable_like,
};
pub use postgres::{connect, migrate, ping};

/// 门面类型别名：业务代码应引用 `DbPool` 而非 `PgPool`。
/// PG 家族（PostgreSQL / 金仓 / openGauss）阶段仍指向 `PgPool`；
/// 达梦适配器实装后替换为 trait object。
pub type DbPool = sqlx::PgPool;

/// 事务门面。
pub type DbTransaction<'c> = sqlx::Transaction<'c, sqlx::Postgres>;

// 保留原始导出以兼容现有代码（后续逐步收敛到 DbPool）
pub use sqlx::{PgPool, Postgres, Transaction};
