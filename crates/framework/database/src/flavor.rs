//! 数据库方言枚举与门面类型。
//!
//! 信创适配的抽象层：业务代码面向 `DbPool`/`DbRow` 编程，
//! 不直接依赖 `sqlx::PgPool`。PG 家族（PostgreSQL / 金仓 / openGauss）
//! 走 sqlx PG 协议；达梦走 ODBC 独立适配器（后续实装）。
//!
//! 详见 docs/xinchuang-adaptation-plan.md §4.1

use std::fmt;
use std::str::FromStr;

/// 支持的数据库方言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseFlavor {
    /// PostgreSQL（开发基线）
    Postgres,
    /// 人大金仓 KingbaseES V8R6（PG 兼容模式）
    Kingbase,
    /// 华为 openGauss（集中式）
    OpenGauss,
    /// 达梦 DM8（ODBC 适配器）
    Dameng,
}

impl DatabaseFlavor {
    /// 从 DATABASE_URL scheme 或 DB_FLAVOR 环境变量推断。
    pub fn detect() -> Self {
        if let Ok(flavor) = std::env::var("DB_FLAVOR")
            && let Ok(f) = Self::from_str(&flavor)
        {
            return f;
        }
        if let Ok(url) = std::env::var("DATABASE_URL") {
            if url.starts_with("kingbase://") || url.starts_with("kingbase8://") {
                return Self::Kingbase;
            }
            if url.starts_with("opengauss://") {
                return Self::OpenGauss;
            }
            if url.starts_with("dm://") || url.starts_with("dameng://") {
                return Self::Dameng;
            }
        }
        Self::Postgres
    }

    /// 迁移文件目录名。
    pub fn migration_dir(&self) -> &'static str {
        match self {
            Self::Postgres => "postgresql",
            Self::Kingbase => "kingbase",
            Self::OpenGauss => "opengauss",
            Self::Dameng => "dm",
        }
    }

    /// 是否走 PG 线协议（sqlx 可直连）。
    pub fn is_pg_family(&self) -> bool {
        matches!(self, Self::Postgres | Self::Kingbase | Self::OpenGauss)
    }

    /// 是否支持 `ON CONFLICT` 语法。
    pub fn supports_on_conflict(&self) -> bool {
        self.is_pg_family()
    }

    /// 是否支持 `jsonb` 类型。
    pub fn supports_jsonb(&self) -> bool {
        self.is_pg_family()
    }

    /// 是否支持 `ilike` 运算符。
    pub fn supports_ilike(&self) -> bool {
        self.is_pg_family()
    }

    /// 是否支持 `ANY(数组)` 绑定。
    pub fn supports_any_array(&self) -> bool {
        self.is_pg_family()
    }
}

impl FromStr for DatabaseFlavor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" | "pg" => Ok(Self::Postgres),
            "kingbase" | "kingbasees" | "kb" => Ok(Self::Kingbase),
            "opengauss" | "og" => Ok(Self::OpenGauss),
            "dameng" | "dm" | "dm8" => Ok(Self::Dameng),
            _ => Err(format!("unknown database flavor: {s}")),
        }
    }
}

impl fmt::Display for DatabaseFlavor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => write!(f, "postgresql"),
            Self::Kingbase => write!(f, "kingbase"),
            Self::OpenGauss => write!(f, "opengauss"),
            Self::Dameng => write!(f, "dm"),
        }
    }
}

/// SQL 可移植化工具：将 `ANY($N)` 数组绑定展开为 `IN (…)` 列表。
///
/// PG 专有写法：`WHERE id = ANY($1)` + `bind(vec![1,2,3])`
/// 三库通用写法：`WHERE id IN ($1, $2, $3)` + `bind(1).bind(2).bind(3)`
///
/// 用法：
/// ```ignore
/// let ids = vec![1i64, 2, 3];
/// let placeholders = expand_in_placeholders(&ids, start_index);
/// let sql = format!("SELECT * FROM t WHERE id IN ({placeholders})");
/// // 依次 bind 每个 id
/// ```
pub fn expand_in_placeholders<T>(values: &[T], start_index: usize) -> String {
    values
        .iter()
        .enumerate()
        .map(|(i, _)| format!("${}", start_index + i))
        .collect::<Vec<_>>()
        .join(", ")
}

/// 下一批占位符的起始索引（在已有 $1..$N 之后追加 IN 列表时使用）。
pub fn next_placeholder_index(existing_binds: usize) -> usize {
    existing_binds + 1
}

/// 生成三库通用的模糊搜索条件（替代 PG 专有 `ILIKE`）。
///
/// PG: `name ILIKE '%keyword%'`
/// 通用: `UPPER(name) LIKE UPPER('%keyword%')`
pub fn portable_like(column: &str, pattern: &str) -> String {
    format!("UPPER({column}) LIKE UPPER('{pattern}')")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavor_parse() {
        assert_eq!(DatabaseFlavor::from_str("postgres").unwrap(), DatabaseFlavor::Postgres);
        assert_eq!(DatabaseFlavor::from_str("kingbase").unwrap(), DatabaseFlavor::Kingbase);
        assert_eq!(DatabaseFlavor::from_str("opengauss").unwrap(), DatabaseFlavor::OpenGauss);
        assert_eq!(DatabaseFlavor::from_str("dm8").unwrap(), DatabaseFlavor::Dameng);
        assert!(DatabaseFlavor::from_str("oracle").is_err());
    }

    #[test]
    fn migration_dirs() {
        assert_eq!(DatabaseFlavor::Postgres.migration_dir(), "postgresql");
        assert_eq!(DatabaseFlavor::Dameng.migration_dir(), "dm");
    }

    #[test]
    fn pg_family() {
        assert!(DatabaseFlavor::Kingbase.is_pg_family());
        assert!(!DatabaseFlavor::Dameng.is_pg_family());
    }

    #[test]
    fn expand_in() {
        let ids = vec![1, 2, 3];
        let ph = expand_in_placeholders(&ids, 1);
        assert_eq!(ph, "$1, $2, $3");
    }

    #[test]
    fn like_portable() {
        let cond = portable_like("name", "%keyword%");
        assert_eq!(cond, "UPPER(name) LIKE UPPER('%keyword%')");
    }
}
