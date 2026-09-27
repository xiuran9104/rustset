//! CMDB model and attribute management. Instances live in `instance.rs`,
//! relations in `relation.rs`.

mod instance;
mod instance_service;
mod net_zone;
mod relation;

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_database::PgPool;
use rustset_framework_security::{CurrentUser, Permission};
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

pub use instance::routes as instance_routes;
pub use net_zone::routes as net_zone_routes;
pub use relation::routes as relation_routes;

#[derive(Clone)]
pub struct CmdbState {
    pub pool: PgPool,
}

/// Same semantics as system_server's require: super_admin passes, everyone
/// else needs the exact permission code.
pub(crate) fn require(user: &CurrentUser, code: &str) -> Result<(), AppError> {
    if user.role_codes.iter().any(|role| role == "super_admin") {
        return Ok(());
    }
    let permission =
        Permission::new(code).map_err(|_| AppError::internal("invalid cmdb policy"))?;
    if user.can(&permission) {
        Ok(())
    } else {
        Err(AppError::forbidden("permission denied"))
    }
}

pub(crate) fn valid_code(code: &str) -> bool {
    let mut characters = code.chars();
    matches!(characters.next(), Some('a'..='z'))
        && code.len() <= 64
        && code
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn routes(state: CmdbState) -> ApiRouter {
    ApiRouter::new()
        .api_route("/cmdb/model/list", get(model_list))
        .api_route("/cmdb/model/page", get(model_page))
        .api_route("/cmdb/model/get", get(model_get))
        .api_route("/cmdb/model/create", post(model_create))
        .api_route("/cmdb/model/update", put(model_update))
        .api_route("/cmdb/model/delete", delete(model_delete))
        .api_route("/cmdb/attribute/list-by-model", get(attribute_list))
        .api_route("/cmdb/attribute/create", post(attribute_create))
        .api_route("/cmdb/attribute/update", put(attribute_update))
        .api_route("/cmdb/attribute/delete", delete(attribute_delete))
        .merge(instance_routes())
        .merge(relation_routes())
        .merge(net_zone_routes())
        .with_state(state)
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ModelPageParams {
    #[serde(default)]
    page_no: Option<i64>,
    #[serde(default)]
    page_size: Option<i64>,
    #[serde(default)]
    keyword: Option<String>,
}

fn page_bounds(page_no: Option<i64>, page_size: Option<i64>) -> (i64, i64) {
    let size = page_size.unwrap_or(20).clamp(1, 200);
    let no = page_no.unwrap_or(1).max(1);
    (size, (no - 1) * size)
}

async fn load_model_row(pool: &PgPool, id: i64) -> Result<Value, AppError> {
    let row = sqlx::query(
        "SELECT id, name, code, description, icon, unique_key, sort, status, create_time, update_time
         FROM cmdb_model WHERE id = $1 AND deleted = 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read model"))?
    .ok_or_else(|| AppError::not_found("model not found"))?;
    let unique_key: Option<String> = row.get("unique_key");
    let description: Option<String> = row.get("description");
    let icon: Option<String> = row.get("icon");
    Ok(json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "code": row.get::<String, _>("code"),
        "description": description,
        "icon": icon,
        "uniqueKey": unique_key,
        "sort": row.get::<i32, _>("sort"),
        "status": row.get::<i16, _>("status"),
        "createTime": row.get::<chrono::NaiveDateTime, _>("create_time").to_string(),
    }))
}

async fn model_count(pool: &PgPool, tenant: &TenantContext, model_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM cmdb_instance WHERE model_id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(model_id)
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

async fn attribute_count(pool: &PgPool, model_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM cmdb_attribute WHERE model_id = $1 AND deleted = 0",
    )
    .bind(model_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

fn model_keyword_clause(keyword: &Option<String>) -> (String, String) {
    match keyword.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(keyword) => (
            " AND (name ILIKE $2 OR code ILIKE $2)".into(),
            format!("%{keyword}%"),
        ),
        None => (String::new(), String::new()),
    }
}

async fn model_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:model:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let rows = sqlx::query(
        "SELECT id, name, code, description, icon, unique_key, sort, status
         FROM cmdb_model WHERE deleted = 0 AND status = 0
         ORDER BY sort, id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read models"))?;
    let mut models = Vec::new();
    for row in rows {
        models.push(json!({
            "id": row.get::<i64, _>("id"),
            "name": row.get::<String, _>("name"),
            "code": row.get::<String, _>("code"),
            "description": row.get::<Option<String>, _>("description"),
            "icon": row.get::<Option<String>, _>("icon"),
            "uniqueKey": row.get::<Option<String>, _>("unique_key"),
            "sort": row.get::<i32, _>("sort"),
            "status": row.get::<i16, _>("status"),
            "instanceCount": model_count(&state.pool, &tenant, row.get::<i64, _>("id")).await,
            "attributeCount": attribute_count(&state.pool, row.get::<i64, _>("id")).await,
        }));
    }
    Ok(Json(ApiResponse::new(models)))
}

async fn model_page(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<ModelPageParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:model:query")?;
    let (size, offset) = page_bounds(params.page_no, params.page_size);
    let (clause, pattern) = model_keyword_clause(&params.keyword);
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM cmdb_model WHERE deleted = 0{clause}"
    ))
    .bind(&pattern)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count models"))?;
    let rows = sqlx::query(&format!(
        "SELECT id FROM cmdb_model WHERE deleted = 0{clause} ORDER BY sort, id LIMIT {size} OFFSET {offset}"
    ))
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read models"))?;
    let mut list = Vec::new();
    for row in rows {
        list.push(load_model_row(&state.pool, row.get::<i64, _>("id")).await?);
    }
    Ok(Json(ApiResponse::new(
        json!({ "list": list, "total": total }),
    )))
}

async fn model_get(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:model:query")?;
    Ok(Json(ApiResponse::new(
        load_model_row(&state.pool, params.id).await?,
    )))
}

fn string_field(payload: &Value, key: &str) -> Result<String, AppError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request(format!("{key} is required")))
        .map(str::to_string)
}

async fn model_create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:model:create")?;
    let name = string_field(&payload, "name")?;
    let code = string_field(&payload, "code")?;
    if !valid_code(&code) {
        return Err(AppError::bad_request(
            "code must be lowercase letters, digits or underscores, starting with a letter",
        ));
    }
    let unique_key = payload
        .get("uniqueKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = &unique_key {
        if !valid_code(key) {
            return Err(AppError::bad_request(
                "uniqueKey must be a valid attribute code",
            ));
        }
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start model create"))?;
    // Shared with Infra's provisioning writer: model codes have no unique index yet.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("cmdb:model:{code}"))
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to lock model code"))?;
    let exists: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_model WHERE code = $1 AND deleted = 0")
            .bind(&code)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to check model code"))?;
    if exists > 0 {
        return Err(AppError::bad_request(format!(
            "model code {code:?} already exists"
        )));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_model (name, code, description, icon, unique_key, sort, creator, updater)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7) RETURNING id",
    )
    .bind(&name)
    .bind(&code)
    .bind(payload.get("description").and_then(Value::as_str))
    .bind(payload.get("icon").and_then(Value::as_str))
    .bind(&unique_key)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to create model"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit model create"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn model_update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:model:update")?;
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = string_field(&payload, "name")?;
    let unique_key = payload
        .get("uniqueKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = &unique_key {
        if !valid_code(key) {
            return Err(AppError::bad_request(
                "uniqueKey must be a valid attribute code",
            ));
        }
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start model update"))?;
    let model = instance_service::lock_model(&mut tx, id).await?;
    if model.unique_key.as_deref() != unique_key {
        instance_service::validate_unique_key_change(&mut tx, id, unique_key).await?;
    }
    let result = sqlx::query(
        "UPDATE cmdb_model SET name = $2, description = $3, icon = $4, unique_key = $5,
                sort = $6, updater = $7, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(id)
    .bind(&name)
    .bind(payload.get("description").and_then(Value::as_str))
    .bind(payload.get("icon").and_then(Value::as_str))
    .bind(&unique_key)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|error| instance_service::mutation_error(error, "failed to update model"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("model not found"));
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit model update"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn model_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:model:delete")?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start model delete"))?;
    instance_service::lock_model(&mut tx, params.id).await?;
    let instances: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_instance WHERE model_id = $1 AND deleted = 0",
    )
    .bind(params.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to count instances"))?;
    if instances > 0 {
        return Err(AppError::bad_request(
            "model still has instances; delete them first",
        ));
    }
    let result = sqlx::query(
        "UPDATE cmdb_model SET deleted = 1, updater = $2, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(params.id)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to delete model"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("model not found"));
    }
    sqlx::query(
        "UPDATE cmdb_attribute SET deleted = 1, updater = $2 WHERE model_id = $1 AND deleted = 0",
    )
    .bind(params.id)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to delete model attributes"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit model delete"))?;
    Ok(Json(ApiResponse::new(())))
}

fn attribute_row(row: &sqlx::postgres::PgRow) -> Value {
    let choices: Option<Value> = row.get("choices");
    let default_value: Option<Value> = row.get("default_value");
    json!({
        "id": row.get::<i64, _>("id"),
        "modelId": row.get::<i64, _>("model_id"),
        "name": row.get::<String, _>("name"),
        "code": row.get::<String, _>("code"),
        "attrType": row.get::<String, _>("attr_type"),
        "required": row.get::<bool, _>("required"),
        "choices": choices,
        "defaultValue": default_value,
        "showInList": row.get::<bool, _>("show_in_list"),
        "sort": row.get::<i32, _>("sort"),
    })
}

async fn attribute_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:model:query")?;
    let model_id: i64 = params
        .get("modelId")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| AppError::bad_request("modelId is required"))?;
    let rows = sqlx::query(
        "SELECT id, model_id, name, code, attr_type, required, choices, default_value, show_in_list, sort
         FROM cmdb_attribute WHERE model_id = $1 AND deleted = 0
         ORDER BY sort, id",
    )
    .bind(model_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read attributes"))?;
    Ok(Json(ApiResponse::new(
        rows.iter().map(attribute_row).collect(),
    )))
}

async fn attribute_create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:attribute:create")?;
    let model_id = payload
        .get("modelId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("modelId is required"))?;
    let name = string_field(&payload, "name")?;
    let code = string_field(&payload, "code")?;
    if !valid_code(&code) {
        return Err(AppError::bad_request(
            "code must be lowercase letters, digits or underscores, starting with a letter",
        ));
    }
    let attr_type = string_field(&payload, "attrType")?;
    let attr_type = rustset_cmdb_api::AttrType::from_code(&attr_type)
        .ok_or_else(|| AppError::bad_request(format!("unknown attrType {attr_type:?}")))?;
    let required = payload
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let show_in_list = payload
        .get("showInList")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let choices = match attr_type {
        rustset_cmdb_api::AttrType::Select | rustset_cmdb_api::AttrType::MultiSelect => {
            let choices = payload
                .get("choices")
                .and_then(Value::as_array)
                .filter(|items| !items.is_empty())
                .ok_or_else(|| {
                    AppError::bad_request("select attributes need a non-empty choices array")
                })?;
            Value::Array(choices.clone())
        }
        _ => Value::Null,
    };
    let default_value = payload
        .get("defaultValue")
        .cloned()
        .filter(|v| !v.is_null());
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_attribute WHERE model_id = $1 AND code = $2 AND deleted = 0",
    )
    .bind(model_id)
    .bind(&code)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to check attribute code"))?;
    if exists > 0 {
        return Err(AppError::bad_request(format!(
            "attribute code {code:?} already exists on this model"
        )));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_attribute
             (model_id, name, code, attr_type, required, choices, default_value, show_in_list, sort, creator, updater)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10) RETURNING id",
    )
    .bind(model_id)
    .bind(&name)
    .bind(&code)
    .bind(attr_type.code())
    .bind(required)
    .bind(choices)
    .bind(&default_value)
    .bind(show_in_list)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to create attribute"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn attribute_update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:attribute:update")?;
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = string_field(&payload, "name")?;
    let attr_type = string_field(&payload, "attrType")?;
    let attr_type = rustset_cmdb_api::AttrType::from_code(&attr_type)
        .ok_or_else(|| AppError::bad_request(format!("unknown attrType {attr_type:?}")))?;
    let required = payload
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let show_in_list = payload
        .get("showInList")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let choices = match attr_type {
        rustset_cmdb_api::AttrType::Select | rustset_cmdb_api::AttrType::MultiSelect => {
            let choices = payload
                .get("choices")
                .and_then(Value::as_array)
                .filter(|items| !items.is_empty())
                .ok_or_else(|| {
                    AppError::bad_request("select attributes need a non-empty choices array")
                })?;
            Value::Array(choices.clone())
        }
        _ => Value::Null,
    };
    let default_value = payload
        .get("defaultValue")
        .cloned()
        .filter(|v| !v.is_null());
    let result = sqlx::query(
        "UPDATE cmdb_attribute SET name = $2, attr_type = $3, required = $4, choices = $5,
                default_value = $6, show_in_list = $7, sort = $8, updater = $9, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(id)
    .bind(&name)
    .bind(attr_type.code())
    .bind(required)
    .bind(choices)
    .bind(&default_value)
    .bind(show_in_list)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to update attribute"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("attribute not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn attribute_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:attribute:delete")?;
    let result = sqlx::query(
        "UPDATE cmdb_attribute SET deleted = 1, updater = $2, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(params.id)
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to delete attribute"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("attribute not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

#[cfg(test)]
mod tenant_tests;
