use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, soft_delete, soft_delete_list,
    table_create, table_get, table_list, table_page, table_update,
};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_web::AppError;
use serde_json::Value;

const PROVIDER: TableSpec = TableSpec {
    table: "infra_service_provider",
    seq: "infra_service_provider_seq",
};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/service-provider/page", get(page))
        .api_route("/infra/service-provider/list", get(list))
        .api_route("/infra/service-provider/get", get(get_one))
        .api_route("/infra/service-provider/create", post(create))
        .api_route("/infra/service-provider/update", put(update))
        .api_route("/infra/service-provider/delete", delete(delete_one))
        .api_route("/infra/service-provider/delete-list", delete(delete_list))
}

async fn page(
    State(state): State<InfraState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    table_page(&state.pool, PROVIDER, params).await
}
async fn list(State(state): State<InfraState>) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    table_list(&state.pool, PROVIDER).await
}
async fn get_one(
    State(state): State<InfraState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    table_get(&state.pool, PROVIDER, id_param(&params)?).await
}
async fn create(
    State(state): State<InfraState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    table_create(&state.pool, PROVIDER, payload).await
}
async fn update(
    State(state): State<InfraState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    table_update(&state.pool, PROVIDER, payload).await
}
async fn delete_one(
    State(state): State<InfraState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    soft_delete(&state.pool, PROVIDER.table, id_param(&params)?).await
}
async fn delete_list(
    State(state): State<InfraState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    soft_delete_list(&state.pool, PROVIDER.table, ids_param(&params)).await
}
