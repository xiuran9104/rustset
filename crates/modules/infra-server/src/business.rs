use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, soft_delete, table_create, table_get,
    table_list, table_page, table_update,
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
use std::collections::HashMap;

const BUSINESS_APP: TableSpec = TableSpec {
    table: "infra_business_application",
    seq: "infra_business_application_seq",
};
const ENDPOINT: TableSpec = TableSpec {
    table: "infra_application_endpoint",
    seq: "infra_application_endpoint_seq",
};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/business-application/page", get(app_page))
        .api_route("/infra/business-application/list", get(app_list))
        .api_route("/infra/business-application/get", get(app_get))
        .api_route("/infra/business-application/create", post(app_create))
        .api_route("/infra/business-application/update", put(app_update))
        .api_route("/infra/business-application/delete", delete(app_delete))
        .api_route("/infra/application-endpoint/page", get(ep_page))
        .api_route("/infra/application-endpoint/list", get(ep_list))
        .api_route(
            "/infra/application-endpoint/list-by-app",
            get(ep_list_by_app),
        )
        .api_route("/infra/application-endpoint/get", get(ep_get))
        .api_route("/infra/application-endpoint/create", post(ep_create))
        .api_route("/infra/application-endpoint/update", put(ep_update))
        .api_route("/infra/application-endpoint/delete", delete(ep_delete))
        .api_route(
            "/infra/application-endpoint/delete-list",
            delete(ep_delete_list),
        )
}

async fn app_page(
    State(state): State<InfraState>,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    table_page(&state.pool, BUSINESS_APP, p).await
}
async fn app_list(
    State(state): State<InfraState>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    table_list(&state.pool, BUSINESS_APP).await
}
async fn app_get(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let id = id_param(&p)?;
    let mut app = crate::table_get_value(&state.pool, BUSINESS_APP, id).await?;
    let endpoints = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_application_endpoint t WHERE business_application_id=$1 AND deleted=0 ORDER BY id").bind(id).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed"))?.into_iter().map(crate::table_value).collect::<Vec<_>>();
    if let Some(obj) = app.as_object_mut() {
        obj.insert("endpoints".into(), Value::Array(endpoints));
    }
    Ok(Json(ApiResponse::new(app)))
}
async fn app_create(
    State(state): State<InfraState>,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    table_create(&state.pool, BUSINESS_APP, p).await
}
async fn app_update(
    State(state): State<InfraState>,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    table_update(&state.pool, BUSINESS_APP, p).await
}
async fn app_delete(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let id = id_param(&p)?;
    let _ = sqlx::query("UPDATE infra_application_endpoint SET deleted=1, update_time=now() WHERE business_application_id=$1 AND deleted=0").bind(id).execute(&state.pool).await;
    soft_delete(&state.pool, BUSINESS_APP.table, id).await
}

async fn ep_page(
    State(state): State<InfraState>,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    table_page(&state.pool, ENDPOINT, p).await
}
async fn ep_list(
    State(state): State<InfraState>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    table_list(&state.pool, ENDPOINT).await
}
async fn ep_list_by_app(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    crate::table_list_by_i64(
        &state.pool,
        ENDPOINT,
        "business_application_id",
        crate::id_named_param(&p, "businessApplicationId")?,
    )
    .await
}
async fn ep_get(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    table_get(&state.pool, ENDPOINT, id_param(&p)?).await
}
async fn ep_create(
    State(state): State<InfraState>,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    table_create(&state.pool, ENDPOINT, p).await
}
async fn ep_update(
    State(state): State<InfraState>,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    table_update(&state.pool, ENDPOINT, p).await
}
async fn ep_delete(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    soft_delete(&state.pool, ENDPOINT.table, id_param(&p)?).await
}
async fn ep_delete_list(
    State(state): State<InfraState>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    crate::soft_delete_list(&state.pool, ENDPOINT.table, ids_param(&p)).await
}
