//! Workshop 结构体 — 工坊运行时实例 + 成员管理 + SpiritRoot 资格校验。
//!
//! 参考: godot-skill-system skills/skillManager.gd:115-637 (MIT)
//!       Phase-工坊系统-类型契约.md §三 3.2

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use taiji_types::agent::{AgentId, SpiritRoot};
use taiji_types::workshop_dungeon::{WorkshopId, WorkshopMember, WorkshopStatus};

use crate::config::WorkshopConfig;
use crate::error::WorkshopError;

/// 工坊运行时实例。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workshop {
    pub id: WorkshopId,
    pub config: WorkshopConfig,
    pub status: WorkshopStatus,
    pub members: Vec<WorkshopMember>,
    pub created_at: DateTime<Utc>,
    pub member_limit: u32,
}

impl Workshop {
    /// 创建新工坊。
    pub fn new(config: WorkshopConfig) -> Self {
        Self {
            id: WorkshopId::new(),
            config,
            status: WorkshopStatus::Active,
            members: Vec::new(),
            created_at: Utc::now(),
            member_limit: 10,
        }
    }

    /// 设置成员上限。
    pub fn with_member_limit(mut self, limit: u32) -> Self {
        self.member_limit = limit;
        self
    }

    // ── 成员管理 ──

    /// Agent 加入工坊（含 SpiritRoot 校验）。
    pub fn add_member(&mut self, agent_id: AgentId, spirit_root: SpiritRoot) -> Result<(), WorkshopError> {
        if self.status != WorkshopStatus::Active {
            return Err(WorkshopError::WorkshopNotActive);
        }
        if self.members.iter().any(|m| m.agent_id == agent_id) {
            return Err(WorkshopError::AlreadyMember(agent_id, self.id));
        }
        if (self.members.len() as u32) >= self.member_limit {
            return Err(WorkshopError::WorkshopFull(self.members.len() as u32, self.member_limit));
        }
        if !Self::validate_spirit_root(spirit_root, &self.config.required_spirit_roots) {
            return Err(WorkshopError::SpiritRootMismatch(
                spirit_root,
                self.config.required_spirit_roots.clone(),
            ));
        }
        self.members.push(WorkshopMember {
            agent_id,
            spirit_root,
            role: "成员".into(),
            joined_at: Utc::now(),
            task_count: 0,
        });
        Ok(())
    }

    /// Agent 离开工坊。
    pub fn remove_member(&mut self, agent_id: &AgentId) -> Result<(), WorkshopError> {
        let idx = self.members.iter().position(|m| m.agent_id == *agent_id)
            .ok_or_else(|| WorkshopError::NotMember(agent_id.clone(), self.id))?;
        self.members.remove(idx);
        Ok(())
    }

    /// 检查 Agent 是否是成员。
    pub fn has_member(&self, agent_id: &AgentId) -> bool {
        self.members.iter().any(|m| m.agent_id == *agent_id)
    }

    /// 成员数量。
    pub fn member_count(&self) -> u32 {
        self.members.len() as u32
    }

    // ── SpiritRoot 资格校验 ──

    /// 校验 SpiritRoot 是否在工坊允许列表中。
    /// 如果 required 为空，视为无限制（返回 true）。
    pub fn validate_spirit_root(spirit_root: SpiritRoot, required: &[SpiritRoot]) -> bool {
        if required.is_empty() {
            return true;
        }
        required.contains(&spirit_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_workshop_configs;

    fn make_workshop() -> Workshop {
        let configs = default_workshop_configs();
        Workshop::new(configs.into_iter().find(|c| c.workshop_type == taiji_types::workshop_dungeon::WorkshopType::Tianji).unwrap())
    }

    fn make_id() -> AgentId { AgentId::new() }

    #[test]
    fn test_workshop_new() {
        let ws = make_workshop();
        assert_eq!(ws.status, WorkshopStatus::Active);
        assert!(ws.members.is_empty());
    }

    #[test]
    fn test_add_member_valid_spirit_root() {
        let mut ws = make_workshop();
        let id = make_id();
        assert!(ws.add_member(id, SpiritRoot::Metal).is_ok());
        assert_eq!(ws.member_count(), 1);
    }

    #[test]
    fn test_add_member_invalid_spirit_root() {
        let mut ws = make_workshop();
        // 天机坊允许 Metal/Earth, Wood 应被拒绝
        let id = make_id();
        let result = ws.add_member(id, SpiritRoot::Wood);
        assert!(result.is_err());
        assert!(matches!(result, Err(WorkshopError::SpiritRootMismatch(_, _))));
    }

    #[test]
    fn test_add_duplicate_member() {
        let mut ws = make_workshop();
        let id = make_id();
        ws.add_member(id.clone(), SpiritRoot::Metal).unwrap();
        let result = ws.add_member(id, SpiritRoot::Metal);
        assert!(matches!(result, Err(WorkshopError::AlreadyMember(_, _))));
    }

    #[test]
    fn test_remove_member() {
        let mut ws = make_workshop();
        let id = make_id();
        ws.add_member(id.clone(), SpiritRoot::Metal).unwrap();
        ws.remove_member(&id).unwrap();
        assert_eq!(ws.member_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_member() {
        let mut ws = make_workshop();
        let result = ws.remove_member(&make_id());
        assert!(matches!(result, Err(WorkshopError::NotMember(_, _))));
    }

    #[test]
    fn test_has_member() {
        let mut ws = make_workshop();
        let id = make_id();
        assert!(!ws.has_member(&id));
        ws.add_member(id.clone(), SpiritRoot::Metal).unwrap();
        assert!(ws.has_member(&id));
    }

    #[test]
    fn test_validate_spirit_root_empty_required() {
        assert!(Workshop::validate_spirit_root(SpiritRoot::Metal, &[]));
        assert!(Workshop::validate_spirit_root(SpiritRoot::Wood, &[]));
    }

    #[test]
    fn test_validate_spirit_root_specific() {
        let required = vec![SpiritRoot::Metal, SpiritRoot::Earth];
        assert!(Workshop::validate_spirit_root(SpiritRoot::Metal, &required));
        assert!(Workshop::validate_spirit_root(SpiritRoot::Earth, &required));
        assert!(!Workshop::validate_spirit_root(SpiritRoot::Wood, &required));
        assert!(!Workshop::validate_spirit_root(SpiritRoot::Fire, &required));
    }

    #[test]
    fn test_workshop_type_spirit_roots() {
        use taiji_types::workshop_dungeon::WorkshopType;
        assert_eq!(WorkshopType::Tianji.default_spirit_roots(), vec![SpiritRoot::Metal, SpiritRoot::Earth]);
        assert_eq!(WorkshopType::Jinsuan.default_spirit_roots(), vec![SpiritRoot::Metal]);
        assert_eq!(WorkshopType::Danqing.default_spirit_roots(), vec![SpiritRoot::Wood, SpiritRoot::Fire]);
        assert_eq!(WorkshopType::Liuying.default_spirit_roots(), vec![SpiritRoot::Wood]);
    }
}
