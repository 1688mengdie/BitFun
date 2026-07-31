//! EventBus<M: MessageBus> — 基于 message-bus 的语义事件层。
//!
//! 参考: modules/event-bus/接口设计.md §二（v2.4）
//! 泛型化设计：将 message-bus 作为物理传输层注入，EventCodec 做序列化桥梁。

use std::sync::Arc;

use taiji_infra_message_bus::bus::MessageBus;
use tokio::sync::mpsc;
use tracing::warn;

use crate::codec::{CodecConfig, EventCodec};
use crate::envelope::{TaijiEventEnvelope, TaijiEventPriority};
use crate::error::{EventBusError, EventBusResult};
use crate::event::TaijiEvent;
use crate::router::{EventRouter, EventSubscriber};
use crate::scheduler::KpiScheduler;

/// EventBus 配置。
pub struct EventBusConfig {
    /// 物理传输层的 topic 前缀（如 "lvpa.event."）。
    pub topic_prefix: String,
    /// EventCodec 序列化格式配置。
    pub codec_config: CodecConfig,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            topic_prefix: "lvpa.event.".into(),
            codec_config: CodecConfig::default(),
        }
    }
}

/// 任务堂 — 基于 message-bus 的语义事件层。
pub struct EventBus<M: MessageBus> {
    /// 物理传输层（message-bus 注入）。
    msg_bus: M,
    /// 序列化桥梁：TaijiEvent ↔ Bytes。
    codec: EventCodec,
    /// 内部订阅者路由表。
    router: EventRouter,
    /// KPI 评分调度器。
    scheduler: KpiScheduler,
    /// topic 前缀。
    topic_prefix: String,
}

impl<M: MessageBus> EventBus<M> {
    /// 创建 EventBus 实例。
    pub fn new(msg_bus: M, config: EventBusConfig) -> Self {
        Self {
            msg_bus,
            codec: EventCodec::new(config.codec_config),
            router: EventRouter::new(),
            scheduler: KpiScheduler::new(),
            topic_prefix: config.topic_prefix,
        }
    }

    /// 发布事件。
    ///
    /// 流程：
    /// 1. 创建 TaijiEventEnvelope（含 id + 时间戳）
    /// 2. codec.serialize 序列化为 Bytes
    /// 3. msg_bus.publish 投递到物理传输层
    /// 4. router.route 同步派发给内部订阅者
    pub async fn publish(
        &self,
        event: TaijiEvent,
        priority: TaijiEventPriority,
    ) -> EventBusResult<String> {
        let envelope = TaijiEventEnvelope::new(event, priority);
        let envelope_id = envelope.id.clone();

        // 序列化
        let bytes = self.codec.serialize(&envelope.event)?;

        // 构造 topic：{prefix}{priority_name}
        let priority_name = match priority {
            TaijiEventPriority::Critical => "critical",
            TaijiEventPriority::High => "high",
            TaijiEventPriority::Normal => "normal",
            TaijiEventPriority::Low => "low",
        };
        let topic = format!("{}{}", self.topic_prefix, priority_name);

        // 投递到物理传输层
        self.msg_bus
            .publish(&topic, bytes)
            .map_err(|e| EventBusError::Topic(e.to_string()))?;

        // 同步派发给内部订阅者
        self.router.route(&envelope.event).await;

        Ok(envelope_id)
    }

    /// 订阅事件类型。
    ///
    /// 底层通过 msg_bus.subscribe(topic) 接收字节流，
    /// 后台任务反序列化后通过 mpsc channel 返回给调用者。
    pub fn subscribe(&self, event_type: &str) -> EventBusResult<mpsc::Receiver<TaijiEvent>> {
        let topic = format!("{}{}", self.topic_prefix, event_type);
        let mut byte_rx = self
            .msg_bus
            .subscribe(&topic)
            .map_err(|e| EventBusError::Topic(e.to_string()))?;

        let codec_config = self.codec.config.clone();
        let (tx, rx) = mpsc::channel::<TaijiEvent>(64);

        // 后台反序列化任务
        tokio::spawn(async move {
            while let Some(bytes) = byte_rx.recv().await {
                let codec = EventCodec::new(codec_config.clone());
                match codec.deserialize(&bytes) {
                    Ok(event) => {
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("event_bus subscribe: deserialize failed: {}", e);
                    }
                }
            }
        });

        Ok(rx)
    }

    /// 注册内部订阅者。
    pub fn register_subscriber(&self, id: String, subscriber: Arc<dyn EventSubscriber>) {
        self.router.subscribe(id, subscriber);
    }

    /// 获取 KPI 调度器引用。
    pub fn scheduler(&self) -> &KpiScheduler {
        &self.scheduler
    }

    /// 获取 EventRouter 引用。
    pub fn router(&self) -> &EventRouter {
        &self.router
    }
}
