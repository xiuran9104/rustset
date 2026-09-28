use crate::{InfraState, QueryParams, i32_field, opt_i64_field, opt_str_field, str_field};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_web::AppError;
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
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let page_no = params.page_no.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (page_no - 1) * page_size;
    let total =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM infra_network_zone WHERE deleted=0")
            .fetch_one(&state.pool)
            .await
            .map_err(|_| AppError::internal("failed to count"))?;
    let list = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_network_zone t WHERE deleted=0 ORDER BY priority ASC LIMIT $1 OFFSET $2").bind(page_size).bind(offset).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed to list"))?.into_iter().map(crate::table_value).collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

async fn list(State(state): State<InfraState>) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_network_zone t WHERE deleted=0 ORDER BY priority ASC",
    )
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
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let id = params.get("id").cloned().unwrap_or_default();
    let v = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_network_zone t WHERE id=$1 AND deleted=0",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?
    .ok_or_else(|| AppError::not_found("not found"))?;
    Ok(Json(ApiResponse::new(crate::table_value(v))))
}

async fn create(
    State(state): State<InfraState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO infra_network_zone (id, name, cidr, priority, cloud_platform_id, cloud_platform_name, machine_room_id, machine_room_name) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(&id).bind(&str_field(&payload, "name")).bind(&str_field(&payload, "cidr")).bind(i32_field(&payload, "priority", 0))
        .bind(opt_i64_field(&payload, "cloudPlatformId")).bind(opt_str_field(&payload, "cloudPlatformName"))
        .bind(opt_i64_field(&payload, "machineRoomId")).bind(opt_str_field(&payload, "machineRoomName"))
        .execute(&state.pool).await.map_err(|_| AppError::internal("failed to create"))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    State(state): State<InfraState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    sqlx::query("UPDATE infra_network_zone SET name=$2, cidr=$3, priority=$4, cloud_platform_id=$5, cloud_platform_name=$6, machine_room_id=$7, machine_room_name=$8, update_time=now() WHERE id=$1 AND deleted=0")
        .bind(&id).bind(&str_field(&payload, "name")).bind(&str_field(&payload, "cidr")).bind(i32_field(&payload, "priority", 0))
        .bind(opt_i64_field(&payload, "cloudPlatformId")).bind(opt_str_field(&payload, "cloudPlatformName"))
        .bind(opt_i64_field(&payload, "machineRoomId")).bind(opt_str_field(&payload, "machineRoomName"))
        .execute(&state.pool).await.map_err(|_| AppError::internal("failed to update"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn delete_one(
    State(state): State<InfraState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let id = params.get("id").cloned().unwrap_or_default();
    sqlx::query("UPDATE infra_network_zone SET deleted=1, update_time=now() WHERE id=$1")
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed to delete"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn delete_list(
    State(state): State<InfraState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    for id in params
        .get("ids")
        .into_iter()
        .flat_map(|ids| ids.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let _ =
            sqlx::query("UPDATE infra_network_zone SET deleted=1, update_time=now() WHERE id=$1")
                .bind(&id)
                .execute(&state.pool)
                .await;
    }
    Ok(Json(ApiResponse::new(())))
}
