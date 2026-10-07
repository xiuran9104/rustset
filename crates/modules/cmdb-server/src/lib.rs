//! CMDB model and attribute management. Instances live in `instance.rs`,
//! relations in `relation.rs`.

mod instance;
mod instance_service;
mod net_zone;
mod relation;
mod trigger;

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_cmdb_api::{
    AttrType, CreateAttributeRequest, CreateModelRequest, UpdateAttributeRequest,
    UpdateModelRequest,
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
        .merge(trigger::routes())
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
        "SELECT m.id, m.name, m.code, m.description, m.icon, m.unique_key, m.sort, m.status,
                m.create_time,
                (SELECT count(*) FROM cmdb_instance i
                 WHERE i.model_id = m.id AND i.deleted = 0) AS instance_count,
                (SELECT count(*) FROM cmdb_attribute a
                 WHERE a.model_id = m.id AND a.deleted = 0) AS attribute_count
         FROM cmdb_model m WHERE m.id = $1 AND m.deleted = 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read model"))?
    .ok_or_else(|| AppError::not_found("model not found"))?;
    Ok(model_row(&row))
}

fn model_row(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "code": row.get::<String, _>("code"),
        "description": row.get::<Option<String>, _>("description"),
        "icon": row.get::<Option<String>, _>("icon"),
        "uniqueKey": row.get::<Option<String>, _>("unique_key"),
        "sort": row.get::<i32, _>("sort"),
        "status": row.get::<i16, _>("status"),
        "createTime": row.get::<chrono::NaiveDateTime, _>("create_time").to_string(),
        "instanceCount": row.get::<i64, _>("instance_count"),
        "attributeCount": row.get::<i64, _>("attribute_count"),
    })
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

fn model_keyword_pattern(keyword: &Option<String>) -> Option<String> {
    keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

async fn model_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:model:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let rows = sqlx::query(
        "SELECT m.id, m.name, m.code, m.description, m.icon, m.unique_key, m.sort, m.status,
                m.create_time,
                (SELECT count(*) FROM cmdb_instance i
                 WHERE i.model_id = m.id AND i.deleted = 0) AS instance_count,
                (SELECT count(*) FROM cmdb_attribute a
                 WHERE a.model_id = m.id AND a.deleted = 0) AS attribute_count
         FROM cmdb_model m WHERE m.deleted = 0 AND m.status = 0
         ORDER BY m.sort, m.id",
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
    let pattern = model_keyword_pattern(&params.keyword);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_model m
         WHERE m.deleted = 0 AND ($1::text IS NULL OR m.name ILIKE $1 OR m.code ILIKE $1)",
    )
    .bind(&pattern)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count models"))?;
    let rows = sqlx::query(
        "SELECT m.id, m.name, m.code, m.description, m.icon, m.unique_key, m.sort, m.status,
                m.create_time,
                (SELECT count(*) FROM cmdb_instance i
                 WHERE i.model_id = m.id AND i.deleted = 0) AS instance_count,
                (SELECT count(*) FROM cmdb_attribute a
                 WHERE a.model_id = m.id AND a.deleted = 0) AS attribute_count
         FROM cmdb_model m
         WHERE m.deleted = 0 AND ($1::text IS NULL OR m.name ILIKE $1 OR m.code ILIKE $1)
         ORDER BY m.sort, m.id LIMIT $2 OFFSET $3",
    )
    .bind(&pattern)
    .bind(size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read models"))?;
    let list: Vec<_> = rows.iter().map(model_row).collect();
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
    Json(request): Json<CreateModelRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid model request"))?;
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
    if let Some(key) = &unique_key
        && !valid_code(key)
    {
        return Err(AppError::bad_request(
            "uniqueKey must be a valid attribute code",
        ));
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
    .bind(unique_key)
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
    Json(request): Json<UpdateModelRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid model request"))?;
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
    if let Some(key) = &unique_key
        && !valid_code(key)
    {
        return Err(AppError::bad_request(
            "uniqueKey must be a valid attribute code",
        ));
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start model update"))?;
    let model = instance_service::lock_model(&mut tx, id).await?;
    if model.unique_key.as_deref() != unique_key {
        if let Some(key) = unique_key {
            let attribute_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM cmdb_attribute
                 WHERE model_id = $1 AND code = $2 AND deleted = 0)",
            )
            .bind(id)
            .bind(key)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to validate unique-key attribute"))?;
            if !attribute_exists {
                return Err(AppError::bad_request(
                    "uniqueKey must reference an active model attribute",
                ));
            }
        }
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
    .bind(unique_key)
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
        "expression": row.get::<Option<String>, _>("expression"),
        "color": row.get::<Option<String>, _>("color"),
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
        "SELECT id, model_id, name, code, attr_type, required, choices, default_value, expression, color, show_in_list, sort
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

#[derive(Clone)]
struct AttributeSchema {
    attr_type: AttrType,
    required: bool,
    choices: Option<Value>,
    default_value: Option<Value>,
    expression: Option<String>,
    color: Option<String>,
}

fn attribute_schema(payload: &Value) -> Result<AttributeSchema, AppError> {
    let attr_type = string_field(payload, "attrType")?;
    let attr_type = AttrType::from_code(&attr_type)
        .ok_or_else(|| AppError::bad_request(format!("unknown attrType {attr_type:?}")))?;
    let required = payload
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expression = payload
        .get("expression")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let color = payload
        .get("color")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(color) = &color
        && !(matches!(color.len(), 7 | 9)
            && color.starts_with('#')
            && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(AppError::bad_request(
            "color must be a #RRGGBB or #RRGGBBAA value",
        ));
    }
    if expression.is_some() && !matches!(attr_type, AttrType::Number | AttrType::Float) {
        return Err(AppError::bad_request(
            "computed attributes must use number or float type",
        ));
    }
    if expression.is_some() && required {
        return Err(AppError::bad_request(
            "computed attributes cannot be marked required",
        ));
    }
    let choices = match attr_type {
        AttrType::Select | AttrType::MultiSelect => {
            let items = payload
                .get("choices")
                .and_then(Value::as_array)
                .filter(|items| !items.is_empty())
                .ok_or_else(|| {
                    AppError::bad_request("select attributes need a non-empty choices array")
                })?;
            let mut values = std::collections::BTreeSet::new();
            let mut normalized = Vec::with_capacity(items.len());
            for item in items {
                let Some(label) = item
                    .get("label")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(AppError::bad_request(
                        "each choice needs non-empty label and value strings",
                    ));
                };
                let Some(value) = item
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(AppError::bad_request(
                        "each choice needs non-empty label and value strings",
                    ));
                };
                if !values.insert(value.to_string()) {
                    return Err(AppError::bad_request(
                        "choice values must be unique within an attribute",
                    ));
                }
                normalized.push(json!({"label": label, "value": value}));
            }
            Some(Value::Array(normalized))
        }
        _ => None,
    };
    let default_value = payload
        .get("defaultValue")
        .cloned()
        .filter(|value| !value.is_null());
    if expression.is_some() && default_value.is_some() {
        return Err(AppError::bad_request(
            "computed attributes cannot have a default value",
        ));
    }
    if let Some(default) = &default_value {
        if instance_service::is_empty_value(default) {
            return Err(AppError::bad_request(
                "defaultValue cannot be empty; omit it when no default is needed",
            ));
        }
        attr_type
            .validate(default, choices.as_ref())
            .map_err(|reason| AppError::bad_request(format!("invalid defaultValue: {reason}")))?;
    }
    Ok(AttributeSchema {
        attr_type,
        required,
        choices,
        default_value,
        expression,
        color,
    })
}

fn reconcile_attribute_map(
    attributes: &mut serde_json::Map<String, Value>,
    code: &str,
    schema: &AttributeSchema,
    backfill_optional: bool,
) -> Result<bool, String> {
    let current = attributes.get(code);
    if current.is_none_or(instance_service::is_empty_value) {
        if let Some(default) = &schema.default_value
            && (schema.required || backfill_optional)
        {
            attributes.insert(code.to_string(), default.clone());
            return Ok(true);
        }
        if schema.required {
            return Err("required value is missing and no default is configured".into());
        }
        return Ok(false);
    }
    schema
        .attr_type
        .validate(current.unwrap_or(&Value::Null), schema.choices.as_ref())
        .map_err(|reason| format!("existing value is invalid: {reason}"))?;
    Ok(false)
}

async fn reconcile_attribute_instances(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
    code: &str,
    schema: &AttributeSchema,
    backfill_optional: bool,
    actor: &str,
) -> Result<(), AppError> {
    let rows = sqlx::query(
        "SELECT id, attributes FROM cmdb_instance
         WHERE model_id = $1 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(model_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| AppError::internal("failed to lock instances for attribute change"))?;
    let mut updates = Vec::new();
    let mut invalid = Vec::new();
    for row in rows {
        let id: i64 = row.get("id");
        let value: Value = row.get("attributes");
        let Some(mut attributes) = value.as_object().cloned() else {
            invalid.push(format!(
                "instance {id}: attributes payload is not an object"
            ));
            continue;
        };
        match reconcile_attribute_map(&mut attributes, code, schema, backfill_optional) {
            Ok(true) => updates.push((id, Value::Object(attributes))),
            Ok(false) => {}
            Err(reason) => invalid.push(format!("instance {id}: {reason}")),
        }
    }
    if !invalid.is_empty() {
        let remaining = invalid.len().saturating_sub(5);
        invalid.truncate(5);
        let suffix = if remaining == 0 {
            String::new()
        } else {
            format!("; and {remaining} more")
        };
        return Err(AppError::bad_request(format!(
            "attribute change is incompatible with existing data: {}{suffix}",
            invalid.join("; ")
        )));
    }
    for (id, attributes) in updates {
        sqlx::query(
            "UPDATE cmdb_instance SET attributes = $2, updater = $3, update_time = now()
             WHERE id = $1 AND deleted = 0",
        )
        .bind(id)
        .bind(attributes)
        .bind(actor)
        .execute(&mut *connection)
        .await
        .map_err(|_| AppError::internal("failed to backfill attribute values"))?;
    }
    Ok(())
}

async fn attribute_create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(request): Json<CreateAttributeRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid attribute request"))?;
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
    let schema = attribute_schema(&payload)?;
    let show_in_list = payload
        .get("showInList")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start attribute create"))?;
    let model = instance_service::lock_model(&mut tx, model_id).await?;
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_attribute WHERE model_id = $1 AND code = $2 AND deleted = 0",
    )
    .bind(model_id)
    .bind(&code)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to check attribute code"))?;
    if exists > 0 {
        return Err(AppError::bad_request(format!(
            "attribute code {code:?} already exists on this model"
        )));
    }
    reconcile_attribute_instances(&mut tx, model_id, &code, &schema, true, &user.username).await?;
    if model.unique_key.as_deref() == Some(code.as_str()) {
        instance_service::validate_unique_key_change(&mut tx, model_id, Some(&code)).await?;
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_attribute
             (model_id, name, code, attr_type, required, choices, default_value, expression, color, show_in_list, sort, creator, updater)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12) RETURNING id",
    )
    .bind(model_id)
    .bind(&name)
    .bind(&code)
    .bind(schema.attr_type.code())
    .bind(schema.required)
    .bind(&schema.choices)
    .bind(&schema.default_value)
    .bind(&schema.expression)
    .bind(&schema.color)
    .bind(show_in_list)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to create attribute"))?;
    let tenant = TenantContext::from_user(&user)?;
    instance_service::recompute_model_instances(&mut tx, &tenant, model_id, &user.username).await?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit attribute create"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn attribute_update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(request): Json<UpdateAttributeRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let payload = serde_json::to_value(request)
        .map_err(|_| AppError::bad_request("invalid attribute request"))?;
    require(&user, "cmdb:attribute:update")?;
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = string_field(&payload, "name")?;
    let schema = attribute_schema(&payload)?;
    let show_in_list = payload
        .get("showInList")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let model_id: i64 =
        sqlx::query_scalar("SELECT model_id FROM cmdb_attribute WHERE id = $1 AND deleted = 0")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| AppError::internal("failed to read attribute"))?
            .ok_or_else(|| AppError::not_found("attribute not found"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start attribute update"))?;
    let model = instance_service::lock_model(&mut tx, model_id).await?;
    let row = sqlx::query(
        "SELECT model_id, code FROM cmdb_attribute
         WHERE id = $1 AND model_id = $2 AND deleted = 0 FOR UPDATE",
    )
    .bind(id)
    .bind(model_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock attribute"))?
    .ok_or_else(|| AppError::not_found("attribute not found"))?;
    let code: String = row.get("code");
    reconcile_attribute_instances(&mut tx, model_id, &code, &schema, false, &user.username).await?;
    if model.unique_key.as_deref() == Some(code.as_str()) {
        instance_service::validate_unique_key_change(&mut tx, model_id, Some(&code)).await?;
    }
    let result = sqlx::query(
        "UPDATE cmdb_attribute SET name = $2, attr_type = $3, required = $4, choices = $5,
                default_value = $6, expression = $7, color = $8, show_in_list = $9, sort = $10, updater = $11, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(id)
    .bind(&name)
    .bind(schema.attr_type.code())
    .bind(schema.required)
    .bind(&schema.choices)
    .bind(&schema.default_value)
    .bind(&schema.expression)
    .bind(&schema.color)
    .bind(show_in_list)
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to update attribute"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("attribute not found"));
    }
    let tenant = TenantContext::from_user(&user)?;
    instance_service::recompute_model_instances(&mut tx, &tenant, model_id, &user.username).await?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit attribute update"))?;
    Ok(Json(ApiResponse::new(())))
}

async fn attribute_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:attribute:delete")?;
    let model_id: i64 =
        sqlx::query_scalar("SELECT model_id FROM cmdb_attribute WHERE id = $1 AND deleted = 0")
            .bind(params.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| AppError::internal("failed to read attribute"))?
            .ok_or_else(|| AppError::not_found("attribute not found"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start attribute delete"))?;
    let model = instance_service::lock_model(&mut tx, model_id).await?;
    let code: String = sqlx::query_scalar(
        "SELECT code FROM cmdb_attribute
         WHERE id = $1 AND model_id = $2 AND deleted = 0 FOR UPDATE",
    )
    .bind(params.id)
    .bind(model_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock attribute"))?
    .ok_or_else(|| AppError::not_found("attribute not found"))?;
    if model.unique_key.as_deref() == Some(code.as_str()) {
        return Err(AppError::bad_request(
            "the model unique key uses this attribute; change the unique key first",
        ));
    }
    let result = sqlx::query(
        "UPDATE cmdb_attribute SET deleted = 1, updater = $2, update_time = now()
         WHERE id = $1 AND deleted = 0",
    )
    .bind(params.id)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to delete attribute"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("attribute not found"));
    }
    sqlx::query(
        "UPDATE cmdb_instance
         SET attributes = attributes - $2, updater = $3, update_time = now()
         WHERE model_id = $1 AND deleted = 0 AND attributes ? $2",
    )
    .bind(model_id)
    .bind(&code)
    .bind(&user.username)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to remove deleted attribute values"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit attribute delete"))?;
    Ok(Json(ApiResponse::new(())))
}

#[cfg(test)]
mod tenant_tests;
