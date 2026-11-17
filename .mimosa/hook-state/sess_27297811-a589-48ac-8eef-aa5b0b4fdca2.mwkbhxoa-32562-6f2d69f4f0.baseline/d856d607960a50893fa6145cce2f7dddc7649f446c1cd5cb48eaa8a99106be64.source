//! 数据库错误分类门面（信创适配 §4.1.1「错误分类统一」）。
//!
//! 业务层只匹配 [`DbError`]，不直接匹配 sqlx/ODBC 原生错误码。
//! PG 家族（PostgreSQL / 金仓 / openGauss 共享 SQLSTATE 语义）在
//! [`From<sqlx::Error>`] 中完成映射；达梦适配器实装后在 ODBC 层做
//! SQLSTATE → `DbError` 的等价映射，业务分类保持不变。

use std::fmt;

/// 跨数据库统一的错误分类。
///
/// 覆盖方案 §4.1.7 要求合同测试证明的类别：唯一约束冲突、外键冲突、
/// 死锁、锁等待超时、连接中断；其余归入 `DataError`（调用方输入问题）
/// 或 `Other`（服务端故障）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbError {
    /// 唯一约束冲突（PG `23505`）。
    UniqueViolation {
        constraint: Option<String>,
        message: String,
    },
    /// 外键约束冲突（PG `23503`）。
    ForeignKeyViolation {
        constraint: Option<String>,
        message: String,
    },
    /// 非空约束冲突（PG `23502`）。
    NotNullViolation {
        constraint: Option<String>,
        message: String,
    },
    /// CHECK 约束冲突（PG `23514`）。
    CheckViolation {
        constraint: Option<String>,
        message: String,
    },
    /// 其他数据/约束类错误（PG class 22/23），属于调用方输入问题。
    DataError { code: String, message: String },
    /// 死锁被数据库检测并中止（PG `40P01`）。
    DeadlockDetected,
    /// 锁等待超时（PG `55P03` lock_not_available）。
    LockWaitTimeout,
    /// 可串行化冲突（PG `40001`）。
    SerializationConflict,
    /// 语句被取消（PG `57014`，含 statement_timeout）。
    QueryCancelled,
    /// 连接中断或不可用（PG class 08、IO 错误）。
    ConnectionInterrupted,
    /// 其他错误。
    Other(String),
}

impl DbError {
    /// 是否为调用方数据问题（应映射为 4xx 而非 5xx）。
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::UniqueViolation { .. }
                | Self::ForeignKeyViolation { .. }
                | Self::NotNullViolation { .. }
                | Self::CheckViolation { .. }
                | Self::DataError { .. }
        )
    }

    /// 命中的约束名（如有）。
    pub fn constraint(&self) -> Option<&str> {
        match self {
            Self::UniqueViolation { constraint, .. }
            | Self::ForeignKeyViolation { constraint, .. }
            | Self::NotNullViolation { constraint, .. }
            | Self::CheckViolation { constraint, .. } => constraint.as_deref(),
            _ => None,
        }
    }

    /// 数据库侧消息（无则返回分类名）。
    pub fn message(&self) -> &str {
        match self {
            Self::UniqueViolation { message, .. }
            | Self::ForeignKeyViolation { message, .. }
            | Self::NotNullViolation { message, .. }
            | Self::CheckViolation { message, .. }
            | Self::DataError { message, .. }
            | Self::Other(message) => message,
            Self::DeadlockDetected => "deadlock detected",
            Self::LockWaitTimeout => "lock wait timeout",
            Self::SerializationConflict => "serialization failure",
            Self::QueryCancelled => "query cancelled",
            Self::ConnectionInterrupted => "connection interrupted",
        }
    }

    /// 是否为指定约束上的唯一冲突。
    pub fn is_unique_violation_on(&self, constraint: &str) -> bool {
        matches!(self, Self::UniqueViolation { .. }) && self.constraint() == Some(constraint)
    }

    /// 由 SQLSTATE 码构造分类（供 PG 家族与达梦 ODBC 适配器共用）。
    pub fn from_code(code: &str, constraint: Option<String>, message: String) -> Self {
        let violation = |constraint: Option<String>, message: String| match code {
            "23502" => Self::NotNullViolation { constraint, message },
            "23503" => Self::ForeignKeyViolation { constraint, message },
            "23505" => Self::UniqueViolation { constraint, message },
            "23514" => Self::CheckViolation { constraint, message },
            _ => Self::DataError {
                code: code.to_owned(),
                message,
            },
        };
        match code {
            "40P01" => Self::DeadlockDetected,
            "55P03" => Self::LockWaitTimeout,
            "40001" => Self::SerializationConflict,
            "57014" => Self::QueryCancelled,
            _ if code.starts_with("08") => Self::ConnectionInterrupted,
            _ if code.starts_with("22") || code.starts_with("23") => {
                violation(constraint, message)
            }
            _ => Self::Other(format!("[{code}] {message}")),
        }
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UniqueViolation { .. } => write!(f, "unique violation: {}", self.message()),
            Self::ForeignKeyViolation { .. } => write!(f, "foreign key violation: {}", self.message()),
            Self::NotNullViolation { .. } => write!(f, "not-null violation: {}", self.message()),
            Self::CheckViolation { .. } => write!(f, "check violation: {}", self.message()),
            Self::DataError { code, .. } => write!(f, "data error [{code}]: {}", self.message()),
            other => write!(f, "{other:?}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<sqlx::Error> for DbError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(database) = &error {
            let code = database.code().map(|code| code.into_owned()).unwrap_or_default();
            return Self::from_code(
                &code,
                database.constraint().map(str::to_owned),
                database.message().to_owned(),
            );
        }
        match &error {
            sqlx::Error::Io(_) | sqlx::Error::WorkerCrashed => Self::ConnectionInterrupted,
            other => Self::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(constraint: &str) -> DbError {
        DbError::from_code(
            "23505",
            Some(constraint.to_owned()),
            "duplicate key".into(),
        )
    }

    #[test]
    fn classifies_pg_sqlstates() {
        assert!(matches!(unique("pk"), DbError::UniqueViolation { .. }));
        assert!(matches!(
            DbError::from_code("23503", None, "fk".into()),
            DbError::ForeignKeyViolation { .. }
        ));
        assert!(matches!(
            DbError::from_code("40P01", None, "deadlock".into()),
            DbError::DeadlockDetected
        ));
        assert!(matches!(
            DbError::from_code("55P03", None, "lock".into()),
            DbError::LockWaitTimeout
        ));
        assert!(matches!(
            DbError::from_code("08006", None, "down".into()),
            DbError::ConnectionInterrupted
        ));
        assert!(matches!(
            DbError::from_code("22P02", None, "bad text".into()),
            DbError::DataError { .. }
        ));
        assert!(matches!(
            DbError::from_code("42P01", None, "no table".into()),
            DbError::Other(_)
        ));
    }

    #[test]
    fn client_error_classification() {
        assert!(unique("pk").is_client_error());
        assert!(DbError::from_code("23502", None, "null".into()).is_client_error());
        assert!(!DbError::DeadlockDetected.is_client_error());
        assert!(!DbError::ConnectionInterrupted.is_client_error());
        assert!(!DbError::from_code("53300", None, "conns".into()).is_client_error());
    }

    #[test]
    fn unique_violation_constraint_probe() {
        let error = unique("cmdb_instance_unique_value_key");
        assert!(error.is_unique_violation_on("cmdb_instance_unique_value_key"));
        assert!(!error.is_unique_violation_on("other_key"));
        assert_eq!(error.constraint(), Some("cmdb_instance_unique_value_key"));
    }
}
