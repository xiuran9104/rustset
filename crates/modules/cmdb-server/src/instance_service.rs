//! Transaction boundary for instance mutations. All API writers (including
//! imports) lock the model first, then the instance, and use one connection.
//! Model key changes take the same lock. Migration 0025 also maintains a
//! database unique-value registry for writers outside this service.

use rustset_cmdb_api::AttrType;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use serde_json::{Map, Value};
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;

pub(crate) fn mutation_error(error: sqlx::Error, fallback: &str) -> AppError {
    if error.as_database_error().is_some_and(|error| {
        error.is_unique_violation() && error.constraint() == Some("cmdb_instance_unique_value_key")
    }) {
        return AppError::bad_request("unique key already has an instance with this value");
    }
    AppError::internal(fallback)
}

pub(crate) struct AttributeDef {
    pub(crate) code: String,
    pub(crate) attr_type: AttrType,
    required: bool,
    choices: Option<Value>,
    default_value: Option<Value>,
}

pub(crate) struct ModelHeader {
    pub(crate) id: i64,
    pub(crate) unique_key: Option<String>,
}

pub(crate) async fn load_model(
    executor: impl sqlx::PgExecutor<'_>,
    model_id: i64,
) -> Result<ModelHeader, AppError> {
    let row = sqlx::query("SELECT id, unique_key FROM cmdb_model WHERE id = $1 AND deleted = 0")
        .bind(model_id)
        .fetch_optional(executor)
        .await
        .map_err(|_| AppError::internal("failed to read model"))?
        .ok_or_else(|| AppError::not_found("model not found"))?;
    Ok(ModelHeader {
        id: row.get("id"),
        unique_key: row.get("unique_key"),
    })
}

pub(crate) async fn load_attributes(
    executor: impl sqlx::PgExecutor<'_>,
    model_id: i64,
) -> Result<Vec<AttributeDef>, AppError> {
    let rows = sqlx::query(
        "SELECT code, attr_type, required, choices, default_value
         FROM cmdb_attribute WHERE model_id = $1 AND deleted = 0",
    )
    .bind(model_id)
    .fetch_all(executor)
    .await
    .map_err(|_| AppError::internal("failed to read model attributes"))?;
    let mut defs = Vec::new();
    for row in rows {
        let attr_type: String = row.get("attr_type");
        let Some(attr_type) = AttrType::from_code(&attr_type) else {
            return Err(AppError::internal(format!(
                "model attribute {attr_type:?} has an unknown type"
            )));
        };
        defs.push(AttributeDef {
            code: row.get("code"),
            attr_type,
            required: row.get("required"),
            choices: row.get("choices"),
            default_value: row.get("default_value"),
        });
    }
    Ok(defs)
}

fn is_empty_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

/// Validate a payload against the model definition. On create the full
/// required-check applies and defaults are filled; on update only the
/// provided keys are checked (merge semantics). Unknown keys are rejected
/// so instances cannot drift away from their model.
fn validate_payload(
    attributes: &[AttributeDef],
    payload: &Value,
    is_create: bool,
) -> Result<Map<String, Value>, AppError> {
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::bad_request("payload must be a JSON object"))?;
    if attributes.is_empty() {
        return Err(AppError::bad_request(
            "model has no attributes; define them first",
        ));
    }
    let by_code: BTreeMap<&str, &AttributeDef> = attributes
        .iter()
        .map(|def| (def.code.as_str(), def))
        .collect();

    let mut unknown = Vec::new();
    for key in object.keys() {
        if key != "id" && !by_code.contains_key(key.as_str()) {
            unknown.push(key.clone());
        }
    }
    if !unknown.is_empty() {
        return Err(AppError::bad_request(format!(
            "unknown attributes for this model: {}",
            unknown.join(", ")
        )));
    }

    let mut missing = Vec::new();
    let mut invalid = Vec::new();
    let mut data = Map::new();
    for def in attributes {
        match object.get(&def.code) {
            None | Some(Value::Null) if is_create => {
                if let Some(default) = &def.default_value {
                    data.insert(def.code.clone(), default.clone());
                } else if def.required {
                    missing.push(def.code.clone());
                }
            }
            provided => {
                if let Some(value) = provided {
                    if is_empty_value(value) {
                        if def.required && is_create {
                            missing.push(def.code.clone());
                        }
                    } else if let Err(reason) = def.attr_type.validate(value, def.choices.as_ref())
                    {
                        invalid.push(format!("{}: {reason}", def.code));
                    } else {
                        data.insert(def.code.clone(), value.clone());
                    }
                }
            }
        }
    }
    if !missing.is_empty() {
        return Err(AppError::bad_request(format!(
            "missing required attributes: {}",
            missing.join(", ")
        )));
    }
    if !invalid.is_empty() {
        return Err(AppError::bad_request(format!(
            "invalid attribute values: {}",
            invalid.join("; ")
        )));
    }
    if data.is_empty() {
        return Err(AppError::bad_request("no attribute values provided"));
    }
    Ok(data)
}

async fn enforce_unique_key(
    connection: &mut sqlx::PgConnection,
    model: &ModelHeader,
    tenant: &TenantContext,
    data: &Map<String, Value>,
    instance_id: Option<i64>,
) -> Result<(), AppError> {
    let Some(key) = model.unique_key.as_deref().filter(|key| !key.is_empty()) else {
        return Ok(());
    };
    let Some(value) = data.get(key).filter(|v| !is_empty_value(v)) else {
        return Ok(());
    };
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_instance
         WHERE model_id = $1 AND deleted = 0 AND id <> $2 AND attributes->$3 = $4::jsonb AND tenant_id = $5",
    )
    .bind(model.id)
    .bind(instance_id.unwrap_or(0))
    .bind(key)
    .bind(value)
    .bind(tenant.id())
    .fetch_one(connection)
    .await
    .map_err(|_| AppError::internal("failed to check unique key"))?;
    if duplicate > 0 {
        return Err(AppError::bad_request(format!(
            "unique key {key:?} already has an instance with value {value:?}"
        )));
    }
    Ok(())
}

/// A real row lock works across gateway processes and is released on rollback.
/// Always acquire this before instance locks to keep lock ordering consistent.
pub(crate) async fn lock_model(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
) -> Result<ModelHeader, AppError> {
    let row = sqlx::query(
        "SELECT id, unique_key FROM cmdb_model WHERE id = $1 AND deleted = 0 FOR UPDATE",
    )
    .bind(model_id)
    .fetch_optional(connection)
    .await
    .map_err(|_| AppError::internal("failed to lock model"))?
    .ok_or_else(|| AppError::not_found("model not found"))?;
    Ok(ModelHeader {
        id: row.get("id"),
        unique_key: row.get("unique_key"),
    })
}

pub(crate) async fn create(
    pool: &PgPool,
    tenant: &TenantContext,
    model_id: i64,
    attributes: Map<String, Value>,
    actor: &str,
) -> Result<i64, AppError> {
    if model_id <= 0 {
        return Err(AppError::bad_request("modelId is required"));
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start create"))?;
    let model = lock_model(&mut tx, model_id).await?;
    let definitions = load_attributes(&mut *tx, model_id).await?;
    let data = validate_payload(&definitions, &Value::Object(attributes), true)?;
    enforce_unique_key(&mut tx, &model, tenant, &data, None).await?;
    let id = sqlx::query_scalar(
        "INSERT INTO cmdb_instance (model_id, attributes, creator, updater, tenant_id) VALUES ($1, $2, $3, $3, $4) RETURNING id"
    ).bind(model_id).bind(Value::Object(data)).bind(actor).bind(tenant.id())
        .fetch_one(&mut *tx).await.map_err(|error| mutation_error(error, "failed to create instance"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit create"))?;
    Ok(id)
}

pub(crate) async fn update(
    pool: &PgPool,
    tenant: &TenantContext,
    id: i64,
    attributes: Map<String, Value>,
    actor: &str,
) -> Result<(), AppError> {
    if id <= 0 {
        return Err(AppError::bad_request("id is required"));
    }
    // model_id is immutable through the instance API. Recheck it after locking.
    let model_id: i64 = sqlx::query_scalar(
        "SELECT model_id FROM cmdb_instance WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(id)
    .bind(tenant.id())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read instance"))?
    .ok_or_else(|| AppError::not_found("instance not found"))?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start update"))?;
    let model = lock_model(&mut tx, model_id).await?;
    let existing: Value = sqlx::query_scalar(
        "SELECT attributes FROM cmdb_instance WHERE id = $1 AND model_id = $2 AND tenant_id = $3 AND deleted = 0 FOR UPDATE"
    ).bind(id).bind(model_id).bind(tenant.id()).fetch_optional(&mut *tx).await
        .map_err(|_| AppError::internal("failed to lock instance"))?
        .ok_or_else(|| AppError::not_found("instance not found"))?;
    let definitions = load_attributes(&mut *tx, model_id).await?;
    let patch = validate_payload(&definitions, &Value::Object(attributes), false)?;
    let mut merged = existing.as_object().cloned().unwrap_or_default();
    merged.extend(patch);
    enforce_unique_key(&mut tx, &model, tenant, &merged, Some(id)).await?;
    sqlx::query("UPDATE cmdb_instance SET attributes = $2, updater = $3, update_time = now() WHERE id = $1 AND tenant_id = $4 AND deleted = 0")
        .bind(id).bind(Value::Object(merged)).bind(actor).bind(tenant.id()).execute(&mut *tx).await
        .map_err(|error| mutation_error(error, "failed to update instance"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit update"))?;
    Ok(())
}

pub(crate) fn parse_ids(raw: &str) -> Result<Vec<i64>, AppError> {
    let mut ids = raw
        .split(',')
        .map(|item| item.trim().parse::<i64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::bad_request("ids must contain positive integers"))?;
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids must contain positive integers"));
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub(crate) async fn delete(
    pool: &PgPool,
    tenant: &TenantContext,
    ids: &[i64],
    actor: &str,
    require_existing: bool,
) -> Result<(), AppError> {
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids must contain positive integers"));
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start delete"))?;
    // Match create/update ordering and the database registry triggers.
    sqlx::query(
        "SELECT id FROM cmdb_model WHERE id IN (SELECT model_id FROM cmdb_instance WHERE id = ANY($1) AND tenant_id = $2 AND deleted = 0) ORDER BY id FOR UPDATE",
    )
    .bind(ids)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock models"))?;
    // Lock in ID order so overlapping bulk requests cannot reverse lock order.
    sqlx::query(
        "SELECT id FROM cmdb_instance WHERE id = ANY($1) AND tenant_id = $2 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(ids)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock instances"))?;
    let result = sqlx::query("UPDATE cmdb_instance SET deleted = 1, updater = $2, update_time = now() WHERE id = ANY($1) AND tenant_id = $3 AND deleted = 0")
        .bind(ids).bind(actor).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to delete instances"))?;
    if require_existing && result.rows_affected() == 0 {
        return Err(AppError::not_found("instance not found"));
    }
    sqlx::query("UPDATE cmdb_relation SET deleted = 1, updater = $2 WHERE deleted = 0 AND tenant_id = $3 AND (source_id = ANY($1) OR target_id = ANY($1))")
        .bind(ids).bind(actor).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to detach relations"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit delete"))?;
    Ok(())
}

/// Called under the model lock before changing its unique-key definition.
pub(crate) async fn validate_unique_key_change(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
    key: Option<&str>,
) -> Result<(), AppError> {
    let Some(key) = key.filter(|key| !key.is_empty()) else {
        return Ok(());
    };
    let values: Vec<Value> = sqlx::query_scalar(
        "SELECT attributes->$2 FROM cmdb_instance WHERE model_id = $1 AND deleted = 0 AND attributes ? $2 GROUP BY tenant_id, attributes->$2 HAVING count(*) > 1"
    ).bind(model_id).bind(key).fetch_all(connection).await
        .map_err(|_| AppError::internal("failed to validate unique key"))?;
    if values.iter().any(|value| !is_empty_value(value)) {
        return Err(AppError::bad_request(
            "existing instances have duplicate values for this unique key",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
