use axum::{Json, http::StatusCode, response::IntoResponse};
use rustset_framework_common::ApiResponse;

use crate::AccessDenied;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityError {
    MissingCredentials,
    InvalidCredentials,
    PermissionDenied,
}

impl From<AccessDenied> for SecurityError {
    fn from(_: AccessDenied) -> Self {
        Self::PermissionDenied
    }
}

impl IntoResponse for SecurityError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::MissingCredentials => (StatusCode::UNAUTHORIZED, 401, "authentication required"),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, 401, "invalid or expired token"),
            Self::PermissionDenied => (StatusCode::FORBIDDEN, 403, "permission denied"),
        };
        let body = ApiResponse {
            code,
            data: (),
            message: message.into(),
        };
        (status, Json(body)).into_response()
    }
}
