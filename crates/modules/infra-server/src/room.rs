use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, tenant_soft_delete,
    tenant_table_create, tenant_table_get, tenant_table_list, tenant_table_page,
    tenant_table_update,
};
use aide::axum::{
    ApiRouter,
    routing::{delete, get, post, put},
};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use serde_json::Value;
use std::collections::HashMap;

const ROOM: TableSpec = TableSpec {
    table: "infra_machine_room",
    seq: "infra_machine_room_seq",
};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/machine-room/page", get(page))
        .api_route("/infra/machine-room/list", get(list))
        .api_route("/infra/machine-room/get", get(get_one))
        .api_route("/infra/machine-room/create", post(create))
        .api_route("/infra/machine-room/update", put(update))
        .api_route("/infra/machine-room/delete", delete(delete_one))
        .api_route("/infra/machine-room/delete-list", delete(delete_list))
}

fn tenant(user: &CurrentUser) -> Result<i64, AppError> {
    Ok(TenantContext::from_user(user)?.id())
}
async fn page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(&s.pool, tenant(&user)?, ROOM, p).await
}
async fn list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&s.pool, tenant(&user)?, ROOM).await
}
async fn get_one(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(&s.pool, tenant(&user)?, ROOM, id_param(&p)?).await
}
async fn create(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(&s.pool, tenant(&user)?, ROOM, p).await
}
async fn update(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(&s.pool, tenant(&user)?, ROOM, p).await
}
async fn delete_one(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(&s.pool, tenant(&user)?, ROOM.table, &[id_param(&p)?]).await
}
async fn delete_list(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(&s.pool, tenant(&user)?, ROOM.table, &ids_param(&p)).await
}
