use std::time::Duration;

use rustset_framework_security::CurrentUser;
use tracing::warn;
use uuid::Uuid;

use crate::{SystemState, infrastructure};

// v3 invalidates authorization snapshots that still contain the removed BPM
// permissions. Ongoing role/menu changes still use the explicit invalidators.
const CURRENT_USER_CACHE_NAMESPACE: &str = "system:current-user:v3";
const CURRENT_USER_CACHE_TTL: Duration = Duration::from_secs(300);

pub async fn load_current_user(
    state: &SystemState,
    account: &infrastructure::UserAccount,
) -> anyhow::Result<CurrentUser> {
    let Some(cache) = &state.cache else {
        return infrastructure::load_current_user(&state.pool, account).await;
    };
    let key = cache.key(CURRENT_USER_CACHE_NAMESPACE, account.id.to_string());
    match cache.get_json::<CurrentUser>(&key).await {
        Ok(Some(user)) if user.tenant_id == account.tenant_id => return Ok(user),
        Ok(_) => {}
        Err(error) => warn!(%error, "failed to read current user cache"),
    }

    let user = infrastructure::load_current_user(&state.pool, account).await?;
    if let Err(error) = cache.set_json(&key, &user, CURRENT_USER_CACHE_TTL).await {
        warn!(%error, "failed to write current user cache");
    }
    Ok(user)
}

pub async fn invalidate_current_user(state: &SystemState, user_id: Uuid) {
    let Some(cache) = &state.cache else {
        return;
    };
    let key = cache.key(CURRENT_USER_CACHE_NAMESPACE, user_id.to_string());
    if let Err(error) = cache.delete(&key).await {
        warn!(%error, %user_id, "failed to invalidate current user cache");
    }
}

pub async fn invalidate_all_current_users(state: &SystemState) {
    let Some(cache) = &state.cache else {
        return;
    };
    let pattern = cache.key(CURRENT_USER_CACHE_NAMESPACE, "*");
    if let Err(error) = cache.delete_by_pattern(&pattern).await {
        warn!(%error, "failed to invalidate current user cache by pattern");
    }
}
