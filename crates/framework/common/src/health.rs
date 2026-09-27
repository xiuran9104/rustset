use axum::{Json, routing::get};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: &'static str,
    pub checked_at: DateTime<Utc>,
}

pub fn health_route(service_name: &'static str) -> axum::Router {
    let handler = move || async move {
        Json(HealthResponse {
            service: service_name.to_string(),
            status: "ok",
            checked_at: Utc::now(),
        })
    };
    axum::Router::new()
        .route("/health", get(handler.clone()))
        .route("/health/live", get(handler))
}
