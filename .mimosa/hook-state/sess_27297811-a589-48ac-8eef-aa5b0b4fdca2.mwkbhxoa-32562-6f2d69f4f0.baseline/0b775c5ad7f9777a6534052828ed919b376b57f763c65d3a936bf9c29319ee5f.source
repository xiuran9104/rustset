//! 任务投递消息总线（docs/xinchuang-adaptation-plan.md §4.3 语义合同）。
//!
//! 定位（底线）：**数据库是任务状态的唯一真相源**。JetStream 承担投递与
//! 唤醒（低延迟、跨进程、可替换为国产 MQ），消息丢失或总线不可用不损害
//! 任务可靠性——worker 的数据库轮询兜底恒在；"消息已发送"永远不作为
//! 任务已受理的凭据。
//!
//! 替换国产 MQ（RocketMQ 5 / TongLINK/Q 等）时按语义合同验收：
//! 持久化与重启恢复、durable consumer（消费组）、显式 ACK、ACK 超时重投、
//! 至少一次 + 幂等消费、最大投递次数/重试间隔/死信、堆积监控、副本与故障恢复。
//! 合同逐项落在 `tests/jetstream_contract.rs`，替换实现必须原样通过。

use std::time::Duration;

use futures::StreamExt as _;

/// 总线错误。
#[derive(Debug)]
pub struct MqError(pub String);

impl std::fmt::Display for MqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MqError {}

/// JetStream 总线配置（§3.3：不支持的组合显式报错，不静默降级）。
#[derive(Debug, Clone)]
pub struct JetStreamConfig {
    /// NATS 连接串（默认 `NATS_URL`）。
    pub url: String,
    /// 任务事件流名。
    pub stream: String,
    /// 流主题前缀（任务事件落在 `<subject_prefix>.*`）。
    pub subject_prefix: String,
    /// 流副本数（集群部署 ≥3；单机 1）。
    pub replicas: usize,
    /// ACK 超时：超时未 ACK 即重投（至少一次投递的节拍）。
    pub ack_wait: Duration,
    /// 最大投递次数；耗尽后进死信处置（MAX_DELIVERIES advisory → DLQ 流）。
    pub max_deliver: i64,
    /// 重投退避序列（逐次应用的 ack_wait 序列）。
    pub backoff: Vec<Duration>,
    /// 发布幂等去重窗口（Nats-Msg-Id 在该窗口内去重）。
    pub duplicate_window: Duration,
}

impl JetStreamConfig {
    pub fn from_env() -> Result<Self, MqError> {
        let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
        if !url.starts_with("nats://") && !url.starts_with("tls://") {
            return Err(MqError(format!(
                "NATS_URL must use nats:// or tls:// scheme, got: {url}"
            )));
        }
        Ok(Self {
            url,
            stream: std::env::var("NATS_STREAM").unwrap_or_else(|_| "RUSTSET_TASKS".into()),
            subject_prefix: std::env::var("NATS_SUBJECT_PREFIX")
                .unwrap_or_else(|_| "rustset.task".into()),
            replicas: std::env::var("NATS_STREAM_REPLICAS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
            ack_wait: Duration::from_secs(
                std::env::var("NATS_ACK_WAIT_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(30),
            ),
            max_deliver: std::env::var("NATS_MAX_DELIVER")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(5),
            backoff: vec![
                Duration::from_secs(1),
                Duration::from_secs(5),
                Duration::from_secs(30),
            ],
            duplicate_window: Duration::from_secs(
                std::env::var("NATS_DUPLICATE_WINDOW_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(120),
            ),
        })
    }
}

/// JetStream 任务总线。
#[derive(Clone)]
pub struct JetStreamBus {
    client: async_nats::Client,
    context: async_nats::jetstream::Context,
    config: JetStreamConfig,
}

impl std::fmt::Debug for JetStreamBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JetStreamBus")
            .field("url", &self.config.url)
            .field("stream", &self.config.stream)
            .finish_non_exhaustive()
    }
}

impl JetStreamBus {
    /// 连接并确保流存在。文件存储是持久化与重启恢复的载体；
    /// 副本数按配置（集群 ≥3 时故障恢复由 Raft 元数据 + 副本日志承担）。
    pub async fn connect(config: JetStreamConfig) -> Result<Self, MqError> {
        let client = async_nats::connect(&config.url)
            .await
            .map_err(|error| MqError(format!("nats connect {}: {error}", config.url)))?;
        let context = async_nats::jetstream::new(client.clone());

        let stream_config = async_nats::jetstream::stream::Config {
            name: config.stream.clone(),
            subjects: vec![format!("{}.*", config.subject_prefix)],
            storage: async_nats::jetstream::stream::StorageType::File,
            num_replicas: config.replicas,
            // 发布幂等：同 Nats-Msg-Id 在窗口内只落一条。
            duplicate_window: config.duplicate_window,
            // 死信转发器按序号取原文需要 direct get。
            allow_direct: true,
            ..Default::default()
        };
        context
            .get_or_create_stream(stream_config)
            .await
            .map_err(|error| MqError(format!("ensure stream: {error}")))?;

        Ok(Self {
            client,
            context,
            config,
        })
    }

    pub fn config(&self) -> &JetStreamConfig {
        &self.config
    }

    /// 发布任务事件。`message_id` 作为 Nats-Msg-Id：同 id 在去重窗口内
    /// 重复发布只落一条（幂等发布）。返回 false 表示被去重窗口吞掉。
    pub async fn publish(
        &self,
        subject_kind: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<bool, MqError> {
        let subject = format!("{}.{}", self.config.subject_prefix, subject_kind);
        let future = self
            .context
            .send_publish(
                subject.clone(),
                async_nats::jetstream::message::PublishMessage::build()
                    .payload(payload.to_vec().into())
                    .message_id(message_id),
            )
            .await
            .map_err(|error| MqError(format!("publish {subject}: {error}")))?;
        let ack = future
            .await
            .map_err(|error| MqError(format!("publish ack {subject}: {error}")))?;
        Ok(!ack.duplicate)
    }

    /// 绑定 durable pull consumer（消费组：同名 durable 在多 worker 间
    /// 竞争消费，每条消息恰好投递给一个成员）。
    pub async fn durable_consumer(
        &self,
        durable_name: &str,
        filter_subject: &str,
    ) -> Result<async_nats::jetstream::consumer::PullConsumer, MqError> {
        let consumer_config = async_nats::jetstream::consumer::pull::Config {
            durable_name: Some(durable_name.to_owned()),
            filter_subject: filter_subject.to_owned(),
            // 显式 ACK（合同）；ack_wait 即 ACK 超时；max_deliver 耗尽后
            // 由死信转发器接手；backoff 即重投间隔序列。
            ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
            ack_wait: self.config.ack_wait,
            max_deliver: self.config.max_deliver,
            backoff: self.config.backoff.clone(),
            ..Default::default()
        };
        let stream = self
            .context
            .get_stream(&self.config.stream)
            .await
            .map_err(|error| MqError(format!("get stream {}: {error}", self.config.stream)))?;
        stream
            .get_or_create_consumer(durable_name, consumer_config)
            .await
            .map_err(|error| MqError(format!("ensure consumer {durable_name}: {error}")))
    }

    /// 删除整个流（运维工具：重建流配置、清理死信积压时使用）。
    pub async fn delete_stream(&self) -> Result<(), MqError> {
        self.context
            .delete_stream(&self.config.stream)
            .await
            .map_err(|error| MqError(format!("delete stream: {error}")))?;
        Ok(())
    }

    /// 清空流内消息（运维/测试用；消费配置与历史保留）。
    pub async fn purge(&self) -> Result<(), MqError> {
        let stream = self
            .context
            .get_stream(&self.config.stream)
            .await
            .map_err(|error| MqError(format!("get stream: {error}")))?;
        stream
            .purge()
            .await
            .map_err(|error| MqError(format!("purge: {error}")))?;
        Ok(())
    }

    /// 拉取一批消息（显式拉取；批大小与等待时长由调用方控制）。
    pub async fn fetch_batch(
        consumer: &async_nats::jetstream::consumer::PullConsumer,
        batch: usize,
        wait: Duration,
    ) -> Result<Vec<async_nats::jetstream::Message>, MqError> {
        let mut batch_iter = consumer
            .batch()
            .max_messages(batch)
            .expires(wait)
            .messages()
            .await
            .map_err(|error| MqError(format!("fetch batch: {error}")))?;
        let mut messages = Vec::new();
        while let Some(message) = batch_iter.next().await {
            messages.push(message.map_err(|error| MqError(format!("fetch message: {error}")))?);
        }
        Ok(messages)
    }

    /// 死信处置：订阅 MAX_DELIVERIES advisory，把投递次数耗尽的消息原文
    /// 复制进 DLQ 流（`<stream>_DLQ`），供运维工具与人工处置。
    pub async fn spawn_dead_letter_forwarder(&self) -> Result<(), MqError> {
        let advisory_subject = format!(
            "$JS.EVENT.ADVISORY.CONSUMER.MAX_DELIVERIES.{}.>",
            self.config.stream
        );
        let mut advisory = self
            .client
            .subscribe(advisory_subject)
            .await
            .map_err(|error| MqError(format!("subscribe advisory: {error}")))?;
        let stream = self.config.stream.clone();
        // DLQ 使用与业务流完全独立的主题空间（{stream}_DLQ.*）：
        // NATS 的流创建会对 subjects 做保守相交检查，业务通配
        // （{prefix}.*）与 DLQ 的深层通配会被误判重叠。
        let dlq_stream_name = format!("{}_DLQ", stream);
        let dlq_subject_prefix = dlq_stream_name.clone();
        let context = self.context.clone();

        let dlq_stream_config = async_nats::jetstream::stream::Config {
            name: dlq_stream_name.clone(),
            subjects: vec![format!("{dlq_subject_prefix}.>")],
            storage: async_nats::jetstream::stream::StorageType::File,
            ..Default::default()
        };
        if let Err(error) = context.get_or_create_stream(dlq_stream_config).await {
            return Err(MqError(format!("ensure dlq stream: {error}")));
        }

        tokio::spawn(async move {
            while let Some(advisory_message) = advisory.next().await {
                let advisory_json: serde_json::Value =
                    match serde_json::from_slice(&advisory_message.payload) {
                        Ok(value) => value,
                        Err(error) => {
                            tracing::warn!(%error, "malformed max-deliveries advisory");
                            continue;
                        }
                    };
                // advisory v1 的 payload 不含原始主题（consumer 只是名字
                // 字符串），按 stream_seq 取原流消息——主题与载荷都来自原文。
                let stream_sequence = advisory_json["stream_seq"].as_u64().unwrap_or(0);
                let (origin_subject, dead_payload) = match context.get_stream(&stream).await {
                    Ok(stream_handle) => match stream_handle.direct_get(stream_sequence).await {
                        Ok(stored) => (stored.subject.to_string(), stored.payload.clone()),
                        Err(error) => {
                            tracing::warn!(%error, stream_sequence, "dead letter direct get failed");
                            continue;
                        }
                    },
                    Err(error) => {
                        tracing::warn!(%error, stream_sequence, "dead letter stream lookup failed");
                        continue;
                    }
                };
                // DLQ 主题：{prefix}.dlq.{原文主题末段}（由独立的 DLQ 流
                // 承载多段主题；业务流通配不会误捕获）。
                let suffix = origin_subject
                    .rsplit('.')
                    .next()
                    .unwrap_or("unknown");
                let dlq_subject = format!("{dlq_subject_prefix}.{suffix}");
                let dlq_result = match context
                    .send_publish(
                        dlq_subject.clone(),
                        async_nats::jetstream::message::PublishMessage::build()
                            .payload(dead_payload)
                            .message_id(format!("dlq-{stream_sequence}")),
                    )
                    .await
                {
                    Ok(future) => future.await,
                    Err(error) => Err(error),
                };
                match dlq_result {
                    Ok(_) => {
                        tracing::warn!(
                            %origin_subject,
                            stream_sequence,
                            %dlq_subject,
                            "message moved to dead-letter stream after exhausting max deliveries"
                        );
                    }
                    Err(error) => {
                        tracing::error!(%error, %dlq_subject, "dead letter forward failed");
                    }
                }
            }
        });
        Ok(())
    }

    /// 堆积与投递监控快照（合同：消息堆积监控）。
    pub async fn consumer_stats(&self, durable_name: &str) -> Result<ConsumerStats, MqError> {
        let mut consumer = self
            .durable_consumer(durable_name, &format!("{}.*", self.config.subject_prefix))
            .await?;
        let info = consumer
            .info()
            .await
            .map_err(|error| MqError(format!("consumer info: {error}")))?;
        Ok(ConsumerStats {
            num_pending: info.num_pending,
            num_ack_pending: info.num_ack_pending as u64,
            num_redelivered: info.num_redelivered as u64,
            delivered: info.delivered.stream_sequence,
            acknowledged: info.ack_floor.stream_sequence,
        })
    }
}

/// 消费者投递状态快照。
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerStats {
    /// 已持久化未投递数（堆积量）。
    pub num_pending: u64,
    /// 已投递未 ACK 数（处理中）。
    pub num_ack_pending: u64,
    /// 发生过重投的消息数。
    pub num_redelivered: u64,
    /// 流投递位点。
    pub delivered: u64,
    /// 流 ACK 位点。
    pub acknowledged: u64,
}

/// 从环境解析总线：`NATS_URL` 未配置返回 `None`（DB 轮询兜底，不报错）；
/// 配置了但连接失败向上报错（§3.3：不允许静默降级）。
pub async fn connect_from_env() -> Result<Option<JetStreamBus>, MqError> {
    if std::env::var("NATS_URL").is_err() {
        return Ok(None);
    }
    JetStreamBus::connect(JetStreamConfig::from_env()?)
        .await
        .map(Some)
}
