use crate::{InfraState, QueryParams};
use aide::axum::ApiRouter;
use aide::axum::routing::{get, put};
use axum::{
    Json,
    extract::{Query, State},
};
use chrono::Utc;
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use serde_json::Value;
use std::collections::HashMap;

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/risk/page", get(page))
        .api_route("/infra/risk/list", get(list))
        .api_route("/infra/risk/get", get(get_one))
        .api_route("/infra/risk/{id}/status/{status}", put(update_status))
        .api_route("/infra/risk/{id}/resolve", put(resolve))
}

async fn page(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let pn = p.page_no.unwrap_or(1).max(1);
    let ps = p.page_size.unwrap_or(10).clamp(1, 200);
    let off = (pn - 1) * ps;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM infra_risk WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?;
    let list = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_risk t WHERE tenant_id=$3 AND deleted=0 ORDER BY create_time DESC LIMIT $1 OFFSET $2").bind(ps).bind(off).bind(tenant.id()).fetch_all(&s.pool).await.map_err(|_| AppError::internal("failed"))?.into_iter().map(crate::table_value).collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

async fn list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_risk t WHERE tenant_id=$1 AND deleted=0 ORDER BY create_time DESC",
    )
    .bind(tenant.id())
    .fetch_all(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?
    .into_iter()
    .map(crate::table_value)
    .collect();
    Ok(Json(ApiResponse::new(list)))
}

async fn get_one(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = p.get("id").cloned().unwrap_or_default();
    let v = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM infra_risk t WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(&id)
    .bind(tenant.id())
    .fetch_optional(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?
    .ok_or_else(|| AppError::not_found("not found"))?;
    Ok(Json(ApiResponse::new(crate::table_value(v))))
}

async fn update_status(
    State(s): State<InfraState>,
    user: CurrentUser,
    axum::extract::Path((id, status)): axum::extract::Path<(String, String)>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    if ![
        "verified",
        "resolved",
        "ignored",
        "open",
        "false_positive",
        "pending_review",
    ]
    .contains(&status.as_str())
    {
        return Err(AppError::bad_request("Invalid status"));
    }
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query("UPDATE infra_risk SET status=$2, update_time=$3::timestamp WHERE id=$1 AND tenant_id=$4 AND deleted=0")
        .bind(&id)
        .bind(&status)
        .bind(&now)
        .bind(tenant.id())
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new("Updated".to_string())))
}

async fn resolve(
    State(s): State<InfraState>,
    user: CurrentUser,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "UPDATE infra_risk SET status='resolved', update_time=$2::timestamp WHERE id=$1 AND tenant_id=$3 AND deleted=0",
    )
    .bind(&id)
    .bind(&now)
        .bind(tenant.id())
    .execute(&s.pool)
    .await
    .map_err(|_| AppError::internal("failed"))?;
    Ok(Json(ApiResponse::new("Resolved".to_string())))
}
