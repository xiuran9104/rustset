use crate::{InfraState, QueryParams};
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
use rustset_infra_api::{CreateNetworkZoneRequest, UpdateNetworkZoneRequest};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/network-zone/page", get(page))
        .api_route("/infra/network-zone/list", get(list))
        .api_route("/infra/network-zone/get", get(get_one))
        .api_route("/infra/network-zone/create", post(create))
        .api_route("/infra/network-zone/update", put(update))
        .api_route("/infra/network-zone/delete", delete(delete_one))
        .api_route("/infra/network-zone/delete-list", delete(delete_list))
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
        "SELECT count(*) FROM infra_network_zone WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count"))?;
    let list = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) - 'tenant_id' FROM infra_network_zone t WHERE tenant_id=$1 AND deleted=0 ORDER BY priority ASC LIMIT $2 OFFSET $3").bind(tenant.id()).bind(page_size).bind(offset).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed to list"))?.into_iter().map(crate::table_value).collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

async fn list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) - 'tenant_id' FROM infra_network_zone t WHERE tenant_id=$1 AND deleted=0 ORDER BY priority ASC",
    )
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list"))?
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
        "SELECT to_jsonb(t) - 'tenant_id' FROM infra_network_zone t WHERE id=$1 AND tenant_id=$2 AND deleted=0",
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
    Json(payload): Json<CreateNetworkZoneRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO infra_network_zone (id, tenant_id, name, cidr, priority, cloud_platform_id, cloud_platform_name, machine_room_id, machine_room_name) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(&id).bind(tenant.id()).bind(payload.name).bind(payload.cidr).bind(payload.priority)
        .bind(payload.cloud_platform_id).bind(payload.cloud_platform_name)
        .bind(payload.machine_room_id).bind(payload.machine_room_name)
        .execute(&state.pool).await.map_err(|error| crate::record_query_error("create network zone", error))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<UpdateNetworkZoneRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let result = sqlx::query("UPDATE infra_network_zone SET name=$3, cidr=$4, priority=$5, cloud_platform_id=$6, cloud_platform_name=$7, machine_room_id=$8, machine_room_name=$9, update_time=now() WHERE id=$1 AND tenant_id=$2 AND deleted=0")
        .bind(&payload.id).bind(tenant.id()).bind(payload.name).bind(payload.cidr).bind(payload.priority)
        .bind(payload.cloud_platform_id).bind(payload.cloud_platform_name)
        .bind(payload.machine_room_id).bind(payload.machine_room_name)
        .execute(&state.pool).await.map_err(|error| crate::record_query_error("update network zone", error))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("network zone not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete_one(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = params.get("id").cloned().unwrap_or_default();
    let result = sqlx::query("UPDATE infra_network_zone SET deleted=1, update_time=now() WHERE id=$1 AND tenant_id=$2 AND deleted=0")
        .bind(&id)
        .bind(tenant.id())
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed to delete"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("network zone not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let ids = params
        .get("ids")
        .into_iter()
        .flat_map(|ids| ids.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err(AppError::bad_request("ids are required"));
    }
    sqlx::query("UPDATE infra_network_zone SET deleted=1, update_time=now() WHERE id=ANY($1) AND tenant_id=$2 AND deleted=0")
        .bind(&ids)
        .bind(tenant.id())
        .execute(&state.pool)
        .await
        .map_err(|error| crate::record_query_error("delete network zones", error))?;
    Ok(Json(ApiResponse::new(())))
}
