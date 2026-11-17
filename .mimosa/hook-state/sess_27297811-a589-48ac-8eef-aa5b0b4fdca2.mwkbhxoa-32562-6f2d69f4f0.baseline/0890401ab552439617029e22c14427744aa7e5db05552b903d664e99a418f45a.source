use std::{
    collections::hash_map::DefaultHasher,
    env,
    hash::{Hash, Hasher},
    net::IpAddr,
    time::Duration,
};

use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustset_framework_common::ApiResponse;
use tracing::warn;

use crate::RedisClient;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub namespace: String,
    pub max_requests: u64,
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            namespace: "rate-limit".to_string(),
            max_requests: 300,
            window: Duration::from_secs(60),
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        Self {
            namespace: env::var("RATE_LIMIT_NAMESPACE").unwrap_or_else(|_| "rate-limit".into()),
            max_requests: env::var("RATE_LIMIT_MAX_REQUESTS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(300),
            window: Duration::from_secs(
                env::var("RATE_LIMIT_WINDOW_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(60),
            ),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitState {
    redis: RedisClient,
    config: RateLimitConfig,
}

impl RateLimitState {
    pub fn new(redis: RedisClient, config: RateLimitConfig) -> Self {
        Self { redis, config }
    }
}

pub async fn rate_limit(
    State(state): State<RateLimitState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    if path == "/health" || path.starts_with("/health/") {
        return next.run(request).await;
    }
    let actor = client_key(&request);
    let key = state.redis.key(
        &state.config.namespace,
        format!("{}:{}:{}", actor, method, path),
    );

    match state
        .redis
        .increment_with_ttl(&key, state.config.window)
        .await
    {
        Ok(count) if count > state.config.max_requests => {
            let body = ApiResponse {
                code: 429,
                data: (),
                message: "rate limit exceeded".to_string(),
            };
            (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response()
        }
        Ok(_) => next.run(request).await,
        Err(error) => {
            warn!(%error, "redis rate limit check failed");
            next.run(request).await
        }
    }
}

fn client_key(request: &Request) -> String {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            request
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|connect_info| match connect_info.0.ip() {
                    IpAddr::V4(ip) => ip.to_string(),
                    IpAddr::V6(ip) => ip.to_string(),
                })
        })
        .or_else(|| {
            request
                .headers()
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .map(|value| {
                    let mut hasher = DefaultHasher::new();
                    value.hash(&mut hasher);
                    format!("auth-{:x}", hasher.finish())
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};

    use super::client_key;

    #[test]
    fn uses_first_forwarded_for_ip() {
        let request = Request::builder()
            .header("x-forwarded-for", "10.0.0.1, 10.0.0.2")
            .body(Body::empty())
            .unwrap();

        assert_eq!(client_key(&request), "10.0.0.1");
    }

    #[test]
    fn separates_authenticated_clients_without_connect_info() {
        let first = Request::builder()
            .header("authorization", "Bearer first")
            .body(Body::empty())
            .unwrap();
        let second = Request::builder()
            .header("authorization", "Bearer second")
            .body(Body::empty())
            .unwrap();
        assert_ne!(client_key(&first), client_key(&second));
        assert!(client_key(&first).starts_with("auth-"));
    }
}
