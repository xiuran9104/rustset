//! 对外 MCP 服务的 API Key 鉴权：密钥哈希定位 → 绑定用户 → CurrentUser。
//! 角色权限与租户隔离完全继承绑定用户（以数据库最新状态为准）。

use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{SystemState, cache, infrastructure};

pub fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// 校验 MCP API Key 并加载绑定用户的最新角色/权限/租户上下文。
/// 密钥不匹配、已禁用、已过期或绑定用户被停用/删除时拒绝。
pub async fn authenticate_mcp_key(
    state: &SystemState,
    api_key: &str,
) -> Result<CurrentUser, AppError> {
    let key = api_key.trim();
    if key.len() < 20 {
        return Err(AppError::unauthorized("invalid mcp api key"));
    }
    let hash = sha256_hex(key);
    let identity = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT u.identity_uuid
         FROM system_mcp_api_key k
         JOIN system_users u ON u.id = k.user_id AND u.deleted = 0 AND u.status = 0
         WHERE k.api_key_hash = $1 AND k.deleted = 0 AND k.enabled = 1
           AND (k.expires_time IS NULL OR k.expires_time > now())",
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to verify mcp api key"))?
    .flatten()
    .ok_or_else(|| AppError::unauthorized("invalid mcp api key"))?;

    // 尽力而为地记录最后使用时间；失败不影响鉴权结果。
    let _ = sqlx::query(
        "UPDATE system_mcp_api_key SET last_used_time = now()
         WHERE api_key_hash = $1 AND deleted = 0",
    )
    .bind(&hash)
    .execute(&state.pool)
    .await;

    let account = infrastructure::find_account_by_id(&state.pool, identity)
        .await
        .map_err(|_| AppError::internal("failed to load mcp account"))?
        .ok_or_else(|| AppError::unauthorized("invalid mcp api key"))?;
    let mut current_user = cache::load_current_user(state, &account)
        .await
        .map_err(|_| AppError::internal("failed to load mcp permissions"))?;
    // 与 database_auth 相同：租户归属以数据库中的最新账号为准。
    current_user.tenant_id = account.tenant_id.clone();
    Ok(current_user)
}
