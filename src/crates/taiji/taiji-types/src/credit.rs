//! 评分（Credit）类型 — Agent 信誉与贡献度量。

use serde::{Deserialize, Serialize};

/// Agent 综合评分。
///
/// 由 event-bus KPI 调度器维护，反映 Agent 的任务执行质量和信誉。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AgentCredit {
    /// 基础评分（0.0 - 100.0）。
    pub score: f64,
    /// 宗门贡献。
    pub contribution: f64,
    /// 任务成功率。
    pub success_rate: f64,
    /// 道心（稳定度 0-100）。
    pub daoxin: u32,
    /// 审核通过率。
    pub review_pass_rate: f64,
    /// 返工率。
    pub rework_rate: f64,
    /// KPI 奖励分。
    pub kpi_bonus: f64,
}

impl Default for AgentCredit {
    fn default() -> Self {
        Self {
            score: 50.0,
            contribution: 0.0,
            success_rate: 1.0,
            daoxin: 50,
            review_pass_rate: 1.0,
            rework_rate: 0.0,
            kpi_bonus: 0.0,
        }
    }
}
