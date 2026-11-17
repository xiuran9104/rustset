use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::Response,
};

use crate::{CurrentUser, SecurityError, TokenService};

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = SecurityError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .ok_or(SecurityError::MissingCredentials)
    }
}

pub async fn authenticate(
    State(tokens): State<TokenService>,
    mut request: Request,
    next: Next,
) -> Result<Response, SecurityError> {
    // A gateway-level database authenticator may already have injected a
    // fresher CurrentUser than the authorization snapshot embedded in the JWT.
    if request.extensions().get::<CurrentUser>().is_some() {
        return Ok(next.run(request).await);
    }
    let authorization = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(SecurityError::MissingCredentials)?;
    let token = authorization
        .strip_prefix("Bearer ")
        .ok_or(SecurityError::InvalidCredentials)?;
    let claims = tokens.verify_access_token(token)?;

    request.extensions_mut().insert(claims.user);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn_with_state,
        routing::get,
    };
    use tower::ServiceExt;

    use crate::{
        CurrentUser, DataScope, PermissionSet, SecurityConfig, TokenService, authenticate,
    };

    fn token_service() -> TokenService {
        TokenService::new(
            SecurityConfig::new(
                "a-secret-with-at-least-thirty-two-bytes",
                "issuer",
                "audience",
                Duration::from_secs(60),
            )
            .unwrap(),
        )
    }

    async fn identity(user: CurrentUser) -> String {
        user.user_id
    }

    #[tokio::test]
    async fn authenticates_bearer_token_and_injects_user() {
        let tokens = token_service();
        let token = tokens
            .issue_access_token(CurrentUser {
                user_id: "user-1".into(),
                username: "reader".into(),
                tenant_id: None,
                role_codes: vec![],
                permissions: PermissionSet::default(),
                data_scope: DataScope::SelfOnly,
            })
            .unwrap();
        let app = Router::new()
            .route("/identity", get(identity))
            .layer(from_fn_with_state(tokens, authenticate));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/identity")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_missing_credentials_with_json_401() {
        let app = Router::new()
            .route("/identity", get(identity))
            .layer(from_fn_with_state(token_service(), authenticate));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/identity")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()["content-type"], "application/json");
    }
}
