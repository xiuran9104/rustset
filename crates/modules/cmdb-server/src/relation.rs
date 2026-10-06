//! CMDB relations between configuration items: bind, unbind, and lookup in
//! both directions.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post};
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
use std::collections::{BTreeMap, BTreeSet};

use crate::{CmdbState, require};

pub fn routes() -> ApiRouter<CmdbState> {
    ApiRouter::new()
        .api_route("/cmdb/relation/list-by-instance", get(relation_list))
        .api_route("/cmdb/relation/topology", get(relation_topology))
        .api_route("/cmdb/relation/bind", post(relation_bind))
        .api_route("/cmdb/relation/unbind", delete(relation_unbind))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InstanceIdParams {
    #[serde(rename = "instanceId")]
    instance_id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct TopologyParams {
    instance_id: i64,
    depth: Option<u8>,
    limit: Option<usize>,
}

fn topology_bounds(depth: Option<u8>, limit: Option<usize>) -> Result<(u8, usize), AppError> {
    let depth = depth.unwrap_or(2);
    let limit = limit.unwrap_or(80);
    if !(1..=4).contains(&depth) {
        return Err(AppError::bad_request("depth must be between 1 and 4"));
    }
    if !(2..=200).contains(&limit) {
        return Err(AppError::bad_request("limit must be between 2 and 200"));
    }
    Ok((depth, limit))
}

async fn instance_exists(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    id: i64,
) -> Result<bool, AppError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM cmdb_instance WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(id)
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .map_err(|_| AppError::internal("failed to read instance"))
}

async fn relation_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    if !instance_exists(&state.pool, &tenant, params.instance_id).await? {
        return Err(AppError::not_found("instance not found"));
    }
    let rows = sqlx::query(
        "SELECT r.id, r.source_id, r.target_id, r.relation
         FROM cmdb_relation r
         JOIN cmdb_instance source ON source.id = r.source_id AND source.tenant_id = r.tenant_id AND source.deleted = 0
         JOIN cmdb_instance target ON target.id = r.target_id AND target.tenant_id = r.tenant_id AND target.deleted = 0
         WHERE r.tenant_id = $2 AND r.deleted = 0 AND (r.source_id = $1 OR r.target_id = $1)
         ORDER BY r.id DESC",
    )
    .bind(params.instance_id)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read relations"))?;
    Ok(Json(ApiResponse::new(
        rows.iter()
            .map(|row| {
                json!({
                    "id": row.get::<i64, _>("id"),
                    "sourceId": row.get::<i64, _>("source_id"),
                    "targetId": row.get::<i64, _>("target_id"),
                    "relation": row.get::<String, _>("relation"),
                })
            })
            .collect::<Vec<_>>(),
    )))
}

async fn relation_topology(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<TopologyParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:instance:query")?;
    let tenant = TenantContext::from_user(&user)?;
    let (depth, limit) = topology_bounds(params.depth, params.limit)?;
    let topology = load_topology(&state.pool, &tenant, params.instance_id, depth, limit).await?;
    Ok(Json(ApiResponse::new(topology)))
}

async fn load_topology(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    root_id: i64,
    max_depth: u8,
    node_limit: usize,
) -> Result<Value, AppError> {
    if root_id <= 0 || !instance_exists(pool, tenant, root_id).await? {
        return Err(AppError::not_found("instance not found"));
    }
    let edge_limit = node_limit.saturating_mul(4).min(800);
    let mut node_depths = BTreeMap::from([(root_id, 0u8)]);
    let mut frontier = vec![root_id];
    let mut edges = BTreeSet::<i64>::new();
    let mut truncated = false;

    for current_depth in 1..=max_depth {
        if frontier.is_empty() || edges.len() >= edge_limit {
            break;
        }
        let seen_edges: Vec<i64> = edges.iter().copied().collect();
        let remaining_edges = edge_limit - edges.len();
        let rows = sqlx::query(
            "SELECT relation.id, relation.source_id, relation.target_id
             FROM cmdb_relation AS relation
             JOIN cmdb_instance AS source
               ON source.id = relation.source_id AND source.deleted = 0
             JOIN cmdb_instance AS target
               ON target.id = relation.target_id AND target.deleted = 0
             WHERE relation.tenant_id = $4 AND relation.deleted = 0
               AND (relation.source_id = ANY($1) OR relation.target_id = ANY($1))
               AND NOT (relation.id = ANY($2))
             ORDER BY relation.id
             LIMIT $3",
        )
        .bind(&frontier)
        .bind(&seen_edges)
        .bind((remaining_edges + 1) as i64)
        .bind(tenant.id())
        .fetch_all(pool)
        .await
        .map_err(|_| AppError::internal("failed to read relation topology"))?;
        if rows.len() > remaining_edges {
            truncated = true;
        }

        let mut next_frontier = BTreeSet::new();
        for row in rows.into_iter().take(remaining_edges) {
            let id: i64 = row.get("id");
            let source_id: i64 = row.get("source_id");
            let target_id: i64 = row.get("target_id");
            for instance_id in [source_id, target_id] {
                if node_depths.contains_key(&instance_id) {
                    continue;
                }
                if node_depths.len() >= node_limit {
                    truncated = true;
                    continue;
                }
                node_depths.insert(instance_id, current_depth);
                next_frontier.insert(instance_id);
            }
            if node_depths.contains_key(&source_id) && node_depths.contains_key(&target_id) {
                edges.insert(id);
            }
        }
        frontier = next_frontier.into_iter().collect();
    }

    let node_ids: Vec<i64> = node_depths.keys().copied().collect();
    let edge_rows = sqlx::query(
        "SELECT relation.id, relation.source_id, relation.target_id, relation.relation
         FROM cmdb_relation AS relation
         JOIN cmdb_instance AS source
           ON source.id = relation.source_id AND source.deleted = 0
         JOIN cmdb_instance AS target
           ON target.id = relation.target_id AND target.deleted = 0
         WHERE relation.tenant_id = $3 AND relation.deleted = 0
           AND relation.source_id = ANY($1) AND relation.target_id = ANY($1)
         ORDER BY relation.id
         LIMIT $2",
    )
    .bind(&node_ids)
    .bind((edge_limit + 1) as i64)
    .bind(tenant.id())
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to read topology edges"))?;
    if edge_rows.len() > edge_limit {
        truncated = true;
    }
    let topology_edges: Vec<Value> = edge_rows
        .into_iter()
        .take(edge_limit)
        .map(|row| {
            json!({
                "id": row.get::<i64, _>("id"),
                "sourceId": row.get::<i64, _>("source_id"),
                "targetId": row.get::<i64, _>("target_id"),
                "relation": row.get::<String, _>("relation"),
            })
        })
        .collect();
    let rows = sqlx::query(
        "SELECT instance.id, instance.model_id, instance.attributes,
                model.name AS model_name, model.code AS model_code, model.unique_key
         FROM cmdb_instance AS instance
         JOIN cmdb_model AS model ON model.id = instance.model_id AND model.deleted = 0
         WHERE instance.id = ANY($1) AND instance.deleted = 0
         ORDER BY instance.id",
    )
    .bind(&node_ids)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to read topology instances"))?;
    let nodes: Vec<Value> = rows
        .iter()
        .map(|row| {
            let id: i64 = row.get("id");
            let attributes: Value = row.get("attributes");
            let unique_key: Option<String> = row.get("unique_key");
            json!({
                "id": id,
                "modelId": row.get::<i64, _>("model_id"),
                "modelName": row.get::<String, _>("model_name"),
                "modelCode": row.get::<String, _>("model_code"),
                "label": topology_label(&attributes, unique_key.as_deref(), id),
                "depth": node_depths.get(&id).copied().unwrap_or_default(),
                "root": id == root_id,
            })
        })
        .collect();
    Ok(json!({
        "rootId": root_id,
        "nodes": nodes,
        "edges": topology_edges,
        "truncated": truncated,
        "depth": max_depth,
        "limit": node_limit,
    }))
}

fn topology_label(attributes: &Value, unique_key: Option<&str>, id: i64) -> String {
    let candidates =
        unique_key
            .into_iter()
            .chain(["name", "hostname", "server_name", "ip", "title", "code"]);
    for key in candidates {
        let Some(value) = attributes.get(key) else {
            continue;
        };
        let label = match value {
            Value::String(text) => text.trim().to_string(),
            Value::Number(_) | Value::Bool(_) => value.to_string(),
            _ => String::new(),
        };
        if !label.is_empty() {
            return label;
        }
    }
    format!("CI #{id}")
}

async fn relation_bind(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let tenant = TenantContext::from_user(&user)?;
    let source_id = payload
        .get("sourceId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("sourceId is required"))?;
    let target_id = payload
        .get("targetId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("targetId is required"))?;
    if source_id == target_id {
        return Err(AppError::bad_request("cannot relate an instance to itself"));
    }
    let relation = payload
        .get("relation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("relates_to")
        .to_string();
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start relation bind"))?;
    // Endpoint row locks also serialize overlapping binds and instance deletes.
    let endpoints = sqlx::query("SELECT id FROM cmdb_instance WHERE id = ANY($1) AND tenant_id = $2 AND deleted = 0 ORDER BY id FOR UPDATE")
        .bind(vec![source_id, target_id]).bind(tenant.id()).fetch_all(&mut *tx).await
        .map_err(|_| AppError::internal("failed to lock relation endpoints"))?;
    if endpoints.len() != 2 {
        return Err(AppError::not_found("instance not found"));
    }
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_relation
         WHERE source_id = $1 AND target_id = $2 AND relation = $3 AND tenant_id = $4 AND deleted = 0",
    )
    .bind(source_id)
    .bind(target_id)
    .bind(&relation)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to check relation"))?;
    if duplicate > 0 {
        return Err(AppError::bad_request("relation already exists"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_relation (source_id, target_id, relation, creator, updater, tenant_id)
         VALUES ($1, $2, $3, $4, $4, $5) RETURNING id",
    )
    .bind(source_id)
    .bind(target_id)
    .bind(&relation)
    .bind(&user.username)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to create relation"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit relation bind"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn relation_unbind(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<InstanceIdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:instance:update")?;
    let tenant = TenantContext::from_user(&user)?;
    let result = sqlx::query(
        "UPDATE cmdb_relation SET deleted = 1, updater = $2
         WHERE id = $1 AND tenant_id = $3 AND deleted = 0",
    )
    .bind(params.instance_id)
    .bind(&user.username)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to remove relation"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("relation not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_topology_bounds() {
        assert_eq!(topology_bounds(None, None).unwrap(), (2, 80));
        assert_eq!(topology_bounds(Some(4), Some(200)).unwrap(), (4, 200));
        assert!(topology_bounds(Some(0), None).is_err());
        assert!(topology_bounds(Some(5), None).is_err());
        assert!(topology_bounds(None, Some(1)).is_err());
        assert!(topology_bounds(None, Some(201)).is_err());
    }

    #[test]
    fn labels_prefer_the_unique_key_then_common_names() {
        assert_eq!(
            topology_label(
                &json!({"asset_id": 42, "name": "server"}),
                Some("asset_id"),
                1
            ),
            "42"
        );
        assert_eq!(
            topology_label(&json!({"name": "server"}), None, 1),
            "server"
        );
        assert_eq!(topology_label(&json!({"nested": {}}), None, 7), "CI #7");
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
    async fn topology_is_cycle_safe_depth_bounded_and_excludes_deleted_endpoints() {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use std::{
            str::FromStr,
            time::{SystemTime, UNIX_EPOCH},
        };

        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
        let admin = sqlx::PgPool::connect(&url).await.unwrap();
        let schema = format!(
            "cmdb_topology_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&url)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();
        let ddl = include_str!("../../../../sql/postgresql/0009_cmdb_core.sql")
            .split("-- Menus:")
            .next()
            .unwrap()
            .replace("public.", "");
        sqlx::raw_sql(&ddl).execute(&pool).await.unwrap();
        let model_id: i64 = sqlx::query_scalar(
            "INSERT INTO cmdb_model (name, code, unique_key)
             VALUES ('Server', 'server', 'name') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let ids: Vec<i64> = sqlx::query_scalar(
            "INSERT INTO cmdb_instance (model_id, attributes)
             VALUES ($1, '{\"name\":\"a\"}'), ($1, '{\"name\":\"b\"}'),
                    ($1, '{\"name\":\"c\"}'), ($1, '{\"name\":\"d\"}')
             RETURNING id",
        )
        .bind(model_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cmdb_relation (source_id, target_id, relation)
             VALUES ($1, $2, 'depends_on'), ($3, $1, 'connects_to'),
                    ($2, $3, 'runs_on'), ($3, $4, 'contains')",
        )
        .bind(ids[0])
        .bind(ids[1])
        .bind(ids[2])
        .bind(ids[3])
        .execute(&pool)
        .await
        .unwrap();

        let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
        let one_hop = load_topology(&pool, &tenant, ids[0], 1, 20).await.unwrap();
        assert_eq!(one_hop["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(one_hop["edges"].as_array().unwrap().len(), 3);
        let two_hops = load_topology(&pool, &tenant, ids[0], 2, 20).await.unwrap();
        assert_eq!(two_hops["nodes"].as_array().unwrap().len(), 4);
        assert_eq!(two_hops["edges"].as_array().unwrap().len(), 4);
        let limited = load_topology(&pool, &tenant, ids[0], 2, 2).await.unwrap();
        assert_eq!(limited["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(limited["truncated"], true);

        sqlx::query("UPDATE cmdb_instance SET deleted = 1 WHERE id = $1")
            .bind(ids[1])
            .execute(&pool)
            .await
            .unwrap();
        let without_deleted = load_topology(&pool, &tenant, ids[0], 2, 20).await.unwrap();
        assert!(
            without_deleted["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["id"] != ids[1])
        );

        pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }
}
