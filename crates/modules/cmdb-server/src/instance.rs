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
use rustset_cmdb_api::{BatchUpdateInstanceRequest, CreateInstanceRequest, UpdateInstanceRequest};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use serde::Deserialize;
use serde_json::{Map, Value, json};
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
        .api_route("/cmdb/instance/update-list", put(instance_update_list))
        .api_route("/cmdb/instance/delete", delete(instance_delete))
        .api_route("/cmdb/instance/delete-list", delete(instance_delete_list))
        .api_route("/cmdb/instance/export", get(instance_export))
        .api_route("/cmdb/instance/import", post(instance_import))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct InstancePageParams {
    model_id: i64,
    #[serde(default)]
    page_no: Option<i64>,
    #[serde(default)]
    page_size: Option<i64>,
    #[serde(default)]
    keyword: Option<String>,
}

fn instance_row(row: &sqlx::postgres::PgRow) -> Value {
    let attributes: Value = row.get("attributes");
    json!({
        "id": row.get::<i64, _>("id"),
        "modelId": row.get::<i64, _>("model_id"),
        "attributes": attributes,
        "createTime": row.get::<chrono::NaiveDateTime, _>("create_time").to_string(),
        "updateTime": row.get::<chrono::NaiveDateTime, _>("update_time").to_string(),
    })
}

fn page_bounds(page_no: Option<i64>, page_size: Option<i64>) -> (i64, i64) {
    let size = page_size.unwrap_or(20).clamp(1, 200);
    let no = page_no.unwrap_or(1).max(1);
    (size, (no - 1) * size)
}

async fn instance_page(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstancePageParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:instance:query")?;
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
         WHERE model_id = $1 AND deleted = 0
           AND ($2::text IS NULL OR attributes::text ILIKE $2)",
    )
    .bind(params.model_id)
    .bind(&keyword)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count instances"))?;
    let rows = sqlx::query(
        "SELECT id, model_id, attributes, create_time, update_time
         FROM cmdb_instance
         WHERE model_id = $1 AND deleted = 0
           AND ($2::text IS NULL OR attributes::text ILIKE $2)
         ORDER BY id DESC LIMIT $3 OFFSET $4",
    )
    .bind(params.model_id)
    .bind(&keyword)
    .bind(size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read instances"))?;
    Ok(Json(ApiResponse::new(json!({
        "list": rows.iter().map(instance_row).collect::<Vec<_>>(),
        "total": total,
    }))))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InstanceIdParams {
    id: i64,
}

async fn instance_get(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let row = sqlx::query(
        "SELECT id, model_id, attributes, create_time, update_time
         FROM cmdb_instance WHERE id = $1 AND deleted = 0",
    )
    .bind(params.id)
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
    let id = instance_service::create(
        &state.pool,
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
    instance_service::update(&state.pool, payload.id, payload.attributes, &user.username).await?;
    Ok(Json(ApiResponse::new(())))
}

async fn instance_update_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<BatchUpdateInstanceRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let result = instance_service::batch_update(
        &state.pool,
        &payload.ids,
        payload.attributes,
        &user.username,
    )
    .await?;
    Ok(Json(ApiResponse::new(json!({
        "updated": result.updated,
        "unchanged": result.unchanged,
    }))))
}

async fn instance_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:delete")?;
    instance_service::delete(&state.pool, &[params.id], &user.username, true).await?;
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
    let ids = instance_service::parse_ids(&params.ids)?;
    instance_service::delete(&state.pool, &ids, &user.username, false).await?;
    Ok(Json(ApiResponse::new(())))
}

// ---------- Excel export / import ----------

#[derive(Debug, Deserialize, JsonSchema)]
struct ExportParams {
    model_id: i64,
}

async fn instance_export(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<ExportParams>,
) -> Result<axum::response::Response, AppError> {
    use rust_xlsxwriter::Workbook;

    require(&user, "cmdb:instance:query")?;
    let model = load_model(&state.pool, params.model_id).await?;
    let attributes = load_attributes(&state.pool, params.model_id).await?;
    let rows = sqlx::query(
        "SELECT attributes FROM cmdb_instance WHERE model_id = $1 AND deleted = 0 ORDER BY id LIMIT 10000",
    )
    .bind(params.model_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read instances"))?;

    let mut workbook = Workbook::new();
    let worksheet = workbook
        .add_worksheet()
        .set_name("instances")
        .map_err(|_| AppError::internal("failed to create sheet"))?;
    for (col, attr) in attributes.iter().enumerate() {
        worksheet
            .write_string(0, col as u16, &attr.code)
            .map_err(|_| AppError::internal("failed to write header"))?;
    }
    for (row_index, row) in rows.iter().enumerate() {
        let payload: Value = row.get("attributes");
        for (col, attr) in attributes.iter().enumerate() {
            let cell = payload.get(&attr.code).map(cell_text).unwrap_or_default();
            worksheet
                .write_string((row_index + 1) as u32, col as u16, cell)
                .map_err(|_| AppError::internal("failed to write row"))?;
        }
    }
    let bytes = workbook
        .save_to_buffer()
        .map_err(|_| AppError::internal("failed to build excel"))?;
    axum::response::Response::builder()
        .header(
            "content-type",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            "content-disposition",
            format!(
                "attachment; filename=\"cmdb_instances_model_{}.xlsx\"",
                model.id
            ),
        )
        .body(axum::body::Body::from(bytes))
        .map_err(|_| AppError::internal("failed to build download"))
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(_) => value.to_string(),
        Value::Array(items) => items.iter().map(cell_text).collect::<Vec<_>>().join(","),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

async fn instance_import(
    State(state): State<CmdbState>,
    user: CurrentUser,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    use calamine::{Data, DataType as _, Reader, Xlsx};

    require(&user, "cmdb:instance:create")?;
    let mut model_id = 0;
    let mut atomic = false;
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
            Some("mode") => {
                let mode = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad_request("invalid import mode"))?;
                atomic = mode.trim().eq_ignore_ascii_case("atomic");
            }
            Some("atomic") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad_request("invalid atomic flag"))?;
                atomic = matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
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

    let mut reader = Xlsx::new(std::io::Cursor::new(file_bytes))
        .map_err(|_| AppError::bad_request("无法读取 xlsx 文件"))?;
    let sheet = reader.sheet_names().first().cloned().unwrap_or_default();
    let range = reader
        .worksheet_range(&sheet)
        .map_err(|_| AppError::bad_request("无法读取工作表"))?;

    let mut created = 0i64;
    let mut atomic_rows = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    for (row_index, row) in range.rows().enumerate() {
        if row_index == 0 {
            let unknown: Vec<String> = row
                .iter()
                .filter_map(|cell| cell.get_string().map(str::to_string))
                .filter(|code| !code.trim().is_empty() && !by_code.contains_key(code.trim()))
                .collect();
            if !unknown.is_empty() {
                return Err(AppError::bad_request(format!(
                    "表头包含模型未定义的属性: {}",
                    unknown.join(", ")
                )));
            }
            continue;
        }
        if row.iter().all(|cell| matches!(cell, Data::Empty)) {
            continue;
        }
        let mut payload = Map::new();
        for (col, cell) in row.iter().enumerate() {
            let Some(code) = range
                .rows()
                .next()
                .and_then(|header| header.get(col))
                .and_then(|cell| cell.get_string().map(str::to_string))
                .map(|code| code.trim().to_string())
                .filter(|code| !code.is_empty())
            else {
                continue;
            };
            let Some(def) = by_code.get(code.as_str()) else {
                continue;
            };
            let raw = match cell {
                Data::Empty => continue,
                Data::String(text) => Value::String(text.clone()),
                Data::Float(number) => {
                    if def.attr_type.code().contains("float") {
                        json!(number)
                    } else {
                        json!(*number as i64)
                    }
                }
                Data::Int(number) => json!(number),
                Data::Bool(flag) => json!(flag),
                Data::DateTime(excel) => json!(excel.to_string()),
                other => Value::String(other.to_string()),
            };
            payload.insert(code, raw);
        }
        if payload.is_empty() {
            continue;
        }
        let row_no = row_index + 1;
        if atomic {
            atomic_rows.push((row_no, payload));
            continue;
        }
        match instance_service::create(&state.pool, model_id, payload, &user.username).await {
            Ok(_) => created += 1,
            Err(error) => errors.push(json!({"row": row_no, "error": error.message()})),
        }
    }

    if atomic {
        created = instance_service::create_many(&state.pool, model_id, atomic_rows, &user.username)
            .await?
            .try_into()
            .map_err(|_| AppError::internal("atomic import row count overflow"))?;
        return Ok(Json(ApiResponse::new(json!({
            "created": created,
            "failed": 0,
            "atomic": true,
            "errors": [],
        }))));
    }

    Ok(Json(ApiResponse::new(json!({
        "created": created,
        "failed": errors.len(),
        "atomic": false,
        "errors": errors.into_iter().take(50).collect::<Vec<_>>(),
    }))))
}
