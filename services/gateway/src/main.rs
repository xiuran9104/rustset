use aide::axum::{ApiRouter, routing::get};
use aide::openapi::{Components, Info, OpenApi, ReferenceOr, SecurityScheme, Server};
use axum::{Json, middleware::from_fn_with_state};
use indexmap::IndexMap;
use rustset_framework_common::{ApiResponse, ServiceConfig, health_route, init_tracing, serve};
use rustset_framework_database::{DatabaseConfig, connect, migrate};
use rustset_framework_redis::{RateLimitConfig, RateLimitState, RedisClient, RedisConfig};
use rustset_framework_security::{SecurityConfig, TokenService};
use rustset_framework_web::{AppError, WebConfig, apply_web_layers};
use schemars::JsonSchema;
use serde::Serialize;
use tracing::warn;

mod audit;
mod openapi;

const SERVICE_NAME: &str = "gateway";

#[derive(Debug, Serialize, JsonSchema)]
struct GatewayIndex {
    service: &'static str,
    modules: [&'static str; 4],
}

/// 按路径前缀归组，供 Scalar 侧边栏分组展示。
fn tag_for(path: &str) -> &'static str {
    const RULES: &[(&str, &str)] = &[
        ("/system/auth", "认证"),
        ("/system", "系统管理"),
        ("/infra", "资产与基础设施"),
        ("/cmdb", "CMDB"),
        ("/ai", "AI 大模型"),
        ("/chat/", "AI 开放接口"),
        ("/mj/", "AI 开放接口"),
        ("/identity", "AI 开放接口"),
        ("/login", "AI 开放接口"),
        ("/health", "运维"),
        ("/upload", "文件"),
        ("/", "运维"),
    ];
    RULES
        .iter()
        .find(|(prefix, _)| path.starts_with(prefix))
        .map(|(_, tag)| *tag)
        .unwrap_or("其他")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(SERVICE_NAME);

    let database = connect(&DatabaseConfig::from_env()?).await?;
    migrate(&database).await?;
    let redis = connect_redis().await;
    let tokens = TokenService::new(SecurityConfig::from_env()?);
    let system_state = rustset_system_server::SystemState::with_cache(
        database.clone(),
        tokens.clone(),
        redis.clone(),
    );
    let object_storage = rustset_infra_server::object_storage::ObjectStorage::from_env()
        .map_err(anyhow::Error::msg)?;
    object_storage
        .ensure_bucket()
        .await
        .map_err(anyhow::Error::msg)?;
    let infra_state = rustset_infra_server::InfraState::new(database.clone(), object_storage);
    let ai_state = rustset_ai_server::AiState::new(database.clone(), tokens);
    let cmdb_state = rustset_cmdb_server::CmdbState {
        pool: database.clone(),
    };
    system_state.bootstrap().await?;
    let database_auth = system_state.database_auth_state();

    let mut api_doc = OpenApi {
        info: Info {
            title: "RustSet Gateway API".to_string(),
            description: Some(
                "RustSet 资产、CMDB、云资源、运维与 AI 管理接口。\
                 文档由 aide 从路由与处理器类型推导生成，请求/响应结构以实际实现为准。"
                    .to_string(),
            ),
            version: "0.1.0".to_string(),
            ..Info::default()
        },
        servers: vec![Server {
            url: "/api".to_string(),
            ..Server::default()
        }],
        components: Some(Components {
            security_schemes: {
                let mut schemes = IndexMap::new();
                schemes.insert(
                    "bearerAuth".to_string(),
                    ReferenceOr::Item(SecurityScheme::Http {
                        scheme: "bearer".to_string(),
                        bearer_format: Some("JWT".to_string()),
                        description: Some("登录接口返回的访问令牌".to_string()),
                        extensions: Default::default(),
                    }),
                );
                schemes
            },
            ..Components::default()
        }),
        ..OpenApi::default()
    };

    let app = ApiRouter::new()
        .api_route("/", get(index))
        // 文档端点自身不进文档。
        .route("/openapi.json", get(openapi::document))
        .merge(rustset_system_server::routes(system_state))
        .merge(rustset_infra_server::routes(infra_state))
        .merge(rustset_ai_server::routes(ai_state))
        .merge(rustset_cmdb_server::routes(cmdb_state))
        .finish_api(&mut api_doc);

    // finish 之后统一补标签： aide 只登记方法与结构，不含业务分组。
    if let Some(paths) = api_doc.paths.as_mut() {
        for (route, reference) in paths.paths.iter_mut() {
            let ReferenceOr::Item(item) = reference else {
                continue;
            };
            let tag = tag_for(route);
            for operation in [
                &mut item.get,
                &mut item.put,
                &mut item.post,
                &mut item.delete,
                &mut item.patch,
                &mut item.head,
                &mut item.options,
                &mut item.trace,
            ] {
                if let Some(operation) = operation.as_mut() {
                    operation.tags.push(tag.to_string());
                }
            }
        }
    }
    openapi::publish(api_doc);

    let mut app = app
        .merge(health_route(SERVICE_NAME))
        .fallback(not_found)
        .layer(from_fn_with_state(
            audit::AuditState::new(database),
            audit::record,
        ))
        .layer(from_fn_with_state(
            database_auth,
            rustset_system_server::authenticate_from_database,
        ));

    if let Some(redis) = redis {
        app = app.layer(from_fn_with_state(
            RateLimitState::new(redis, RateLimitConfig::from_env()),
            rustset_framework_redis::rate_limit,
        ));
    }

    let app = apply_web_layers(app, WebConfig::from_env());

    serve(ServiceConfig::from_env(SERVICE_NAME, 8080), app).await
}

async fn connect_redis() -> Option<RedisClient> {
    let config = RedisConfig::from_env()?;
    match RedisClient::connect(&config).await {
        Ok(client) => Some(client),
        Err(error) => {
            warn!(%error, "redis is configured but unavailable; cache and rate limit disabled");
            None
        }
    }
}

async fn not_found() -> AppError {
    AppError::not_found("route not found")
}

async fn index() -> Json<ApiResponse<GatewayIndex>> {
    Json(ApiResponse::new(GatewayIndex {
        service: SERVICE_NAME,
        modules: ["system", "infra", "ai", "cmdb"],
    }))
}
