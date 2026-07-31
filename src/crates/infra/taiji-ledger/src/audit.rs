//! 审计日志类型
//!
//! 来源: modules/ledger/接口设计.md §1 — AuditEntry / AuditFilter / AuditSummary

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use taiji_types::agent::AgentId;

/// 审计结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    Allowed,
    Denied,
    Error,
}

/// 审计记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub entry_id: String,
    pub timestamp: DateTime<Utc>,
    pub agent_id: AgentId,
    pub action: String,
    pub resource: String,
    pub result: AuditResult,
    pub detail: serde_json::Value,
}

/// 审计过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFilter {
    pub agent_id: Option<AgentId>,
    pub action: Option<String>,
    pub result: Option<AuditResult>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

impl AuditFilter {
    /// 空过滤器（返回全部记录）
    pub fn empty() -> Self {
        Self {
            agent_id: None,
            action: None,
            result: None,
            since: None,
            until: None,
            limit: None,
        }
    }

    /// 按 agent_id 过滤
    pub fn with_agent(agent_id: AgentId) -> Self {
        Self {
            agent_id: Some(agent_id),
            ..Self::empty()
        }
    }

    /// 检查条目是否匹配
    pub fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(ref agent_id) = self.agent_id {
            if &entry.agent_id != agent_id {
                return false;
            }
        }
        if let Some(ref action) = self.action {
            if &entry.action != action {
                return false;
            }
        }
        if let Some(ref result) = self.result {
            if &entry.result != result {
                return false;
            }
        }
        if let Some(since) = self.since {
            if entry.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if entry.timestamp > until {
                return false;
            }
        }
        true
    }
}

/// 审计摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_entries: u64,
    pub allowed_count: u64,
    pub denied_count: u64,
    pub error_count: u64,
}

impl AuditSummary {
    /// 从审计记录列表计算摘要
    pub fn from_entries(entries: &[AuditEntry]) -> Self {
        let total = entries.len() as u64;
        let allowed = entries.iter().filter(|e| e.result == AuditResult::Allowed).count() as u64;
        let denied = entries.iter().filter(|e| e.result == AuditResult::Denied).count() as u64;
        let error = entries.iter().filter(|e| e.result == AuditResult::Error).count() as u64;
        Self {
            total_entries: total,
            allowed_count: allowed,
            denied_count: denied,
            error_count: error,
        }
    }
}
