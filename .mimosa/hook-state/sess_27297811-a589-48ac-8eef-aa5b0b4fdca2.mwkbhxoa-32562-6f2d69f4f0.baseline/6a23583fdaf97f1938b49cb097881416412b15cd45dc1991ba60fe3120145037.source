use chrono::{Duration, Utc};
use rustset_system_api::{LoginRequest, RefreshTokenRequest, TokenResponse};
use tracing::warn;
use uuid::Uuid;

use crate::{SystemState, audit, cache, infrastructure, oauth2_token};

const MAX_FAILED_LOGIN_ATTEMPTS: i32 = 5;
const LOGIN_LOCK_DURATION_MINUTES: i64 = 15;

pub enum LoginError {
    InvalidCredentials,
    Disabled,
    Locked,
    Internal,
}

pub async fn login(
    state: &SystemState,
    request: LoginRequest,
) -> Result<TokenResponse, LoginError> {
    let username = request.username.trim();
    if username.is_empty() || request.password.is_empty() {
        return Err(LoginError::InvalidCredentials);
    }

    let account =
        infrastructure::find_account_by_username(&state.pool, username, request.tenant_id)
            .await
            .map_err(|_| LoginError::Internal)?
            .ok_or(LoginError::InvalidCredentials)?;

    if account.status == "disabled" {
        return Err(LoginError::Disabled);
    }
    if is_locked(&account) {
        return Err(LoginError::Locked);
    }

    let password_matches = state
        .passwords
        .verify(&request.password, &account.password_hash)
        .map_err(|_| LoginError::Internal)?;
    if !password_matches {
        record_failed_login(state, &account).await?;
        return Err(LoginError::InvalidCredentials);
    }

    infrastructure::record_successful_login(&state.pool, account.id)
        .await
        .map_err(|_| LoginError::Internal)?;
    let response = issue_token_pair(state, account.id).await?;
    audit::record_login(state, Some(account.id), &account.username, 100, 0).await;
    Ok(response)
}

#[derive(Debug)]
pub enum RefreshError {
    Invalid,
    Disabled,
    Locked,
    Internal,
}

pub async fn refresh(
    state: &SystemState,
    request: RefreshTokenRequest,
) -> Result<TokenResponse, RefreshError> {
    let token = request.refresh_token.trim();
    if token.is_empty() {
        return Err(RefreshError::Invalid);
    }

    let user_id = oauth2_token::verify_refresh_token(&state.pool, token)
        .await
        .map_err(refresh_error)?;
    let account = infrastructure::find_account_by_id(&state.pool, user_id)
        .await
        .map_err(|_| RefreshError::Internal)?
        .ok_or(RefreshError::Invalid)?;

    if account.status == "disabled" {
        return Err(RefreshError::Disabled);
    }
    if is_locked(&account) {
        return Err(RefreshError::Locked);
    }

    oauth2_token::revoke_refresh_token(&state.pool, token)
        .await
        .map_err(refresh_error)?;
    let response = issue_token_pair(state, user_id)
        .await
        .map_err(|_| RefreshError::Internal)?;
    audit::record(
        state,
        None,
        "refresh",
        "system.oauth2_token",
        Some(user_id.to_string()),
        serde_json::json!({ "username": account.username }),
    )
    .await;
    Ok(response)
}

pub async fn logout(state: &SystemState, refresh_token: Option<&str>) -> Result<(), RefreshError> {
    if let Some(token) = refresh_token
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let user_id = oauth2_token::revoke_refresh_token(&state.pool, token)
            .await
            .map_err(refresh_error)?;
        audit::record_login(state, Some(user_id), "", 200, 0).await;
    }
    Ok(())
}

async fn issue_token_pair(state: &SystemState, user_id: Uuid) -> Result<TokenResponse, LoginError> {
    let account = infrastructure::find_account_by_id(&state.pool, user_id)
        .await
        .map_err(|_| LoginError::Internal)?
        .ok_or(LoginError::InvalidCredentials)?;
    let user = cache::load_current_user(state, &account)
        .await
        .map_err(|_| LoginError::Internal)?;
    let access_token = state
        .tokens
        .issue_access_token(user)
        .map_err(|_| LoginError::Internal)?;
    let refresh_token = oauth2_token::create_token_pair(
        &state.pool,
        user_id,
        &access_token,
        state.tokens.access_token_ttl_seconds(),
    )
    .await
    .map_err(|error| {
        warn!(?error, "failed to persist OAuth2 token pair");
        LoginError::Internal
    })?;

    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: state.tokens.access_token_ttl_seconds(),
    })
}

async fn record_failed_login(
    state: &SystemState,
    account: &infrastructure::UserAccount,
) -> Result<(), LoginError> {
    let attempts = account.failed_login_attempts.saturating_add(1);
    let locked_until = (attempts >= MAX_FAILED_LOGIN_ATTEMPTS)
        .then(|| Utc::now() + Duration::minutes(LOGIN_LOCK_DURATION_MINUTES));
    infrastructure::record_failed_login(&state.pool, account.id, attempts, locked_until)
        .await
        .map_err(|_| LoginError::Internal)
}

fn is_locked(account: &infrastructure::UserAccount) -> bool {
    if let Some(locked_until) = account.locked_until {
        return locked_until > Utc::now();
    }
    account.status == "locked"
}

fn refresh_error(error: oauth2_token::TokenError) -> RefreshError {
    match error {
        oauth2_token::TokenError::Invalid => RefreshError::Invalid,
        oauth2_token::TokenError::Internal => RefreshError::Internal,
    }
}
