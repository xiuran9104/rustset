use crate::{
    InfraState, QueryParams, TableSpec, id_param, ids_param, tenant_soft_delete,
    tenant_table_create, tenant_table_get, tenant_table_get_value, tenant_table_list,
    tenant_table_list_by_i64, tenant_table_page, tenant_table_update,
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
    CreateCloudPlatformRequest, CreateCloudProviderConfigRequest, CreateCloudZoneRequest,
    UpdateCloudPlatformRequest, UpdateCloudProviderConfigRequest, UpdateCloudZoneRequest,
};
use serde_json::{Value, json};
use std::collections::HashMap;

const CLOUD_ZONE: TableSpec = TableSpec {
    table: "infra_cloud_zone",
    seq: "infra_cloud_zone_seq",
};
const CLOUD_PLATFORM: TableSpec = TableSpec {
    table: "infra_cloud_platform",
    seq: "infra_cloud_platform_seq",
};
const CLOUD_PROVIDER_CONFIG: TableSpec = TableSpec {
    table: "infra_cloud_provider_config",
    seq: "infra_cloud_provider_config_seq",
};

fn tenant(user: &CurrentUser) -> Result<TenantContext, AppError> {
    TenantContext::from_user(user)
}

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/cloud-zone/page", get(zone_page))
        .api_route("/infra/cloud-zone/list", get(zone_list))
        .api_route("/infra/cloud-zone/get", get(zone_get))
        .api_route("/infra/cloud-zone/create", post(zone_create))
        .api_route("/infra/cloud-zone/update", put(zone_update))
        .api_route("/infra/cloud-zone/delete", delete(zone_delete))
        .api_route("/infra/cloud-zone/delete-list", delete(zone_delete_list))
        .api_route("/infra/cloud-platform/page", get(platform_page))
        .api_route("/infra/cloud-platform/list", get(platform_list))
        .api_route(
            "/infra/cloud-platform/list-by-zone",
            get(platform_list_by_zone),
        )
        .api_route("/infra/cloud-platform/get", get(platform_get))
        .api_route("/infra/cloud-platform/create", post(platform_create))
        .api_route("/infra/cloud-platform/update", put(platform_update))
        .api_route("/infra/cloud-platform/delete", delete(platform_delete))
        .api_route(
            "/infra/cloud-platform/delete-list",
            delete(platform_delete_list),
        )
        .api_route("/infra/cloud-provider-config/page", get(config_page))
        .api_route("/infra/cloud-provider-config/list", get(config_list))
        .api_route("/infra/cloud-provider-config/get", get(config_get))
        .api_route("/infra/cloud-provider-config/create", post(config_create))
        .api_route("/infra/cloud-provider-config/update", put(config_update))
        .api_route("/infra/cloud-provider-config/delete", delete(config_delete))
        .api_route(
            "/infra/cloud-provider-config/delete-list",
            delete(config_delete_list),
        )
        .api_route(
            "/infra/cloud-provider-config/test-connection",
            post(config_test_connection),
        )
}

async fn zone_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(&state.pool, tenant(&user)?.id(), CLOUD_ZONE, params).await
}
async fn zone_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&state.pool, tenant(&user)?.id(), CLOUD_ZONE).await
}
async fn zone_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_ZONE,
        id_param(&params)?,
    )
    .await
}
async fn zone_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<CreateCloudZoneRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_ZONE,
        crate::request_value(payload)?,
    )
    .await
}
async fn zone_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<UpdateCloudZoneRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_ZONE,
        crate::request_value(payload)?,
    )
    .await
}
async fn zone_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_ZONE.table,
        &[id_param(&params)?],
    )
    .await
}
async fn zone_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_ZONE.table,
        &ids_param(&params),
    )
    .await
}

async fn platform_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(&state.pool, tenant(&user)?.id(), CLOUD_PLATFORM, params).await
}
async fn platform_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&state.pool, tenant(&user)?.id(), CLOUD_PLATFORM).await
}
async fn platform_list_by_zone(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list_by_i64(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM,
        "zone_id",
        crate::id_named_param(&params, "zoneId")?,
    )
    .await
}
async fn platform_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM,
        id_param(&params)?,
    )
    .await
}
async fn platform_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<CreateCloudPlatformRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM,
        crate::request_value(payload)?,
    )
    .await
}
async fn platform_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<UpdateCloudPlatformRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM,
        crate::request_value(payload)?,
    )
    .await
}
async fn platform_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM.table,
        &[id_param(&params)?],
    )
    .await
}
async fn platform_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PLATFORM.table,
        &ids_param(&params),
    )
    .await
}

async fn config_page(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    tenant_table_page(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG,
        params,
    )
    .await
}
async fn config_list(
    State(state): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    tenant_table_list(&state.pool, tenant(&user)?.id(), CLOUD_PROVIDER_CONFIG).await
}
async fn config_get(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    tenant_table_get(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG,
        id_param(&params)?,
    )
    .await
}
async fn config_create(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<CreateCloudProviderConfigRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    tenant_table_create(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG,
        crate::request_value(payload)?,
    )
    .await
}
async fn config_update(
    State(state): State<InfraState>,
    user: CurrentUser,
    Json(payload): Json<UpdateCloudProviderConfigRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_table_update(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG,
        crate::request_value(payload)?,
    )
    .await
}
async fn config_delete(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG.table,
        &[id_param(&params)?],
    )
    .await
}
async fn config_delete_list(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    tenant_soft_delete(
        &state.pool,
        tenant(&user)?.id(),
        CLOUD_PROVIDER_CONFIG.table,
        &ids_param(&params),
    )
    .await
}

async fn config_test_connection(
    State(state): State<InfraState>,
    user: CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let id = id_param(&params)?;
    let tenant = tenant(&user)?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let config =
        tenant_table_get_value(&state.pool, tenant.id(), CLOUD_PROVIDER_CONFIG, id).await?;
    let ticket = json!({"cloudPlatformId": config.get("platformId")});
    let (target, _) =
        crate::ticket::load_platform_target(&state, &tenant, &ticket, Some(id)).await?;
    let template =
        rustset_framework_tofu::render_connection_check(&target).map_err(AppError::bad_request)?;
    let executor = rustset_framework_tofu::TofuExecutor::from_env();
    let workspace = executor
        .ensure_workspace(&format!("connection-{id}-{}", uuid::Uuid::new_v4()))
        .map_err(AppError::bad_request)?;
    rustset_framework_tofu::TofuExecutor::write_file(&workspace, "main.tf", &template)
        .map_err(AppError::bad_request)?;
    rustset_framework_tofu::TofuExecutor::write_file(
        &workspace,
        "probe.auto.tfvars.json",
        &json!({"region": target.region}).to_string(),
    )
    .map_err(AppError::bad_request)?;
    let init = executor.init(&workspace).await;
    let (success, message) = if init.success {
        let probe = executor
            .run(
                &["plan", "-input=false", "-no-color", "-lock-timeout=5s"],
                &rustset_framework_tofu::credential_env(&target),
                &workspace,
            )
            .await;
        if probe.success {
            (true, "连接成功：已通过云 API 查询区域/可用区")
        } else {
            (false, "云 API 查询失败，请检查凭据、区域、权限和网络")
        }
    } else {
        (
            false,
            "Provider 初始化失败，请检查 OpenTofu 安装和 Provider 下载网络",
        )
    };
    sqlx::query("UPDATE infra_cloud_provider_config SET last_test_time=$2::timestamp, last_test_result=$3, update_time=now() WHERE id=$1 AND tenant_id=$4 AND deleted=0")
        .bind(id).bind(&now).bind(message).bind(tenant.id()).execute(&state.pool).await.map_err(|_| AppError::internal("failed to save connection test"))?;
    // A probe never manages resources; only its uniquely created temporary files.
    let _ = std::fs::remove_dir_all(&workspace);
    Ok(Json(ApiResponse::new(
        json!({"success": success, "message": message, "testedAt": now}),
    )))
}
