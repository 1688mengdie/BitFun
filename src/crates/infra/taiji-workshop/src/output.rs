//! WorkshopOutputStore — 工坊产出记录管理。
//!
//! 参考: Phase-工坊系统-类型契约.md §三 — 产出追踪

use chrono::Utc;
use serde::{Deserialize, Serialize};

use taiji_types::agent::AgentId;
use taiji_types::workshop_dungeon::{WorkshopId, WorkshopOutput};

/// 工坊产出存储。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkshopOutputStore {
    outputs: Vec<WorkshopOutput>,
}

impl WorkshopOutputStore {
    pub fn new() -> Self {
        Self { outputs: Vec::new() }
    }

    /// 记录一条产出。
    pub fn record(
        &mut self,
        workshop_id: WorkshopId,
        node_name: String,
        produced_by: AgentId,
        data: serde_json::Value,
    ) -> WorkshopOutput {
        let output = WorkshopOutput {
            output_id: format!("out-{}-{}", workshop_id, self.outputs.len() + 1),
            workshop_id,
            node_name,
            produced_by,
            data,
            created_at: Utc::now(),
        };
        self.outputs.push(output.clone());
        output
    }

    /// 获取某节点的所有产出。
    pub fn get_by_node(&self, node_name: &str) -> Vec<&WorkshopOutput> {
        self.outputs.iter().filter(|o| o.node_name == node_name).collect()
    }

    /// 检查某节点是否已有产出。
    pub fn has_node_output(&self, node_name: &str) -> bool {
        self.outputs.iter().any(|o| o.node_name == node_name)
    }

    /// 获取所有产出。
    pub fn all_outputs(&self) -> &[WorkshopOutput] {
        &self.outputs
    }

    /// 产出数量。
    pub fn count(&self) -> usize {
        self.outputs.len()
    }

    /// 获取已完成的节点名称集合。
    pub fn completed_node_names(&self) -> std::collections::HashSet<String> {
        self.outputs.iter().map(|o| o.node_name.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id() -> AgentId { AgentId::new() }

    #[test]
    fn test_record_output() {
        let mut store = WorkshopOutputStore::new();
        let ws_id = WorkshopId::new();
        let agent_id = make_id();
        let output = store.record(ws_id, "需求分析".into(), agent_id, serde_json::json!({"result": "ok"}));
        assert_eq!(output.node_name, "需求分析");
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_has_node_output() {
        let mut store = WorkshopOutputStore::new();
        let ws_id = WorkshopId::new();
        store.record(ws_id, "需求分析".into(), make_id(), serde_json::json!({}));
        assert!(store.has_node_output("需求分析"));
        assert!(!store.has_node_output("编码实现"));
    }

    #[test]
    fn test_completed_node_names() {
        let mut store = WorkshopOutputStore::new();
        let ws_id = WorkshopId::new();
        store.record(ws_id, "A".into(), make_id(), serde_json::json!({}));
        store.record(ws_id, "B".into(), make_id(), serde_json::json!({}));
        let names = store.completed_node_names();
        assert!(names.contains("A"));
        assert!(names.contains("B"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_empty_store() {
        let store = WorkshopOutputStore::new();
        assert_eq!(store.count(), 0);
        assert!(store.all_outputs().is_empty());
    }
}
