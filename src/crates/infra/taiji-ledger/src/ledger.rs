//! Ledger trait + InMemoryLedger 实现
//!
//! 来源: modules/ledger/接口设计.md §1 — Ledger trait

use async_trait::async_trait;
use std::sync::RwLock;
use taiji_types::agent::AgentId;

use crate::audit::{AuditEntry, AuditFilter, AuditSummary};
use crate::error::LedgerError;

/// 功德簿审计日志 — 被动接收，仅追加写入
#[async_trait]
pub trait Ledger: Send + Sync {
    /// 追加审计记录
    async fn record(&self, entry: AuditEntry) -> Result<(), LedgerError>;

    /// 查询 Agent 的审计历史
    async fn query(&self, agent_id: &AgentId, filter: AuditFilter) -> Result<Vec<AuditEntry>, LedgerError>;

    /// 获取审计统计摘要
    async fn summary(&self, agent_id: &AgentId) -> Result<AuditSummary, LedgerError>;
}

/// 内存 Ledger 实现（Vec + RwLock）
pub struct InMemoryLedger {
    entries: RwLock<Vec<AuditEntry>>,
}

impl InMemoryLedger {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Ledger for InMemoryLedger {
    async fn record(&self, entry: AuditEntry) -> Result<(), LedgerError> {
        self.entries.write().unwrap().push(entry);
        Ok(())
    }

    async fn query(&self, agent_id: &AgentId, filter: AuditFilter) -> Result<Vec<AuditEntry>, LedgerError> {
        let entries = self.entries.read().unwrap();
        let filtered: Vec<AuditEntry> = entries
            .iter()
            .filter(|e| &e.agent_id == agent_id)
            .filter(|e| filter.matches(e))
            .cloned()
            .collect();
        let limit = filter.limit.unwrap_or(usize::MAX);
        Ok(filtered.into_iter().take(limit).collect())
    }

    async fn summary(&self, agent_id: &AgentId) -> Result<AuditSummary, LedgerError> {
        let entries = self.entries.read().unwrap();
        let agent_entries: Vec<AuditEntry> = entries
            .iter()
            .filter(|e| &e.agent_id == agent_id)
            .cloned()
            .collect();
        Ok(AuditSummary::from_entries(&agent_entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditResult;
    use chrono::{DateTime, TimeZone, Utc};

    fn make_entry(agent_id: &AgentId, action: &str, result: AuditResult, ts: DateTime<Utc>) -> AuditEntry {
        AuditEntry {
            entry_id: format!("entry_{}", ts.timestamp_millis()),
            timestamp: ts,
            agent_id: agent_id.clone(),
            action: action.into(),
            resource: "test.rs".into(),
            result,
            detail: serde_json::json!({"key": "value"}),
        }
    }

    #[tokio::test]
    async fn test_record_and_query() {
        let ledger = InMemoryLedger::new();
        let agent = AgentId::new();
        for i in 0..3 {
            let ts = Utc.timestamp_opt(1700000000 + i, 0).unwrap();
            ledger.record(make_entry(&agent, "read", AuditResult::Allowed, ts)).await.unwrap();
        }
        let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_filter_by_result() {
        let ledger = InMemoryLedger::new();
        let agent = AgentId::new();
        ledger.record(make_entry(&agent, "write", AuditResult::Allowed, Utc::now())).await.unwrap();
        ledger.record(make_entry(&agent, "delete", AuditResult::Denied, Utc::now())).await.unwrap();
        let filter = AuditFilter {
            result: Some(AuditResult::Denied),
            ..AuditFilter::with_agent(agent.clone())
        };
        let results = ledger.query(&agent, filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "delete");
    }

    #[tokio::test]
    async fn test_summary() {
        let ledger = InMemoryLedger::new();
        let agent = AgentId::new();
        ledger.record(make_entry(&agent, "read", AuditResult::Allowed, Utc::now())).await.unwrap();
        ledger.record(make_entry(&agent, "write", AuditResult::Allowed, Utc::now())).await.unwrap();
        ledger.record(make_entry(&agent, "delete", AuditResult::Denied, Utc::now())).await.unwrap();
        ledger.record(make_entry(&agent, "exec", AuditResult::Error, Utc::now())).await.unwrap();
        let summary = ledger.summary(&agent).await.unwrap();
        assert_eq!(summary.total_entries, 4);
        assert_eq!(summary.allowed_count, 2);
        assert_eq!(summary.denied_count, 1);
        assert_eq!(summary.error_count, 1);
    }

    #[tokio::test]
    async fn test_time_range_filter() {
        let ledger = InMemoryLedger::new();
        let agent = AgentId::new();
        let t1 = Utc.timestamp_opt(1700000000, 0).unwrap();
        let t2 = Utc.timestamp_opt(1700000100, 0).unwrap();
        let t3 = Utc.timestamp_opt(1700000200, 0).unwrap();
        ledger.record(make_entry(&agent, "a", AuditResult::Allowed, t1)).await.unwrap();
        ledger.record(make_entry(&agent, "b", AuditResult::Allowed, t2)).await.unwrap();
        ledger.record(make_entry(&agent, "c", AuditResult::Allowed, t3)).await.unwrap();
        let filter = AuditFilter {
            since: Some(t2),
            until: None,
            ..AuditFilter::with_agent(agent.clone())
        };
        let results = ledger.query(&agent, filter).await.unwrap();
        assert_eq!(results.len(), 2, "since=t2 应返回 2 条");
    }

    #[test]
    fn test_audit_entry_serde_roundtrip() {
        let entry = make_entry(&AgentId::new(), "read", AuditResult::Allowed, Utc::now());
        let json = serde_json::to_string(&entry).unwrap();
        let back: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, entry.agent_id);
        assert_eq!(back.action, entry.action);
        assert_eq!(back.result, entry.result);
    }

    #[test]
    fn test_audit_result_serde() {
        for r in &[AuditResult::Allowed, AuditResult::Denied, AuditResult::Error] {
            let json = serde_json::to_string(r).unwrap();
            let back: AuditResult = serde_json::from_str(&json).unwrap();
            assert_eq!(*r, back);
        }
    }
}
