use crate::{InfraState, QueryParams, bool_field, str_field};
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
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

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
}

async fn page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let page_no = params.page_no.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (page_no - 1) * page_size;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM infra_task WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?;
    let list = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_task t WHERE tenant_id=$3 AND deleted=0 ORDER BY create_time DESC LIMIT $1 OFFSET $2").bind(page_size).bind(offset).bind(tenant.id()).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed"))?.into_iter().map(crate::table_value).collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

async fn list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_task t WHERE tenant_id=$1 AND deleted=0 ORDER BY create_time DESC",
    )
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?
    .into_iter()
    .map(crate::table_value)
    .collect();
    Ok(Json(ApiResponse::new(list)))
}

async fn get_one(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = params.get("id").cloned().unwrap_or_default();
    let v = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_task t WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(&id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?
    .ok_or_else(|| AppError::not_found("not found"))?;
    Ok(Json(ApiResponse::new(crate::table_value(v))))
}

async fn create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO infra_task (id, name, target, status, port_policy, domain_brute, service_detection, os_detection, site_identify, created_by, tenant_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(&id).bind(&str_field(&payload, "name")).bind(&str_field(&payload, "target")).bind("pending").bind(&str_field(&payload, "portPolicy"))
        .bind(bool_field(&payload, "domainBrute", false) as i32).bind(bool_field(&payload, "serviceDetection", false) as i32)
        .bind(bool_field(&payload, "osDetection", false) as i32).bind(bool_field(&payload, "siteIdentify", false) as i32)
        .bind(&user.username).bind(tenant.id()).execute(&state.pool).await.map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = str_field(&payload, "id");
    if id.is_empty() {
        return Err(AppError::bad_request("id is required"));
    }
    sqlx::query("UPDATE infra_task SET name=$2, target=$3, port_policy=$4, domain_brute=$5, service_detection=$6, os_detection=$7, site_identify=$8, update_time=now() WHERE id=$1 AND tenant_id=$9 AND deleted=0")
        .bind(&id).bind(&str_field(&payload, "name")).bind(&str_field(&payload, "target")).bind(&str_field(&payload, "portPolicy"))
        .bind(bool_field(&payload, "domainBrute", false) as i32).bind(bool_field(&payload, "serviceDetection", false) as i32)
        .bind(bool_field(&payload, "osDetection", false) as i32).bind(bool_field(&payload, "siteIdentify", false) as i32)
        .bind(tenant.id()).execute(&state.pool).await.map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn delete_one(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    sqlx::query("UPDATE infra_task SET deleted=1, update_time=now() WHERE id=$1 AND tenant_id=$2")
        .bind(&params.get("id").cloned().unwrap_or_default())
        .bind(tenant.id())
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let ids = parse_task_ids(params.get("ids").map(String::as_str).unwrap_or_default())?;
    delete_tasks(&state.pool, &tenant, &ids).await?;
    Ok(Json(ApiResponse::new(())))
}

async fn trigger_scan(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let target_ip = str_field(&payload, "targetIp");
    let ports: Vec<i32> = payload
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_i64().map(|n| n as i32))
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                21, 22, 23, 25, 53, 80, 443, 3306, 3389, 5432, 6379, 8080, 27017,
            ]
        });
    let pool = state.pool.clone();
    let tip = target_ip.clone();
    let pr = ports.clone();
    tokio::spawn(async move {
        for port in &ports {
            let addr = format!("{}:{}", tip, port);
            if let Ok(sa) = addr.parse::<std::net::SocketAddr>() {
                let open = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    tokio::net::TcpStream::connect(sa),
                )
                .await
                .is_ok_and(|result| result.is_ok());
                if open {
                    let rid = Uuid::new_v4().to_string();
                    let _ = sqlx::query("INSERT INTO infra_risk (id, asset_ip, port, severity, description, status, create_time, update_time, tenant_id) VALUES ($1,$2,$3,$4,$5,$6,now(),now(),$7)").bind(&rid).bind(&tip).bind(*port).bind("Low").bind(format!("Open port {} detected", port)).bind("open").bind(tenant.id()).execute(&pool).await;
                }
            }
        }
    });
    Ok(Json(ApiResponse::new(
        json!({"message": format!("Scan started for {}", target_ip), "targetIp": target_ip, "ports": pr}),
    )))
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
        "UPDATE infra_task SET deleted=1, update_time=now() WHERE id = ANY($1) AND tenant_id=$2 AND deleted=0",
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
