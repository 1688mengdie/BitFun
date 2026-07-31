//! Agent 管理器 — 注册、查找、销毁、身外化身、转世重生。

use std::collections::HashMap;

use taiji_types::agent::{AgentId, AgentStatus, SpiritRoot};
use tokio::sync::RwLock;

use crate::agent::AgentTrait;
use crate::error::AgentSystemError;
use crate::event_bus::{AgentStatusChangeEvent, EventBus, NoopEventBus};
use crate::lifecycle::Lifecycle;

/// Agent 管理器 — 管理所有 Agent 实例，事件广播集成。
///
/// 内部使用 RwLock<HashMap> 实现并发安全访问。
/// 状态变更时通过 event_bus 广播 AgentStatusChangeEvent。
pub struct AgentManager {
    agents: RwLock<HashMap<String, Box<dyn AgentTrait>>>,
    event_bus: Box<dyn EventBus>,
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentManager {
    /// 创建空的 Agent 管理器（无事件广播）。
    pub fn new() -> Self {
        Self::with_event_bus(Box::new(NoopEventBus))
    }

    /// 创建带事件广播的 Agent 管理器。
    pub fn with_event_bus(event_bus: Box<dyn EventBus>) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            event_bus,
        }
    }

    /// 注册一个 Agent。重复 agent_id 返回 Err。
    /// 成功注册时广播 Idle 状态事件。
    pub async fn register(&self, agent: Box<dyn AgentTrait>) -> Result<(), AgentSystemError> {
        let id = agent.agent_id().to_string();
        let aid = AgentId::parse(&id).unwrap_or_else(|_| AgentId::new());
        let mut agents = self.agents.write().await;
        if agents.contains_key(&id) {
            return Err(AgentSystemError::AgentAlreadyExists(id));
        }
        agents.insert(id, agent);
        // 广播注册事件（状态变为 Idle）
        self.emit_status_change(Some(aid), AgentStatus::Idle).await;
        Ok(())
    }

    /// 注销一个 Agent。成功时广播 Destroyed 状态事件。
    pub async fn unregister(&self, id: &AgentId) -> Result<(), AgentSystemError> {
        let mut agents = self.agents.write().await;
        agents.remove(&id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(id.to_string())
        })?;
        self.emit_status_change(Some(id.clone()), AgentStatus::Destroyed).await;
        Ok(())
    }

    /// 检查 Agent 是否存在。
    pub async fn has(&self, id: &AgentId) -> bool {
        let agents = self.agents.read().await;
        agents.contains_key(&id.to_string())
    }

    /// 获取 Agent 的摘要信息。
    pub async fn get_info(&self, id: &AgentId) -> Result<serde_json::Value, AgentSystemError> {
        let agents = self.agents.read().await;
        let agent = agents.get(&id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(id.to_string())
        })?;
        Ok(serde_json::json!({
            "agent_id": agent.agent_id().to_string(),
            "spirit_root": agent.spirit_root(),
            "realm": agent.realm(),
            "status": agent.status(),
        }))
    }

    /// 列出所有 Agent ID。
    pub async fn list(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// 列出所有 Agent 的摘要信息。
    pub async fn list_info(&self) -> Vec<serde_json::Value> {
        let agents = self.agents.read().await;
        agents.values().map(|agent| {
            serde_json::json!({
                "agent_id": agent.agent_id().to_string(),
                "spirit_root": agent.spirit_root(),
                "realm": agent.realm(),
                "status": agent.status(),
            })
        }).collect()
    }

    /// 更新 Agent 状态（由 AgentTrait 实现调用）。
    /// 如果状态转换合法，广播状态变更事件。
    pub async fn update_status(&self, id: &AgentId, new_status: AgentStatus) -> Result<(), AgentSystemError> {
        let agents = self.agents.read().await;
        let agent = agents.get(&id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(id.to_string())
        })?;
        let old_status = agent.status();
        drop(agents);

        // 验证状态转换
        let transition = Lifecycle::validate(old_status, new_status)?;
        let _ = transition;

        // 无需直接修改 Agent 内部状态（由 AgentTrait 实现自行管理）
        // 只广播事件
        self.emit_status_change(Some(id.clone()), new_status).await;
        Ok(())
    }

    /// 身外化身（Fork 子 Agent）。
    pub async fn fork_agent(
        &self,
        parent_id: &AgentId,
        child_name: &str,
        spirit_root: SpiritRoot,
    ) -> Result<AgentId, AgentSystemError> {
        let agents = self.agents.read().await;
        let parent = agents.get(&parent_id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(parent_id.to_string())
        })?;
        let child = parent.fork(child_name, spirit_root).await?;
        let child_id = child.agent_id().clone();
        drop(agents);

        let mut agents_w = self.agents.write().await;
        agents_w.insert(child_id.to_string(), child);
        // 广播 Fork 事件
        self.emit_status_change(Some(child_id.clone()), AgentStatus::Idle).await;
        Ok(child_id)
    }

    /// 转世重生 — 重置 Agent 状态到指定历史点。
    pub async fn reincarnate(&self, id: &AgentId, commit: &str) -> Result<(), AgentSystemError> {
        let agents = self.agents.read().await;
        let _agent = agents.get(&id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(id.to_string())
        })?;
        drop(agents);

        let mut agents_w = self.agents.write().await;
        let agent_mut = agents_w.get_mut(&id.to_string()).ok_or_else(|| {
            AgentSystemError::AgentNotFound(id.to_string())
        })?;
        agent_mut.reincarnate(commit).await
    }

    // ── 内部方法 ──

    async fn emit_status_change(&self, agent_id: Option<AgentId>, to: AgentStatus) {
        if let Some(id) = agent_id {
            self.event_bus.publish_status_change(AgentStatusChangeEvent {
                agent_id: id,
                from: AgentStatus::Idle, // 简化：外部关心 to 状态
                to,
            }).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentTrait;
    use crate::event_bus::MockEventBus;
    use async_trait::async_trait;
    use taiji_types::agent::{AgentConfig, AgentId, AgentState};
    use taiji_types::realm::Realm;

    struct MockAgent {
        id: AgentId,
        root: SpiritRoot,
        config: AgentConfig,
        state: AgentState,
    }

    impl MockAgent {
        fn new(id: AgentId, root: SpiritRoot) -> Self {
            Self {
                id,
                root,
                config: AgentConfig::default(),
                state: AgentState {
                    session_id: "test".into(),
                    status: AgentStatus::Idle,
                    context: vec![],
                    summary: None,
                    cur_iter: 0,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            }
        }
    }

    #[async_trait]
    impl AgentTrait for MockAgent {
        fn agent_id(&self) -> &AgentId { &self.id }
        fn spirit_root(&self) -> SpiritRoot { self.root }
        fn realm(&self) -> Realm { Realm::QiRefining }
        fn status(&self) -> AgentStatus { self.state.status }
        fn config(&self) -> &AgentConfig { &self.config }
        fn state(&self) -> &AgentState { &self.state }

        async fn reply(&mut self, input: &str) -> Result<String, AgentSystemError> {
            Ok(format!("echo: {}", input))
        }

        async fn fork(&self, _child_name: &str, _spirit_root: SpiritRoot) -> Result<Box<dyn AgentTrait>, AgentSystemError> {
            let child = MockAgent::new(AgentId::new(), self.root);
            Ok(Box::new(child))
        }
    }

    // ── 基本功能测试 ──

    #[tokio::test]
    async fn test_register_agent() {
        let manager = AgentManager::new();
        let agent = Box::new(MockAgent::new(AgentId::new(), SpiritRoot::Metal));
        assert!(manager.register(agent).await.is_ok());
    }

    #[tokio::test]
    async fn test_duplicate_register() {
        let manager = AgentManager::new();
        let id = AgentId::new();
        let agent1 = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        let agent2 = Box::new(MockAgent::new(id.clone(), SpiritRoot::Wood));
        assert!(manager.register(agent1).await.is_ok());
        assert!(manager.register(agent2).await.is_err());
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let manager = AgentManager::new();
        assert!(!manager.has(&AgentId::new()).await);
    }

    #[tokio::test]
    async fn test_list() {
        let manager = AgentManager::new();
        let a1 = Box::new(MockAgent::new(AgentId::new(), SpiritRoot::Metal));
        let a2 = Box::new(MockAgent::new(AgentId::new(), SpiritRoot::Fire));
        manager.register(a1).await.unwrap();
        manager.register(a2).await.unwrap();
        assert_eq!(manager.list().await.len(), 2);
    }

    #[tokio::test]
    async fn test_unregister() {
        let manager = AgentManager::new();
        let id = AgentId::new();
        let agent = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        manager.register(agent).await.unwrap();
        assert!(manager.has(&id).await);
        manager.unregister(&id).await.unwrap();
        assert!(!manager.has(&id).await);
    }

    #[tokio::test]
    async fn test_fork_agent() {
        let manager = AgentManager::new();
        let parent_id = AgentId::new();
        let parent = Box::new(MockAgent::new(parent_id.clone(), SpiritRoot::Metal));
        manager.register(parent).await.unwrap();
        let child_id = manager.fork_agent(&parent_id, "child", SpiritRoot::Metal).await.unwrap();
        assert!(manager.has(&child_id).await);
        assert_eq!(manager.list().await.len(), 2);
    }

    #[tokio::test]
    async fn test_get_info() {
        let manager = AgentManager::new();
        let id = AgentId::new();
        let agent = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        manager.register(agent).await.unwrap();
        let info = manager.get_info(&id).await.unwrap();
        assert_eq!(info["spirit_root"], "metal");
    }

    #[tokio::test]
    async fn test_list_info() {
        let manager = AgentManager::new();
        let agent = Box::new(MockAgent::new(AgentId::new(), SpiritRoot::Wood));
        manager.register(agent).await.unwrap();
        let infos = manager.list_info().await;
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0]["spirit_root"], "wood");
    }

    // ── 事件广播集成测试 ──

    #[tokio::test]
    async fn test_register_emits_event_using_arc() {
        use std::sync::Arc;
        let mock_bus = Arc::new(MockEventBus::new());
        let manager = AgentManager::with_event_bus(Box::new(mock_bus.clone()));

        let id = AgentId::new();
        let agent = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        manager.register(agent).await.unwrap();

        let events = mock_bus.events().await;
        assert_eq!(events.len(), 1, "register should emit 1 event");
        assert_eq!(events[0].to, AgentStatus::Idle);
    }

    #[tokio::test]
    async fn test_unregister_emits_event() {
        use std::sync::Arc;
        let mock_bus = Arc::new(MockEventBus::new());
        let manager = AgentManager::with_event_bus(Box::new(mock_bus.clone()));

        let id = AgentId::new();
        let agent = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        manager.register(agent).await.unwrap();
        manager.unregister(&id).await.unwrap();

        let events = mock_bus.events().await;
        assert_eq!(events.len(), 2, "register + unregister should emit 2 events");
        // First event: register → Idle
        // Second event: unregister → Destroyed
        assert_eq!(events[0].to, AgentStatus::Idle);
        assert_eq!(events[1].to, AgentStatus::Destroyed);
    }

    #[tokio::test]
    async fn test_fork_emits_event_for_child() {
        use std::sync::Arc;
        let mock_bus = Arc::new(MockEventBus::new());
        let manager = AgentManager::with_event_bus(Box::new(mock_bus.clone()));

        let parent_id = AgentId::new();
        let parent = Box::new(MockAgent::new(parent_id.clone(), SpiritRoot::Metal));
        manager.register(parent).await.unwrap(); // event 1: register
        let child_id = manager.fork_agent(&parent_id, "child", SpiritRoot::Metal).await.unwrap(); // event 2: fork

        let events = mock_bus.events().await;
        assert_eq!(events.len(), 2, "register + fork should emit 2 events");
        assert!(manager.has(&child_id).await);
    }

    #[tokio::test]
    async fn test_update_status_emits_event() {
        use std::sync::Arc;
        let mock_bus = Arc::new(MockEventBus::new());
        let manager = AgentManager::with_event_bus(Box::new(mock_bus.clone()));

        let id = AgentId::new();
        let agent = Box::new(MockAgent::new(id.clone(), SpiritRoot::Metal));
        manager.register(agent).await.unwrap(); // event 1

        manager.update_status(&id, AgentStatus::Running).await.unwrap(); // event 2

        let events = mock_bus.events().await;
        assert_eq!(events.len(), 2, "should have 2 events");
        assert_eq!(events[1].to, AgentStatus::Running);
    }

    #[tokio::test]
    async fn test_update_status_rejects_unknown_agent() {
        let manager = AgentManager::new();
        let result = manager.update_status(&AgentId::new(), AgentStatus::Running).await;
        assert!(result.is_err(), "unknown agent should return Err");
    }
}
