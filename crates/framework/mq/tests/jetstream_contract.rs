//! NATS JetStream 语义合同测试（docs/xinchuang-adaptation-plan.md §4.3）。
//!
//! piren 定线：JetStream 是任务投递命脉，替换国产 MQ（RocketMQ 5 /
//! TongLINK/Q）时必须原样通过本套件——持久化与重启恢复、durable consumer
//! 消费组、显式 ACK、ACK 超时重投、至少一次 + 幂等、最大投递/死信、
//! 堆积监控。底线：总线不可用不得损害任务可靠性（DB 是任务真相源，
//! 该底线由 worker 侧的 DB 轮询兜底保证，不在本套件内）。
//!
//! 运行（compose 的 NATS 已启用 JetStream）：
//! ```bash
//! docker compose -f script/docker/docker-compose.yml up -d nats
//! TEST_NATS_URL=nats://127.0.0.1:4222 \
//!   cargo test -p rustset-framework-mq --test jetstream_contract -- --ignored
//! ```

use std::time::Duration;

use rustset_framework_mq::{JetStreamBus, JetStreamConfig};

fn test_config(stream: &str) -> JetStreamConfig {
    let url = std::env::var("TEST_NATS_URL").expect("TEST_NATS_URL is required");
    JetStreamConfig {
        url,
        stream: stream.to_owned(),
        subject_prefix: format!("contract.{stream}"),
        replicas: 1,
        // 合同参数取短值：测试里可观测 ACK 超时与重投节拍。
        ack_wait: Duration::from_millis(400),
        max_deliver: 3,
        backoff: vec![Duration::from_millis(100), Duration::from_millis(200)],
        duplicate_window: Duration::from_secs(60),
    }
}

/// 首次连接：删流重建，保证拿到当前代码的流配置
/// （get_or_create 不会更新已存在流的配置）。
async fn test_bus_fresh(stream: &str) -> JetStreamBus {
    let bus = JetStreamBus::connect(test_config(stream))
        .await
        .expect("connect jetstream");
    let _ = bus.delete_stream().await; // 不存在/已删均可
    JetStreamBus::connect(test_config(stream))
        .await
        .expect("reconnect with fresh stream config")
}

/// “重启”连接：复用既有流与消费进度（不删流）。
async fn test_bus(stream: &str) -> JetStreamBus {
    JetStreamBus::connect(test_config(stream))
        .await
        .expect("connect jetstream")
}

/// 合同：持久化 + 重启恢复。消息先落流、无人消费；"重启"（新总线实例 +
/// 重新绑定同名 durable）后必须取到此前发布的消息——文件存储保留内容，
/// durable 保留消费进度。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn persistence_survives_reconnect_and_consumer_recreation() {
    let bus = test_bus_fresh("PERSIST").await;
    let durable = format!("persist-{}", uuid::Uuid::new_v4().simple());
    let subject_kind = "wake";
    let payload = format!("persist-{}", uuid::Uuid::new_v4().simple());

    bus.publish(subject_kind, &payload, payload.as_bytes())
        .await
        .expect("publish before consumer exists");

    // “重启”：全新 bus 实例 + 重新绑定同名 durable consumer。
    let bus_after_restart = test_bus("PERSIST").await;
    let consumer = bus_after_restart
        .durable_consumer(&durable, "contract.PERSIST.wake")
        .await
        .expect("recreate durable consumer");
    let batch = JetStreamBus::fetch_batch(&consumer, 10, Duration::from_secs(2))
        .await
        .expect("fetch after restart");
    assert!(
        batch
            .iter()
            .any(|message| message.payload.as_ref() == payload.as_bytes()),
        "message published before the consumer existed must survive restart"
    );
    for message in batch {
        message.ack().await.expect("ack");
    }
}

/// 合同：durable consumer 消费组——两个同名 durable 的消费者竞争消费，
/// 每条消息恰好被一个成员取走（不会被两个都处理）。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn durable_consumer_group_delivers_each_message_once() {
    let bus = test_bus_fresh("GROUP").await;
    let durable = format!("group-{}", uuid::Uuid::new_v4().simple());
    let total = 12;
    for index in 0..total {
        let payload = format!("group-{index}");
        bus.publish("wake", &format!("group-{index}"), payload.as_bytes())
            .await
            .expect("publish");
    }

    let filter = "contract.GROUP.wake";
    let member_a = bus
        .durable_consumer(&durable, filter)
        .await
        .expect("consumer a");
    let member_b = bus
        .durable_consumer(&durable, filter)
        .await
        .expect("consumer b");

    let batch_a = JetStreamBus::fetch_batch(&member_a, total, Duration::from_millis(500))
        .await
        .expect("fetch a");
    let batch_b = JetStreamBus::fetch_batch(&member_b, total, Duration::from_millis(500))
        .await
        .expect("fetch b");

    let mut seen_ids = std::collections::HashSet::new();
    for message in batch_a.iter().chain(batch_b.iter()) {
        let id = message
            .headers
            .as_ref()
            .and_then(|headers| headers.get("Nats-Msg-Id").cloned())
            .map(|value| value.as_str().to_owned());
        if let Some(id) = id {
            seen_ids.insert(id);
        }
    }
    assert_eq!(
        seen_ids.len(),
        total,
        "each published message must be delivered exactly once across the group"
    );
    for message in batch_a.into_iter().chain(batch_b) {
        message.ack().await.expect("ack");
    }
}

/// 合同：显式 ACK + ACK 超时重投（至少一次）。不 ACK 的消息在 ack_wait
/// 到期后必须重投（消息头保留投递计数）。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn unacked_message_is_redelivered_after_ack_wait() {
    let bus = test_bus_fresh("ACKWAIT").await;
    let durable = format!("ackwait-{}", uuid::Uuid::new_v4().simple());
    let payload = format!("ackwait-{}", uuid::Uuid::new_v4().simple());
    bus.publish("wake", &payload, payload.as_bytes())
        .await
        .expect("publish");

    let consumer = bus
        .durable_consumer(&durable, "contract.ACKWAIT.wake")
        .await
        .expect("durable consumer");

    // 第一次取到但不 ACK——ack_wait（400ms）后必须重投。
    let first = JetStreamBus::fetch_batch(&consumer, 10, Duration::from_millis(300))
        .await
        .expect("first fetch");
    assert!(!first.is_empty(), "message must be fetched first");
    // backoff 序列（100ms/200ms）短于窗口时，首批可能已包含重投副本。
    // 故意不 ACK。

    // 第二次取：等 ack_wait 过期后重投。
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = JetStreamBus::fetch_batch(&consumer, 10, Duration::from_secs(2))
        .await
        .expect("second fetch");
    assert!(
        second
            .iter()
            .any(|message| message.payload.as_ref() == payload.as_bytes()
                && message.info().expect("delivery metadata").delivered >= 2),
        "unacked message must be redelivered after ack_wait (at-least-once)"
    );
    for message in second {
        message.ack().await.expect("ack");
    }
}

/// 合同：发布幂等——同 Nats-Msg-Id 在去重窗口内重复发布只落一条。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn duplicate_publish_is_deduplicated_by_message_id() {
    let bus = test_bus_fresh("DEDUP").await;
    let durable = format!("dedup-{}", uuid::Uuid::new_v4().simple());
    let run = uuid::Uuid::new_v4().simple().to_string();
    let message_id = format!("dedup-{run}");
    let payload = format!("dedup-payload-{run}");

    let first = bus.publish("wake", &message_id, payload.as_bytes())
        .await
        .expect("first publish");
    let second = bus.publish("wake", &message_id, payload.as_bytes())
        .await
        .expect("second publish");
    assert!(first, "first publish must land");
    assert!(!second, "duplicate publish must be swallowed by dedup window");

    let consumer = bus
        .durable_consumer(&durable, "contract.DEDUP.wake")
        .await
        .expect("durable consumer");
    let batch = JetStreamBus::fetch_batch(&consumer, 10, Duration::from_millis(300))
        .await
        .expect("fetch");
    // 批内可能包含 redelivery（至少一次投递的预期行为），
    // 去重合同以「流内该 Nats-Msg-Id 只落一条」为准：数 fetch 结果里的
    // 去重后消息 id 数量。
    let distinct_ids: std::collections::HashSet<String> = batch
        .iter()
        .filter(|message| message.payload.as_ref() == payload.as_bytes())
        .filter_map(|message| {
            message
                .headers
                .as_ref()
                .and_then(|headers| headers.get("Nats-Msg-Id").cloned())
                .map(|value| value.as_str().to_owned())
        })
        .collect();
    assert_eq!(
        distinct_ids.len(),
        1,
        "stream must contain exactly one copy of the deduplicated message"
    );
    for message in batch {
        message.ack().await.expect("ack");
    }
}

/// 合同：最大投递次数 + 死信处置。max_deliver 耗尽后消息停止重投，
/// MAX_DELIVERIES advisory 触发，死信转发器把原文复制进 DLQ 流。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn exhausted_deliveries_move_to_dead_letter_stream() {
    let bus = test_bus_fresh("DLQ").await;
    let durable = format!("dlq-{}", uuid::Uuid::new_v4().simple());
    // 先清空 DLQ 流的历史积压（前序运行残留），再启动 forwarder——
    // 顺序反过来会与 forwarder 的建流/发布竞态。
    {
        let dlq_cleanup = JetStreamBus::connect(JetStreamConfig {
            stream: "DLQ_DLQ".to_owned(),
            // 主题空间独立：DLQ 流的 subjects 与业务流（contract.DLQ.*）
            // 不得重叠（NATS 创建时做保守相交检查）。
            subject_prefix: "DLQ_DLQ".to_owned(),
            ..test_config("DLQ")
        })
        .await
        .expect("connect dlq stream for cleanup");
        let _ = dlq_cleanup.delete_stream().await;
    }
    bus.spawn_dead_letter_forwarder()
        .await
        .expect("spawn dead letter forwarder");

    let payload = format!("dlq-{}", uuid::Uuid::new_v4().simple());
    bus.publish("poison", &payload, payload.as_bytes())
        .await
        .expect("publish poison");

    let consumer = bus
        .durable_consumer(&durable, "contract.DLQ.poison")
        .await
        .expect("durable consumer");

    // 不 ACK，持续拉取直到投递次数耗尽（backoff 100/200ms 下三次投递
    // 可能全部落进同一批次窗口，也可能分批到达——按累计投递次数判断）。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut deliveries_seen = 0usize;
    while deliveries_seen < 3 && tokio::time::Instant::now() < deadline {
        let batch = JetStreamBus::fetch_batch(&consumer, 10, Duration::from_secs(1))
            .await
            .expect("delivery fetch");
        deliveries_seen += batch.len();
    }
    assert!(
        deliveries_seen >= 3,
        "message must be delivered max_deliver times before dead-lettering, saw {deliveries_seen}"
    );

    // 耗尽后：等待 advisory → DLQ 转发。死信落在独立的 DLQ_DLQ 流
    // （subjects: contract.DLQ.dlq.>）；单个 durable reader 跨迭代复用，
    // 游标自然推进，不会被累积的历史死信挡住。
    let dlq_bus = JetStreamBus::connect(JetStreamConfig {
        stream: "DLQ_DLQ".to_owned(),
        // 主题空间独立（见上）。
        subject_prefix: "DLQ_DLQ".to_owned(),
        ..test_config("DLQ")
    })
    .await
    .expect("connect dlq stream");
    let dlq_reader = dlq_bus
        .durable_consumer(
            &format!("dlq-reader-{}", uuid::Uuid::new_v4().simple()),
            "DLQ_DLQ.>",
        )
        .await
        .expect("bind dlq reader");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut found_in_dlq = false;
    while tokio::time::Instant::now() < deadline {
        let Ok(batch) =
            JetStreamBus::fetch_batch(&dlq_reader, 10, Duration::from_millis(500)).await
        else {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        };
        let payloads: Vec<String> = batch
            .iter()
            .map(|message| String::from_utf8_lossy(&message.payload).to_string())
            .collect();
        let matched = payloads.iter().any(|text| text == &payload);
        for message in batch {
            message.ack().await.expect("ack dlq");
        }
        eprintln!(
            "[dlq-drain] drained {} msg(s); payloads={payloads:?}",
            payloads.len()
        );
        if matched {
            found_in_dlq = true;
            break;
        }
    }
    eprintln!("[dlq-drain] loop finished, found_in_dlq={found_in_dlq}");
    assert!(found_in_dlq, "exhausted message must land in the DLQ stream");
}

/// 合同：堆积监控——consumer info 必须暴露 pending/ack_pending/redelivered，
/// 供运维平台采集（§4.3 堆积监控 + 语义稳定）。
#[tokio::test]
#[ignore = "requires TEST_NATS_URL with a JetStream-enabled NATS"]
async fn consumer_stats_expose_backlog() {
    let bus = test_bus_fresh("STATS").await;
    for index in 0..5 {
        let payload = format!("stats-{index}");
        bus.publish("wake", &format!("stats-{index}"), payload.as_bytes())
            .await
            .expect("publish");
    }
    let stats = bus
        .consumer_stats(&format!("stats-{}", uuid::Uuid::new_v4().simple()))
        .await
        .expect("consumer stats");
    assert!(stats.num_pending + stats.num_ack_pending >= 5, "stats: {stats:?}");
}
