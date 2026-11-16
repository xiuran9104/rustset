//! 流程定义 CRUD：definition jsonb 校验（engine::validate_definition）、
//! 保存 version+1；删除前检查无 running 实例。

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post, put};
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

use crate::{InfraState, Page, QueryParams};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/flow-definition/page", get(page))
        .api_route("/infra/flow-definition/get", get(get_one))
        .api_route("/infra/flow-definition/create", post(create))
        .api_route("/infra/flow-definition/update", put(update))
        .api_route("/infra/flow-definition/delete", get(delete))
}

#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SaveRequest {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    /// 0=启用 1=停用
    status: Option<i16>,
    /// {"steps":[{key,name,type,config,condition}]}
    definition: Value,
}

fn row_value(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "description": row.get::<String, _>("description"),
        "status": row.get::<i16, _>("status"),
        "definition": row.get::<Value, _>("definition"),
        "version": row.get::<i32, _>("version"),
        "stepCount": row.get::<Value, _>("definition")
            .get("steps").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "createTime": row.get::<chrono::NaiveDateTime, _>("create_time").to_string(),
        "updateTime": row.get::<chrono::NaiveDateTime, _>("update_time").to_string(),
    })
}

async fn page(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let pn = params.page_no.unwrap_or(1).max(1);
    let ps = params.page_size.unwrap_or(10).clamp(1, 200);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_flow_definition WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count flow definitions"))?;
    let rows = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(d) - 'tenant_id' - 'creator' - 'updater' - 'deleted'
         FROM infra_flow_definition d
         WHERE d.tenant_id=$1 AND d.deleted=0
         ORDER BY d.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(tenant.id())
    .bind(ps)
    .bind((pn - 1) * ps)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list flow definitions"))?;
    // 补 camelCase 展示字段（to_jsonb 是 snake_case 列名）。
    let list: Vec<Value> = rows
        .into_iter()
        .map(|mut row| {
            if let Some(object) = row.as_object_mut() {
                let step_count = object
                    .get("definition")
                    .and_then(|d| d.get("steps"))
                    .and_then(Value::as_array)
                    .map(|a| a.len())
                    .unwrap_or(0);
                object.insert("stepCount".into(), json!(step_count));
            }
            row
        })
        .collect();
    Ok(Json(ApiResponse::new(Page { list, total })))
}

async fn get_one(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let row = sqlx::query(
        "SELECT id, name, description, status, definition, version, create_time, update_time
         FROM infra_flow_definition WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read flow definition"))?
    .ok_or_else(|| AppError::not_found("flow definition not found"))?;
    Ok(Json(ApiResponse::new(row_value(&row))))
}

async fn create(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("名称必填且不超过 128 字符"));
    }
    super::engine::validate_definition(&request.definition)
        .map_err(AppError::bad_request)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO infra_flow_definition (name, description, status, definition, version, creator, updater, tenant_id)
         VALUES ($1, $2, $3, $4, 1, $5, $5, $6) RETURNING id",
    )
    .bind(name)
    .bind(request.description.unwrap_or_default())
    .bind(request.status.unwrap_or(0))
    .bind(&request.definition)
    .bind(&user.username)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::bad_request(format!("创建流程定义失败: {e}")))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = request.id.ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("名称必填且不超过 128 字符"));
    }
    super::engine::validate_definition(&request.definition)
        .map_err(AppError::bad_request)?;
    let result = sqlx::query(
        "UPDATE infra_flow_definition
         SET name=$2, description=$3, status=$4, definition=$5,
             version=version+1, updater=$6, update_time=now()
         WHERE id=$1 AND tenant_id=$7 AND deleted=0",
    )
    .bind(id)
    .bind(name)
    .bind(request.description.unwrap_or_default())
    .bind(request.status.unwrap_or(0))
    .bind(&request.definition)
    .bind(&user.username)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to update flow definition"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("flow definition not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let running: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_flow_instance
         WHERE definition_id=$1 AND tenant_id=$2 AND status='running' AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to check running instances"))?;
    if running > 0 {
        return Err(AppError::bad_request("该定义存在运行中的实例，不能删除"));
    }
    let result = sqlx::query(
        "UPDATE infra_flow_definition SET deleted=1, updater=$3, update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to delete flow definition"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("flow definition not found"));
    }
    Ok(Json(ApiResponse::new(())))
}
