//! CMDB configuration-item instances: dynamic JSONB payloads validated
//! against the owning model's attribute definitions, with unique-key
//! enforcement (veops-style 唯一键) and keyword search.

use schemars::JsonSchema;

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_cmdb_api::{
    CreateInstanceRequest, InstanceImportError, InstanceImportResponse, InstancePageParams,
    InstancePageResponse, InstanceResponse, UpdateInstanceRequest,
};
use rustset_framework_common::{ApiResponse, csv as csv_exchange};
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use serde::Deserialize;
use serde_json::{Map, Value};
use sqlx::Row;

use crate::{
    CmdbState,
    instance_service::{self, AttributeDef, load_attributes, load_model},
    require,
};

pub fn routes() -> ApiRouter<CmdbState> {
    ApiRouter::new()
        .api_route("/cmdb/instance/page", get(instance_page))
        .api_route("/cmdb/instance/get", get(instance_get))
        .api_route("/cmdb/instance/create", post(instance_create))
        .api_route("/cmdb/instance/update", put(instance_update))
        .api_route("/cmdb/instance/delete", delete(instance_delete))
        .api_route("/cmdb/instance/delete-list", delete(instance_delete_list))
        .api_route("/cmdb/instance/export", get(instance_export))
        .api_route(
            "/cmdb/instance/import-template",
            get(instance_import_template),
        )
        .api_route("/cmdb/instance/import", post(instance_import))
}

fn instance_row(row: &sqlx::postgres::PgRow) -> InstanceResponse {
    InstanceResponse {
        id: row.get("id"),
        model_id: row.get("model_id"),
        attributes: row.get("attributes"),
        create_time: row
            .get::<chrono::NaiveDateTime, _>("create_time")
            .to_string(),
        update_time: row
            .get::<chrono::NaiveDateTime, _>("update_time")
            .to_string(),
    }
}

fn page_bounds(page_no: Option<i64>, page_size: Option<i64>) -> (i64, i64) {
    let size = page_size.unwrap_or(20).clamp(1, 200);
    let no = page_no.unwrap_or(1).max(1);
    (size, (no - 1).saturating_mul(size))
}

async fn instance_page(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstancePageParams>,
) -> Result<Json<ApiResponse<InstancePageResponse>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    load_model(&state.pool, params.model_id).await?;
    let (size, offset) = page_bounds(params.page_no, params.page_size);
    let keyword = params
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| format!("%{k}%"));
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_instance
         WHERE model_id = $1 AND tenant_id = $3 AND deleted = 0
           AND ($2::text IS NULL OR attributes::text ILIKE $2)",
    )
    .bind(params.model_id)
    .bind(&keyword)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count instances"))?;
    let rows = sqlx::query(
        "SELECT id, model_id, attributes, create_time, update_time
         FROM cmdb_instance
         WHERE model_id = $1 AND tenant_id = $5 AND deleted = 0
           AND ($2::text IS NULL OR attributes::text ILIKE $2)
         ORDER BY id DESC LIMIT $3 OFFSET $4",
    )
    .bind(params.model_id)
    .bind(&keyword)
    .bind(size)
    .bind(offset)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read instances"))?;
    Ok(Json(ApiResponse::new(InstancePageResponse {
        list: rows.iter().map(instance_row).collect(),
        total,
    })))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InstanceIdParams {
    id: i64,
}

async fn instance_get(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<InstanceResponse>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let row = sqlx::query(
        "SELECT id, model_id, attributes, create_time, update_time
         FROM cmdb_instance WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read instance"))?
    .ok_or_else(|| AppError::not_found("instance not found"))?;
    Ok(Json(ApiResponse::new(instance_row(&row))))
}

async fn instance_create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<CreateInstanceRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:instance:create")?;
    let tenant = TenantContext::from_user(&user)?;
    let id = instance_service::create(
        &state.pool,
        &tenant,
        payload.model_id,
        payload.attributes,
        &user.username,
    )
    .await?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn instance_update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<UpdateInstanceRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let tenant = TenantContext::from_user(&user)?;
    instance_service::update(
        &state.pool,
        &tenant,
        payload.id,
        payload.attributes,
        &user.username,
    )
    .await?;
    Ok(Json(ApiResponse::new(())))
}

async fn instance_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:delete")?;
    let tenant = TenantContext::from_user(&user)?;
    instance_service::delete(&state.pool, &tenant, &[params.id], &user.username, true).await?;
    Ok(Json(ApiResponse::new(())))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DeleteListParams {
    ids: String,
}

async fn instance_delete_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<DeleteListParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:delete")?;
    let tenant = TenantContext::from_user(&user)?;
    let ids = instance_service::parse_ids(&params.ids)?;
    instance_service::delete(&state.pool, &tenant, &ids, &user.username, false).await?;
    Ok(Json(ApiResponse::new(())))
}

// ---------- Tenant-scoped CSV export / import ----------

#[derive(Debug, Deserialize, JsonSchema)]
struct ExportParams {
    model_id: i64,
}

async fn instance_export(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<ExportParams>,
) -> Result<axum::response::Response, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let model = load_model(&state.pool, params.model_id).await?;
    let attributes = load_attributes(&state.pool, params.model_id).await?;
    let rows = sqlx::query(
        "SELECT attributes FROM cmdb_instance WHERE model_id = $1 AND tenant_id = $2 AND deleted = 0 ORDER BY id LIMIT 10001",
    )
    .bind(params.model_id)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read instances"))?;

    if rows.len() > csv_exchange::MAX_ROWS {
        return Err(AppError::bad_request("单次导出上限为 10000 条"));
    }
    let headers = attributes
        .iter()
        .map(|attribute| attribute.code.as_str())
        .collect::<Vec<_>>();
    let csv_rows = rows
        .iter()
        .map(|row| {
            let payload: Value = row.get("attributes");
            attributes
                .iter()
                .map(|attribute| {
                    payload
                        .get(&attribute.code)
                        .map(cell_text)
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    csv_exchange::response(
        &format!("cmdb_instances_model_{}.csv", model.id),
        &headers,
        &csv_rows,
    )
    .map_err(AppError::internal)
}

async fn instance_import_template(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<ExportParams>,
) -> Result<axum::response::Response, AppError> {
    require(&user, "cmdb:instance:create")?;
    TenantContext::from_user(&user)?;
    load_model(&state.pool, params.model_id).await?;
    let attributes = load_attributes(&state.pool, params.model_id).await?;
    let headers = attributes
        .iter()
        .map(|attribute| attribute.code.as_str())
        .collect::<Vec<_>>();
    csv_exchange::response(
        &format!("cmdb_instances_model_{}_template.csv", params.model_id),
        &headers,
        &[],
    )
    .map_err(AppError::internal)
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(_) => value.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn csv_cell(def: &AttributeDef, text: &str) -> Result<Value, String> {
    use rustset_cmdb_api::AttrType;
    match def.attr_type {
        AttrType::Number => text
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| format!("{} 必须为整数", def.code)),
        AttrType::Float => text
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(Value::from)
            .ok_or_else(|| format!("{} 必须为有限数字", def.code)),
        AttrType::Bool => match text.to_ascii_lowercase().as_str() {
            "true" | "1" | "是" => Ok(Value::Bool(true)),
            "false" | "0" | "否" => Ok(Value::Bool(false)),
            _ => Err(format!("{} 必须为 true 或 false", def.code)),
        },
        AttrType::Json => {
            serde_json::from_str(text).map_err(|_| format!("{} 必须为 JSON", def.code))
        }
        AttrType::MultiSelect => {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                if value.is_array() {
                    return Ok(value);
                }
            }
            Ok(Value::Array(
                text.split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(|item| Value::String(item.to_owned()))
                    .collect(),
            ))
        }
        _ => Ok(Value::String(text.to_owned())),
    }
}

async fn instance_import(
    State(state): State<CmdbState>,
    user: CurrentUser,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<ApiResponse<InstanceImportResponse>>, AppError> {
    require(&user, "cmdb:instance:create")?;
    let tenant = TenantContext::from_user(&user)?;
    let mut model_id = 0;
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("invalid multipart payload"))?
    {
        match field.name() {
            Some("modelId") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad_request("invalid modelId"))?;
                model_id = text.trim().parse().unwrap_or(0);
            }
            Some("file") => {
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| AppError::bad_request("invalid file"))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }
    if model_id <= 0 {
        return Err(AppError::bad_request("modelId is required"));
    }
    let Some(file_bytes) = file_bytes else {
        return Err(AppError::bad_request("file is required"));
    };
    load_model(&state.pool, model_id).await?;
    let attributes = load_attributes(&state.pool, model_id).await?;
    let by_code: std::collections::BTreeMap<&str, &AttributeDef> = attributes
        .iter()
        .map(|def| (def.code.as_str(), def))
        .collect();

    let table = csv_exchange::decode(&file_bytes).map_err(AppError::bad_request)?;
    let unknown = table
        .headers
        .iter()
        .filter(|code| !by_code.contains_key(code.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(AppError::bad_request(format!(
            "表头包含模型未定义的属性: {}",
            unknown.join(", ")
        )));
    }

    let mut created = 0i64;
    let mut errors: Vec<InstanceImportError> = Vec::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        if row.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        let mut payload = Map::new();
        let mut row_error = None;
        for (code, cell) in table.headers.iter().zip(row) {
            if cell.is_empty() {
                continue;
            }
            let Some(def) = by_code.get(code.as_str()) else {
                continue;
            };
            match csv_cell(def, cell) {
                Ok(value) => {
                    payload.insert(code.clone(), value);
                }
                Err(error) => {
                    row_error = Some(error);
                    break;
                }
            }
        }
        let row_no = row_index + 2;
        if let Some(error) = row_error {
            errors.push(InstanceImportError { row: row_no, error });
            continue;
        }
        if payload.is_empty() {
            continue;
        }
        match instance_service::create(&state.pool, &tenant, model_id, payload, &user.username)
            .await
        {
            Ok(_) => created += 1,
            Err(error) => errors.push(InstanceImportError {
                row: row_no,
                error: error.message().to_owned(),
            }),
        }
    }

    Ok(Json(ApiResponse::new(InstanceImportResponse {
        created,
        failed: errors.len(),
        errors: errors.into_iter().take(50).collect(),
    })))
}
