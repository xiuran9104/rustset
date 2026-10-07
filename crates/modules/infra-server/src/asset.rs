mod csv;
mod policy_risk;
mod repository;
use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, tenant_soft_delete,
    tenant_table_create, tenant_table_get, tenant_table_list, tenant_table_page,
    tenant_table_update,
};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use rustset_infra_api::{
    AssetPortRequest, CreateNetworkPolicyRequest, DiscoverAssetsRequest, DiscoverAssetsResponse,
    UpdateNetworkPolicyRequest,
};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashMap;

const ASSET: TableSpec = TableSpec {
    table: "infra_asset",
    seq: "infra_asset_seq",
};
const CLOUD_ASSET: TableSpec = TableSpec {
    table: "infra_cloud_asset",
    seq: "infra_cloud_asset_seq",
};
const CLOUD_RESOURCE: TableSpec = TableSpec {
    table: "infra_cloud_resource",
    seq: "infra_cloud_resource_seq",
};
const PHYSICAL_RESOURCE: TableSpec = TableSpec {
    table: "infra_physical_resource",
    seq: "infra_physical_resource_seq",
};
const NETWORK_POLICY: TableSpec = TableSpec {
    table: "infra_network_policy",
    seq: "infra_network_policy_seq",
};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .merge(csv::routes())
        .api_route("/infra/asset/page", get(asset_page))
        .api_route("/infra/asset/list", get(asset_list))
        .api_route("/infra/asset/get", get(asset_get))
        .api_route("/infra/asset/create", post(asset_create))
        .api_route("/infra/asset/update", put(asset_update))
        .api_route("/infra/asset/delete", delete(asset_delete))
        .api_route("/infra/asset/delete-list", delete(asset_delete_list))
        .api_route("/infra/asset/discover", post(asset_discover))
        .api_route("/infra/asset/sync-cmdb", post(asset_sync_cmdb))
        .api_route("/infra/asset/{id}/port/add", post(asset_add_port))
        .api_route("/infra/asset/{id}/port/{port}", put(asset_update_port))
        .api_route("/infra/asset/{id}/port/{port}", delete(asset_delete_port))
        .api_route("/infra/network-policy/page", get(network_policy_page))
        .api_route("/infra/network-policy/list", get(network_policy_list))
        .api_route("/infra/network-policy/get", get(network_policy_get))
        .api_route("/infra/network-policy/create", post(network_policy_create))
        .api_route("/infra/network-policy/update", put(network_policy_update))
        .api_route(
            "/infra/network-policy/delete",
            delete(network_policy_delete),
        )
        .api_route(
            "/infra/network-policy/delete-list",
            delete(network_policy_delete_list),
        )
        .api_route("/infra/cloud-asset/page", get(cloud_asset_page))
        .api_route("/infra/cloud-asset/list", get(cloud_asset_list))
        .api_route("/infra/cloud-asset/discover", post(cloud_asset_discover))
        .api_route("/infra/cloud-asset/get", get(cloud_asset_get))
        .api_route("/infra/cloud-asset/create", post(cloud_asset_create))
        .api_route("/infra/cloud-asset/update", put(cloud_asset_update))
        .api_route("/infra/cloud-asset/delete", delete(cloud_asset_delete))
        .api_route("/infra/cloud-resource/page", get(cloud_resource_page))
        .api_route("/infra/cloud-resource/list", get(cloud_resource_list))
        .api_route("/infra/cloud-resource/get", get(cloud_resource_get))
        .api_route("/infra/cloud-resource/create", post(cloud_resource_create))
        .api_route("/infra/cloud-resource/update", put(cloud_resource_update))
        .api_route(
            "/infra/network-policy/recheck-risks",
            post(network_policy_recheck_risks),
        )
        .api_route(
            "/infra/cloud-resource/delete",
            delete(cloud_resource_delete),
        )
        .api_route(
            "/infra/cloud-resource/delete-list",
            delete(cloud_resource_delete_list),
        )
        .api_route("/infra/physical-resource/page", get(physical_resource_page))
        .api_route("/infra/physical-resource/list", get(physical_resource_list))
        .api_route("/infra/physical-resource/get", get(physical_resource_get))
        .api_route(
            "/infra/physical-resource/create",
            post(physical_resource_create),
        )
        .api_route(
            "/infra/physical-resource/update",
            put(physical_resource_update),
        )
        .api_route(
            "/infra/physical-resource/delete",
            delete(physical_resource_delete),
        )
        .api_route(
            "/infra/physical-resource/delete-list",
            delete(physical_resource_delete_list),
        )
}

async fn network_policy_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::page(&state.pool, &tenant, p).await
}
async fn network_policy_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::list(&state.pool, &tenant).await
}
async fn network_policy_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    Ok(Json(ApiResponse::new(
        repository::get(&state.pool, NETWORK_POLICY, &tenant, id_param(&p)?).await?,
    )))
}
async fn network_policy_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<CreateNetworkPolicyRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::create(&state.pool, &tenant, crate::request_value(p)?).await
}
async fn network_policy_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<UpdateNetworkPolicyRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::update(&state.pool, &tenant, crate::request_value(p)?).await
}
async fn network_policy_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::delete(&state.pool, &tenant, &[id_param(&p)?]).await
}
async fn network_policy_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::delete(&state.pool, &tenant, &ids_param(&p)).await
}

async fn network_policy_recheck_risks(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<policy_risk::RiskMatchSummary>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    policy_risk::recheck(&state.pool, &tenant).await
}

async fn asset_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    repository::page(&state.pool, ASSET, &tenant, p).await
}
async fn asset_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    repository::list(&state.pool, ASSET, &tenant).await
}
async fn asset_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    Ok(Json(ApiResponse::new(
        repository::get(&state.pool, ASSET, &tenant, id_param(&p)?).await?,
    )))
}
async fn asset_sync_cmdb(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<crate::asset_cmdb_sync::SyncSummary>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let summary = crate::asset_cmdb_sync::sync_assets(&state.pool, &tenant, &user.username).await?;
    Ok(Json(ApiResponse::new(summary)))
}

async fn asset_discover(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<DiscoverAssetsRequest>,
) -> Result<Json<ApiResponse<DiscoverAssetsResponse>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let target_text = payload.ips.join(",");
    let targets = crate::task::parse_targets(&target_text)?;
    let ports = crate::task::normalize_ports(payload.ports)?;
    if ports.len() > 64 {
        return Err(AppError::bad_request("资产发现每次最多探测 64 个端口"));
    }
    let count = targets.len();
    let task_id = crate::task::enqueue(
        &state.pool,
        &tenant,
        &user.username,
        "资产发现",
        &target_text,
        "asset_discovery",
        "custom",
        targets,
        ports.clone(),
        payload.idempotency_key.as_deref(),
        payload.max_attempts,
        payload.timeout_seconds,
        &json!({}),
    )
    .await?;
    Ok(Json(ApiResponse::new(DiscoverAssetsResponse {
        task_id,
        targets: count,
        ports,
    })))
}
async fn asset_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let Json(created) = repository::create(&state.pool, ASSET, &tenant, p).await?;
    if let Ok(asset_id) = created.data.parse::<i64>() {
        auto_attribute_ownership(&state.pool, &tenant, asset_id).await?;
    }
    Ok(Json(created))
}

/// Longest-prefix segment match over cmdb_net_zone; fills net_zone_id and
/// organization_name when the IP falls inside a managed segment and the
/// ownership is not manually pinned.
pub(crate) async fn auto_attribute_ownership(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    asset_id: i64,
) -> Result<(), AppError> {
    let Some((asset_id, ip)) = sqlx::query_as::<_, (i64, String)>(
        // Skip only assets whose organization a human chose; the default
        // 'manual' marker with an empty organization still gets attributed.
        "SELECT id, ip FROM infra_asset WHERE id = $1 AND tenant_id = $2 AND deleted = 0
           AND NOT (ownership_source = 'manual'
                    AND organization_name IS NOT NULL AND organization_name <> '')",
    )
    .bind(asset_id)
    .bind(tenant.id())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read asset ownership"))?
    else {
        return Ok(());
    };
    let segments = sqlx::query(
        "SELECT id, cidr FROM cmdb_net_zone WHERE deleted = 0 AND tenant_id = $1 AND cidr IS NOT NULL AND cidr <> ''",
    )
    .bind(tenant.id())
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to read network segments"))?;
    let mut best: Option<(u8, i64)> = None;
    for row in &segments {
        let cidr: String = row.get("cidr");
        let Some((address, prefix)) = cidr.split_once('/') else {
            continue;
        };
        let (Ok(network), Ok(prefix)) = (
            address.trim().parse::<std::net::Ipv4Addr>(),
            prefix.trim().parse::<u8>(),
        ) else {
            continue;
        };
        if prefix > 32 {
            continue;
        }
        let Ok(ip_addr) = ip.trim().parse::<std::net::Ipv4Addr>() else {
            return Ok(());
        };
        let mask = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix as u32)
        };
        if (u32::from(network) & mask) == (u32::from(ip_addr) & mask)
            && best
                .as_ref()
                .map(|(best_prefix, _)| prefix > *best_prefix)
                .unwrap_or(true)
        {
            best = Some((prefix, row.get("id")));
        }
    }
    let Some((_, zone_id)) = best else {
        return Ok(());
    };
    // Attribute the nearest company/subsidiary ancestor's name, falling
    // back to the matched segment itself.
    sqlx::query(
        "UPDATE infra_asset a
         SET net_zone_id = $2,
             organization_name = COALESCE((
                 WITH RECURSIVE ancestors AS (
                     SELECT id, name, zone_type, parent_id FROM cmdb_net_zone WHERE id = $2 AND tenant_id = $3
                     UNION
                     SELECT z.id, z.name, z.zone_type, z.parent_id
                     FROM cmdb_net_zone z JOIN ancestors an ON z.id = an.parent_id WHERE z.tenant_id = $3
                 )
                 SELECT name FROM ancestors
                 WHERE zone_type IN ('company','subsidiary')
                 ORDER BY id LIMIT 1
             ), (SELECT name FROM cmdb_net_zone WHERE id = $2 AND tenant_id = $3), a.organization_name),
             ownership_source = 'segment',
             update_time = now()
         WHERE a.id = $1 AND a.tenant_id = $3 AND a.deleted = 0",
    )
    .bind(asset_id)
    .bind(zone_id)
    .bind(tenant.id())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update asset ownership"))?;
    Ok(())
}
async fn asset_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = p
        .get("id")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let _ = repository::update(&state.pool, ASSET, &tenant, p).await?;
    auto_attribute_ownership(&state.pool, &tenant, id).await?;
    Ok(Json(ApiResponse::new(())))
}
async fn asset_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    repository::delete(&state.pool, ASSET, &tenant, &[id_param(&p)?]).await
}
async fn asset_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    repository::delete(&state.pool, ASSET, &tenant, &ids_param(&p)).await
}

async fn asset_add_port(
    State(state): State<InfraState>,
    user: CurrentUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<AssetPortRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let port_num = payload
        .port
        .filter(|port| (1..=65535).contains(port))
        .ok_or_else(|| AppError::bad_request("port must be between 1 and 65535"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to edit ports"))?;
    let existing = sqlx::query_scalar::<_,Value>("SELECT to_jsonb(a) FROM infra_asset a WHERE id=$1 AND tenant_id=$2 AND deleted=0 FOR UPDATE")
        .bind(id).bind(tenant.id()).fetch_optional(&mut *tx).await.map_err(|_|AppError::internal("failed to read asset"))?
        .ok_or_else(||AppError::not_found("asset not found"))?;
    let mut ports: Vec<Value> = existing
        .get("ports")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    if ports
        .iter()
        .any(|p| p.get("port").and_then(|v| v.as_i64()) == Some(port_num as i64))
    {
        return Err(AppError::bad_request("Port already exists"));
    }
    ports.push(json!({"port": port_num, "isOpen": true, "service": payload.service, "banner": payload.banner, "isBound": payload.is_bound.unwrap_or(false), "systemName": payload.system_name, "middleware": payload.middleware}));
    let ports_json = serde_json::to_string(&ports).unwrap_or_default();
    sqlx::query("UPDATE infra_asset SET ports=$2, update_time=now() WHERE id=$1 AND tenant_id=$3 AND deleted=0")
        .bind(id)
        .bind(&ports_json)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to add port"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit ports"))?;
    Ok(Json(ApiResponse::new(json!({"id": id, "ports": ports}))))
}

async fn asset_update_port(
    State(state): State<InfraState>,
    user: CurrentUser,
    axum::extract::Path((id, port_num)): axum::extract::Path<(i64, i32)>,
    Json(payload): Json<AssetPortRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to edit ports"))?;
    let existing = sqlx::query_scalar::<_,Value>("SELECT to_jsonb(a) FROM infra_asset a WHERE id=$1 AND tenant_id=$2 AND deleted=0 FOR UPDATE")
        .bind(id).bind(tenant.id()).fetch_optional(&mut *tx).await.map_err(|_|AppError::internal("failed to read asset"))?
        .ok_or_else(||AppError::not_found("asset not found"))?;
    let mut ports: Vec<Value> = existing
        .get("ports")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    if let Some(p) = ports
        .iter_mut()
        .find(|p| p.get("port").and_then(|v| v.as_i64()) == Some(port_num as i64))
    {
        let is_bound = payload
            .is_bound
            .unwrap_or_else(|| p.get("isBound").and_then(Value::as_bool).unwrap_or(false));
        *p = json!({"port": port_num, "isOpen": true, "service": payload.service, "banner": payload.banner, "isBound": is_bound, "systemName": payload.system_name, "middleware": payload.middleware});
    }
    let ports_json = serde_json::to_string(&ports).unwrap_or_default();
    sqlx::query("UPDATE infra_asset SET ports=$2, update_time=now() WHERE id=$1 AND tenant_id=$3 AND deleted=0")
        .bind(id)
        .bind(&ports_json)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit ports"))?;
    Ok(Json(ApiResponse::new(json!({"id": id, "ports": ports}))))
}

async fn asset_delete_port(
    State(state): State<InfraState>,
    user: CurrentUser,
    axum::extract::Path((id, port_num)): axum::extract::Path<(i64, i32)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to edit ports"))?;
    let existing = sqlx::query_scalar::<_,Value>("SELECT to_jsonb(a) FROM infra_asset a WHERE id=$1 AND tenant_id=$2 AND deleted=0 FOR UPDATE")
        .bind(id).bind(tenant.id()).fetch_optional(&mut *tx).await.map_err(|_|AppError::internal("failed to read asset"))?
        .ok_or_else(||AppError::not_found("asset not found"))?;
    let mut ports: Vec<Value> = existing
        .get("ports")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    ports.retain(|p| p.get("port").and_then(|v| v.as_i64()) != Some(port_num as i64));
    let ports_json = serde_json::to_string(&ports).unwrap_or_default();
    sqlx::query("UPDATE infra_asset SET ports=$2, update_time=now() WHERE id=$1 AND tenant_id=$3 AND deleted=0")
        .bind(id)
        .bind(&ports_json)
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed"))?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit ports"))?;
    Ok(Json(ApiResponse::new(json!({"id": id, "ports": ports}))))
}

async fn cloud_asset_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET,
        p,
    )
    .await
}
async fn cloud_asset_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET,
    )
    .await
}

async fn cloud_asset_discover(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let rows = sqlx::query("SELECT id, ecs_name, ecs_status, resource_id, cloud_region, cloud_category, cloud_provider_config_id, platform_name, instance_id, ecs_type, ecs_os, cpu_cores, memory_gb, ip_address, remarks FROM infra_cloud_resource WHERE tenant_id=$1 AND deleted = 0 ORDER BY id")
        .bind(tenant.id()).fetch_all(&state.pool).await.map_err(|_| AppError::internal("failed to read cloud resources"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start cloud discovery"))?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(tenant.id())
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to lock cloud discovery"))?;
    let mut created = 0_i64;
    let mut updated = 0_i64;
    for source in rows {
        let instance_id: String = source.get("instance_id");
        let config_id: Option<i64> = source.get("cloud_provider_config_id");
        let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM infra_cloud_asset WHERE instance_id = $1 AND cloud_provider_config_id IS NOT DISTINCT FROM $2 AND tenant_id=$3 AND deleted = 0 FOR UPDATE")
            .bind(&instance_id).bind(config_id).bind(tenant.id()).fetch_optional(&mut *tx).await.map_err(|_| AppError::internal("failed to match cloud asset"))?;
        if let Some(id) = existing {
            sqlx::query("UPDATE infra_cloud_asset SET provider_type=$2, platform_name=$3, region_id=$4, name=$5, status=$6, private_ip=$7, cpu_cores=$8, memory_gb=$9, instance_type=$10, os_name=$11, raw_payload=$12, synced_at=to_char(now(),'YYYY-MM-DD HH24:MI:SS'), updater=$13, update_time=now() WHERE id=$1 AND tenant_id=$14")
                .bind(id).bind(source.get::<String, _>("cloud_category")).bind(source.get::<Option<String>, _>("platform_name")).bind(source.get::<String, _>("cloud_region")).bind(source.get::<String, _>("ecs_name")).bind(source.get::<String, _>("ecs_status")).bind(source.get::<String, _>("ip_address")).bind(source.get::<i32, _>("cpu_cores")).bind(source.get::<i32, _>("memory_gb")).bind(source.get::<String, _>("ecs_type")).bind(source.get::<String, _>("ecs_os")).bind(source.get::<Option<String>, _>("remarks")).bind(&user.username).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to update discovered cloud asset"))?;
            updated += 1;
        } else {
            sqlx::query("INSERT INTO infra_cloud_asset (cloud_provider_config_id, provider_type, platform_name, region_id, instance_id, name, status, private_ip, cpu_cores, memory_gb, instance_type, os_name, raw_payload, synced_at, creator, updater, tenant_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,to_char(now(),'YYYY-MM-DD HH24:MI:SS'),$14,$14,$15)")
                .bind(config_id).bind(source.get::<String, _>("cloud_category")).bind(source.get::<Option<String>, _>("platform_name")).bind(source.get::<String, _>("cloud_region")).bind(&instance_id).bind(source.get::<String, _>("ecs_name")).bind(source.get::<String, _>("ecs_status")).bind(source.get::<String, _>("ip_address")).bind(source.get::<i32, _>("cpu_cores")).bind(source.get::<i32, _>("memory_gb")).bind(source.get::<String, _>("ecs_type")).bind(source.get::<String, _>("ecs_os")).bind(source.get::<Option<String>, _>("remarks")).bind(&user.username).bind(tenant.id()).execute(&mut *tx).await.map_err(|_| AppError::internal("failed to create discovered cloud asset"))?;
            created += 1;
        }
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit cloud discovery"))?;
    Ok(Json(ApiResponse::new(
        json!({"source": "infra_cloud_resource", "created": created, "updated": updated, "total": created + updated}),
    )))
}
async fn cloud_asset_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET,
        id_param(&p)?,
    )
    .await
}
async fn cloud_asset_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET,
        p,
    )
    .await
}
async fn cloud_asset_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET,
        p,
    )
    .await
}
async fn cloud_asset_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_ASSET.table,
        &[id_param(&p)?],
    )
    .await
}

async fn cloud_resource_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE,
        p,
    )
    .await
}
async fn cloud_resource_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE,
    )
    .await
}
async fn cloud_resource_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE,
        id_param(&p)?,
    )
    .await
}
async fn cloud_resource_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(mut p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    if let Some(object) = p.as_object_mut() {
        normalize_security_product(object);
        let resource_id = uuid::Uuid::new_v4().to_string();
        object
            .entry("resourceId".to_string())
            .or_insert(Value::String(resource_id.clone()));
        object
            .entry("instanceId".to_string())
            .or_insert(Value::String(resource_id));
    }
    tenant_table_create(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE,
        p,
    )
    .await
}
async fn cloud_resource_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(mut p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    if let Some(object) = p.as_object_mut() {
        normalize_security_product(object);
    }
    tenant_table_update(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE,
        p,
    )
    .await
}
async fn cloud_resource_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE.table,
        &[id_param(&p)?],
    )
    .await
}
async fn cloud_resource_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        CLOUD_RESOURCE.table,
        &ids_param(&p),
    )
    .await
}

async fn physical_resource_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE,
        p,
    )
    .await
}
async fn physical_resource_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE,
    )
    .await
}
async fn physical_resource_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE,
        id_param(&p)?,
    )
    .await
}
async fn physical_resource_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(mut p): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    if let Some(object) = p.as_object_mut() {
        normalize_security_product(object);
    }
    tenant_table_create(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE,
        p,
    )
    .await
}
async fn physical_resource_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(mut p): Json<Value>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    if let Some(object) = p.as_object_mut() {
        normalize_security_product(object);
    }
    tenant_table_update(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE,
        p,
    )
    .await
}
async fn physical_resource_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE.table,
        &[id_param(&p)?],
    )
    .await
}
async fn physical_resource_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        TenantContext::from_user(&user)?.id(),
        PHYSICAL_RESOURCE.table,
        &ids_param(&p),
    )
    .await
}

/// `hasSecurityProduct` arrives as a boolean from the form but persists as an
/// integer flag.
fn normalize_security_product(object: &mut serde_json::Map<String, Value>) {
    if let Some(Value::Bool(enabled)) = object.get("hasSecurityProduct").cloned() {
        object.insert(
            "hasSecurityProduct".to_string(),
            Value::Number((enabled as i32).into()),
        );
    }
}
