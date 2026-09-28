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
    expression: Option<String>,
}

pub(crate) struct ModelHeader {
    pub(crate) id: i64,
    pub(crate) unique_key: Option<String>,
}

pub(crate) struct BatchUpdateSummary {
    pub(crate) updated: usize,
    pub(crate) unchanged: usize,
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
        "SELECT code, attr_type, required, choices, default_value, expression
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
            expression: row.get("expression"),
        });
    }
    Ok(defs)
}

pub(crate) fn is_empty_value(value: &Value) -> bool {
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

    if object.keys().any(|key| {
        by_code
            .get(key.as_str())
            .is_some_and(|def| def.expression.is_some())
    }) {
        return Err(AppError::bad_request(
            "computed attributes cannot be supplied directly",
        ));
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
                    if is_empty_value(default) {
                        if def.required {
                            missing.push(def.code.clone());
                        }
                    } else if let Err(reason) =
                        def.attr_type.validate(default, def.choices.as_ref())
                    {
                        invalid.push(format!("{} default: {reason}", def.code));
                    } else {
                        data.insert(def.code.clone(), default.clone());
                    }
                } else if def.required {
                    missing.push(def.code.clone());
                }
            }
            provided => {
                if let Some(value) = provided {
                    if is_empty_value(value) {
                        if def.required {
                            missing.push(def.code.clone());
                        } else if !is_create {
                            // An explicitly empty optional value clears the field on patch.
                            // Absence still means "leave unchanged".
                            data.insert(def.code.clone(), value.clone());
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

fn evaluate_computed_attributes(
    definitions: &[AttributeDef],
    data: &mut Map<String, Value>,
) -> Result<(), AppError> {
    let mut pending: Vec<_> = definitions
        .iter()
        .filter_map(|definition| {
            definition
                .expression
                .as_deref()
                .map(|expression| (definition, expression))
        })
        .collect();
    while !pending.is_empty() {
        let mut progressed = false;
        let mut unresolved = Vec::new();
        for (definition, expression) in pending {
            match eval_formula(expression, data) {
                Ok(value) => {
                    if !value.is_finite() {
                        return Err(AppError::bad_request(format!(
                            "computed attribute {:?} produced a non-finite value",
                            definition.code
                        )));
                    }
                    let json = if definition.attr_type == AttrType::Number {
                        let integer = value.trunc();
                        if integer != value
                            || integer < i64::MIN as f64
                            || integer > i64::MAX as f64
                        {
                            return Err(AppError::bad_request(format!(
                                "computed attribute {:?} must produce an integer",
                                definition.code
                            )));
                        }
                        Value::from(integer as i64)
                    } else {
                        serde_json::Number::from_f64(value)
                            .map(Value::Number)
                            .ok_or_else(|| AppError::bad_request("computed value is not finite"))?
                    };
                    data.insert(definition.code.clone(), json);
                    progressed = true;
                }
                Err(error) if error.starts_with("unresolved attribute ") => {
                    unresolved.push((definition, expression))
                }
                Err(error) => {
                    return Err(AppError::bad_request(format!(
                        "computed attribute {:?}: {error}",
                        definition.code
                    )));
                }
            }
        }
        if !progressed {
            let codes = unresolved
                .iter()
                .map(|(definition, _)| definition.code.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(AppError::bad_request(format!(
                "computed attributes contain a missing dependency or cycle: {codes}"
            )));
        }
        pending = unresolved;
    }
    Ok(())
}

fn eval_formula(expression: &str, values: &Map<String, Value>) -> Result<f64, String> {
    struct Parser<'a> {
        input: &'a [u8],
        pos: usize,
        values: &'a Map<String, Value>,
    }
    impl Parser<'_> {
        fn ws(&mut self) {
            while self
                .input
                .get(self.pos)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.pos += 1;
            }
        }
        fn expr(&mut self) -> Result<f64, String> {
            let mut value = self.term()?;
            loop {
                self.ws();
                match self.input.get(self.pos) {
                    Some(b'+') => {
                        self.pos += 1;
                        value += self.term()?;
                    }
                    Some(b'-') => {
                        self.pos += 1;
                        value -= self.term()?;
                    }
                    _ => return Ok(value),
                }
            }
        }
        fn term(&mut self) -> Result<f64, String> {
            let mut value = self.factor()?;
            loop {
                self.ws();
                match self.input.get(self.pos) {
                    Some(b'*') => {
                        self.pos += 1;
                        value *= self.factor()?;
                    }
                    Some(b'/') => {
                        self.pos += 1;
                        let rhs = self.factor()?;
                        if rhs == 0.0 {
                            return Err("division by zero".into());
                        }
                        value /= rhs;
                    }
                    _ => return Ok(value),
                }
            }
        }
        fn factor(&mut self) -> Result<f64, String> {
            self.ws();
            match self.input.get(self.pos).copied() {
                Some(b'+') => {
                    self.pos += 1;
                    self.factor()
                }
                Some(b'-') => {
                    self.pos += 1;
                    Ok(-self.factor()?)
                }
                Some(b'(') => {
                    self.pos += 1;
                    let value = self.expr()?;
                    self.ws();
                    if self.input.get(self.pos) != Some(&b')') {
                        return Err("expected ')'".into());
                    }
                    self.pos += 1;
                    Ok(value)
                }
                Some(b'$') if self.input.get(self.pos + 1) == Some(&b'{') => {
                    self.pos += 2;
                    let start = self.pos;
                    while self.input.get(self.pos).is_some_and(|byte| *byte != b'}') {
                        self.pos += 1;
                    }
                    if self.input.get(self.pos) != Some(&b'}') {
                        return Err("unterminated ${attribute} reference".into());
                    }
                    let code = std::str::from_utf8(&self.input[start..self.pos])
                        .map_err(|_| "invalid attribute reference")?;
                    self.pos += 1;
                    let Some(value) = self.values.get(code) else {
                        return Err(format!("unresolved attribute {code}"));
                    };
                    value
                        .as_f64()
                        .ok_or_else(|| format!("attribute {code:?} is not numeric"))
                }
                Some(byte) if byte.is_ascii_digit() || byte == b'.' => {
                    let start = self.pos;
                    while self.input.get(self.pos).is_some_and(|byte| {
                        byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
                    }) {
                        if self.pos > start
                            && matches!(self.input[self.pos], b'+' | b'-')
                            && !matches!(self.input[self.pos - 1], b'e' | b'E')
                        {
                            break;
                        }
                        self.pos += 1;
                    }
                    std::str::from_utf8(&self.input[start..self.pos])
                        .map_err(|_| "invalid number")?
                        .parse::<f64>()
                        .map_err(|_| "invalid number".into())
                }
                _ => Err("expected a number, attribute reference, or '('".into()),
            }
        }
    }
    let mut parser = Parser {
        input: expression.as_bytes(),
        pos: 0,
        values,
    };
    let result = parser.expr()?;
    parser.ws();
    if parser.pos != parser.input.len() {
        return Err("unexpected input".into());
    }
    Ok(result)
}

pub(crate) async fn recompute_model_instances(
    connection: &mut sqlx::PgConnection,
    tenant: &TenantContext,
    model_id: i64,
    actor: &str,
) -> Result<(), AppError> {
    let model = load_model(&mut *connection, model_id).await?;
    let definitions = load_attributes(&mut *connection, model_id).await?;
    let rows = sqlx::query(
        "SELECT id, attributes FROM cmdb_instance
         WHERE model_id = $1 AND tenant_id = $2 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(model_id)
    .bind(tenant.id())
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| AppError::internal("failed to lock instances for formula update"))?;
    for row in rows {
        let id: i64 = row.get("id");
        let current: Value = row.get("attributes");
        let mut attributes = current.as_object().cloned().ok_or_else(|| {
            AppError::bad_request(format!("instance {id} has invalid attributes"))
        })?;
        for definition in &definitions {
            if definition.expression.is_some() {
                attributes.remove(&definition.code);
            }
        }
        evaluate_computed_attributes(&definitions, &mut attributes)?;
        apply_triggers(connection, model_id, &mut attributes).await?;
        enforce_unique_key(connection, &model, tenant, &attributes, Some(id)).await?;
        sqlx::query(
            "UPDATE cmdb_instance SET attributes = $2, updater = $3, update_time = now()
             WHERE id = $1 AND tenant_id = $4 AND deleted = 0",
        )
        .bind(id)
        .bind(Value::Object(attributes))
        .bind(actor)
        .bind(tenant.id())
        .execute(&mut *connection)
        .await
        .map_err(|_| AppError::internal("failed to recompute instance values"))?;
    }
    Ok(())
}

pub(crate) async fn apply_triggers(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
    data: &mut Map<String, Value>,
) -> Result<(), AppError> {
    let rows = sqlx::query("SELECT condition_code, condition_value, action_code, action_value FROM cmdb_attribute_trigger WHERE model_id = $1 AND enabled = true AND deleted = 0 ORDER BY id")
        .bind(model_id).fetch_all(&mut *connection).await
        .map_err(|_| AppError::internal("failed to read attribute triggers"))?;
    for row in rows {
        let condition_code: String = row.get("condition_code");
        let condition_value: Value = row.get("condition_value");
        if data.get(&condition_code) == Some(&condition_value) {
            let action_code: String = row.get("action_code");
            let action_value: Value = row.get("action_value");
            data.insert(action_code, action_value);
        }
    }
    Ok(())
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

/// Create a complete import batch under one model lock and one transaction.
/// The row number is retained so an atomic import can report the failing Excel
/// row without committing any preceding rows.
pub(crate) async fn create_many(
    pool: &PgPool,
    tenant: &TenantContext,
    model_id: i64,
    rows: Vec<(usize, Map<String, Value>)>,
    actor: &str,
) -> Result<usize, AppError> {
    if model_id <= 0 {
        return Err(AppError::bad_request("modelId is required"));
    }
    if rows.is_empty() {
        return Err(AppError::bad_request("import contains no data rows"));
    }
    let row_count = rows.len();
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start atomic import"))?;
    let model = lock_model(&mut tx, model_id).await?;
    let definitions = load_attributes(&mut *tx, model_id).await?;
    for (row_number, attributes) in rows {
        let mut data =
            validate_payload(&definitions, &Value::Object(attributes), true).map_err(|error| {
                AppError::bad_request(format!("row {row_number}: {}", error.message()))
            })?;
        evaluate_computed_attributes(&definitions, &mut data).map_err(|error| {
            AppError::bad_request(format!("row {row_number}: {}", error.message()))
        })?;
        apply_triggers(&mut tx, model_id, &mut data)
            .await
            .map_err(|error| {
                AppError::bad_request(format!("row {row_number}: {}", error.message()))
            })?;
        enforce_unique_key(&mut tx, &model, tenant, &data, None)
            .await
            .map_err(|error| {
                AppError::bad_request(format!("row {row_number}: {}", error.message()))
            })?;
        sqlx::query(
            "INSERT INTO cmdb_instance (model_id, attributes, creator, updater, tenant_id)
             VALUES ($1, $2, $3, $3, $4)",
        )
        .bind(model_id)
        .bind(Value::Object(data))
        .bind(actor)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to insert atomic import row"))?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit atomic import"))?;
    Ok(row_count)
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

pub(crate) async fn batch_update(
    pool: &PgPool,
    tenant: &TenantContext,
    ids: &[i64],
    attributes: Map<String, Value>,
    actor: &str,
) -> Result<BatchUpdateSummary, AppError> {
    let ids = normalize_batch_ids(ids)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start batch update"))?;

    // Read the owning model first, then acquire the model lock used by every
    // CMDB writer. Instance rows are locked only after the model lock.
    let model_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT model_id FROM cmdb_instance
         WHERE id = ANY($1) AND tenant_id = $2 AND deleted = 0 ORDER BY id",
    )
    .bind(&ids)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to read instances for batch update"))?;
    if model_ids.len() != ids.len() {
        return Err(AppError::not_found(
            "one or more instances selected for batch update no longer exist",
        ));
    }
    let model_id = model_ids[0];
    if model_ids.iter().any(|candidate| *candidate != model_id) {
        return Err(AppError::bad_request(
            "batch update instances must belong to the same model",
        ));
    }

    let model = lock_model(&mut tx, model_id).await?;
    let rows = sqlx::query(
        "SELECT id, attributes FROM cmdb_instance
         WHERE id = ANY($1) AND model_id = $2 AND tenant_id = $3 AND deleted = 0
         ORDER BY id FOR UPDATE",
    )
    .bind(&ids)
    .bind(model_id)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock instances for batch update"))?;
    if rows.len() != ids.len() {
        return Err(AppError::not_found(
            "one or more instances selected for batch update no longer exist",
        ));
    }

    let definitions = load_attributes(&mut *tx, model_id).await?;
    let patch = validate_payload(&definitions, &Value::Object(attributes), false)?;
    let mut merged_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.get("id");
        let existing: Value = row.get("attributes");
        let mut merged = existing.as_object().cloned().unwrap_or_default();
        for (key, value) in &patch {
            if is_empty_value(value) {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
        for definition in &definitions {
            if definition.expression.is_some() {
                merged.remove(&definition.code);
            }
        }
        evaluate_computed_attributes(&definitions, &mut merged)?;
        apply_triggers(&mut tx, model_id, &mut merged).await?;
        merged_rows.push((id, existing, Value::Object(merged)));
    }
    validate_batch_unique_key(&mut tx, &model, tenant, &ids, &merged_rows).await?;

    let changes: Vec<Value> = merged_rows
        .iter()
        .filter(|(_, existing, merged)| existing != merged)
        .map(|(id, _, attributes)| {
            serde_json::json!({
                "id": id,
                "attributes": attributes,
            })
        })
        .collect();
    if !changes.is_empty() {
        let result = sqlx::query(
            "UPDATE cmdb_instance AS instance
             SET attributes = change.attributes, updater = $2, update_time = now()
             FROM jsonb_to_recordset($1::jsonb) AS change(id bigint, attributes jsonb)
             WHERE instance.id = change.id AND instance.model_id = $3 AND instance.tenant_id = $4 AND instance.deleted = 0",
        )
        .bind(Value::Array(changes.clone()))
        .bind(actor)
        .bind(model_id)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to update selected instances"))?;
        if result.rows_affected() != changes.len() as u64 {
            return Err(AppError::internal(
                "batch update changed an unexpected number of instances",
            ));
        }
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit batch update"))?;
    Ok(BatchUpdateSummary {
        updated: changes.len(),
        unchanged: ids.len() - changes.len(),
    })
}

fn normalize_batch_ids(ids: &[i64]) -> Result<Vec<i64>, AppError> {
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids must contain positive integers"));
    }
    let mut normalized = ids.to_vec();
    normalized.sort_unstable();
    normalized.dedup();
    if normalized.len() > 500 {
        return Err(AppError::bad_request(
            "batch update supports at most 500 instances",
        ));
    }
    Ok(normalized)
}

async fn validate_batch_unique_key(
    connection: &mut sqlx::PgConnection,
    model: &ModelHeader,
    tenant: &TenantContext,
    ids: &[i64],
    rows: &[(i64, Value, Value)],
) -> Result<(), AppError> {
    let Some(key) = model.unique_key.as_deref().filter(|key| !key.is_empty()) else {
        return Ok(());
    };
    let values: Vec<Value> = rows
        .iter()
        .filter_map(|(_, _, attributes)| attributes.get(key).cloned())
        .filter(|value| !is_empty_value(value))
        .collect();
    for (index, value) in values.iter().enumerate() {
        if values[index + 1..].contains(value) {
            return Err(AppError::bad_request(format!(
                "batch update would duplicate unique key {key:?} with value {value:?}"
            )));
        }
    }
    if values.is_empty() {
        return Ok(());
    }
    let conflict: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM cmdb_instance AS instance
            WHERE instance.model_id = $1 AND instance.tenant_id = $5 AND instance.deleted = 0
              AND NOT (instance.id = ANY($2))
              AND EXISTS (
                  SELECT 1 FROM jsonb_array_elements($4::jsonb) AS candidate(value)
                  WHERE instance.attributes->$3 = candidate.value
              )
         )",
    )
    .bind(model.id)
    .bind(ids)
    .bind(key)
    .bind(Value::Array(values))
    .bind(tenant.id())
    .fetch_one(connection)
    .await
    .map_err(|_| AppError::internal("failed to validate batch unique keys"))?;
    if conflict {
        return Err(AppError::bad_request(format!(
            "batch update conflicts with an existing {key:?} unique-key value"
        )));
    }
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
