use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, tenant_soft_delete,
    tenant_table_create, tenant_table_get, tenant_table_get_value, tenant_table_list,
    tenant_table_list_by_i64, tenant_table_page, tenant_table_update,
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
use rustset_infra_api::{
    CreateApplicationEndpointRequest, CreateBusinessApplicationRequest,
    UpdateApplicationEndpointRequest, UpdateBusinessApplicationRequest,
};
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

fn tenant(user: &CurrentUser) -> Result<TenantContext, AppError> {
    TenantContext::from_user(user)
}
async fn app_page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(&s.pool, tenant(&user)?.id(), BUSINESS_APP, p).await
}
async fn app_list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&s.pool, tenant(&user)?.id(), BUSINESS_APP).await
}
async fn app_get(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = tenant(&user)?;
    let id = id_param(&p)?;
    let mut app = tenant_table_get_value(&s.pool, tenant.id(), BUSINESS_APP, id).await?;
    let endpoints = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_application_endpoint t WHERE business_application_id=$1 AND tenant_id=$2 AND deleted=0 ORDER BY id")
        .bind(id).bind(tenant.id()).fetch_all(&s.pool).await.map_err(|_| AppError::internal("failed to list application endpoints"))?
        .into_iter().map(crate::table_value).collect::<Vec<_>>();
    if let Some(object) = app.as_object_mut() {
        object.insert("endpoints".into(), Value::Array(endpoints));
    }
    Ok(Json(ApiResponse::new(app)))
}
async fn app_create(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<CreateBusinessApplicationRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &s.pool,
        tenant(&user)?.id(),
        BUSINESS_APP,
        crate::request_value(p)?,
    )
    .await
}
async fn app_update(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<UpdateBusinessApplicationRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &s.pool,
        tenant(&user)?.id(),
        BUSINESS_APP,
        crate::request_value(p)?,
    )
    .await
}
async fn app_delete(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = tenant(&user)?;
    let id = id_param(&p)?;
    let mut tx = s
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start application delete"))?;
    let result = sqlx::query("UPDATE infra_business_application SET deleted=1, update_time=now() WHERE id=$1 AND tenant_id=$2 AND deleted=0")
        .bind(id).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to delete application"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("record not found"));
    }
    sqlx::query("UPDATE infra_application_endpoint SET deleted=1, update_time=now() WHERE business_application_id=$1 AND tenant_id=$2 AND deleted=0")
        .bind(id).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to delete application endpoints"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit application delete"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn ep_page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(&s.pool, tenant(&user)?.id(), ENDPOINT, p).await
}
async fn ep_list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&s.pool, tenant(&user)?.id(), ENDPOINT).await
}
async fn ep_list_by_app(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list_by_i64(
        &s.pool,
        tenant(&user)?.id(),
        ENDPOINT,
        "business_application_id",
        crate::id_named_param(&p, "businessApplicationId")?,
    )
    .await
}
async fn ep_get(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(&s.pool, tenant(&user)?.id(), ENDPOINT, id_param(&p)?).await
}
async fn ep_create(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<CreateApplicationEndpointRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &s.pool,
        tenant(&user)?.id(),
        ENDPOINT,
        crate::request_value(p)?,
    )
    .await
}
async fn ep_update(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<UpdateApplicationEndpointRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &s.pool,
        tenant(&user)?.id(),
        ENDPOINT,
        crate::request_value(p)?,
    )
    .await
}
async fn ep_delete(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &s.pool,
        tenant(&user)?.id(),
        ENDPOINT.table,
        &[id_param(&p)?],
    )
    .await
}
async fn ep_delete_list(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(&s.pool, tenant(&user)?.id(), ENDPOINT.table, &ids_param(&p)).await
}
