//! 任务数据模型
//!
//! 来源: modules/db-store/接口设计.md:236-253 — TaskEntity
//! 来源: 架构总纲 §3 — event-bus（任务堂）

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 任务记录
///
/// 对应 `tasks` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntity {
    /// UUID v7
    pub id: String,
    /// 执行者 ID
    pub agent_id: String,
    /// R-ID 贯穿全链
    pub r_id: String,
    /// pending | running | completed | failed | cancelled
    pub status: String,
    /// 任务类型
    pub task_type: String,
    /// 优先级（0~100）
    pub priority: i32,
    /// 输入参数
    pub input: Value,
    /// 输出结果
    pub output: Option<Value>,
    /// 错误信息
    pub error: Option<String>,
    /// 创建时间（ISO 8601 UTC）
    pub created_at: String,
    /// 完成时间（ISO 8601 UTC）
    pub completed_at: Option<String>,
}
