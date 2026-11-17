//! PostgreSQL connection pool, migrations, and readiness primitives.
//! 信创适配：通过 `DatabaseFlavor` 枚举和可移植化工具支持多数据库。
//!
//! 门面边界（docs/xinchuang-adaptation-plan.md §4.1.1）：业务代码只引用
//! `DbPool` / `DbTransaction` / `DbQueryBuilder` / `DbError`，不再出现
//! `PgPool` / `Postgres` / `Transaction` 等 sqlx 类型。PG 家族阶段这些
//! 门面是指向 sqlx PG 实现的类型别名；达梦适配器实装后换成独立实现，
//! 业务代码不动。行读取（`sqlx::Row`/`PgRow`）在 PG 家族阶段保持
//! sqlx 原生用法，随达梦 `DbRow` 门面一并收敛。

mod config;
mod error;
mod flavor;
mod postgres;

pub use config::{DatabaseConfig, DatabaseConfigError};
pub use error::DbError;
pub use flavor::{
    BindIn, DatabaseFlavor, bind_in, expand_in_placeholders, next_placeholder_index,
    portable_like,
};
pub use postgres::{connect, migrate, migrate_with_flavor, ping};

/// 连接池门面。PG 家族（PostgreSQL / 金仓 / openGauss）包装 `sqlx::PgPool`；
/// 达梦适配器实装后替换为 ODBC 连接池实现。
pub type DbPool = sqlx::PgPool;

/// 事务门面，覆盖现有 `FOR UPDATE` 事务流（§4.1.7）。
pub type DbTransaction<'c> = sqlx::Transaction<'c, sqlx::Postgres>;

/// 动态查询构造门面（替代业务代码中的 `QueryBuilder::<Postgres>`）。
pub type DbQueryBuilder = sqlx::QueryBuilder<sqlx::Postgres>;

/// 连接池构造选项门面（测试与直连场景使用；主路径走 [`connect`]）。
pub use sqlx::postgres::{PgConnectOptions as DbConnectOptions, PgPoolOptions as DbPoolOptions};
