use std::time::Duration;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use rustset_framework_database::PgPool;
use rustset_framework_redis::RedisClient;
use serde::Serialize;

use crate::audit::{AuditMetrics, AuditMetricsSnapshot};

#[derive(Clone)]
pub struct RuntimeHealthState {
    pool: PgPool,
    cache: Dependency,
    rate_limit: Dependency,
    audit: AuditMetrics,
}

#[derive(Clone)]
pub struct Dependency {
    pub configured: bool,
    pub required: bool,
    pub client: Option<RedisClient>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadinessResponse {
    status: &'static str,
    database: ComponentStatus,
    cache: ComponentStatus,
    rate_limit: ComponentStatus,
    database_pool: PoolSnapshot,
    tasks: TaskSnapshot,
    audit: AuditMetricsSnapshot,
    checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComponentStatus {
    configured: bool,
    required: bool,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoolSnapshot {
    size: u32,
    idle: usize,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSnapshot {
    queued: i64,
    retrying: i64,
    running: i64,
    oldest_wait_seconds: Option<i64>,
}

impl RuntimeHealthState {
    pub fn new(
        pool: PgPool,
        cache: Dependency,
        rate_limit: Dependency,
        audit: AuditMetrics,
    ) -> Self {
        Self {
            pool,
            cache,
            rate_limit,
            audit,
        }
    }
}

pub fn routes(state: RuntimeHealthState) -> Router {
    Router::new()
        .route("/health/ready", get(readiness))
        .with_state(state)
}

async fn readiness(State(state): State<RuntimeHealthState>) -> Response {
    let database_available = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool),
    )
    .await
    .is_ok_and(|result| result.is_ok());
    let (cache, rate_limit) = tokio::join!(
        component_status(&state.cache),
        component_status(&state.rate_limit)
    );
    let tasks = if database_available {
        task_snapshot(&state.pool).await.unwrap_or_default()
    } else {
        TaskSnapshot::default()
    };
    let ready = database_available
        && (!cache.required || cache.available)
        && (!rate_limit.required || rate_limit.available);
    let response = ReadinessResponse {
        status: if ready { "ready" } else { "not_ready" },
        database: ComponentStatus {
            configured: true,
            required: true,
            available: database_available,
        },
        cache,
        rate_limit,
        database_pool: PoolSnapshot {
            size: state.pool.size(),
            idle: state.pool.num_idle(),
        },
        tasks,
        audit: state.audit.snapshot(),
        checked_at: chrono::Utc::now(),
    };
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response)).into_response()
}

async fn component_status(dependency: &Dependency) -> ComponentStatus {
    let available = match &dependency.client {
        Some(client) => tokio::time::timeout(Duration::from_secs(1), client.ping())
            .await
            .is_ok_and(|result| result.is_ok()),
        None => false,
    };
    ComponentStatus {
        configured: dependency.configured,
        required: dependency.required,
        available,
    }
}

async fn task_snapshot(pool: &PgPool) -> Result<TaskSnapshot, sqlx::Error> {
    let row: (i64, i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT
            count(*) FILTER (WHERE status='queued'),
            count(*) FILTER (WHERE status='retrying'),
            count(*) FILTER (WHERE status='running'),
            EXTRACT(EPOCH FROM now()-min(next_attempt_at)
                FILTER (WHERE status IN ('queued','retrying')))::bigint
         FROM infra_task WHERE deleted=0",
    )
    .fetch_one(pool)
    .await?;
    Ok(TaskSnapshot {
        queued: row.0,
        retrying: row.1,
        running: row.2,
        oldest_wait_seconds: row.3.map(|seconds| seconds.max(0)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn optional_unconfigured_dependency_is_degraded_but_not_required() {
        let status = component_status(&Dependency {
            configured: false,
            required: false,
            client: None,
        })
        .await;
        assert!(!status.available);
        assert!(!status.required);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the latest migrations"]
    async fn reads_persisted_task_backlog() {
        let pool = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let snapshot = task_snapshot(&pool).await.unwrap();
        assert!(snapshot.queued >= 0);
        assert!(snapshot.retrying >= 0);
        assert!(snapshot.running >= 0);
    }
}
