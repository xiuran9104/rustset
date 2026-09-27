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
mod runtime_health;

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
    let redis_config = RedisConfig::from_env();
    let redis_configured = redis_config.is_some();
    let cache_required = env_bool("CACHE_REDIS_REQUIRED", false);
    let rate_limit_required = env_bool("RATE_LIMIT_REDIS_REQUIRED", false);
    let cache_enabled = env_bool("CACHE_REDIS_ENABLED", redis_configured) || cache_required;
    let rate_limit_enabled =
        env_bool("RATE_LIMIT_REDIS_ENABLED", redis_configured) || rate_limit_required;
    let cache_redis = connect_redis(
        redis_config.as_ref(),
        "cache",
        cache_enabled,
        cache_required,
    )
    .await?;
    let rate_limit_redis = connect_redis(
        redis_config.as_ref(),
        "rate_limit",
        rate_limit_enabled,
        rate_limit_required,
    )
    .await?;
    let tokens = TokenService::new(SecurityConfig::from_env()?);
    let system_state = rustset_system_server::SystemState::with_cache(
        database.clone(),
        tokens.clone(),
        cache_redis.clone(),
    );
    let infra_state = rustset_infra_server::InfraState::new(database.clone());
    infra_state.start_workers();
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

    let audit_state = audit::AuditState::new(database.clone());
    let runtime_health = runtime_health::RuntimeHealthState::new(
        database.clone(),
        runtime_health::Dependency {
            configured: cache_enabled,
            required: cache_required,
            client: cache_redis,
        },
        runtime_health::Dependency {
            configured: rate_limit_enabled,
            required: rate_limit_required,
            client: rate_limit_redis.clone(),
        },
        audit_state.metrics(),
    );
    let mut app = app
        .merge(health_route(SERVICE_NAME))
        .merge(runtime_health::routes(runtime_health))
        .fallback(not_found)
        .layer(from_fn_with_state(audit_state, audit::record))
        .layer(from_fn_with_state(
            database_auth,
            rustset_system_server::authenticate_from_database,
        ));

    if let Some(redis) = rate_limit_redis {
        app = app.layer(from_fn_with_state(
            RateLimitState::new(redis, RateLimitConfig::from_env()),
            rustset_framework_redis::rate_limit,
        ));
    }

    let app = apply_web_layers(app, WebConfig::from_env());

    serve(ServiceConfig::from_env(SERVICE_NAME, 8080), app).await
}

async fn connect_redis(
    config: Option<&RedisConfig>,
    purpose: &'static str,
    enabled: bool,
    required: bool,
) -> anyhow::Result<Option<RedisClient>> {
    if !enabled {
        return Ok(None);
    }
    let Some(config) = config else {
        if required {
            anyhow::bail!("REDIS_URL is required for {purpose}");
        }
        warn!(purpose, "redis feature enabled without REDIS_URL; disabled");
        return Ok(None);
    };
    let client = match RedisClient::connect(config).await {
        Ok(client) => Some(client),
        Err(error) => {
            if required {
                return Err(anyhow::anyhow!(
                    "required redis {purpose} is unavailable: {error}"
                ));
            }
            warn!(%error, purpose, "redis dependency unavailable; feature disabled");
            None
        }
    };
    Ok(client)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
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
