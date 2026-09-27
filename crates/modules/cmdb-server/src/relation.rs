//! CMDB relations between configuration items: bind, unbind, and lookup in
//! both directions.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::{CmdbState, require};

pub fn routes() -> ApiRouter<CmdbState> {
    ApiRouter::new()
        .api_route("/cmdb/relation/list-by-instance", get(relation_list))
        .api_route("/cmdb/relation/bind", post(relation_bind))
        .api_route("/cmdb/relation/unbind", delete(relation_unbind))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InstanceIdParams {
    #[serde(rename = "instanceId")]
    instance_id: i64,
}

async fn instance_exists(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    id: i64,
) -> Result<bool, AppError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM cmdb_instance WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(id)
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .map_err(|_| AppError::internal("failed to read instance"))
}

async fn relation_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    if !instance_exists(&state.pool, &tenant, params.instance_id).await? {
        return Err(AppError::not_found("instance not found"));
    }
    let rows = sqlx::query(
        "SELECT r.id, r.source_id, r.target_id, r.relation
         FROM cmdb_relation r
         JOIN cmdb_instance source ON source.id = r.source_id AND source.tenant_id = r.tenant_id AND source.deleted = 0
         JOIN cmdb_instance target ON target.id = r.target_id AND target.tenant_id = r.tenant_id AND target.deleted = 0
         WHERE r.tenant_id = $2 AND r.deleted = 0 AND (r.source_id = $1 OR r.target_id = $1)
         ORDER BY r.id DESC",
    )
    .bind(params.instance_id)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read relations"))?;
    Ok(Json(ApiResponse::new(
        rows.iter()
            .map(|row| {
                json!({
                    "id": row.get::<i64, _>("id"),
                    "sourceId": row.get::<i64, _>("source_id"),
                    "targetId": row.get::<i64, _>("target_id"),
                    "relation": row.get::<String, _>("relation"),
                })
            })
            .collect::<Vec<_>>(),
    )))
}

async fn relation_bind(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let tenant = TenantContext::from_user(&user)?;
    let source_id = payload
        .get("sourceId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("sourceId is required"))?;
    let target_id = payload
        .get("targetId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("targetId is required"))?;
    if source_id == target_id {
        return Err(AppError::bad_request("cannot relate an instance to itself"));
    }
    let relation = payload
        .get("relation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("relates_to")
        .to_string();
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start relation bind"))?;
    // Endpoint row locks also serialize overlapping binds and instance deletes.
    let endpoints = sqlx::query("SELECT id FROM cmdb_instance WHERE id = ANY($1) AND tenant_id = $2 AND deleted = 0 ORDER BY id FOR UPDATE")
        .bind(vec![source_id, target_id]).bind(tenant.id()).fetch_all(&mut *tx).await
        .map_err(|_| AppError::internal("failed to lock relation endpoints"))?;
    if endpoints.len() != 2 {
        return Err(AppError::not_found("instance not found"));
    }
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_relation
         WHERE source_id = $1 AND target_id = $2 AND relation = $3 AND tenant_id = $4 AND deleted = 0",
    )
    .bind(source_id)
    .bind(target_id)
    .bind(&relation)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to check relation"))?;
    if duplicate > 0 {
        return Err(AppError::bad_request("relation already exists"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_relation (source_id, target_id, relation, creator, updater, tenant_id)
         VALUES ($1, $2, $3, $4, $4, $5) RETURNING id",
    )
    .bind(source_id)
    .bind(target_id)
    .bind(&relation)
    .bind(&user.username)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to create relation"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit relation bind"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn relation_unbind(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let tenant = TenantContext::from_user(&user)?;
    let result = sqlx::query(
        "UPDATE cmdb_relation SET deleted = 1, updater = $2
         WHERE id = $1 AND tenant_id = $3 AND deleted = 0",
    )
    .bind(params.instance_id)
    .bind(&user.username)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to remove relation"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("relation not found"));
    }
    Ok(Json(ApiResponse::new(())))
}
