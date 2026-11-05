use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::NaiveDateTime;
use rustset_framework_database::DbPool;
use rustset_framework_database::DbQueryBuilder;
use tokio::sync::mpsc;

#[derive(Debug)]
struct AuditEntry {
    trace_id: String,
    method: String,
    path: String,
    user_agent: String,
    module: String,
    started_at: NaiveDateTime,
    ended_at: NaiveDateTime,
    duration_ms: i32,
    status: i32,
}

#[derive(Clone, Default)]
pub struct AuditMetrics {
    inner: Arc<AuditMetricValues>,
}

#[derive(Default)]
struct AuditMetricValues {
    queued: AtomicUsize,
    persisted: AtomicU64,
    dropped: AtomicU64,
    failed: AtomicU64,
    pruned: AtomicU64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditMetricsSnapshot {
    pub queued: usize,
    pub persisted: u64,
    pub dropped: u64,
    pub failed: u64,
    pub pruned: u64,
}

impl AuditMetrics {
    pub fn snapshot(&self) -> AuditMetricsSnapshot {
        AuditMetricsSnapshot {
            queued: self.inner.queued.load(Ordering::Relaxed),
            persisted: self.inner.persisted.load(Ordering::Relaxed),
            dropped: self.inner.dropped.load(Ordering::Relaxed),
            failed: self.inner.failed.load(Ordering::Relaxed),
            pruned: self.inner.pruned.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
pub struct AuditState {
    sender: mpsc::Sender<AuditEntry>,
    metrics: AuditMetrics,
}

impl AuditState {
    pub fn new(pool: DbPool) -> Self {
        let capacity = env_usize("AUDIT_QUEUE_CAPACITY", 4096).clamp(64, 65_536);
        let batch_size = env_usize("AUDIT_BATCH_SIZE", 100).clamp(1, 1000);
        let flush_ms = env_u64("AUDIT_FLUSH_INTERVAL_MS", 1000).clamp(50, 60_000);
        // 等保/密评要求审计日志留存 ≥6 个月（§4.4.3 整改项 6）。默认 190 天
        // （6 个月 + 安全余量）；下限 30 天防止误配，上限 10 年。
        let retention_days = env_i64("AUDIT_RETENTION_DAYS", 190).clamp(30, 3_650);
        let (sender, receiver) = mpsc::channel(capacity);
        let metrics = AuditMetrics::default();
        tokio::spawn(run_writer(
            pool,
            receiver,
            metrics.clone(),
            batch_size,
            Duration::from_millis(flush_ms),
            retention_days,
        ));
        Self { sender, metrics }
    }

    pub fn metrics(&self) -> AuditMetrics {
        self.metrics.clone()
    }
}

pub async fn record(
    State(state): State<AuditState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let started_at = chrono::Utc::now().naive_utc();
    let timer = Instant::now();
    let method = request.method().to_string();
    let path = request
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_else(|| request.uri().path().to_string());
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .chars()
        .take(500)
        .collect::<String>();
    let request_trace_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let response = next.run(request).await;
    if path == "/health" || path.starts_with("/health/") {
        return response;
    }
    let status = response.status().as_u16() as i32;
    let trace_id = request_trace_id
        .or_else(|| {
            response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let module = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("gateway")
        .to_string();
    let entry = AuditEntry {
        trace_id,
        method,
        path,
        user_agent,
        module,
        started_at,
        ended_at: chrono::Utc::now().naive_utc(),
        duration_ms: timer.elapsed().as_millis().min(i32::MAX as u128) as i32,
        status,
    };
    state.metrics.inner.queued.fetch_add(1, Ordering::Relaxed);
    match state.sender.try_send(entry) {
        Ok(()) => {}
        Err(error) => {
            state.metrics.inner.queued.fetch_sub(1, Ordering::Relaxed);
            state.metrics.inner.dropped.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(%error, "audit queue is full or closed; access log dropped");
        }
    }
    response
}

/// 审计留存清理的执行间隔：启动先清一次积压，此后每 6 小时一轮。
const PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 3600);

/// 单批删除上限，避免长事务与大锁；多批循环直到清完。
const PRUNE_BATCH: i64 = 5_000;

async fn run_writer(
    pool: DbPool,
    mut receiver: mpsc::Receiver<AuditEntry>,
    metrics: AuditMetrics,
    batch_size: usize,
    flush_interval: Duration,
    retention_days: i64,
) {
    let mut batch = Vec::with_capacity(batch_size);
    let mut tick = tokio::time::interval(flush_interval);
    // 启动即清一次积压（Instant 减去 PRUNE_INTERVAL 使首轮 tick 命中）。
    let mut last_prune = Instant::now() - PRUNE_INTERVAL;
    loop {
        tokio::select! {
            entry = receiver.recv() => match entry {
                Some(entry) => {
                    batch.push(entry);
                    if batch.len() >= batch_size {
                        flush(&pool, &metrics, &mut batch).await;
                    }
                }
                None => {
                    flush(&pool, &metrics, &mut batch).await;
                    break;
                }
            },
            _ = tick.tick() => {
                flush(&pool, &metrics, &mut batch).await;
                if last_prune.elapsed() >= PRUNE_INTERVAL {
                    last_prune = Instant::now();
                    prune_expired(&pool, retention_days, &metrics).await;
                }
            },
        }
    }
}

/// 删除超过留存期的访问日志（§4.4.3 #6：等保要求留存 ≥6 个月）。
/// 分批删除避免长事务；失败只记录告警，不影响写入路径。
async fn prune_expired(pool: &DbPool, retention_days: i64, metrics: &AuditMetrics) {
    let Some(cutoff) = chrono::TimeDelta::try_days(retention_days)
        .map(|delta| chrono::Utc::now().naive_utc() - delta)
    else {
        return;
    };
    let mut pruned_total = 0u64;
    loop {
        match sqlx::query(
            "DELETE FROM infra_api_access_log WHERE id IN (
                 SELECT id FROM infra_api_access_log
                 WHERE create_time < $1 AND deleted = 0
                 LIMIT $2)",
        )
        .bind(cutoff)
        .bind(PRUNE_BATCH)
        .execute(pool)
        .await
        {
            Ok(result) => {
                let deleted: u64 = result.rows_affected();
                pruned_total += deleted;
                if deleted < PRUNE_BATCH as u64 {
                    break;
                }
            }
            Err(error) => {
                tracing::warn!(?error, retention_days, "audit retention prune failed");
                return;
            }
        }
    }
    if pruned_total > 0 {
        metrics.inner.pruned.fetch_add(pruned_total, Ordering::Relaxed);
        tracing::info!(pruned = pruned_total, retention_days, "pruned expired audit logs");
    }
}

async fn flush(pool: &DbPool, metrics: &AuditMetrics, batch: &mut Vec<AuditEntry>) {
    if batch.is_empty() {
        return;
    }
    let count = batch.len();
    let mut query = DbQueryBuilder::new(
        "INSERT INTO infra_api_access_log(
            trace_id,application_name,request_method,request_url,user_agent,
            operate_module,begin_time,end_time,duration,result_code,result_msg) ",
    );
    query.push_values(batch.iter(), |mut row, entry| {
        row.push_bind(&entry.trace_id)
            .push_bind("rustset-gateway")
            .push_bind(&entry.method)
            .push_bind(&entry.path)
            .push_bind(&entry.user_agent)
            .push_bind(&entry.module)
            .push_bind(entry.started_at)
            .push_bind(entry.ended_at)
            .push_bind(entry.duration_ms)
            .push_bind(entry.status)
            .push_bind(if entry.status >= 400 { "failed" } else { "ok" });
    });
    let result = query.build().execute(pool).await;
    metrics.inner.queued.fetch_sub(count, Ordering::Relaxed);
    match result {
        Ok(_) => {
            metrics
                .inner
                .persisted
                .fetch_add(count as u64, Ordering::Relaxed);
        }
        Err(error) => {
            metrics
                .inner
                .failed
                .fetch_add(count as u64, Ordering::Relaxed);
            tracing::error!(?error, count, "failed to persist audit log batch");
        }
    }
    batch.clear();
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_i64(name: &str, default: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_snapshot_reports_queue_outcomes() {
        let metrics = AuditMetrics::default();
        metrics.inner.queued.store(3, Ordering::Relaxed);
        metrics.inner.persisted.store(8, Ordering::Relaxed);
        metrics.inner.dropped.store(2, Ordering::Relaxed);
        metrics.inner.failed.store(1, Ordering::Relaxed);
        metrics.inner.pruned.store(6, Ordering::Relaxed);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.queued, 3);
        assert_eq!(snapshot.persisted, 8);
        assert_eq!(snapshot.dropped, 2);
        assert_eq!(snapshot.failed, 1);
        assert_eq!(snapshot.pruned, 6);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the latest migrations"]
    async fn prunes_logs_older_than_retention() {
        let pool = DbPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let trace = format!("audit-prune-{}", std::process::id());
        let now = chrono::Utc::now().naive_utc();
        let entry = |index: i64, started: NaiveDateTime| AuditEntry {
            trace_id: format!("{trace}-{index}"),
            method: "GET".to_owned(),
            path: "/health/test".to_owned(),
            user_agent: "test".to_owned(),
            module: "health".to_owned(),
            started_at: started,
            ended_at: started,
            duration_ms: 1,
            status: 200,
        };
        let mut batch = vec![
            entry(0, now - chrono::TimeDelta::try_days(400).unwrap()),
            entry(1, now),
        ];
        let metrics = AuditMetrics::default();
        metrics.inner.queued.store(batch.len(), Ordering::Relaxed);
        flush(&pool, &metrics, &mut batch).await;
        prune_expired(&pool, 190, &metrics).await;
        assert_eq!(metrics.snapshot().pruned, 1);
        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM infra_api_access_log WHERE trace_id LIKE $1")
                .bind(format!("{trace}-%"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 1);
        sqlx::query("DELETE FROM infra_api_access_log WHERE trace_id LIKE $1")
            .bind(format!("{trace}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the latest migrations"]
    async fn flushes_access_logs_as_one_batch() {
        let pool = DbPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let trace = format!("audit-batch-{}", std::process::id());
        let now = chrono::Utc::now().naive_utc();
        let mut batch = (0..2)
            .map(|index| AuditEntry {
                trace_id: format!("{trace}-{index}"),
                method: "GET".to_owned(),
                path: "/health/test".to_owned(),
                user_agent: "test".to_owned(),
                module: "health".to_owned(),
                started_at: now,
                ended_at: now,
                duration_ms: 1,
                status: 200,
            })
            .collect::<Vec<_>>();
        let metrics = AuditMetrics::default();
        metrics.inner.queued.store(batch.len(), Ordering::Relaxed);
        flush(&pool, &metrics, &mut batch).await;
        assert!(batch.is_empty());
        assert_eq!(metrics.snapshot().persisted, 2);
        assert_eq!(metrics.snapshot().queued, 0);
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM infra_api_access_log WHERE trace_id LIKE $1")
                .bind(format!("{trace}-%"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2);
        sqlx::query("DELETE FROM infra_api_access_log WHERE trace_id LIKE $1")
            .bind(format!("{trace}-%"))
            .execute(&pool)
            .await
            .unwrap();
    }
}
