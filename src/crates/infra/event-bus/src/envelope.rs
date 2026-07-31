//! TaijiEventEnvelope + TaijiEventPriority — 事件信封与优先级。
//!
//! 参考: BitFun contracts/events/src/agentic.rs:7-13（优先级），:548-588（信封）

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::SystemTime;
use uuid::Uuid;

use crate::event::TaijiEvent;

/// 事件优先级（数值越小越紧急）。
///
/// 参考: BitFun agentic.rs:7-13。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum TaijiEventPriority {
    #[serde(rename = "critical")]
    Critical = 0,
    #[serde(rename = "high")]
    High = 1,
    #[serde(rename = "normal")]
    #[default]
    Normal = 2,
    #[serde(rename = "low")]
    Low = 3,
}

/// 事件信封 — 包含事件体、优先级和时间戳。
///
/// 参考: BitFun agentic.rs:548-588。
/// Ord 实现：优先级 ASC → 时间戳 ASC。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaijiEventEnvelope {
    pub id: String,
    pub event: TaijiEvent,
    pub priority: TaijiEventPriority,
    pub timestamp: SystemTime,
}

impl TaijiEventEnvelope {
    pub fn new(event: TaijiEvent, priority: TaijiEventPriority) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event,
            priority,
            timestamp: SystemTime::now(),
        }
    }
}

impl PartialEq for TaijiEventEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TaijiEventEnvelope {}

impl PartialOrd for TaijiEventEnvelope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaijiEventEnvelope {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.timestamp.cmp(&other.timestamp),
            other => other,
        }
    }
}
