//! Token（灵力）管理器。
//!
//! Token 是消耗品，不设"余额"。只记录消耗量和补贴量。
//! 补贴期（炼气~金丹）内的消耗不计入总消耗统计。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::AgentId;
use taiji_types::economy::{CurrencyAmount, TokenAccount, TokenStats};

use crate::EconomyError;

/// Token（灵力）管理器 trait。
#[async_trait]
pub trait TokenManager: Send + Sync {
    /// 记录 Token 消耗（仅非补贴期计入 total_consumed）。
    async fn record_consumption(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError>;

    /// 记录补贴 Token（不计入总消耗统计）。
    async fn record_subsidy(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError>;

    /// 查询 Token 统计。
    async fn get_stats(&self, agent_id: &AgentId) -> Result<TokenStats, EconomyError>;

    /// 查询或创建 TokenAccount。
    async fn get_or_create_account(&self, agent_id: &AgentId) -> Result<TokenAccount, EconomyError>;

    /// 检查补贴是否生效。
    async fn is_subsidy_active(&self, agent_id: &AgentId) -> Result<bool, EconomyError>;

    /// 设置补贴状态（境界升级时调用）。
    async fn set_subsidy_active(&self, agent_id: &AgentId, active: bool) -> Result<(), EconomyError>;
}

/// 内存 Token 管理器实现。
pub struct InMemoryTokenManager {
    accounts: RwLock<HashMap<AgentId, TokenAccount>>,
}

impl InMemoryTokenManager {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenManager for InMemoryTokenManager {
    async fn record_consumption(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.entry(agent_id.clone()).or_insert_with(|| TokenAccount {
            agent_id: agent_id.clone(),
            total_consumed: CurrencyAmount::new(0),
            total_subsidized: CurrencyAmount::new(0),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        });

        if !account.subsidy_active {
            account.total_consumed = account.total_consumed.saturating_add(amount);
        }
        account.updated_at = chrono::Utc::now();
        Ok(())
    }

    async fn record_subsidy(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.entry(agent_id.clone()).or_insert_with(|| TokenAccount {
            agent_id: agent_id.clone(),
            total_consumed: CurrencyAmount::new(0),
            total_subsidized: CurrencyAmount::new(0),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        });

        account.total_subsidized = account.total_subsidized.saturating_add(amount);
        account.updated_at = chrono::Utc::now();
        Ok(())
    }

    async fn get_stats(&self, agent_id: &AgentId) -> Result<TokenStats, EconomyError> {
        let accounts = self.accounts.read().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.get(agent_id).ok_or_else(|| EconomyError::AccountNotFound(agent_id.clone()))?;

        Ok(TokenStats {
            agent_id: agent_id.clone(),
            total_consumed: account.total_consumed,
            total_subsidized: account.total_subsidized,
            net_self_paid: account.total_consumed.saturating_sub(account.total_subsidized),
            subsidy_active: account.subsidy_active,
        })
    }

    async fn get_or_create_account(&self, agent_id: &AgentId) -> Result<TokenAccount, EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        Ok(accounts.entry(agent_id.clone()).or_insert_with(|| TokenAccount {
            agent_id: agent_id.clone(),
            total_consumed: CurrencyAmount::new(0),
            total_subsidized: CurrencyAmount::new(0),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        }).clone())
    }

    async fn is_subsidy_active(&self, agent_id: &AgentId) -> Result<bool, EconomyError> {
        let accounts = self.accounts.read().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.get(agent_id).ok_or_else(|| EconomyError::AccountNotFound(agent_id.clone()))?;
        Ok(account.subsidy_active)
    }

    async fn set_subsidy_active(&self, agent_id: &AgentId, active: bool) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.entry(agent_id.clone()).or_insert_with(|| TokenAccount {
            agent_id: agent_id.clone(),
            total_consumed: CurrencyAmount::new(0),
            total_subsidized: CurrencyAmount::new(0),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        });
        account.subsidy_active = active;
        account.updated_at = chrono::Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id() -> AgentId {
        AgentId::new()
    }

    #[tokio::test]
    async fn test_record_consumption_subsidized() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        mgr.get_or_create_account(&id).await.unwrap();
        // 默认 subsidy_active=true → 消耗不计入
        mgr.record_consumption(&id, CurrencyAmount::new(100)).await.unwrap();
        let stats = mgr.get_stats(&id).await.unwrap();
        assert_eq!(stats.total_consumed.as_u64(), 0);
        assert!(stats.subsidy_active);
    }

    #[tokio::test]
    async fn test_record_consumption_self_paid() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        mgr.get_or_create_account(&id).await.unwrap();
        mgr.set_subsidy_active(&id, false).await.unwrap();
        mgr.record_consumption(&id, CurrencyAmount::new(100)).await.unwrap();
        let stats = mgr.get_stats(&id).await.unwrap();
        assert_eq!(stats.total_consumed.as_u64(), 100);
        assert_eq!(stats.net_self_paid.as_u64(), 100);
    }

    #[tokio::test]
    async fn test_record_subsidy() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        mgr.get_or_create_account(&id).await.unwrap();
        mgr.record_subsidy(&id, CurrencyAmount::new(50)).await.unwrap();
        let stats = mgr.get_stats(&id).await.unwrap();
        assert_eq!(stats.total_subsidized.as_u64(), 50);
    }

    #[tokio::test]
    async fn test_get_or_create_account() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        let acc = mgr.get_or_create_account(&id).await.unwrap();
        assert_eq!(acc.agent_id, id);
        assert!(acc.subsidy_active);
    }

    #[tokio::test]
    async fn test_account_not_found() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        let result = mgr.get_stats(&id).await;
        assert!(matches!(result, Err(EconomyError::AccountNotFound(_))));
    }

    #[tokio::test]
    async fn test_set_subsidy_active() {
        let mgr = InMemoryTokenManager::new();
        let id = make_id();
        mgr.get_or_create_account(&id).await.unwrap();
        assert!(mgr.is_subsidy_active(&id).await.unwrap());
        mgr.set_subsidy_active(&id, false).await.unwrap();
        assert!(!mgr.is_subsidy_active(&id).await.unwrap());
    }
}
