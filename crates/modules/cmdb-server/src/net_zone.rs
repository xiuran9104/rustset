//! Organization network zones: a company / subsidiary / department tree
//! carrying CIDR segments. Asset intake matches an IP against the most
//! specific segment (longest prefix) to auto-fill ownership, and asset
//! pages can jump to the network policies covering the asset's IPs.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::net::Ipv4Addr;

use crate::{CmdbState, require};

pub fn routes() -> ApiRouter<CmdbState> {
    ApiRouter::new()
        .api_route("/cmdb/net-zone/tree", get(net_zone_tree))
        .api_route("/cmdb/net-zone/list", get(net_zone_list))
        .api_route("/cmdb/net-zone/get", get(net_zone_get))
        .api_route("/cmdb/net-zone/create", post(net_zone_create))
        .api_route("/cmdb/net-zone/update", put(net_zone_update))
        .api_route("/cmdb/net-zone/delete", delete(net_zone_delete))
        .api_route("/cmdb/net-zone/resolve", post(net_zone_resolve))
        .api_route("/cmdb/net-zone/identify-assets", post(identify_assets))
        .api_route("/cmdb/net-zone/policies-by-ip", get(policies_by_ip))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NetZonePageParams {
    #[serde(rename = "parentId", default)]
    parent_id: Option<i64>,
    #[serde(rename = "tenantId", default)]
    tenant_id: Option<i64>,
}

fn tenant_id_for(user: &CurrentUser, requested: Option<i64>) -> Result<i64, AppError> {
    let current = user
        .tenant_id
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok());
    if user.role_codes.iter().any(|role| role == "super_admin") {
        return requested
            .or(current)
            .ok_or_else(|| AppError::bad_request("tenantId is required"));
    }
    let current = current.ok_or_else(|| AppError::forbidden("tenant context is required"))?;
    if requested.is_some_and(|tenant_id| tenant_id != current) {
        return Err(AppError::forbidden(
            "cannot access another tenant's net zones",
        ));
    }
    Ok(current)
}

fn parse_cidr(cidr: &str) -> Option<(Ipv4Addr, u8)> {
    let (address, prefix) = cidr.split_once('/')?;
    let address: Ipv4Addr = address.trim().parse().ok()?;
    let prefix: u8 = prefix.trim().parse().ok()?;
    (prefix <= 32).then_some((address, prefix))
}

/// True when `ip` falls inside `cidr` (v4 only; the ledger stores v4 here).
pub fn cidr_contains(cidr: &str, ip: &str) -> bool {
    let Some((network, prefix)) = parse_cidr(cidr) else {
        return false;
    };
    let Ok(ip_addr) = ip.trim().parse::<Ipv4Addr>() else {
        return false;
    };
    let network_bits = u32::from(network);
    let ip_bits = u32::from(ip_addr);
    if prefix == 0 {
        return true;
    }
    let mask = u32::MAX << (32 - prefix as u32);
    (network_bits & mask) == (ip_bits & mask)
}

async fn net_zone_tree(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<NetZonePageParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:net-zone:query")?;
    let tenant_id = tenant_id_for(&user, params.tenant_id)?;
    let rows = sqlx::query(
        "SELECT id, name, parent_id, zone_type, cidr, sort, description
         FROM cmdb_net_zone WHERE deleted = 0 AND tenant_id = $1 ORDER BY sort, id",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read net zones"))?;
    let mut nodes: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.get::<i64, _>("id"),
                "name": row.get::<String, _>("name"),
                "parentId": row.get::<i64, _>("parent_id"),
                "zoneType": row.get::<String, _>("zone_type"),
                "cidr": row.get::<Option<String>, _>("cidr"),
                "sort": row.get::<i32, _>("sort"),
                "description": row.get::<Option<String>, _>("description"),
                "children": [],
            })
        })
        .collect();
    // Build the tree bottom-up: promote child nodes into their parents
    // until only roots remain. Parent ids arrive as node payload copies.
    let node_count = nodes.len();
    let parent_of: Vec<i64> = nodes
        .iter()
        .map(|node| node.get("parentId").and_then(Value::as_i64).unwrap_or(0))
        .collect();
    let mut promoted = vec![false; node_count];
    for _ in 0..node_count {
        let mut changed = false;
        let index_of_id = |nodes: &[Value], id: i64| -> Option<usize> {
            nodes
                .iter()
                .position(|node| node.get("id").and_then(Value::as_i64) == Some(id))
        };
        for child_index in 0..node_count {
            if promoted[child_index] {
                continue;
            }
            let parent_id = parent_of[child_index];
            if parent_id == 0 {
                continue;
            }
            let Some(parent_index) = index_of_id(&nodes, parent_id) else {
                continue;
            };
            if parent_index == child_index || promoted[parent_index] {
                continue;
            }
            let child = nodes[child_index].clone();
            let parent_children = nodes[parent_index].as_object_mut().map(|object| {
                object
                    .entry("children")
                    .or_insert_with(|| Value::Array(vec![]))
            });
            if let Some(children) = parent_children.and_then(Value::as_array_mut) {
                children.push(child);
                promoted[child_index] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut roots = Vec::new();
    for (index, node) in nodes.into_iter().enumerate() {
        if !promoted[index] {
            roots.push(node);
        }
    }
    let nodes = roots;
    Ok(Json(ApiResponse::new(json!({ "tree": nodes }))))
}

async fn net_zone_list(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<NetZonePageParams>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:net-zone:query")?;
    let tenant_id = tenant_id_for(&user, params.tenant_id)?;
    let rows = sqlx::query(
        "SELECT id, name, parent_id, zone_type, cidr, sort, description
         FROM cmdb_net_zone WHERE deleted = 0 AND tenant_id = $1
           AND ($2::bigint IS NULL OR parent_id = $2)
         ORDER BY sort, id",
    )
    .bind(tenant_id)
    .bind(params.parent_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read net zones"))?;
    Ok(Json(ApiResponse::new(
        rows.iter()
            .map(|row| {
                json!({
                    "id": row.get::<i64, _>("id"),
                    "name": row.get::<String, _>("name"),
                    "parentId": row.get::<i64, _>("parent_id"),
                    "zoneType": row.get::<String, _>("zone_type"),
                    "cidr": row.get::<Option<String>, _>("cidr"),
                    "sort": row.get::<i32, _>("sort"),
                    "description": row.get::<Option<String>, _>("description"),
                })
            })
            .collect(),
    )))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    id: i64,
    #[serde(rename = "tenantId", default)]
    tenant_id: Option<i64>,
}

async fn net_zone_get(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:net-zone:query")?;
    let tenant_id = tenant_id_for(&user, params.tenant_id)?;
    let row = sqlx::query(
        "SELECT id, name, parent_id, zone_type, cidr, sort, description
         FROM cmdb_net_zone WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(params.id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read net zone"))?
    .ok_or_else(|| AppError::not_found("net zone not found"))?;
    Ok(Json(ApiResponse::new(json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "parentId": row.get::<i64, _>("parent_id"),
        "zoneType": row.get::<String, _>("zone_type"),
        "cidr": row.get::<Option<String>, _>("cidr"),
        "sort": row.get::<i32, _>("sort"),
        "description": row.get::<Option<String>, _>("description"),
    }))))
}

async fn net_zone_create(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "cmdb:net-zone:create")?;
    let tenant_id = tenant_id_for(&user, payload.get("tenantId").and_then(Value::as_i64))?;
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("name is required"))?;
    let zone_type = payload
        .get("zoneType")
        .and_then(Value::as_str)
        .unwrap_or("company");
    if !matches!(
        zone_type,
        "company" | "subsidiary" | "department" | "segment"
    ) {
        return Err(AppError::bad_request(
            "zoneType must be company / subsidiary / department / segment",
        ));
    }
    let cidr = payload.get("cidr").and_then(Value::as_str).map(str::trim);
    if let Some(cidr) = cidr.filter(|value| !value.is_empty())
        && parse_cidr(cidr).is_none()
    {
        return Err(AppError::bad_request(
            "cidr must be a valid IPv4 CIDR, e.g. 10.1.0.0/16",
        ));
    }
    let parent_id = payload
        .get("parentId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .unwrap_or(0);
    if parent_id > 0 {
        let parent_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cmdb_net_zone
             WHERE id = $1 AND tenant_id = $2 AND deleted = 0)",
        )
        .bind(parent_id)
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::internal("failed to check parent"))?;
        if !parent_exists {
            return Err(AppError::not_found("parent net zone not found"));
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_net_zone
         (tenant_id, name, parent_id, zone_type, cidr, sort, description, creator, updater)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) RETURNING id",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(parent_id)
    .bind(zone_type)
    .bind(cidr.filter(|value| !value.is_empty()))
    .bind(payload.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
    .bind(payload.get("description").and_then(Value::as_str))
    .bind(&user.username)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to create net zone"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

async fn net_zone_update(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:net-zone:update")?;
    let tenant_id = tenant_id_for(&user, payload.get("tenantId").and_then(Value::as_i64))?;
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    if payload.get("parentId").and_then(Value::as_i64) == Some(id) {
        return Err(AppError::bad_request("a net zone cannot be its own parent"));
    }
    let cidr = payload.get("cidr").and_then(Value::as_str).map(str::trim);
    if let Some(cidr) = cidr.filter(|value| !value.is_empty())
        && parse_cidr(cidr).is_none()
    {
        return Err(AppError::bad_request("cidr must be a valid IPv4 CIDR"));
    }
    let result = sqlx::query(
        "UPDATE cmdb_net_zone SET name = COALESCE($2, name), parent_id = COALESCE($3, parent_id),
                zone_type = COALESCE($4, zone_type), cidr = $5, sort = COALESCE($6, sort),
                description = $7, updater = $8, update_time = now()
         WHERE id = $1 AND tenant_id = $9 AND deleted = 0",
    )
    .bind(id)
    .bind(
        payload
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty()),
    )
    .bind(
        payload
            .get("parentId")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0),
    )
    .bind(payload.get("zoneType").and_then(Value::as_str))
    .bind(cidr.filter(|value| !value.is_empty()))
    .bind(
        payload
            .get("sort")
            .and_then(Value::as_i64)
            .map(|sort| sort as i32),
    )
    .bind(payload.get("description").and_then(Value::as_str))
    .bind(&user.username)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to update net zone"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("net zone not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn net_zone_delete(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<IdParams>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require(&user, "cmdb:net-zone:delete")?;
    let tenant_id = tenant_id_for(&user, params.tenant_id)?;
    let has_children: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cmdb_net_zone
         WHERE parent_id = $1 AND tenant_id = $2 AND deleted = 0",
    )
    .bind(params.id)
    .bind(tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to check children"))?;
    if has_children > 0 {
        return Err(AppError::bad_request("请先删除其子节点"));
    }
    let result = sqlx::query(
        "UPDATE cmdb_net_zone SET deleted = 1, updater = $2, update_time = now()
         WHERE id = $1 AND tenant_id = $3 AND deleted = 0",
    )
    .bind(params.id)
    .bind(&user.username)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to delete net zone"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("net zone not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

/// Longest-prefix match of one IP against all segments.
async fn resolve_segment(
    state: &CmdbState,
    tenant_id: i64,
    ip: &str,
) -> Result<Option<(i64, String, String)>, AppError> {
    let rows = sqlx::query(
        "SELECT id, name, zone_type, cidr FROM cmdb_net_zone
         WHERE deleted = 0 AND tenant_id = $1 AND cidr IS NOT NULL AND cidr <> ''",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read segments"))?;
    let mut best: Option<(u8, i64, String, String)> = None;
    for row in rows {
        let cidr = row.get::<Option<String>, _>("cidr").unwrap_or_default();
        if !cidr_contains(&cidr, ip) {
            continue;
        }
        let prefix = parse_cidr(&cidr).map(|(_, prefix)| prefix).unwrap_or(0);
        if best
            .as_ref()
            .map(|(best_prefix, _, _, _)| prefix > *best_prefix)
            .unwrap_or(true)
        {
            best = Some((
                prefix,
                row.get::<i64, _>("id"),
                row.get::<String, _>("name"),
                row.get::<String, _>("zone_type"),
            ));
        }
    }
    Ok(best.map(|(_, id, name, zone_type)| (id, name, zone_type)))
}

/// The org name for a segment: nearest company/subsidiary ancestor's name.
async fn organization_for_zone(state: &CmdbState, tenant_id: i64, mut zone_id: i64) -> String {
    for _ in 0..8 {
        let row = sqlx::query(
            "SELECT name, zone_type, parent_id FROM cmdb_net_zone
             WHERE id = $1 AND tenant_id = $2 AND deleted = 0",
        )
        .bind(zone_id)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await;
        let Ok(Some(row)) = row else { break };
        let zone_type: String = row.get("zone_type");
        let name: String = row.get("name");
        if zone_type == "company" || zone_type == "subsidiary" {
            return name;
        }
        match row.try_get::<i64, _>("parent_id") {
            Ok(parent) if parent > 0 => zone_id = parent,
            _ => break,
        }
    }
    String::new()
}

async fn net_zone_resolve(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:net-zone:query")?;
    let tenant_id = tenant_id_for(&user, payload.get("tenantId").and_then(Value::as_i64))?;
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("ip is required"))?;
    match resolve_segment(&state, tenant_id, ip).await? {
        Some((zone_id, name, zone_type)) => {
            let organization = organization_for_zone(&state, tenant_id, zone_id).await;
            Ok(Json(ApiResponse::new(json!({
                "matched": true,
                "netZoneId": zone_id,
                "netZoneName": name,
                "zoneType": zone_type,
                "organization": organization,
            }))))
        }
        None => Ok(Json(ApiResponse::new(json!({ "matched": false })))),
    }
}

/// Re-attribute all assets whose ownership is not manually pinned.
async fn identify_assets(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Json(_payload): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "cmdb:net-zone:update")?;
    let tenant_id = rustset_framework_tenant::TenantContext::from_user(&user)?.id();
    if _payload
        .get("tenantId")
        .and_then(Value::as_i64)
        .is_some_and(|id| id != tenant_id)
    {
        return Err(AppError::forbidden(
            "cannot reattribute another tenant's assets",
        ));
    }
    let assets = sqlx::query(
        "SELECT id, ip FROM infra_asset
         WHERE deleted = 0 AND tenant_id = $1
           AND NOT (ownership_source = 'manual'
                    AND organization_name IS NOT NULL AND organization_name <> '')",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read assets"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start asset attribution"))?;
    let mut matched = 0i64;
    let mut unmatched = 0i64;
    for asset in &assets {
        let id: i64 = asset.get("id");
        let ip: String = asset.get("ip");
        match resolve_segment(&state, tenant_id, &ip).await? {
            Some((zone_id, _, _)) => {
                let organization = organization_for_zone(&state, tenant_id, zone_id).await;
                sqlx::query(
                    "UPDATE infra_asset SET net_zone_id = $2, organization_name = $3,
                            ownership_source = 'segment', update_time = now()
                     WHERE id = $1 AND tenant_id = $4",
                )
                .bind(id)
                .bind(zone_id)
                .bind(organization)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| AppError::internal("failed to attribute asset"))?;
                matched += 1;
            }
            None => {
                sqlx::query(
                    "UPDATE infra_asset SET net_zone_id = NULL, ownership_source = 'segment',
                            update_time = now()
                     WHERE id = $1 AND tenant_id = $2",
                )
                .bind(id)
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| AppError::internal("failed to clear asset attribution"))?;
                unmatched += 1;
            }
        }
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit asset attribution"))?;
    Ok(Json(ApiResponse::new(
        json!({ "matched": matched, "unmatched": unmatched }),
    )))
}

/// Network policies whose source or destination IP covers the asset IP —
/// backing the asset-page jump (需求 D).
#[derive(Debug, Deserialize, JsonSchema)]
struct PoliciesByIpParams {
    ip: String,
    #[serde(default)]
    limit: Option<i64>,
}

async fn policies_by_ip(
    State(state): State<CmdbState>,
    user: CurrentUser,
    Query(params): Query<PoliciesByIpParams>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&user, "cmdb:net-zone:query")?;
    let tenant = rustset_framework_tenant::TenantContext::from_user(&user)?;
    let ip = params.ip.trim();
    if ip.is_empty() {
        return Err(AppError::bad_request("ip is required"));
    }
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let rows = sqlx::query(
        "SELECT id, firewall_name, source_ip, destination_ip, service_port,
                traffic_direction, action, applicant, application_date, relation
         FROM (
             SELECT id, firewall_name, source_ip, destination_ip, service_port,
                    traffic_direction, action, applicant, application_date,
                    'source'::text AS relation
             FROM infra_network_policy
             WHERE deleted = 0 AND tenant_id = $3 AND source_ip ILIKE $1
             UNION ALL
             SELECT id, firewall_name, source_ip, destination_ip, service_port,
                    traffic_direction, action, applicant, application_date,
                    'destination'::text
             FROM infra_network_policy
             WHERE deleted = 0 AND tenant_id = $3 AND destination_ip ILIKE $1
         ) matched
         ORDER BY id LIMIT $2",
    )
    .bind(format!("%{ip}%"))
    .bind(limit)
    .bind(tenant.id())
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to search policies"))?;
    Ok(Json(ApiResponse::new(
        rows.iter()
            .map(|row| {
                json!({
                    "id": row.get::<i64, _>("id"),
                    "firewallName": row.get::<String, _>("firewall_name"),
                    "sourceIp": row.get::<String, _>("source_ip"),
                    "destinationIp": row.get::<String, _>("destination_ip"),
                    "servicePort": row.get::<String, _>("service_port"),
                    "trafficDirection": row.get::<String, _>("traffic_direction"),
                    "action": row.get::<String, _>("action"),
                    "applicant": row.get::<String, _>("applicant"),
                    "applicationDate": row.get::<chrono::NaiveDate, _>("application_date").to_string(),
                    "relation": row.get::<String, _>("relation"),
                })
            })
            .collect(),
    )))
}

#[cfg(test)]
mod tests {
    use super::cidr_contains;

    #[test]
    fn matches_prefixes() {
        assert!(cidr_contains("10.1.0.0/16", "10.1.2.3"));
        assert!(cidr_contains("10.1.0.0/16", "10.1.255.255"));
        assert!(!cidr_contains("10.1.0.0/16", "10.2.0.1"));
        assert!(cidr_contains("10.1.2.0/24", "10.1.2.3"));
        assert!(!cidr_contains("10.1.3.0/24", "10.1.2.3"));
        assert!(cidr_contains("0.0.0.0/0", "8.8.8.8"));
        assert!(cidr_contains("192.168.1.7/32", "192.168.1.7"));
        assert!(!cidr_contains("192.168.1.7/32", "192.168.1.8"));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(!cidr_contains("not-a-cidr", "10.1.1.1"));
        assert!(!cidr_contains("10.1.0.0/33", "10.1.1.1"));
        assert!(!cidr_contains("10.1.0.0/16", "not-an-ip"));
        assert!(!cidr_contains("10.1.0.0/16", "2001:db8::1"));
    }
}
