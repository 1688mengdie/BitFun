//! TaijiEvent — LVPA 领域事件枚举。
//!
//! 参考: BitFun contracts/events/src/agentic.rs:69-366
//! 裁剪 BitFun 专有变体（DeepReview/ImageAnalysis/SessionModelAutoMigrated 等），
//! 保留 LVPA 需要的 4 类事件：Agent 生命周期 / 境界评分 / 任务调度 / 工具执行 / 系统。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use taiji_types::agent::AgentId;
use taiji_types::economy::TreasureItem;
use taiji_types::realm::Realm;

use crate::tool_event::ToolEventData;

/// LVPA 领域事件枚举。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaijiEvent {
    // ── Agent 生命周期 ──
    #[serde(rename = "agent:created")]
    AgentCreated {
        agent_id: AgentId,
        name: String,
        realm: Realm,
        timestamp: std::time::SystemTime,
    },

    #[serde(rename = "agent:state_changed")]
    AgentStateChanged {
        agent_id: AgentId,
        new_state: String,
    },

    #[serde(rename = "agent:forked")]
    AgentForked {
        parent_id: AgentId,
        child_id: AgentId,
    },

    // ── 境界/评分 ──
    #[serde(rename = "agent:realm_upgraded")]
    RealmUpgraded {
        agent_id: AgentId,
        from: Realm,
        to: Realm,
    },

    #[serde(rename = "agent:credit_changed")]
    CreditChanged {
        agent_id: AgentId,
        new_score: f64,
        delta: f64,
    },

    // ── 任务调度 ──
    #[serde(rename = "task:published")]
    TaskPublished {
        task_id: Uuid,
        publisher_id: AgentId,
        task_type: String,
        priority: i32,
    },

    #[serde(rename = "task:claimed")]
    TaskClaimed {
        task_id: Uuid,
        agent_id: AgentId,
    },

    #[serde(rename = "task:completed")]
    TaskCompleted {
        task_id: Uuid,
        agent_id: AgentId,
        success: bool,
    },

    // ── 工具执行 ──
    #[serde(rename = "tool:event")]
    ToolEvent {
        agent_id: AgentId,
        tool_event: ToolEventData,
    },

    // ── 系统级 ──
    #[serde(rename = "system:error")]
    SystemError {
        agent_id: Option<AgentId>,
        error: String,
        recoverable: bool,
    },

    #[serde(rename = "config:changed")]
    ConfigChanged {
        path: String,
        old_value: Option<serde_json::Value>,
        new_value: serde_json::Value,
    },

    // ── 经济事件 ──
    #[serde(rename = "treasure:consumed")]
    TreasureConsumed {
        agent_id: AgentId,
        item: TreasureItem,
        reason: String,
    },
}
