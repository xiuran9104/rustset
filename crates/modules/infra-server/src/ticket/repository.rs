//! Tenant-scoped ticket access. The owner is supplied by authentication, never
//! copied from the request; it remains stable for later provisioning retries.
use super::*;
use rustset_framework_tenant::TenantContext;
use sqlx::PgPool;

pub(super) async fn get(pool: &PgPool, tenant: &TenantContext, id: i64) -> Result<Value, AppError> {
    let value = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_resource_ticket t WHERE id=$1 AND tenant_id=$2 AND deleted=0")
        .bind(id).bind(tenant.id()).fetch_optional(pool).await
        .map_err(|_| AppError::internal("failed to read ticket"))?
        .ok_or_else(|| AppError::not_found("ticket not found"))?;
    Ok(crate::table_value(value))
}

pub(super) async fn page(
    pool: &PgPool,
    tenant: &TenantContext,
    params: QueryParams,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (params.page_no.unwrap_or(1).max(1) - 1).saturating_mul(size);
    let total = sqlx::query_scalar(
        "SELECT count(*) FROM infra_resource_ticket WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count tickets"))?;
    let list = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM infra_resource_ticket t WHERE tenant_id=$1 AND deleted=0 ORDER BY id DESC LIMIT $2 OFFSET $3")
        .bind(tenant.id()).bind(size).bind(offset).fetch_all(pool).await
        .map_err(|_| AppError::internal("failed to list tickets"))?.into_iter().map(crate::table_value).collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

pub(super) async fn create(
    pool: &PgPool,
    tenant: &TenantContext,
    payload: Value,
) -> Result<(Json<ApiResponse<String>>, bool), AppError> {
    let mut payload = crate::camel_payload_to_snake(payload);
    let object = payload
        .as_object_mut()
        .ok_or_else(|| AppError::bad_request("ticket must be an object"))?;
    object.insert("tenant_id".into(), json!(tenant.id()));
    let idempotency_key = object
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if idempotency_key
        .as_ref()
        .is_some_and(|value| value.len() > 128)
    {
        return Err(AppError::bad_request("idempotencyKey 最多 128 个字符"));
    }
    object.insert(
        "idempotency_key".into(),
        idempotency_key.clone().map_or(Value::Null, Value::String),
    );
    let missing = crate::missing_required_fields(pool, TICKET.table, &payload).await?;
    if !missing.is_empty() {
        return Err(AppError::bad_request(format!(
            "missing required fields: {}",
            missing.join(", ")
        )));
    }
    let mut columns = crate::table_writable_columns(pool, TICKET.table, &payload, false).await?;
    // Generic CRUD deliberately excludes tenant_id; only this scoped path may
    // supply the authenticated owner after normalizing all request aliases.
    columns.push("tenant_id".into());
    let values = columns
        .iter()
        .map(|column| format!("r.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO infra_resource_ticket ({}) SELECT {values} FROM jsonb_populate_record(NULL::infra_resource_ticket, $1::jsonb) r
         ON CONFLICT(tenant_id,idempotency_key) WHERE deleted=0 AND idempotency_key IS NOT NULL
         DO UPDATE SET update_time=infra_resource_ticket.update_time
         RETURNING id, (xmax = 0) AS inserted",
        columns.join(", ")
    );
    let (id, inserted): (i64, bool) = sqlx::query_as(&sql)
        .bind(payload)
        .fetch_one(pool)
        .await
        .map_err(|error| crate::record_query_error("create", error))?;
    Ok((Json(ApiResponse::new(id.to_string())), inserted))
}

pub(super) async fn update(
    pool: &PgPool,
    tenant: &TenantContext,
    payload: Value,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    get(pool, tenant, id).await?;
    let mut payload = crate::camel_payload_to_snake(payload);
    if let Some(object) = payload.as_object_mut() {
        object.remove("tenant_id");
    }
    let columns = crate::table_writable_columns(pool, TICKET.table, &payload, true).await?;
    if columns.is_empty() {
        return Ok(Json(ApiResponse::new(())));
    }
    let set = columns
        .iter()
        .map(|column| format!("{column}=r.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE infra_resource_ticket t SET {set}, update_time=now() FROM jsonb_populate_record(NULL::infra_resource_ticket, $2::jsonb) r WHERE t.id=$1 AND t.tenant_id=$3 AND t.deleted=0"
    );
    let result = sqlx::query(&sql)
        .bind(id)
        .bind(payload)
        .bind(tenant.id())
        .execute(pool)
        .await
        .map_err(|error| crate::record_query_error("update", error))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("ticket not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

pub(super) async fn delete(
    pool: &PgPool,
    tenant: &TenantContext,
    ids: &[i64],
) -> Result<Json<ApiResponse<()>>, AppError> {
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids are required"));
    }
    sqlx::query("UPDATE infra_resource_ticket SET deleted=1, update_time=now() WHERE id=ANY($1) AND tenant_id=$2 AND deleted=0")
        .bind(ids).bind(tenant.id()).execute(pool).await.map_err(|_| AppError::internal("failed to delete tickets"))?;
    Ok(Json(ApiResponse::new(())))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL and PostgreSQL CREATEDB permission"]
    async fn crud_never_accepts_a_request_tenant_or_cross_tenant_id() {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use std::str::FromStr;
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        let admin = PgPool::connect(&url).await.unwrap();
        let database = format!("ticket_tenant_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!(
            "CREATE DATABASE {database} TEMPLATE template0 ENCODING 'UTF8'"
        ))
        .execute(&admin)
        .await
        .unwrap();
        let pool = PgPoolOptions::new()
            .connect_with(
                PgConnectOptions::from_str(&url)
                    .unwrap()
                    .database(&database),
            )
            .await
            .unwrap();
        rustset_framework_database::migrate(&pool).await.unwrap();
        let other_id: i64 = sqlx::query_scalar(
            "SELECT id FROM system_tenant WHERE id <> 1 AND id > 0 ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let a = TenantContext::from_persisted_id(Some(1)).unwrap();
        let b = TenantContext::from_persisted_id(Some(other_id)).unwrap();
        let (Json(created), inserted) = create(&pool, &a, json!({"resourceType":"ecs","ecsName":"a","ticketStatus":"pending_approval","createdBy":"test","tenantId":987654,"tenant_id":987654,"idempotencyKey":"request-1"})).await.unwrap();
        assert!(inserted);
        let id: i64 = created.data.parse().unwrap();
        let (Json(replayed), inserted) = create(&pool, &a, json!({"resourceType":"ecs","ecsName":"duplicate","ticketStatus":"pending_approval","createdBy":"test","idempotencyKey":"request-1"})).await.unwrap();
        assert!(!inserted);
        assert_eq!(replayed.data, id.to_string());
        assert_eq!(get(&pool, &a, id).await.unwrap()["tenantId"], 1);
        assert!(get(&pool, &b, id).await.is_err());
        assert!(
            update(&pool, &b, json!({"id":id,"ecsName":"stolen"}))
                .await
                .is_err()
        );
        let _ = update(
            &pool,
            &a,
            json!({"id":id,"tenantId":987654,"ecsName":"renamed"}),
        )
        .await
        .unwrap();
        let record = get(&pool, &a, id).await.unwrap();
        assert_eq!(record["tenantId"], 1);
        assert_eq!(record["ecsName"], "renamed");
        assert_eq!(
            page(
                &pool,
                &b,
                QueryParams {
                    page_no: None,
                    page_size: None
                }
            )
            .await
            .unwrap()
            .0
            .data
            .total,
            0
        );
        let _ = delete(&pool, &b, &[id]).await.unwrap();
        assert!(get(&pool, &a, id).await.is_ok());
        let _ = delete(&pool, &a, &[id]).await.unwrap();
        assert!(get(&pool, &a, id).await.is_err());
        pool.close().await;
        sqlx::query(&format!("DROP DATABASE {database}"))
            .execute(&admin)
            .await
            .unwrap();
    }
}
