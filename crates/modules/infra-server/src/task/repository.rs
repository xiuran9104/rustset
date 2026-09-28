use chrono::{DateTime, NaiveDateTime, Utc};
use rustset_infra_api::ScanTaskResponse;
use sqlx::{PgPool, Row, postgres::PgRow};

const SELECT_TASK: &str = "SELECT id,name,target,status,start_time,end_time,found_assets,
    found_risks,port_policy,domain_brute,service_detection,os_detection,site_identify,
    created_by,task_kind,scan_ports,total_targets,completed_targets,error_message,
    attempt_count,max_attempts,timeout_seconds,next_attempt_at,cancel_requested,
    create_time,update_time FROM infra_task";

fn response(row: PgRow) -> ScanTaskResponse {
    ScanTaskResponse {
        id: row.get("id"),
        name: row.get("name"),
        target: row.get("target"),
        status: row.get("status"),
        start_time: row.get("start_time"),
        end_time: row.get("end_time"),
        found_assets: row.get("found_assets"),
        found_risks: row.get("found_risks"),
        port_policy: row.get("port_policy"),
        domain_brute: row.get::<i32, _>("domain_brute") != 0,
        service_detection: row.get::<i32, _>("service_detection") != 0,
        os_detection: row.get::<i32, _>("os_detection") != 0,
        site_identify: row.get::<i32, _>("site_identify") != 0,
        created_by: row.get("created_by"),
        task_kind: row.get("task_kind"),
        scan_ports: row.get("scan_ports"),
        total_targets: row.get("total_targets"),
        completed_targets: row.get("completed_targets"),
        error_message: row.get("error_message"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        timeout_seconds: row.get("timeout_seconds"),
        next_attempt_at: row.get::<DateTime<Utc>, _>("next_attempt_at").to_rfc3339(),
        cancel_requested: row.get("cancel_requested"),
        create_time: row
            .get::<NaiveDateTime, _>("create_time")
            .and_utc()
            .to_rfc3339(),
        update_time: row
            .get::<NaiveDateTime, _>("update_time")
            .and_utc()
            .to_rfc3339(),
    }
}

pub(crate) async fn count(pool: &PgPool, tenant_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM infra_task WHERE tenant_id=$1 AND deleted=0")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
}

pub(crate) async fn page(
    pool: &PgPool,
    tenant_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<ScanTaskResponse>, sqlx::Error> {
    let query = format!(
        "{SELECT_TASK} WHERE tenant_id=$3 AND deleted=0 ORDER BY create_time DESC LIMIT $1 OFFSET $2"
    );
    sqlx::query(&query)
        .bind(limit)
        .bind(offset)
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().map(response).collect())
}

pub(crate) async fn list(
    pool: &PgPool,
    tenant_id: i64,
) -> Result<Vec<ScanTaskResponse>, sqlx::Error> {
    let query = format!("{SELECT_TASK} WHERE tenant_id=$1 AND deleted=0 ORDER BY create_time DESC");
    sqlx::query(&query)
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().map(response).collect())
}

pub(crate) async fn list_kind(
    pool: &PgPool,
    tenant_id: i64,
    task_kind: &str,
) -> Result<Vec<ScanTaskResponse>, sqlx::Error> {
    let query = format!(
        "{SELECT_TASK} WHERE tenant_id=$1 AND task_kind=$2 AND deleted=0 ORDER BY create_time DESC LIMIT 200"
    );
    sqlx::query(&query)
        .bind(tenant_id)
        .bind(task_kind)
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().map(response).collect())
}

pub(crate) async fn get(
    pool: &PgPool,
    tenant_id: i64,
    id: &str,
) -> Result<Option<ScanTaskResponse>, sqlx::Error> {
    let query = format!("{SELECT_TASK} WHERE id=$1 AND tenant_id=$2 AND deleted=0");
    sqlx::query(&query)
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map(|row| row.map(response))
}
