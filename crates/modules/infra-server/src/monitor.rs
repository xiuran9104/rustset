use chrono::{TimeDelta, Utc};
use std::fs;

use axum::{Json, extract::State};
use rustset_framework_common::ApiResponse;
use rustset_framework_web::AppError;
use serde_json::{Value, json};
use sqlx::Row;

use crate::InfraState;

pub(super) async fn postgresql(
    State(state): State<InfraState>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let database = sqlx::query(
        "SELECT version() AS version,
                current_database() AS database_name,
                pg_database_size(current_database()) AS database_size,
                current_setting('max_connections')::bigint AS max_connections,
                pg_postmaster_start_time() AS started_at,
                numbackends, xact_commit, xact_rollback, blks_read, blks_hit,
                deadlocks, temp_bytes
         FROM pg_stat_database WHERE datname=current_database()",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read PostgreSQL statistics"))?;
    let connections = sqlx::query(
        "SELECT coalesce(state, 'unknown') AS state, count(*)::bigint AS count
         FROM pg_stat_activity WHERE datname=current_database()
         GROUP BY state ORDER BY count(*) DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read PostgreSQL connections"))?;
    let tables = sqlx::query(
        "SELECT schemaname, relname,
                n_live_tup::bigint AS live_rows, n_dead_tup::bigint AS dead_rows,
                pg_total_relation_size(relid)::bigint AS total_size,
                seq_scan::bigint, idx_scan::bigint
         FROM pg_stat_user_tables
         ORDER BY pg_total_relation_size(relid) DESC LIMIT 12",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read PostgreSQL table statistics"))?;
    let active_queries = sqlx::query(
        "SELECT pid::bigint, usename, state,
                (extract(epoch FROM (clock_timestamp()-query_start))*1000)::float8 AS duration_ms,
                left(query, 300) AS query
         FROM pg_stat_activity
         WHERE datname=current_database() AND pid<>pg_backend_pid()
           AND state<>'idle'
         ORDER BY query_start LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read PostgreSQL active queries"))?;
    let hits = database.get::<i64, _>("blks_hit");
    let reads = database.get::<i64, _>("blks_read");
    let cache_hit_ratio = if hits + reads == 0 {
        100.0
    } else {
        hits as f64 * 100.0 / (hits + reads) as f64
    };
    Ok(Json(ApiResponse::new(json!({
        "version": database.get::<String, _>("version"),
        "databaseName": database.get::<String, _>("database_name"),
        "databaseSize": database.get::<i64, _>("database_size"),
        "connections": database.get::<i32, _>("numbackends"),
        "maxConnections": database.get::<i64, _>("max_connections"),
        "startedAt": database.get::<chrono::DateTime<chrono::Utc>, _>("started_at"),
        "transactions": {
            "committed": database.get::<i64, _>("xact_commit"),
            "rolledBack": database.get::<i64, _>("xact_rollback")
        },
        "cacheHitRatio": cache_hit_ratio,
        "deadlocks": database.get::<i64, _>("deadlocks"),
        "temporaryBytes": database.get::<i64, _>("temp_bytes"),
        "connectionStates": connections.into_iter().map(|row| json!({
            "state": row.get::<String, _>("state"), "count": row.get::<i64, _>("count")
        })).collect::<Vec<_>>(),
        "tables": tables.into_iter().map(|row| json!({
            "schema": row.get::<String, _>("schemaname"),
            "table": row.get::<String, _>("relname"),
            "liveRows": row.get::<i64, _>("live_rows"),
            "deadRows": row.get::<i64, _>("dead_rows"),
            "totalSize": row.get::<i64, _>("total_size"),
            "sequentialScans": row.get::<i64, _>("seq_scan"),
            "indexScans": row.get::<i64, _>("idx_scan")
        })).collect::<Vec<_>>(),
        "activeQueries": active_queries.into_iter().map(|row| json!({
            "pid": row.get::<i64, _>("pid"),
            "user": row.get::<String, _>("usename"),
            "state": row.get::<String, _>("state"),
            "durationMs": row.get::<f64, _>("duration_ms"),
            "query": row.get::<String, _>("query")
        })).collect::<Vec<_>>()
    }))))
}

pub(super) async fn rust_service(State(state): State<InfraState>) -> Json<ApiResponse<Value>> {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let value_kb = |name: &str| {
        status
            .lines()
            .find_map(|line| {
                let value = line.strip_prefix(name)?.split_whitespace().next()?;
                value.parse::<u64>().ok()
            })
            .unwrap_or(0)
    };
    let file_descriptors = fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    Json(ApiResponse::new(json!({
        "service": "rustset-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "rust": option_env!("RUSTC_VERSION").unwrap_or("Rust"),
        "processId": std::process::id(),
        "uptimeSeconds": state.started_at.elapsed().as_secs(),
        "cpuCores": std::thread::available_parallelism().map(usize::from).unwrap_or(1),
        "memory": {
            "residentBytes": value_kb("VmRSS:") * 1024,
            "virtualBytes": value_kb("VmSize:") * 1024,
            "peakResidentBytes": value_kb("VmHWM:") * 1024
        },
        "threads": value_kb("Threads:"),
        "fileDescriptors": file_descriptors,
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".into()),
        "healthy": true
    })))
}

pub(super) async fn traces(
    State(state): State<InfraState>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let summary = sqlx::query(
        "SELECT count(*)::bigint AS total,
                count(*) FILTER (WHERE result_code >= 400)::bigint AS failed,
                coalesce(avg(duration), 0)::float8 AS average_duration,
                coalesce(max(duration), 0)::bigint AS maximum_duration
         FROM infra_api_access_log
         WHERE deleted=0 AND create_time >= $1",
    )
    // chrono-computed replacement for `now()-interval '24 hours'` (§4.1.2)
    .bind(Utc::now().naive_utc() - TimeDelta::try_hours(24).unwrap_or_default())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read trace summary"))?;
    let rows = sqlx::query(
        "SELECT coalesce(trace_id, '') AS trace_id,
                coalesce(application_name, '') AS application_name,
                coalesce(request_method, '') AS request_method,
                coalesce(request_url, '') AS request_url,
                duration, coalesce(result_code, 0) AS result_code,
                coalesce(result_msg, '') AS result_msg,
                begin_time,
                end_time
         FROM infra_api_access_log WHERE deleted=0
         ORDER BY create_time DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read traces"))?;
    Ok(Json(ApiResponse::new(json!({
        "window": "24h",
        "total": summary.get::<i64, _>("total"),
        "failed": summary.get::<i64, _>("failed"),
        "averageDurationMs": summary.get::<f64, _>("average_duration"),
        "maximumDurationMs": summary.get::<i64, _>("maximum_duration"),
        "traces": rows.into_iter().map(|row| json!({
            "traceId": row.get::<String, _>("trace_id"),
            "service": row.get::<String, _>("application_name"),
            "method": row.get::<String, _>("request_method"),
            "url": row.get::<String, _>("request_url"),
            "durationMs": row.get::<i32, _>("duration"),
            "status": row.get::<i32, _>("result_code"),
            "message": row.get::<String, _>("result_msg"),
            "startedAt": format_timestamp(row.get::<Option<chrono::NaiveDateTime>, _>("begin_time")),
            "endedAt": format_timestamp(row.get::<Option<chrono::NaiveDateTime>, _>("end_time"))
        })).collect::<Vec<_>>()
    }))))
}

/// Portability replacement for `to_char(ts, 'YYYY-MM-DD HH24:MI:SS.MS')` (§4.1.2).
fn format_timestamp(value: Option<chrono::NaiveDateTime>) -> String {
    value
        .map(|ts| ts.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
        .unwrap_or_default()
}
