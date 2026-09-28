use crate::{CmdbState, require, valid_code};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Path, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use serde_json::{Value, json};
use sqlx::Row;

pub fn routes() -> ApiRouter<CmdbState> {
    ApiRouter::new()
        .api_route("/cmdb/trigger/list/{model_id}", get(list))
        .api_route("/cmdb/trigger/create", post(create))
        .api_route("/cmdb/trigger/update", put(update))
        .api_route("/cmdb/trigger/delete/{id}", delete(remove))
}

fn row(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"), "modelId": row.get::<i64, _>("model_id"),
        "name": row.get::<String, _>("name"), "conditionCode": row.get::<String, _>("condition_code"),
        "conditionValue": row.get::<Value, _>("condition_value"), "actionCode": row.get::<String, _>("action_code"),
        "actionValue": row.get::<Value, _>("action_value"), "enabled": row.get::<bool, _>("enabled")
    })
}

async fn list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Path(model_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:model:query")?;
    let rows = sqlx::query("SELECT id, model_id, name, condition_code, condition_value, action_code, action_value, enabled FROM cmdb_attribute_trigger WHERE model_id = $1 AND deleted = 0 ORDER BY id")
        .bind(model_id).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed to read triggers"))?;
    Ok(Json(ApiResponse::new(rows.iter().map(row).collect())))
}

fn field(payload: &Value, name: &str) -> Result<String, AppError> {
    payload
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::bad_request(format!("{name} is required")))
}

async fn create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&user, "cmdb:attribute:update")?;
    let model_id = payload
        .get("modelId")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0)
        .ok_or_else(|| AppError::bad_request("modelId is required"))?;
    let name = field(&payload, "name")?;
    let condition_code = field(&payload, "conditionCode")?;
    let action_code = field(&payload, "actionCode")?;
    if !valid_code(&condition_code) || !valid_code(&action_code) {
        return Err(AppError::bad_request("trigger attribute codes are invalid"));
    }
    let condition_value = payload
        .get("conditionValue")
        .cloned()
        .filter(|v| !v.is_null())
        .ok_or_else(|| AppError::bad_request("conditionValue is required"))?;
    let action_value = payload
        .get("actionValue")
        .cloned()
        .filter(|v| !v.is_null())
        .ok_or_else(|| AppError::bad_request("actionValue is required"))?;
    let id = sqlx::query_scalar("INSERT INTO cmdb_attribute_trigger (model_id, name, condition_code, condition_value, action_code, action_value, enabled, creator, updater) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8) RETURNING id")
        .bind(model_id).bind(name).bind(condition_code).bind(condition_value).bind(action_code).bind(action_value).bind(payload.get("enabled").and_then(Value::as_bool).unwrap_or(true)).bind(&user.username)
        .fetch_one(&state.pool).await.map_err(|_| AppError::internal("failed to create trigger"))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:attribute:update")?;
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = field(&payload, "name")?;
    let condition_code = field(&payload, "conditionCode")?;
    let action_code = field(&payload, "actionCode")?;
    let condition_value = payload
        .get("conditionValue")
        .cloned()
        .filter(|v| !v.is_null())
        .ok_or_else(|| AppError::bad_request("conditionValue is required"))?;
    let action_value = payload
        .get("actionValue")
        .cloned()
        .filter(|v| !v.is_null())
        .ok_or_else(|| AppError::bad_request("actionValue is required"))?;
    sqlx::query("UPDATE cmdb_attribute_trigger SET name=$2, condition_code=$3, condition_value=$4, action_code=$5, action_value=$6, enabled=$7, updater=$8, update_time=now() WHERE id=$1 AND deleted=0")
        .bind(id).bind(name).bind(condition_code).bind(condition_value).bind(action_code).bind(action_value).bind(payload.get("enabled").and_then(Value::as_bool).unwrap_or(true)).bind(&user.username).execute(&state.pool).await.map_err(|_| AppError::internal("failed to update trigger"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn remove(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:attribute:update")?;
    sqlx::query("UPDATE cmdb_attribute_trigger SET deleted=1, updater=$2, update_time=now() WHERE id=$1 AND deleted=0").bind(id).bind(&user.username).execute(&state.pool).await.map_err(|_| AppError::internal("failed to delete trigger"))?;
    Ok(Json(ApiResponse::new(())))
}
