//! WorkshopManager trait + DefaultWorkshopManager — 工坊系统核心接口与内存实现。
//!
//! 参考: godot-skill-system skills/skillManager.gd:115-637 (MIT)
//!       Phase-工坊系统-类型契约.md §三 3.4-3.5

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::{AgentId, SpiritRoot};
use taiji_types::workshop_dungeon::{WorkshopId, WorkshopMember, WorkshopOutput, WorkshopStatus, WorkshopType};

use crate::config::{default_workshop_configs, WorkshopConfig};
use crate::error::WorkshopError;
use crate::output::WorkshopOutputStore;
use crate::workshop::Workshop;

/// 工坊系统核心接口。
#[async_trait]
pub trait WorkshopManager: Send + Sync {
    // ── 工坊生命周期 ──
    async fn create_workshop(&mut self, config: WorkshopConfig) -> Result<WorkshopId, WorkshopError>;
    async fn get_workshop(&self, id: &WorkshopId) -> Result<Option<Workshop>, WorkshopError>;
    async fn list_workshops(&self) -> Result<Vec<Workshop>, WorkshopError>;
    async fn list_workshops_by_type(&self, wtype: WorkshopType) -> Result<Vec<Workshop>, WorkshopError>;
    async fn close_workshop(&mut self, id: &WorkshopId) -> Result<(), WorkshopError>;

    // ── 成员管理 ──
    async fn join_workshop(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId, spirit_root: SpiritRoot) -> Result<(), WorkshopError>;
    async fn leave_workshop(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId) -> Result<(), WorkshopError>;
    async fn get_members(&self, workshop_id: &WorkshopId) -> Result<Vec<WorkshopMember>, WorkshopError>;
    /// 查询 Agent 所属的所有工坊 ID（多公会支持）。
    async fn get_agent_workshops(&self, agent_id: &AgentId) -> Result<Vec<WorkshopId>, WorkshopError>;

    // ── DAG 产出追踪 ──
    async fn submit_output(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId, node_name: &str, data: serde_json::Value) -> Result<(), WorkshopError>;
    async fn get_outputs(&self, workshop_id: &WorkshopId) -> Result<Vec<WorkshopOutput>, WorkshopError>;
    /// 获取已完成节点名称集合。
    async fn get_completed_nodes(&self, workshop_id: &WorkshopId) -> Result<Vec<String>, WorkshopError>;
}

/// 默认内存工坊管理器。
pub struct DefaultWorkshopManager {
    workshops: RwLock<HashMap<WorkshopId, Workshop>>,
    outputs: RwLock<HashMap<WorkshopId, WorkshopOutputStore>>,
    /// Agent → 所属工坊索引（多公会支持）。
    agent_index: RwLock<HashMap<AgentId, Vec<WorkshopId>>>,
}

impl DefaultWorkshopManager {
    pub fn new() -> Self {
        Self {
            workshops: RwLock::new(HashMap::new()),
            outputs: RwLock::new(HashMap::new()),
            agent_index: RwLock::new(HashMap::new()),
        }
    }

    /// 创建并注册 4 条默认工坊。
    pub fn with_default_workshops() -> Self {
        let mgr = Self::new();
        let configs = default_workshop_configs();
        for config in configs {
            let ws = Workshop::new(config);
            let id = ws.id;
            mgr.workshops.write().unwrap().insert(id, ws);
            mgr.outputs.write().unwrap().insert(id, WorkshopOutputStore::new());
        }
        mgr
    }
}

impl Default for DefaultWorkshopManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkshopManager for DefaultWorkshopManager {
    async fn create_workshop(&mut self, config: WorkshopConfig) -> Result<WorkshopId, WorkshopError> {
        let ws = Workshop::new(config);
        let id = ws.id;
        self.workshops.write().map_err(|e| WorkshopError::Internal(e.to_string()))?.insert(id, ws);
        self.outputs.write().map_err(|e| WorkshopError::Internal(e.to_string()))?.insert(id, WorkshopOutputStore::new());
        Ok(id)
    }

    async fn get_workshop(&self, id: &WorkshopId) -> Result<Option<Workshop>, WorkshopError> {
        let workshops = self.workshops.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        Ok(workshops.get(id).cloned())
    }

    async fn list_workshops(&self) -> Result<Vec<Workshop>, WorkshopError> {
        let workshops = self.workshops.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        Ok(workshops.values().cloned().collect())
    }

    async fn list_workshops_by_type(&self, wtype: WorkshopType) -> Result<Vec<Workshop>, WorkshopError> {
        let workshops = self.workshops.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        Ok(workshops.values().filter(|w| w.config.workshop_type == wtype).cloned().collect())
    }

    async fn close_workshop(&mut self, id: &WorkshopId) -> Result<(), WorkshopError> {
        let mut workshops = self.workshops.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let ws = workshops.get_mut(id).ok_or(WorkshopError::WorkshopNotFound(*id))?;
        ws.status = WorkshopStatus::Closed;
        Ok(())
    }

    async fn join_workshop(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId, spirit_root: SpiritRoot) -> Result<(), WorkshopError> {
        let mut workshops = self.workshops.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let ws = workshops.get_mut(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;
        ws.add_member(agent_id.clone(), spirit_root)?;
        // 更新 agent 索引
        let mut idx = self.agent_index.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        idx.entry(agent_id.clone()).or_default().push(*workshop_id);
        Ok(())
    }

    async fn leave_workshop(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId) -> Result<(), WorkshopError> {
        let mut workshops = self.workshops.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let ws = workshops.get_mut(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;
        ws.remove_member(agent_id)?;
        // 更新 agent 索引
        let mut idx = self.agent_index.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        if let Some(ids) = idx.get_mut(agent_id) {
            ids.retain(|id| id != workshop_id);
        }
        Ok(())
    }

    async fn get_members(&self, workshop_id: &WorkshopId) -> Result<Vec<WorkshopMember>, WorkshopError> {
        let workshops = self.workshops.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let ws = workshops.get(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;
        Ok(ws.members.clone())
    }

    async fn get_agent_workshops(&self, agent_id: &AgentId) -> Result<Vec<WorkshopId>, WorkshopError> {
        let idx = self.agent_index.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        Ok(idx.get(agent_id).cloned().unwrap_or_default())
    }

    async fn submit_output(&mut self, workshop_id: &WorkshopId, agent_id: &AgentId, node_name: &str, data: serde_json::Value) -> Result<(), WorkshopError> {
        // 校验工坊存在
        let workshops = self.workshops.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let ws = workshops.get(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;

        // 校验 Agent 是成员
        if !ws.has_member(agent_id) {
            return Err(WorkshopError::NotMember(agent_id.clone(), *workshop_id));
        }

        // 校验 DAG 节点存在
        let node = ws.config.dag_nodes.iter().find(|n| n.name == node_name)
            .ok_or_else(|| WorkshopError::DagNodeNotFound(node_name.to_string()))?;

        // 校验前置节点已产出
        let outputs = self.outputs.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let store = outputs.get(workshop_id).ok_or_else(|| WorkshopError::Storage("output store not found".into()))?;
        for input_key in &node.input_keys {
            let input_satisfied = ws.config.dag_nodes.iter()
                .filter(|n| n.output_keys.contains(input_key))
                .all(|n| store.has_node_output(&n.name));
            if !input_satisfied {
                let missing: Vec<String> = ws.config.dag_nodes.iter()
                    .filter(|n| n.output_keys.contains(input_key))
                    .map(|n| n.name.clone())
                    .collect();
                return Err(WorkshopError::PrerequisitesNotMet(node_name.to_string(), missing));
            }
        }
        drop(outputs);
        drop(workshops);

        // 记录产出
        let mut outputs = self.outputs.write().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let store = outputs.get_mut(workshop_id).ok_or_else(|| WorkshopError::Storage("output store not found".into()))?;
        store.record(*workshop_id, node_name.to_string(), agent_id.clone(), data);
        Ok(())
    }

    async fn get_outputs(&self, workshop_id: &WorkshopId) -> Result<Vec<WorkshopOutput>, WorkshopError> {
        let outputs = self.outputs.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let store = outputs.get(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;
        Ok(store.all_outputs().to_vec())
    }

    async fn get_completed_nodes(&self, workshop_id: &WorkshopId) -> Result<Vec<String>, WorkshopError> {
        let outputs = self.outputs.read().map_err(|e| WorkshopError::Internal(e.to_string()))?;
        let store = outputs.get(workshop_id).ok_or(WorkshopError::WorkshopNotFound(*workshop_id))?;
        Ok(store.completed_node_names().into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::workshop_dungeon::WorkshopType;

    #[tokio::test]
    async fn test_default_workshops() {
        let mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        assert_eq!(workshops.len(), 4);
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let mut mgr = DefaultWorkshopManager::new();
        let configs = default_workshop_configs();
        let id = mgr.create_workshop(configs.into_iter().next().unwrap()).await.unwrap();
        let ws = mgr.get_workshop(&id).await.unwrap();
        assert!(ws.is_some());
    }

    #[tokio::test]
    async fn test_join_and_leave() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        // 找到接受 Metal 灵根的工坊（天机坊或金算坊）
        let ws_id = workshops.iter()
            .find(|w| w.config.required_spirit_roots.contains(&SpiritRoot::Metal))
            .map(|w| w.id)
            .expect("should have a workshop accepting Metal");
        let agent_id = AgentId::new();

        mgr.join_workshop(&ws_id, &agent_id, SpiritRoot::Metal).await.unwrap();
        let members = mgr.get_members(&ws_id).await.unwrap();
        assert_eq!(members.len(), 1);

        // 多公会支持: 查询 Agent 所属工坊
        let agent_ws = mgr.get_agent_workshops(&agent_id).await.unwrap();
        assert_eq!(agent_ws.len(), 1);
        assert_eq!(agent_ws[0], ws_id);

        mgr.leave_workshop(&ws_id, &agent_id).await.unwrap();
        let members = mgr.get_members(&ws_id).await.unwrap();
        assert_eq!(members.len(), 0);

        // 离开后索引清除
        let agent_ws = mgr.get_agent_workshops(&agent_id).await.unwrap();
        assert!(agent_ws.is_empty());
    }

    #[tokio::test]
    async fn test_list_by_type() {
        let mgr = DefaultWorkshopManager::with_default_workshops();
        let tianji_list = mgr.list_workshops_by_type(WorkshopType::Tianji).await.unwrap();
        assert_eq!(tianji_list.len(), 1);
        assert_eq!(tianji_list[0].config.workshop_type, WorkshopType::Tianji);
    }

    #[tokio::test]
    async fn test_submit_output() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        let tianji: Vec<_> = workshops.into_iter().filter(|w| w.config.workshop_type == WorkshopType::Tianji).collect();
        let ws_id = tianji[0].id;
        let agent_id = AgentId::new();

        mgr.join_workshop(&ws_id, &agent_id, SpiritRoot::Metal).await.unwrap();

        // 提交需求分析产出（无依赖）
        mgr.submit_output(&ws_id, &agent_id, "需求分析", serde_json::json!({"spec": "..."})).await.unwrap();

        let outputs = mgr.get_outputs(&ws_id).await.unwrap();
        assert_eq!(outputs.len(), 1);

        let completed = mgr.get_completed_nodes(&ws_id).await.unwrap();
        assert!(completed.contains(&"需求分析".to_string()));
    }

    #[tokio::test]
    async fn test_submit_output_prerequisites_not_met() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        let tianji: Vec<_> = workshops.into_iter().filter(|w| w.config.workshop_type == WorkshopType::Tianji).collect();
        let ws_id = tianji[0].id;
        let agent_id = AgentId::new();

        mgr.join_workshop(&ws_id, &agent_id, SpiritRoot::Metal).await.unwrap();

        // 跳过需求分析直接提交编码实现 → 应失败
        let result = mgr.submit_output(&ws_id, &agent_id, "编码实现", serde_json::json!({"code": "..."})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_submit_output_not_member() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        let ws_id = workshops[0].id;
        let agent_id = AgentId::new();

        let result = mgr.submit_output(&ws_id, &agent_id, "需求分析", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(WorkshopError::NotMember(_, _))));
    }

    #[tokio::test]
    async fn test_join_invalid_spirit_root() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        let jinsuan: Vec<_> = workshops.into_iter().filter(|w| w.config.workshop_type == WorkshopType::Jinsuan).collect();
        let ws_id = jinsuan[0].id;
        let agent_id = AgentId::new();

        // 金算坊只接受 Metal，Wood 应被拒绝
        let result = mgr.join_workshop(&ws_id, &agent_id, SpiritRoot::Wood).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(WorkshopError::SpiritRootMismatch(_, _))));
    }

    #[tokio::test]
    async fn test_multi_workshop_support() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let agent_id = AgentId::new();
        let workshops = mgr.list_workshops().await.unwrap();

        // Metal 可加入天机坊和金算坊
        for ws in &workshops {
            if ws.config.workshop_type == WorkshopType::Tianji || ws.config.workshop_type == WorkshopType::Jinsuan {
                mgr.join_workshop(&ws.id, &agent_id, SpiritRoot::Metal).await.unwrap();
            }
        }

        let agent_ws = mgr.get_agent_workshops(&agent_id).await.unwrap();
        assert_eq!(agent_ws.len(), 2);
    }

    #[tokio::test]
    async fn test_close_workshop() {
        let mut mgr = DefaultWorkshopManager::with_default_workshops();
        let workshops = mgr.list_workshops().await.unwrap();
        let ws_id = workshops[0].id;

        mgr.close_workshop(&ws_id).await.unwrap();
        let ws = mgr.get_workshop(&ws_id).await.unwrap().unwrap();
        assert_eq!(ws.status, WorkshopStatus::Closed);
    }
}
