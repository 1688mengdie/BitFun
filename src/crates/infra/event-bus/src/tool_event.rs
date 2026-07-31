//! ToolEventData + ToolEventIdentity — 工具执行子事件。
//!
//! 参考: BitFun contracts/events/src/agentic.rs:436-546
//! LVPA 保留核心 10 变体，裁剪 StreamChunk/ConfirmationNeeded/Confirmed/Rejected。

use serde::{Deserialize, Serialize};

/// 工具事件标识。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolEventIdentity {
    pub tool_id: String,
    pub tool_name: String,
}

/// 工具执行子事件枚举。
///
/// 保留 10 个核心变体，裁剪 BitFun 专有的 StreamChunk/ConfirmationNeeded 等。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum ToolEventData {
    #[serde(rename = "early_detected")]
    EarlyDetected {
        identity: ToolEventIdentity,
    },
    #[serde(rename = "params_partial")]
    ParamsPartial {
        identity: ToolEventIdentity,
        params: serde_json::Value,
    },
    #[serde(rename = "queued")]
    Queued {
        identity: ToolEventIdentity,
        position: u32,
    },
    #[serde(rename = "waiting")]
    Waiting {
        identity: ToolEventIdentity,
        dependencies: Vec<String>,
    },
    #[serde(rename = "started")]
    Started {
        identity: ToolEventIdentity,
        params: serde_json::Value,
        timeout_seconds: u64,
    },
    #[serde(rename = "progress")]
    Progress {
        identity: ToolEventIdentity,
        message: String,
        percentage: f64,
    },
    #[serde(rename = "streaming")]
    Streaming {
        identity: ToolEventIdentity,
        chunks_received: u64,
    },
    #[serde(rename = "completed")]
    Completed {
        identity: ToolEventIdentity,
        result: serde_json::Value,
        duration_ms: u64,
    },
    #[serde(rename = "failed")]
    Failed {
        identity: ToolEventIdentity,
        error: String,
        duration_ms: u64,
    },
    #[serde(rename = "cancelled")]
    Cancelled {
        identity: ToolEventIdentity,
        reason: String,
        duration_ms: u64,
    },
}
