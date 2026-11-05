use std::{env, fmt, time::Duration};

use crate::flavor::DatabaseFlavor;

#[derive(Clone)]
pub struct DatabaseConfig {
    pub url: String,
    /// 解析出的数据库方言（§3.3：DB_FLAVOR 与 URL scheme 必须一致，
    /// 冲突或不支持的组合在启动时失败，不静默降级）。
    pub flavor: DatabaseFlavor,
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self, DatabaseConfigError> {
        let url = env::var("DATABASE_URL").map_err(|_| DatabaseConfigError::MissingUrl)?;
        let min_connections = parse_env("DATABASE_MIN_CONNECTIONS", 1)?;
        let max_connections = parse_env("DATABASE_MAX_CONNECTIONS", 20)?;
        let acquire_timeout_seconds = parse_env("DATABASE_ACQUIRE_TIMEOUT_SECONDS", 5)?;

        let mut config = Self::new(
            url,
            min_connections,
            max_connections,
            Duration::from_secs(acquire_timeout_seconds.into()),
        )?;

        // DB_FLAVOR 显式声明方言：postgres:// URL + DB_FLAVOR=kingbase 表示
        // 金仓 PG 兼容模式直连；与 URL scheme 指明的方言冲突时启动失败。
        if let Ok(declared) = env::var("DB_FLAVOR") {
            let declared = DatabaseFlavor::from_str(&declared)
                .map_err(|_| DatabaseConfigError::InvalidFlavor)?;
            if config.flavor != DatabaseFlavor::Postgres && config.flavor != declared {
                return Err(DatabaseConfigError::FlavorMismatch {
                    declared: declared.to_string(),
                    url_scheme: config.flavor.to_string(),
                });
            }
            config.flavor = declared;
        }
        if config.flavor == DatabaseFlavor::Dameng {
            // 达梦需 ODBC 适配器与 sql/dm 翻译链（方案 M3），当前显式拒绝。
            return Err(DatabaseConfigError::DamengUnsupported);
        }
        Ok(config)
    }

    pub fn new(
        url: impl Into<String>,
        min_connections: u32,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, DatabaseConfigError> {
        let url = url.into();
        let Some((scheme, _)) = url.split_once("://") else {
            return Err(DatabaseConfigError::InvalidUrl);
        };
        let Some(flavor) = DatabaseFlavor::from_url_scheme(scheme) else {
            return Err(DatabaseConfigError::InvalidUrl);
        };
        if flavor == DatabaseFlavor::Dameng {
            // 达梦需 ODBC 适配器与 sql/dm 翻译链（方案 M3），当前显式拒绝，
            // 不允许静默落到 PostgreSQL。
            return Err(DatabaseConfigError::DamengUnsupported);
        }
        if max_connections == 0 || min_connections > max_connections {
            return Err(DatabaseConfigError::InvalidPoolSize);
        }
        if acquire_timeout.is_zero() {
            return Err(DatabaseConfigError::InvalidTimeout);
        }

        // 金仓 / openGauss 共用 PG 线协议，归一化后交给 sqlx。
        let url = flavor.normalize_url(&url);
        Ok(Self {
            url,
            flavor,
            min_connections,
            max_connections,
            acquire_timeout,
        })
    }
}

use std::str::FromStr;

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConfig")
            .field("url", &"[REDACTED]")
            .field("flavor", &self.flavor)
            .field("min_connections", &self.min_connections)
            .field("max_connections", &self.max_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .finish()
    }
}

fn parse_env(name: &'static str, default: u32) -> Result<u32, DatabaseConfigError> {
    env::var(name)
        .ok()
        .map(|value| value.parse())
        .transpose()
        .map_err(|_| DatabaseConfigError::InvalidNumber(name))
        .map(|value| value.unwrap_or(default))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseConfigError {
    MissingUrl,
    InvalidUrl,
    /// DB_FLAVOR 与 DATABASE_URL scheme 指明了不同方言（§3.3）。
    FlavorMismatch {
        declared: String,
        url_scheme: String,
    },
    /// DB_FLAVOR=dameng / dm:// URL：ODBC 适配器尚未实装（M3）。
    DamengUnsupported,
    InvalidFlavor,
    InvalidPoolSize,
    InvalidTimeout,
    InvalidNumber(&'static str),
}

impl fmt::Display for DatabaseConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUrl => formatter.write_str("DATABASE_URL is required"),
            Self::InvalidUrl => formatter.write_str(
                "DATABASE_URL must use postgres://, kingbase[8]:// or opengauss:// scheme",
            ),
            Self::FlavorMismatch {
                declared,
                url_scheme,
            } => write!(
                formatter,
                "DB_FLAVOR={declared} conflicts with the DATABASE_URL scheme ({url_scheme}); \
                 align DB_FLAVOR with the URL or use postgres:// with DB_FLAVOR to declare \
                 a PG-compatible flavor"
            ),
            Self::DamengUnsupported => formatter.write_str(
                "Dameng requires the ODBC adapter and the sql/dm migration chain, \
                 which are not available yet (xinchuang plan M3)",
            ),
            Self::InvalidFlavor => formatter.write_str("DB_FLAVOR is not a known flavor"),
            Self::InvalidPoolSize => formatter.write_str("database pool size is invalid"),
            Self::InvalidTimeout => {
                formatter.write_str("database acquire timeout must be positive")
            }
            Self::InvalidNumber(name) => write!(formatter, "{name} must be a positive integer"),
        }
    }
}

impl std::error::Error for DatabaseConfigError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::flavor::DatabaseFlavor;

    use super::{DatabaseConfig, DatabaseConfigError};

    #[test]
    fn validates_pool_bounds() {
        let result = DatabaseConfig::new(
            "postgres://localhost/rustset",
            10,
            5,
            Duration::from_secs(1),
        );
        assert!(matches!(result, Err(DatabaseConfigError::InvalidPoolSize)));
    }

    #[test]
    fn redacts_database_url_in_debug_output() {
        let config = DatabaseConfig::new(
            "postgres://user:secret@localhost/rustset",
            1,
            5,
            Duration::from_secs(1),
        )
        .unwrap();
        let output = format!("{config:?}");

        assert!(!output.contains("secret"));
        assert!(output.contains("REDACTED"));
    }

    #[test]
    fn resolves_flavor_from_url_scheme() {
        let config =
            DatabaseConfig::new("postgres://localhost/rustset", 1, 5, Duration::from_secs(1))
                .unwrap();
        assert_eq!(config.flavor, DatabaseFlavor::Postgres);

        let config = DatabaseConfig::new(
            "kingbase8://u:p@localhost:54321/rustset",
            1,
            5,
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(config.flavor, DatabaseFlavor::Kingbase);
        // 金仓 URL 归一化为 PG 线协议供 sqlx 直连
        assert!(config.url.starts_with("postgres://"));
    }

    #[test]
    fn rejects_unknown_schemes() {
        let result =
            DatabaseConfig::new("mysql://localhost/rustset", 1, 5, Duration::from_secs(1));
        assert!(matches!(result, Err(DatabaseConfigError::InvalidUrl)));
        let result = DatabaseConfig::new("not-a-url", 1, 5, Duration::from_secs(1));
        assert!(matches!(result, Err(DatabaseConfigError::InvalidUrl)));
    }

    #[test]
    fn rejects_dameng_explicitly() {
        let result =
            DatabaseConfig::new("dm://localhost/rustset", 1, 5, Duration::from_secs(1));
        assert!(matches!(result, Err(DatabaseConfigError::DamengUnsupported)));
    }
}
