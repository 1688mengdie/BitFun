//! 事件类型 — 任务堂（event-bus）与传音阵（transport）事件协议。
//!
//! 参考源：
//! - TaijiEvent → BitFun contracts/events/agentic.rs:69-366 枚举模式（带 serde tag）
//! - TransportEvent → BitFun emit_generic(event_name, payload) 模式

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::{AgentId, SpiritRoot};
use crate::credit::AgentCredit;
use crate::realm::Realm;
use crate::workshop_dungeon::{DungeonId, DungeonResult, DungeonStatus, WorkshopId, WorkshopStatus, WorkshopType};

// ── TransportEvent（传音阵事件） ──

/// 传输层事件 — 后端→前端推送。
///
/// 参考 BitFun emit_generic(event_name, payload) 模式。
/// 通过 transport 层序列化为 JSON 推送至前端。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum TransportEvent {
    /// Agent 状态更新。
    #[serde(rename = "agent:updated")]
    AgentUpdated {
        agent_id: AgentId,
        realm: Realm,
        credit: AgentCredit,
    },
    /// 任务状态变更。
    #[serde(rename = "task:status")]
    TaskStatus {
        task_id: Uuid,
        agent_id: AgentId,
        status: String,
    },
    /// 通知消息。
    #[serde(rename = "notification")]
    Notification {
        level: String,
        message: String,
    },

    /// 工坊状态推送。
    #[serde(rename = "workshop:status")]
    WorkshopStatus {
        workshop_id: WorkshopId,
        status: WorkshopStatus,
        member_count: u32,
    },

    /// 副本状态推送。
    #[serde(rename = "dungeon:status")]
    DungeonStatus {
        dungeon_id: DungeonId,
        status: DungeonStatus,
        member_count: u32,
    },
}

// ── TaijiEvent（任务堂事件） ──

/// LVPA 事件 — 任务堂（event-bus）的事件协议。
///
/// 参考 BitFun AgenticEvent（agentic.rs:69-366）枚举 + serde tag 模式。
/// 裁剪 BitFun 专有变体（DeepReview/ImageAnalysis 等），
/// 保留 LVPA 所需的 Agent 生命周期与任务调度事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaijiEvent {
    /// Agent 创建。
    #[serde(rename = "agent:created")]
    AgentCreated {
        agent_id: AgentId,
        name: String,
        realm: Realm,
        timestamp: DateTime<Utc>,
    },
    /// Agent 境界突破。
    #[serde(rename = "agent:realm_upgraded")]
    RealmUpgraded {
        agent_id: AgentId,
        from: Realm,
        to: Realm,
    },
    /// Agent 评分变更。
    #[serde(rename = "agent:credit_changed")]
    CreditChanged {
        agent_id: AgentId,
        new_score: f64,
        delta: f64,
    },
    /// Agent 身外化身（fork 生子）。
    #[serde(rename = "agent:forked")]
    AgentForked {
        parent_id: AgentId,
        child_id: AgentId,
    },
    /// Agent 转世重生（git checkout）。
    #[serde(rename = "agent:reincarnated")]
    AgentReincarnated {
        agent_id: AgentId,
        target_commit: String,
    },
    /// 任务发布。
    #[serde(rename = "task:published")]
    TaskPublished {
        task_id: Uuid,
        publisher_id: AgentId,
        task_type: String,
        priority: u8,
    },
    /// 任务认领。
    #[serde(rename = "task:claimed")]
    TaskClaimed {
        task_id: Uuid,
        agent_id: AgentId,
    },
    /// 任务完成。
    #[serde(rename = "task:completed")]
    TaskCompleted {
        task_id: Uuid,
        agent_id: AgentId,
        success: bool,
    },

    // ========================
    // 工坊事件
    // ========================

    /// Agent 加入工坊。
    #[serde(rename = "workshop:joined")]
    WorkshopJoined {
        workshop_id: WorkshopId,
        workshop_type: WorkshopType,
        agent_id: AgentId,
        spirit_root: SpiritRoot,
    },

    /// Agent 离开工坊。
    #[serde(rename = "workshop:left")]
    WorkshopLeft {
        workshop_id: WorkshopId,
        agent_id: AgentId,
        reason: String,
    },

    /// 工坊 DAG 节点产出。
    #[serde(rename = "workshop:output_created")]
    WorkshopOutputCreated {
        workshop_id: WorkshopId,
        node_name: String,
        agent_id: AgentId,
        output_id: String,
    },

    // ========================
    // 副本事件
    // ========================

    /// 副本发布。
    #[serde(rename = "dungeon:published")]
    DungeonPublished {
        dungeon_id: DungeonId,
        name: String,
        publisher_id: AgentId,
        min_members: u32,
        max_members: u32,
    },

    /// 加入副本队伍。
    #[serde(rename = "dungeon:joined")]
    DungeonJoined {
        dungeon_id: DungeonId,
        agent_id: AgentId,
        current_members: u32,
        max_members: u32,
    },

    /// 离开副本队伍。
    #[serde(rename = "dungeon:left")]
    DungeonLeft {
        dungeon_id: DungeonId,
        agent_id: AgentId,
        current_members: u32,
    },

    /// 副本开始执行。
    #[serde(rename = "dungeon:started")]
    DungeonStarted {
        dungeon_id: DungeonId,
        member_ids: Vec<AgentId>,
    },

    /// 副本完成结算。
    #[serde(rename = "dungeon:completed")]
    DungeonCompleted {
        dungeon_id: DungeonId,
        results: DungeonResult,
    },

    // ========================
    // 经济事件
    // ========================

    /// 天材地宝消耗（转世重生/夺舍等操作触发）。
    #[serde(rename = "treasure:consumed")]
    TreasureConsumed {
        agent_id: AgentId,
        item: crate::economy::TreasureItem,
        reason: String,
    },
}
