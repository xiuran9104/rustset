use crate::{InfraState, QueryParams, bool_field};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use rustset_infra_api::{
    CreateScanTaskRequest, ScanTaskPageResponse, ScanTaskResponse, TaskIdRequest, TaskIdsQuery,
    TriggerScanRequest, TriggerScanResponse, UpdateScanTaskRequest,
};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) mod repository;
pub(crate) mod worker;

pub(crate) fn start_worker(pool: sqlx::PgPool) {
    worker::start(pool);
}

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/task/page", get(page))
        .api_route("/infra/task/list", get(list))
        .api_route("/infra/task/get", get(get_one))
        .api_route("/infra/task/create", post(create))
        .api_route("/infra/task/update", put(update))
        .api_route("/infra/task/delete", delete(delete_one))
        .api_route("/infra/task/delete-list", delete(delete_list))
        .api_route("/infra/task/trigger-scan", post(trigger_scan))
        .api_route("/infra/task/cancel", put(cancel))
        .api_route("/infra/task/retry", post(retry))
}

async fn page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<ScanTaskPageResponse>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let page_no = params.page_no.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (page_no - 1) * page_size;
    let total = repository::count(&state.pool, tenant.id())
        .await
        .map_err(|_| AppError::internal("读取任务数量失败"))?;
    let list = repository::page(&state.pool, tenant.id(), page_size, offset)
        .await
        .map_err(|_| AppError::internal("读取任务列表失败"))?;
    Ok(Json(ApiResponse::new(ScanTaskPageResponse { list, total })))
}

async fn list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<ScanTaskResponse>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let list = repository::list(&state.pool, tenant.id())
        .await
        .map_err(|_| AppError::internal("读取任务列表失败"))?;
    Ok(Json(ApiResponse::new(list)))
}

async fn get_one(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<TaskIdRequest>,
) -> Result<Json<ApiResponse<ScanTaskResponse>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let task = repository::get(&state.pool, tenant.id(), &params.id)
        .await
        .map_err(|_| AppError::internal("读取任务失败"))?
        .ok_or_else(|| AppError::not_found("任务不存在"))?;
    Ok(Json(ApiResponse::new(task)))
}

async fn create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<CreateScanTaskRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let targets = parse_targets(&payload.target)?;
    let policy = payload.port_policy.trim().to_owned();
    let ports = ports_for_policy(&policy)?;
    let options = json!({
        "domainBrute": payload.domain_brute,
        "serviceDetection": payload.service_detection.unwrap_or(true),
        "osDetection": payload.os_detection,
        "siteIdentify": payload.site_identify,
    });
    let id = enqueue(
        &state.pool,
        &tenant,
        &user.username,
        &payload.name,
        &payload.target,
        "scan",
        &policy,
        targets,
        ports,
        payload.idempotency_key.as_deref(),
        payload.max_attempts,
        payload.timeout_seconds,
        &options,
    )
    .await?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<UpdateScanTaskRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    if payload.id.trim().is_empty() {
        return Err(AppError::bad_request("id is required"));
    }
    let targets = parse_targets(&payload.target)?;
    let policy = payload.port_policy.trim().to_owned();
    let ports = ports_for_policy(&policy)?;
    let durable_payload = json!({"targetIps": targets, "ports": ports});
    let result = sqlx::query("UPDATE infra_task SET name=$2, target=$3, port_policy=$4, domain_brute=$5, service_detection=$6, os_detection=$7, site_identify=$8, payload=$9, update_time=now() WHERE id=$1 AND tenant_id=$10 AND deleted=0 AND status IN ('queued','retrying')")
        .bind(&payload.id).bind(&payload.name).bind(&payload.target).bind(&policy)
        .bind(payload.domain_brute as i32).bind(payload.service_detection.unwrap_or(true) as i32)
        .bind(payload.os_detection as i32).bind(payload.site_identify as i32)
        .bind(durable_payload).bind(tenant.id()).execute(&state.pool).await.map_err(|_| AppError::internal("failed"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::bad_request("只有排队或等待重试的任务可以修改"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete_one(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<TaskIdRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    sqlx::query("UPDATE infra_task SET deleted=1,cancel_requested=true,update_time=now() WHERE id=$1 AND tenant_id=$2")
        .bind(&params.id)
        .bind(tenant.id())
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<TaskIdsQuery>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let ids = parse_task_ids(&params.ids)?;
    delete_tasks(&state.pool, &tenant, &ids).await?;
    Ok(Json(ApiResponse::new(())))
}

async fn trigger_scan(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<TriggerScanRequest>,
) -> Result<Json<ApiResponse<TriggerScanResponse>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let target_ip = payload.target_ip.trim().to_owned();
    let targets = parse_targets(&target_ip)?;
    let ports = if payload.ports.is_empty() {
        vec![
            21, 22, 23, 25, 53, 80, 443, 3306, 3389, 5432, 6379, 8080, 27017,
        ]
    } else {
        payload.ports
    };
    let ports = normalize_ports(ports)?;
    let id = enqueue(
        &state.pool,
        &tenant,
        &user.username,
        &format!("扫描 {target_ip}"),
        &target_ip,
        "scan",
        "custom",
        targets,
        ports.clone(),
        payload.idempotency_key.as_deref(),
        payload.max_attempts,
        payload.timeout_seconds,
        &json!({}),
    )
    .await?;
    Ok(Json(ApiResponse::new(TriggerScanResponse {
        task_id: id,
        message: format!("扫描任务已排队: {target_ip}"),
        target_ip,
        ports,
    })))
}

async fn cancel(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<TaskIdRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = payload.id;
    let result = sqlx::query(
        "UPDATE infra_task SET cancel_requested=true,
             status=CASE WHEN status IN ('queued','retrying') THEN 'cancelled' ELSE status END,
             end_time=CASE WHEN status IN ('queued','retrying') THEN now()::text ELSE end_time END,
             update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND deleted=0
           AND status IN ('queued','retrying','running')",
    )
    .bind(id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("取消任务失败"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::bad_request("任务不存在或当前状态不能取消"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn retry(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<TaskIdRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = payload.id;
    let result = sqlx::query(
        "UPDATE infra_task SET status='queued',attempt_count=0,cancel_requested=false,
             next_attempt_at=now(),lease_owner=NULL,lease_expires_at=NULL,
             start_time=NULL,end_time=NULL,error_message=NULL,update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND deleted=0
           AND task_kind IN ('scan','inspection','asset_discovery') AND status IN ('failed','cancelled')",
    )
    .bind(id)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("重试任务失败"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::bad_request("任务不存在或当前状态不能重试"));
    }
    Ok(Json(ApiResponse::new(())))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn enqueue(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    operator: &str,
    name: &str,
    target: &str,
    task_kind: &str,
    policy: &str,
    targets: Vec<std::net::IpAddr>,
    ports: Vec<i32>,
    idempotency_key: Option<&str>,
    max_attempts: Option<i64>,
    timeout_seconds: Option<i64>,
    options: &Value,
) -> Result<String, AppError> {
    if name.trim().is_empty() || name.chars().count() > 256 {
        return Err(AppError::bad_request("任务名称需要 1-256 个字符"));
    }
    let idempotency_key = idempotency_key
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if idempotency_key.is_some_and(|value| value.len() > 128) {
        return Err(AppError::bad_request("idempotencyKey 最多 128 个字符"));
    }
    let max_attempts = max_attempts.unwrap_or(3).clamp(1, 20) as i32;
    let estimated_seconds = ((ports.len().div_ceil(64) * targets.len()) as i64)
        .saturating_add(60)
        .clamp(300, 86_400);
    let timeout_seconds = timeout_seconds
        .unwrap_or(estimated_seconds)
        .clamp(1, 86_400) as i32;
    let id = Uuid::new_v4().to_string();
    let durable_payload = json!({"targetIps":targets,"ports":ports});
    if !matches!(task_kind, "scan" | "asset_discovery") {
        return Err(AppError::bad_request("不支持的任务类型"));
    }
    let existing_or_created: String = sqlx::query_scalar(
        "INSERT INTO infra_task
            (id,name,target,status,port_policy,domain_brute,service_detection,
             os_detection,site_identify,created_by,tenant_id,task_kind,scan_ports,
             total_targets,payload,idempotency_key,max_attempts,timeout_seconds)
         VALUES($1,$2,$3,'queued',$4,$5,$6,$7,$8,$9,$10,$17,$11,$12,$13,$14,$15,$16)
         ON CONFLICT(tenant_id,idempotency_key)
             WHERE deleted=0 AND idempotency_key IS NOT NULL
         DO UPDATE SET update_time=infra_task.update_time
         RETURNING id",
    )
    .bind(&id)
    .bind(name.trim())
    .bind(target)
    .bind(policy)
    .bind(bool_field(options, "domainBrute", false) as i32)
    .bind(bool_field(options, "serviceDetection", true) as i32)
    .bind(bool_field(options, "osDetection", false) as i32)
    .bind(bool_field(options, "siteIdentify", false) as i32)
    .bind(operator)
    .bind(tenant.id())
    .bind(&ports)
    .bind(targets.len() as i32)
    .bind(durable_payload)
    .bind(idempotency_key)
    .bind(max_attempts)
    .bind(timeout_seconds)
    .bind(task_kind)
    .fetch_one(pool)
    .await
    .map_err(|error| crate::record_query_error("enqueue scan task", error))?;
    Ok(existing_or_created)
}

pub(crate) fn parse_targets(value: &str) -> Result<Vec<std::net::IpAddr>, AppError> {
    let targets = value
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<std::net::IpAddr>()
                .map_err(|_| AppError::bad_request(format!("目标必须是明确的 IP 地址: {value}")))
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    if targets.is_empty() || targets.len() > 64 {
        return Err(AppError::bad_request("每个任务需要 1-64 个明确的 IP 地址"));
    }
    Ok(targets.into_iter().collect())
}

pub(crate) fn normalize_ports(ports: Vec<i32>) -> Result<Vec<i32>, AppError> {
    let ports = ports.into_iter().collect::<std::collections::BTreeSet<_>>();
    if ports.is_empty() || ports.iter().any(|port| !(1..=65_535).contains(port)) {
        return Err(AppError::bad_request("端口必须在 1-65535 之间"));
    }
    Ok(ports.into_iter().collect())
}

fn ports_for_policy(policy: &str) -> Result<Vec<i32>, AppError> {
    const TOP: &[i32] = &[
        21, 22, 23, 25, 53, 80, 110, 135, 139, 143, 443, 445, 993, 995, 1433, 1521, 2049, 2375,
        3306, 3389, 5432, 5900, 6379, 8080, 8443, 9200, 11211, 27017,
    ];
    match policy {
        "COMMON" | "TOP100" | "" => Ok(TOP.to_vec()),
        "TOP1000" => {
            let mut ports = (1..=1000).collect::<std::collections::BTreeSet<_>>();
            ports.extend(TOP);
            Ok(ports.into_iter().collect())
        }
        "ALL" => Ok((1..=65_535).collect()),
        _ => Err(AppError::bad_request("不支持的端口策略")),
    }
}

// Task IDs are strings (UUIDs for new tasks), unlike the numeric CRUD IDs.
fn parse_task_ids(raw: &str) -> Result<Vec<String>, AppError> {
    let mut ids: Vec<String> = raw.split(',').map(|id| id.trim().to_owned()).collect();
    if ids.iter().any(|id| id.is_empty()) {
        return Err(AppError::bad_request("ids must contain non-empty task IDs"));
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

async fn delete_tasks(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    ids: &[String],
) -> Result<(), AppError> {
    // One statement is atomic: any database failure rolls back the whole batch.
    // Missing/already deleted IDs are successful no-ops, making retries safe.
    sqlx::query(
        "UPDATE infra_task SET deleted=1,cancel_requested=true,update_time=now() WHERE id = ANY($1) AND tenant_id=$2 AND deleted=0",
    )
    .bind(ids)
    .bind(tenant.id())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to delete tasks"))?;
    Ok(())
}

#[cfg(test)]
mod tests;
