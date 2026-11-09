//! Tenant-scoped asset and network-policy CRUD. Callers use static table specs.
use super::*;
use rustset_framework_tenant::TenantContext;
use rustset_framework_database::{BindIn, DbPool, expand_in_placeholders};

pub async fn get(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    id: i64,
) -> Result<Value, AppError> {
    let value = sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE id=$1 AND tenant_id=$2 AND deleted=0",
        table = spec.table
    )))
    .bind(id)
    .bind(tenant.id())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read record"))?
    .ok_or_else(|| AppError::not_found("record not found"))?;
    Ok(crate::table_value(value))
}

/// 纯数据分页查询（HTTP 处理器与 mcp-server 工具共用）。
pub async fn page_data(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    params: QueryParams,
) -> Result<crate::Page<Value>, AppError> {
    let size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (params.page_no.unwrap_or(1).max(1) - 1).saturating_mul(size);
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM {table} WHERE tenant_id=$1 AND deleted=0",
        table = spec.table
    )))
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count records"))?;
    let list = sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(format!("SELECT to_jsonb(t) FROM {table} t WHERE tenant_id=$1 AND deleted=0 ORDER BY id DESC LIMIT $2 OFFSET $3", table=spec.table)))
        .bind(tenant.id()).bind(size).bind(offset).fetch_all(pool).await
        .map_err(|_| AppError::internal("failed to list records"))?.into_iter().map(crate::table_value).collect();
    Ok(crate::Page { list, total })
}

pub(super) async fn page(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    params: QueryParams,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    Ok(Json(ApiResponse::new(
        page_data(pool, spec, tenant, params).await?,
    )))
}

pub(super) async fn create(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    payload: Value,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let mut payload = crate::camel_payload_to_snake(payload);
    payload
        .as_object_mut()
        .ok_or_else(|| AppError::bad_request("record must be an object"))?
        .insert("tenant_id".into(), json!(tenant.id()));
    let missing = crate::missing_required_fields(pool, spec.table, &payload).await?;
    if !missing.is_empty() {
        return Err(AppError::bad_request(format!(
            "missing required fields: {}",
            missing.join(", ")
        )));
    }
    let mut columns = crate::table_writable_columns(pool, spec.table, &payload, false).await?;
    // Generic CRUD deliberately excludes tenant_id; only this scoped path may
    // supply the authenticated owner after normalizing all request aliases.
    columns.push("tenant_id".into());
    let values = columns
        .iter()
        .map(|column| format!("r.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) SELECT {values} FROM jsonb_populate_record(NULL::{table}, $1) r RETURNING id",
        columns.join(", "),
        table = spec.table
    );
    let id: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(payload)
        .fetch_one(pool)
        .await
        .map_err(|error| crate::record_query_error("create", error))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

pub(super) async fn update(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    payload: Value,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    get(pool, spec, tenant, id).await?;
    let mut payload = crate::camel_payload_to_snake(payload);
    if let Some(object) = payload.as_object_mut() {
        object.remove("tenant_id");
    }
    let columns = crate::table_writable_columns(pool, spec.table, &payload, true).await?;
    if columns.is_empty() {
        return Ok(Json(ApiResponse::new(())));
    }
    let set = columns
        .iter()
        .map(|column| format!("{column}=r.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE {table} t SET {set}, update_time=now() FROM jsonb_populate_record(NULL::{table}, $2) r WHERE t.id=$1 AND t.tenant_id=$3 AND t.deleted=0",
        table = spec.table
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(payload)
        .bind(tenant.id())
        .execute(pool)
        .await
        .map_err(|error| crate::record_query_error("update", error))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("record not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

pub(super) async fn delete(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
    ids: &[i64],
) -> Result<Json<ApiResponse<()>>, AppError> {
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids are required"));
    }
    let placeholders = expand_in_placeholders(ids, 1);
    let sql = format!(
        "UPDATE {table} SET deleted=1, update_time=now() WHERE id IN ({placeholders}) AND tenant_id=${index} AND deleted=0",
        table = spec.table,
        index = ids.len() + 1,
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind_in(ids)
        .bind(tenant.id())
        .execute(pool)
        .await
        .map_err(|_| AppError::internal("failed to delete records"))?;
    Ok(Json(ApiResponse::new(())))
}

pub(super) async fn list(
    pool: &DbPool,
    spec: TableSpec,
    tenant: &TenantContext,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let rows = sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(format!(
        "SELECT to_jsonb(t) FROM {} t WHERE tenant_id=$1 AND deleted=0 ORDER BY id DESC",
        spec.table
    )))
    .bind(tenant.id())
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list records"))?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(crate::table_value).collect(),
    )))
}
