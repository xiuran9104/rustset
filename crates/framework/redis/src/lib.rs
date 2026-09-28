//! Redis framework support: client wiring, JSON cache helpers, and rate limiting.

mod rate_limit;

use std::{env, time::Duration};

use anyhow::Context;
use redis::{AsyncCommands, Client};
use serde::{Serialize, de::DeserializeOwned};

pub use rate_limit::{RateLimitConfig, RateLimitState, rate_limit};

pub const DEFAULT_CACHE_PREFIX: &str = "rustset";

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub key_prefix: String,
    pub connect_timeout: Duration,
}

impl RedisConfig {
    pub fn from_env() -> Option<Self> {
        env::var("REDIS_URL").ok().map(|url| Self {
            url,
            key_prefix: env::var("REDIS_KEY_PREFIX")
                .unwrap_or_else(|_| DEFAULT_CACHE_PREFIX.to_string()),
            connect_timeout: Duration::from_secs(
                env::var("REDIS_CONNECT_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(3),
            ),
        })
    }
}

#[derive(Clone)]
pub struct RedisClient {
    client: Client,
    key_prefix: String,
}

impl RedisClient {
    pub async fn connect(config: &RedisConfig) -> anyhow::Result<Self> {
        let client = Client::open(config.url.as_str()).context("invalid redis url")?;
        let mut connection = tokio::time::timeout(
            config.connect_timeout,
            client.get_multiplexed_async_connection(),
        )
        .await
        .context("timed out connecting to redis")?
        .context("failed to connect to redis")?;
        let _: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .context("failed to ping redis")?;

        Ok(Self {
            client,
            key_prefix: config.key_prefix.clone(),
        })
    }

    pub fn key(&self, namespace: &str, id: impl AsRef<str>) -> String {
        format!("{}:{}:{}", self.key_prefix, namespace, id.as_ref())
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut connection).await?;
        Ok(())
    }

    pub async fn get_json<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let value: Option<String> = connection.get(key).await?;
        value
            .map(|value| serde_json::from_str(&value).context("failed to deserialize redis json"))
            .transpose()
    }

    pub async fn set_json<T>(&self, key: &str, value: &T, ttl: Duration) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let value = serde_json::to_string(value).context("failed to serialize redis json")?;
        let _: () = connection.set_ex(key, value, ttl.as_secs()).await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: usize = connection.del(key).await?;
        Ok(())
    }

    pub async fn delete_by_pattern(&self, pattern: &str) -> anyhow::Result<usize> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let mut cursor: u64 = 0;
        let mut deleted = 0;
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .cursor_arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut connection)
                .await?;
            if !keys.is_empty() {
                let count: usize = connection.del(keys).await?;
                deleted += count;
            }
            if next_cursor == 0 {
                break;
            }
            cursor = next_cursor;
        }
        Ok(deleted)
    }

    pub async fn increment_with_ttl(&self, key: &str, ttl: Duration) -> anyhow::Result<u64> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let count: u64 = connection.incr(key, 1).await?;
        if count == 1 {
            let _: bool = connection.expire(key, ttl.as_secs() as i64).await?;
        }
        Ok(count)
    }
}
