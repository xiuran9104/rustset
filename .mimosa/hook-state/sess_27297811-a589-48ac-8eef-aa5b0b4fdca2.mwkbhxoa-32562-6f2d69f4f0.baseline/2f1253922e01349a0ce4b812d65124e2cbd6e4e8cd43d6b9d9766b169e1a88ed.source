mod application;
mod audit;
pub mod bootstrap;
mod cache;
mod database_auth;
mod infrastructure;
mod management;
mod mcp;
mod oauth2_token;
mod transport;

use rustset_framework_database::DbPool;
use rustset_framework_redis::RedisClient;
use rustset_framework_security::{PasswordService, TokenService};

#[derive(Clone)]
pub struct SystemState {
    pool: DbPool,
    tokens: TokenService,
    passwords: PasswordService,
    cache: Option<RedisClient>,
}

impl SystemState {
    pub fn new(pool: DbPool, tokens: TokenService) -> Self {
        Self::with_cache(pool, tokens, None)
    }

    pub fn with_cache(pool: DbPool, tokens: TokenService, cache: Option<RedisClient>) -> Self {
        Self {
            pool,
            tokens,
            passwords: PasswordService::default(),
            cache,
        }
    }

    pub async fn bootstrap(&self) -> anyhow::Result<()> {
        bootstrap::initialize(self).await
    }

    pub fn database_auth_state(&self) -> DatabaseAuthState {
        DatabaseAuthState::new(self.clone())
    }
}

pub use database_auth::{DatabaseAuthState, authenticate_from_database};
pub use mcp::{authenticate_mcp_key, sha256_hex};
pub use transport::routes;
