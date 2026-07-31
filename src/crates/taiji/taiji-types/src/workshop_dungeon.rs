//! 工坊与副本系统共享类型 — WorkshopId / WorkshopType / WorkshopStatus / ...
//!
//! 参考: 架构总纲 §7.1（工坊与副本）
//!       Phase-工坊系统-类型契约.md §一 — R-WD-001

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::agent::SpiritRoot;

// ============================================================================
// 工坊基础类型
// ============================================================================

/// 工坊唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkshopId(pub Uuid);

impl WorkshopId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for WorkshopId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkshopId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// 工坊类型 — 4 条固定工作流。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkshopType {
    /// 天机坊 — 代码/开发
    #[serde(rename = "tianji")]
    Tianji,
    /// 金算坊 — 交易/量化
    #[serde(rename = "jinsuan")]
    Jinsuan,
    /// 丹青坊 — 美术/设计
    #[serde(rename = "danqing")]
    Danqing,
    /// 留影坊 — 视频/内容
    #[serde(rename = "liuying")]
    Liuying,
}

impl WorkshopType {
    /// 返回所有工坊类型。
    pub fn all() -> Vec<Self> {
        vec![Self::Tianji, Self::Jinsuan, Self::Danqing, Self::Liuying]
    }

    /// 中文名称。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tianji => "天机坊",
            Self::Jinsuan => "金算坊",
            Self::Danqing => "丹青坊",
            Self::Liuying => "留影坊",
        }
    }

    /// 默认本命魂卡要求。
    pub fn default_spirit_roots(&self) -> Vec<SpiritRoot> {
        match self {
            Self::Tianji => vec![SpiritRoot::Metal, SpiritRoot::Earth],
            Self::Jinsuan => vec![SpiritRoot::Metal],
            Self::Danqing => vec![SpiritRoot::Wood, SpiritRoot::Fire],
            Self::Liuying => vec![SpiritRoot::Wood],
        }
    }
}

/// 工坊运行时状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkshopStatus {
    /// 运行中 — 24/7 正常运转。
    #[serde(rename = "active")]
    Active,
    /// 暂停 — 维护中。
    #[serde(rename = "paused")]
    Paused,
    /// 已关闭。
    #[serde(rename = "closed")]
    Closed,
}

/// 工坊成员。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopMember {
    pub agent_id: AgentId,
    pub spirit_root: SpiritRoot,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub task_count: u64,
}

/// 工坊 DAG 节点 — 工坊工作流中的一个步骤。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopDagNode {
    pub name: String,
    pub description: Option<String>,
    pub input_keys: Vec<String>,
    pub output_keys: Vec<String>,
}

/// 工坊产出记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopOutput {
    pub output_id: String,
    pub workshop_id: WorkshopId,
    pub node_name: String,
    pub produced_by: AgentId,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// 副本基础类型
// ============================================================================

/// 副本唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DungeonId(pub Uuid);

impl DungeonId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for DungeonId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DungeonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// 副本状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DungeonStatus {
    /// 招募中 — 等待组队。
    #[serde(rename = "recruiting")]
    Recruiting,
    /// 准备中 — 人数达标，等待开始。
    #[serde(rename = "ready")]
    Ready,
    /// 执行中。
    #[serde(rename = "in_progress")]
    InProgress,
    /// 已完成 — 结算完毕。
    #[serde(rename = "completed")]
    Completed,
    /// 已解散。
    #[serde(rename = "disbanded")]
    Disbanded,
}

/// 副本成员。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DungeonMember {
    pub agent_id: AgentId,
    pub spirit_root: SpiritRoot,
    pub joined_at: DateTime<Utc>,
    pub role: String,
}

/// 副本奖励。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DungeonReward {
    pub base_score: f64,
    pub spirit_stones: u64,
    pub contribution: f64,
}

/// 副本结算成员结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberResult {
    pub agent_id: AgentId,
    pub contribution_share: f64,
    pub score_delta: f64,
    pub spirit_stones_earned: u64,
}

/// 副本结算结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DungeonResult {
    pub dungeon_id: DungeonId,
    pub member_results: Vec<MemberResult>,
    pub completed_at: DateTime<Utc>,
}
