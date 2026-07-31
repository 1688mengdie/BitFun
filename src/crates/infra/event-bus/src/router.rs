//! EventRouter + EventSubscriber — 内部订阅者路由。
//!
//! 参考: modules/event-bus/接口设计.md §四

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::warn;

use crate::error::EventBusResult;
use crate::event::TaijiEvent;

/// 事件订阅者 trait。
#[async_trait]
pub trait EventSubscriber: Send + Sync + 'static {
    async fn on_event(&self, event: &TaijiEvent) -> EventBusResult<()>;
}

/// 内部订阅者路由表。
pub struct EventRouter {
    subscribers: Arc<DashMap<String, Arc<dyn EventSubscriber>>>,
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRouter {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(DashMap::new()),
        }
    }

    /// 分发给所有内部订阅者。
    ///
    /// 错误不传播：单个订阅者失败仅记录 warn，不影响其他订阅者。
    pub async fn route(&self, event: &TaijiEvent) {
        for entry in self.subscribers.iter_mut() {
            let subscriber = entry.value();
            if let Err(e) = subscriber.on_event(event).await {
                warn!(
                    "event_router: subscriber {} failed: {}",
                    entry.key(),
                    e
                );
            }
        }
    }

    /// 注册订阅者。
    pub fn subscribe(&self, id: String, subscriber: Arc<dyn EventSubscriber>) {
        self.subscribers.insert(id, subscriber);
    }

    /// 注销订阅者。
    pub fn unsubscribe(&self, id: &str) {
        self.subscribers.remove(id);
    }

    /// 当前订阅者数量。
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}
