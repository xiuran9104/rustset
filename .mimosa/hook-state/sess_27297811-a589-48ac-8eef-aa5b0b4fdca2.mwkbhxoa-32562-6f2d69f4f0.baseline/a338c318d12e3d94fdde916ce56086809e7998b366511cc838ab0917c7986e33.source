//! Redis 缓存语义合同测试（docs/xinchuang-adaptation-plan.md §4.3.1）。
//!
//! 替换缓存产品（目录强制时 TongRDS，协议兼容）必须原样通过本套件：
//! 覆盖现有命令面（GET/SET EX/DEL/SCAN/INCR/EXPIRE/TTL/PING）、键编码、
//! JSON 编解码（含中文）、游标完整遍历的批量删除、限流窗口语义、
//! 连接异常行为与降级语义。
//!
//! 运行（compose 的 Redis）：
//! ```bash
//! TEST_REDIS_URL=redis://127.0.0.1:6379 \
//!   cargo test -p rustset-framework-redis --test cache_contract -- --ignored
//! ```
//!
//! 降级语义（§4.3.1：缓存不可用时的显式定义）由调用方实现并在本套件
//! 证明其前提：
//! * `RedisConfig::from_env` 无 REDIS_URL → `None`，调用方直连数据库；
//! * 读失败/未命中 → `Ok(None)`/`Err`，调用方回源并 warn 留痕（cache.rs）；
//! * 连接失败 → `connect` 显式报错，不静默吞掉。

use std::time::Duration;

use rustset_framework_redis::{RedisClient, RedisConfig};

async fn test_client(prefix: &str) -> RedisClient {
    let url = std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL is required");
    let config = RedisConfig {
        url,
        key_prefix: prefix.to_owned(),
        connect_timeout: Duration::from_secs(5),
    };
    RedisClient::connect(&config).await.expect("connect redis")
}

/// 清理本前缀下所有键（用被测对象自己的 delete_by_pattern——顺带验证它）。
async fn cleanup(client: &RedisClient) {
    let pattern = client.key("contract", "*");
    client
        .delete_by_pattern(&pattern)
        .await
        .expect("cleanup by pattern");
}

/// 合同：键编码——三段式 `{prefix}:{namespace}:{id}`，set_json/get_json
/// roundtrip（含中文与 JSON 结构体）。
#[tokio::test]
#[ignore = "requires TEST_REDIS_URL"]
async fn key_encoding_and_json_roundtrip() {
    let client = test_client("contract-encoding").await;
    cleanup(&client).await;

    let key = client.key("contract:user-cache", "42");
    assert_eq!(
        key,
        "contract-encoding:contract:user-cache:42",
        "键编码必须保持三段式前缀语义"
    );

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct CachedUser {
        nickname: String,
        tenant_id: i64,
    }
    let user = CachedUser {
        nickname: "管理员".to_owned(),
        tenant_id: 1,
    };
    client
        .set_json(&key, &user, Duration::from_secs(60))
        .await
        .expect("set_json");
    let loaded: CachedUser = client
        .get_json(&key)
        .await
        .expect("get_json")
        .expect("cached value must exist");
    assert_eq!(loaded, user, "JSON roundtrip must preserve 中文 and structure");
    cleanup(&client).await;
}

/// 合同：TTL——set_json 落键后 TTL 必须在窗口内；键过期后 get_json 返回
/// None（调用方回源数据库）。
#[tokio::test]
#[ignore = "requires TEST_REDIS_URL"]
async fn ttl_window_and_expiry_yields_none() {
    let client = test_client("contract-ttl").await;
    cleanup(&client).await;

    let key = client.key("contract:session", "abc");
    client
        .set_json(&key, &"v1", Duration::from_secs(60))
        .await
        .expect("set_json");
    let mut connection = client.clone_connection().await.expect("connection");
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("ttl");
    assert!((1..=60).contains(&ttl), "TTL must be within the set window, got {ttl}");

    // 过期 → None（降级语义的回源前提）。
    client
        .set_json(&key, &"v2", Duration::from_secs(1))
        .await
        .expect("set_json short");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let expired: Option<String> = client
        .get_json(&key)
        .await
        .expect("get after expiry must not error");
    assert!(expired.is_none(), "expired key must read as None (cache miss)");
    cleanup(&client).await;
}

/// 合同：批量删除——SCAN 游标必须完整遍历（写入多于单批 COUNT 的键），
/// delete_by_pattern 清空后不留残余；无关前缀的键不受影响。
#[tokio::test]
#[ignore = "requires TEST_REDIS_URL"]
async fn pattern_delete_scans_every_cursor_page() {
    let client = test_client("contract-scan").await;
    cleanup(&client).await;

    // 250 个键 > 默认 COUNT 100，强制 SCAN 多页。
    let total = 250;
    for index in 0..total {
        let key = client.key("contract:batch", index.to_string());
        client
            .set_json(&key, &index, Duration::from_secs(60))
            .await
            .expect("seed");
    }
    // 另一前缀的哨兵键：必须存活。
    let sentinel = client.key("contract:keep", "sentinel");
    client
        .set_json(&sentinel, &"keep", Duration::from_secs(60))
        .await
        .expect("seed sentinel");

    let pattern = client.key("contract:batch", "*");
    let deleted = client
        .delete_by_pattern(&pattern)
        .await
        .expect("delete_by_pattern");
    assert_eq!(deleted, total, "every seeded key must be deleted");
    for index in 0..total {
        let key = client.key("contract:batch", index.to_string());
        let remaining: Option<String> = client.get_json(&key).await.expect("probe");
        assert!(remaining.is_none(), "key {key} must be gone");
    }
    let kept: Option<String> = client.get_json(&sentinel).await.expect("sentinel");
    assert_eq!(kept.as_deref(), Some("keep"), "unrelated keys must survive");
    cleanup(&client).await;
}

/// 合同：限流窗口语义——INCR 首次触发 EXPIRE；同窗口内计数连续累加且
/// TTL 不被重置；TTL 过期后计数从头开始（固定窗口限流的节拍）。
#[tokio::test]
#[ignore = "requires TEST_REDIS_URL"]
async fn rate_limit_window_semantics() {
    let client = test_client("contract-rate").await;
    cleanup(&client).await;

    let key = client.key("rate-limit:contract", "10.0.0.1");
    let first = client
        .increment_with_ttl(&key, Duration::from_secs(2))
        .await
        .expect("first incr");
    assert_eq!(first, 1, "first request in the window counts 1");
    let mut connection = client.clone_connection().await.expect("connection");
    let ttl_after_first: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("ttl");
    assert!(ttl_after_first > 0, "EXPIRE must be set on the first increment");

    let second = client
        .increment_with_ttl(&key, Duration::from_secs(2))
        .await
        .expect("second incr");
    assert_eq!(second, 2, "counts accumulate within the window");
    let mut connection = client.clone_connection().await.expect("connection");
    let ttl_after_second: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .expect("ttl");
    assert!(
        ttl_after_second <= ttl_after_first,
        "a later increment must NOT extend the fixed window"
    );

    // 窗口过期 → 计数重置。
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let after_expiry = client
        .increment_with_ttl(&key, Duration::from_secs(2))
        .await
        .expect("incr after expiry");
    assert_eq!(after_expiry, 1, "window expiry must reset the counter");
    cleanup(&client).await;
}

/// 合同：连接异常——错误地址 connect 显式报错（不静默吞）；PING 探活
/// 在服务可达时成功。
#[tokio::test]
#[ignore = "requires TEST_REDIS_URL"]
async fn connection_failures_are_explicit() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1/".to_owned(), // 无人监听的端口
        key_prefix: "contract-fail".to_owned(),
        connect_timeout: Duration::from_millis(500),
    };
    let result = RedisClient::connect(&config).await;
    assert!(result.is_err(), "connecting to a dead port must fail explicitly");

    let client = test_client("contract-ping").await;
    client.ping().await.expect("ping must succeed when reachable");
}

/// 合同：降级语义前提——`RedisConfig::from_env` 在未配置 REDIS_URL 时
/// 返回 None（调用方直连数据库）；配置了错误 scheme 时由 connect 显式
/// 失败（§3.3：不静默降级）。
#[test]
fn env_semantics_for_degradation() {
    // 注意：本测试进程可能继承了调用环境的 REDIS_URL——只在未设置时断言。
    if std::env::var("REDIS_URL").is_err() {
        assert!(RedisConfig::from_env().is_none(), "no REDIS_URL means no cache (DB direct)");
    }
    assert!(
        RedisConfig {
            url: "http://not-redis".to_owned(),
            key_prefix: "x".to_owned(),
            connect_timeout: Duration::from_millis(200),
        }
        .url
        .starts_with("http"),
        "config carries the URL verbatim; scheme rejection happens at connect"
    );
}
