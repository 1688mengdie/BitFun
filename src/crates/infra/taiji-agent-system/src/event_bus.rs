//! 事件总线接口 — Agent 状态变更广播的轻量 trait。
//!
//! 采用依赖倒置：taiji-agent-system 定义 EventBus trait，
//! 上层（taiji-agent-runtime 或集成层）负责将 Phase 1 event-bus 包装为此 trait。
//! 测试代码使用 MockEventBus 验证事件广播。

use std::sync::Arc;

use async_trait::async_trait;
use taiji_types::agent::{AgentId, AgentStatus};

/// Agent 状态变更事件。
#[derive(Debug, Clone)]
pub struct AgentStatusChangeEvent {
    pub agent_id: AgentId,
    pub from: AgentStatus,
    pub to: AgentStatus,
}

/// 事件总线接口 — 用于广播 Agent 生命周期事件。
#[async_trait]
pub trait EventBus: Send + Sync {
    /// 发布 Agent 状态变更事件。
    async fn publish_status_change(&self, event: AgentStatusChangeEvent);
}

/// 空实现 — 不广播任何事件（用于测试或未配置 event-bus）。
pub struct NoopEventBus;

#[async_trait]
impl EventBus for NoopEventBus {
    async fn publish_status_change(&self, _event: AgentStatusChangeEvent) {}
}

/// Mock 实现 — 记录发布的事件，供测试验证。
pub struct MockEventBus {
    events: tokio::sync::Mutex<Vec<AgentStatusChangeEvent>>,
}

impl MockEventBus {
    pub fn new() -> Self {
        Self {
            events: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// 获取所有已发布的事件。
    pub async fn events(&self) -> Vec<AgentStatusChangeEvent> {
        self.events.lock().await.clone()
    }

    /// 获取事件数量。
    pub async fn event_count(&self) -> usize {
        self.events.lock().await.len()
    }
}

impl Default for MockEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish_status_change(&self, event: AgentStatusChangeEvent) {
        self.events.lock().await.push(event);
    }
}

#[async_trait]
impl EventBus for Arc<MockEventBus> {
    async fn publish_status_change(&self, event: AgentStatusChangeEvent) {
        self.events.lock().await.push(event);
    }
}
