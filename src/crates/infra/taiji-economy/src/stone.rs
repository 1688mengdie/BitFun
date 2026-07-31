//! 灵石（Stone）管理器。
//!
//! 灵石是资产，有余额（balance）。支持充值/消费/转账。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::AgentId;
use taiji_types::economy::{CurrencyAmount, CurrencyType, StoneAccount, TransactionRecord, TransactionType};

use crate::EconomyError;

/// 灵石（Stone）管理器 trait。
#[async_trait]
pub trait StoneManager: Send + Sync {
    /// 充值（增加余额）。
    async fn deposit(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError>;
    /// 消费（减少余额）。
    async fn withdraw(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError>;
    /// 转账（A→B）。
    async fn transfer(&self, from: &AgentId, to: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError>;
    /// 查询余额。
    async fn get_balance(&self, agent_id: &AgentId) -> Result<CurrencyAmount, EconomyError>;
    /// 查询或创建账户。
    async fn get_or_create_account(&self, agent_id: &AgentId) -> Result<StoneAccount, EconomyError>;
    /// 获取交易历史。
    async fn get_transaction_history(&self, agent_id: &AgentId, limit: u32) -> Result<Vec<TransactionRecord>, EconomyError>;
    /// 获取单条交易记录。
    async fn get_transaction(&self, tx_id: &str) -> Result<Option<TransactionRecord>, EconomyError>;
}

/// 内存灵石管理器实现。
pub struct InMemoryStoneManager {
    accounts: RwLock<HashMap<AgentId, StoneAccount>>,
    transactions: RwLock<Vec<TransactionRecord>>,
}

impl InMemoryStoneManager {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            transactions: RwLock::new(Vec::new()),
        }
    }

    fn record_tx(&self, tx: TransactionRecord) {
        if let Ok(mut txs) = self.transactions.write() {
            txs.push(tx);
        }
    }
}

impl Default for InMemoryStoneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StoneManager for InMemoryStoneManager {
    async fn deposit(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.entry(agent_id.clone()).or_insert_with(|| StoneAccount {
            agent_id: agent_id.clone(),
            balance: CurrencyAmount::new(0),
            lifetime_earnings: CurrencyAmount::new(0),
            updated_at: chrono::Utc::now(),
        });
        account.balance = account.balance.saturating_add(amount);
        account.lifetime_earnings = account.lifetime_earnings.saturating_add(amount);
        account.updated_at = chrono::Utc::now();

        self.record_tx(TransactionRecord {
            tx_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.clone(),
            counterparty: None,
            amount,
            currency_type: CurrencyType::Stone,
            tx_type: TransactionType::Deposit,
            timestamp: chrono::Utc::now(),
            description: "灵石充值".into(),
            metadata: None,
        });
        Ok(())
    }

    async fn withdraw(&self, agent_id: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.get_mut(agent_id).ok_or_else(|| EconomyError::AccountNotFound(agent_id.clone()))?;
        if account.balance < amount {
            return Err(EconomyError::InsufficientBalance(amount, account.balance));
        }
        account.balance = account.balance.saturating_sub(amount);
        account.updated_at = chrono::Utc::now();

        self.record_tx(TransactionRecord {
            tx_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.clone(),
            counterparty: None,
            amount,
            currency_type: CurrencyType::Stone,
            tx_type: TransactionType::Withdrawal,
            timestamp: chrono::Utc::now(),
            description: "灵石消费".into(),
            metadata: None,
        });
        Ok(())
    }

    async fn transfer(&self, from: &AgentId, to: &AgentId, amount: CurrencyAmount) -> Result<(), EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let from_acc = accounts.get_mut(from).ok_or_else(|| EconomyError::AccountNotFound(from.clone()))?;
        if from_acc.balance < amount {
            return Err(EconomyError::InsufficientBalance(amount, from_acc.balance));
        }
        from_acc.balance = from_acc.balance.saturating_sub(amount);
        from_acc.updated_at = chrono::Utc::now();

        let to_acc = accounts.entry(to.clone()).or_insert_with(|| StoneAccount {
            agent_id: to.clone(),
            balance: CurrencyAmount::new(0),
            lifetime_earnings: CurrencyAmount::new(0),
            updated_at: chrono::Utc::now(),
        });
        to_acc.balance = to_acc.balance.saturating_add(amount);
        to_acc.updated_at = chrono::Utc::now();

        self.record_tx(TransactionRecord {
            tx_id: uuid::Uuid::new_v4().to_string(),
            agent_id: from.clone(),
            counterparty: Some(to.clone()),
            amount,
            currency_type: CurrencyType::Stone,
            tx_type: TransactionType::Transfer,
            timestamp: chrono::Utc::now(),
            description: format!("灵石转账至 {}", to),
            metadata: None,
        });
        Ok(())
    }

    async fn get_balance(&self, agent_id: &AgentId) -> Result<CurrencyAmount, EconomyError> {
        let accounts = self.accounts.read().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let account = accounts.get(agent_id).ok_or_else(|| EconomyError::AccountNotFound(agent_id.clone()))?;
        Ok(account.balance)
    }

    async fn get_or_create_account(&self, agent_id: &AgentId) -> Result<StoneAccount, EconomyError> {
        let mut accounts = self.accounts.write().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        Ok(accounts.entry(agent_id.clone()).or_insert_with(|| StoneAccount {
            agent_id: agent_id.clone(),
            balance: CurrencyAmount::new(0),
            lifetime_earnings: CurrencyAmount::new(0),
            updated_at: chrono::Utc::now(),
        }).clone())
    }

    async fn get_transaction_history(&self, agent_id: &AgentId, limit: u32) -> Result<Vec<TransactionRecord>, EconomyError> {
        let txs = self.transactions.read().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        let filtered: Vec<_> = txs.iter()
            .filter(|tx| tx.agent_id == *agent_id || tx.counterparty.as_ref() == Some(agent_id))
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn get_transaction(&self, tx_id: &str) -> Result<Option<TransactionRecord>, EconomyError> {
        let txs = self.transactions.read().map_err(|e| {
            EconomyError::Unknown(format!("lock error: {}", e))
        })?;
        Ok(txs.iter().find(|tx| tx.tx_id == tx_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id() -> AgentId {
        AgentId::new()
    }

    #[tokio::test]
    async fn test_deposit() {
        let mgr = InMemoryStoneManager::new();
        let id = make_id();
        mgr.deposit(&id, CurrencyAmount::new(1000)).await.unwrap();
        assert_eq!(mgr.get_balance(&id).await.unwrap().as_u64(), 1000);
    }

    #[tokio::test]
    async fn test_withdraw_ok() {
        let mgr = InMemoryStoneManager::new();
        let id = make_id();
        mgr.deposit(&id, CurrencyAmount::new(1000)).await.unwrap();
        mgr.withdraw(&id, CurrencyAmount::new(300)).await.unwrap();
        assert_eq!(mgr.get_balance(&id).await.unwrap().as_u64(), 700);
    }

    #[tokio::test]
    async fn test_withdraw_insufficient() {
        let mgr = InMemoryStoneManager::new();
        let id = make_id();
        mgr.deposit(&id, CurrencyAmount::new(100)).await.unwrap();
        let result = mgr.withdraw(&id, CurrencyAmount::new(200)).await;
        assert!(matches!(result, Err(EconomyError::InsufficientBalance(..))));
    }

    #[tokio::test]
    async fn test_transfer() {
        let mgr = InMemoryStoneManager::new();
        let alice = make_id();
        let bob = make_id();
        mgr.deposit(&alice, CurrencyAmount::new(500)).await.unwrap();
        mgr.transfer(&alice, &bob, CurrencyAmount::new(200)).await.unwrap();
        assert_eq!(mgr.get_balance(&alice).await.unwrap().as_u64(), 300);
        assert_eq!(mgr.get_balance(&bob).await.unwrap().as_u64(), 200);
    }

    #[tokio::test]
    async fn test_account_not_found() {
        let mgr = InMemoryStoneManager::new();
        let id = make_id();
        let result = mgr.get_balance(&id).await;
        assert!(matches!(result, Err(EconomyError::AccountNotFound(_))));
    }

    #[tokio::test]
    async fn test_transaction_history() {
        let mgr = InMemoryStoneManager::new();
        let id = make_id();
        mgr.deposit(&id, CurrencyAmount::new(1000)).await.unwrap();
        mgr.withdraw(&id, CurrencyAmount::new(300)).await.unwrap();
        let history = mgr.get_transaction_history(&id, 10).await.unwrap();
        assert_eq!(history.len(), 2);
    }
}
