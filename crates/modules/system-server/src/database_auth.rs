use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use rustset_framework_security::{CurrentUser, SecurityError};
use uuid::Uuid;

use crate::{SystemState, cache, infrastructure};

#[derive(Clone)]
pub struct DatabaseAuthState {
    state: SystemState,
}

impl DatabaseAuthState {
    pub(crate) fn new(state: SystemState) -> Self {
        Self { state }
    }
}

/// Validates bearer tokens against RustSet system storage and replaces JWT-embedded
/// authorization data with the user's current roles and permissions.
/// Requests without a bearer token continue so public routes remain public;
/// their route-level authentication still rejects missing credentials where required.
pub async fn authenticate_from_database(
    State(auth): State<DatabaseAuthState>,
    mut request: Request,
    next: Next,
) -> Result<Response, SecurityError> {
    let Some(authorization) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return Ok(next.run(request).await);
    };
    let token = authorization
        .strip_prefix("Bearer ")
        .ok_or(SecurityError::InvalidCredentials)?;
    let claims = auth.state.tokens.verify_access_token(token)?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| SecurityError::InvalidCredentials)?;

    let is_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
             SELECT 1
             FROM system_oauth2_access_token access
             JOIN system_users users ON users.id = access.user_id
             WHERE access.access_token = $1
               AND users.identity_uuid = $2
               AND access.deleted = 0
               AND access.expires_time > now()
               AND users.deleted = 0
               AND users.status = 0
         )",
    )
    .bind(token)
    .bind(user_id)
    .fetch_one(&auth.state.pool)
    .await
    .map_err(|_| SecurityError::InvalidCredentials)?;
    if !is_active {
        return Err(SecurityError::InvalidCredentials);
    }

    let account = infrastructure::find_account_by_id(&auth.state.pool, user_id)
        .await
        .map_err(|_| SecurityError::InvalidCredentials)?
        .ok_or(SecurityError::InvalidCredentials)?;
    let mut current_user: CurrentUser = cache::load_current_user(&auth.state, &account)
        .await
        .map_err(|_| SecurityError::InvalidCredentials)?;
    // Tenant ownership is authoritative in the freshly loaded account, not in
    // a cached authorization snapshot (which may predate a tenant transfer).
    current_user.tenant_id = account.tenant_id.clone();
    request.extensions_mut().insert(current_user);
    Ok(next.run(request).await)
}
