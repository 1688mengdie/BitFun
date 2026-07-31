//! KpiScheduler — KPI 评分调度器（LVPA 独有）。
//!
//! 参考: 架构总纲 §3 KPI 公式
//! score = success_rate×0.4 + review_pass_rate×0.3 + (1-rework_rate)×0.2 + kpi_bonus×0.1

use std::sync::Arc;

use dashmap::DashMap;

use taiji_types::agent::AgentId;
use taiji_types::credit::AgentCredit;

/// 任务执行结果，用于更新评分。
pub struct TaskResult {
    pub success: bool,
    pub review_passed: bool,
    pub rework: bool,
    pub kpi_bonus: f64,
}

/// KPI 评分调度器 — 美团骑手抢单模式。
pub struct KpiScheduler {
    scores: Arc<DashMap<AgentId, AgentCredit>>,
}

impl Default for KpiScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl KpiScheduler {
    pub fn new() -> Self {
        Self {
            scores: Arc::new(DashMap::new()),
        }
    }

    /// 获取 Agent 评分。
    pub fn get_credit(&self, agent_id: &AgentId) -> AgentCredit {
        self.scores
            .get(agent_id)
            .map(|c| *c)
            .unwrap_or_default()
    }

    /// 设置/更新 Agent 评分。
    pub fn set_credit(&self, agent_id: AgentId, credit: AgentCredit) {
        self.scores.insert(agent_id, credit);
    }

    /// 按评分排序选取最优 Agent（最高分优先）。
    pub fn select_best(&self, candidates: &[AgentId]) -> Option<AgentId> {
        candidates
            .iter()
            .map(|id| {
                let credit = self.get_credit(id);
                (id, self.calculate(&credit))
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id.clone())
    }

    /// 更新评分（任务完成后调用）。
    pub fn update_credit(&self, agent_id: &AgentId, result: &TaskResult) {
        let mut credit = self.get_credit(agent_id);
        // 滑动平均更新
        let alpha = 0.3;
        credit.success_rate = credit.success_rate * (1.0 - alpha)
            + if result.success { 1.0 } else { 0.0 } * alpha;
        credit.review_pass_rate = credit.review_pass_rate * (1.0 - alpha)
            + if result.review_passed { 1.0 } else { 0.0 } * alpha;
        credit.rework_rate = credit.rework_rate * (1.0 - alpha)
            + if result.rework { 1.0 } else { 0.0 } * alpha;
        credit.kpi_bonus = result.kpi_bonus;
        credit.score = self.calculate(&credit);
        self.scores.insert(agent_id.clone(), credit);
    }

    /// KPI 评分公式。
    pub fn calculate(&self, credit: &AgentCredit) -> f64 {
        credit.success_rate * 0.4
            + credit.review_pass_rate * 0.3
            + (1.0 - credit.rework_rate) * 0.2
            + credit.kpi_bonus * 0.1
    }
}
