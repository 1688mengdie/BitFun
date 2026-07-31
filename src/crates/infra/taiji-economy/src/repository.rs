//! 经济数据持久化（InMemory 实现，首版使用）。
//!
//! EconomyRepository trait 定义数据访问接口，后续可委托 db-store。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::AgentId;
use taiji_types::economy::{TokenAccount, StoneAccount, TransactionRecord};

use crate::market::{MarketListing, RoyaltyRecord};
use crate::EconomyError;

/// 经济数据持久化 trait。
#[async_trait]
pub trait EconomyRepository: Send + Sync {
    // Token
    async fn load_token_account(&self, agent_id: &AgentId) -> Result<Option<TokenAccount>, EconomyError>;
    async fn save_token_account(&self, account: &TokenAccount) -> Result<(), EconomyError>;

    // Stone
    async fn load_stone_account(&self, agent_id: &AgentId) -> Result<Option<StoneAccount>, EconomyError>;
    async fn save_stone_account(&self, account: &StoneAccount) -> Result<(), EconomyError>;

    // Transactions
    async fn save_transaction(&self, tx: &TransactionRecord) -> Result<(), EconomyError>;
    async fn get_transactions(&self, agent_id: &AgentId, limit: u32) -> Result<Vec<TransactionRecord>, EconomyError>;

    // Market
    async fn save_listing(&self, listing: &MarketListing) -> Result<(), EconomyError>;
    async fn load_listing(&self, listing_id: &str) -> Result<Option<MarketListing>, EconomyError>;

    // Royalty
    async fn save_royalty(&self, record: &RoyaltyRecord) -> Result<(), EconomyError>;
    async fn get_royalties(&self, agent_id: &AgentId) -> Result<Vec<RoyaltyRecord>, EconomyError>;
}

/// 内存经济数据仓库实现。
pub struct InMemoryEconomyRepository {
    token_accounts: RwLock<HashMap<AgentId, TokenAccount>>,
    stone_accounts: RwLock<HashMap<AgentId, StoneAccount>>,
    transactions: RwLock<Vec<TransactionRecord>>,
    listings: RwLock<HashMap<String, MarketListing>>,
    royalties: RwLock<Vec<RoyaltyRecord>>,
}

impl InMemoryEconomyRepository {
    pub fn new() -> Self {
        Self {
            token_accounts: RwLock::new(HashMap::new()),
            stone_accounts: RwLock::new(HashMap::new()),
            transactions: RwLock::new(Vec::new()),
            listings: RwLock::new(HashMap::new()),
            royalties: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryEconomyRepository {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl EconomyRepository for InMemoryEconomyRepository {
    async fn load_token_account(&self, agent_id: &AgentId) -> Result<Option<TokenAccount>, EconomyError> {
        let accounts = self.token_accounts.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(accounts.get(agent_id).cloned())
    }

    async fn save_token_account(&self, account: &TokenAccount) -> Result<(), EconomyError> {
        let mut accounts = self.token_accounts.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        accounts.insert(account.agent_id.clone(), account.clone());
        Ok(())
    }

    async fn load_stone_account(&self, agent_id: &AgentId) -> Result<Option<StoneAccount>, EconomyError> {
        let accounts = self.stone_accounts.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(accounts.get(agent_id).cloned())
    }

    async fn save_stone_account(&self, account: &StoneAccount) -> Result<(), EconomyError> {
        let mut accounts = self.stone_accounts.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        accounts.insert(account.agent_id.clone(), account.clone());
        Ok(())
    }

    async fn save_transaction(&self, tx: &TransactionRecord) -> Result<(), EconomyError> {
        let mut txs = self.transactions.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        txs.push(tx.clone());
        Ok(())
    }

    async fn get_transactions(&self, agent_id: &AgentId, limit: u32) -> Result<Vec<TransactionRecord>, EconomyError> {
        let txs = self.transactions.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(txs.iter().filter(|t| t.agent_id == *agent_id).take(limit as usize).cloned().collect())
    }

    async fn save_listing(&self, listing: &MarketListing) -> Result<(), EconomyError> {
        let mut listings = self.listings.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        listings.insert(listing.listing_id.clone(), listing.clone());
        Ok(())
    }

    async fn load_listing(&self, listing_id: &str) -> Result<Option<MarketListing>, EconomyError> {
        let listings = self.listings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(listings.get(listing_id).cloned())
    }

    async fn save_royalty(&self, record: &RoyaltyRecord) -> Result<(), EconomyError> {
        let mut royalties = self.royalties.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        royalties.push(record.clone());
        Ok(())
    }

    async fn get_royalties(&self, agent_id: &AgentId) -> Result<Vec<RoyaltyRecord>, EconomyError> {
        let royalties = self.royalties.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(royalties.iter().filter(|r| r.seller_id == *agent_id || r.buyer_id == *agent_id).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::economy::CurrencyAmount;

    #[tokio::test]
    async fn test_token_account_crud() {
        let repo = InMemoryEconomyRepository::new();
        let id = AgentId::new();
        let acc = TokenAccount {
            agent_id: id.clone(),
            total_consumed: CurrencyAmount::new(100),
            total_subsidized: CurrencyAmount::new(50),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        };
        repo.save_token_account(&acc).await.unwrap();
        let loaded = repo.load_token_account(&id).await.unwrap().unwrap();
        assert_eq!(loaded.total_consumed.as_u64(), 100);
    }

    #[tokio::test]
    async fn test_stone_account_crud() {
        let repo = InMemoryEconomyRepository::new();
        let id = AgentId::new();
        let acc = StoneAccount {
            agent_id: id.clone(),
            balance: CurrencyAmount::new(5000),
            lifetime_earnings: CurrencyAmount::new(10000),
            updated_at: chrono::Utc::now(),
        };
        repo.save_stone_account(&acc).await.unwrap();
        let loaded = repo.load_stone_account(&id).await.unwrap().unwrap();
        assert_eq!(loaded.balance.as_u64(), 5000);
    }

    #[tokio::test]
    async fn test_listing_crud() {
        let repo = InMemoryEconomyRepository::new();
        let listing = MarketListing {
            listing_id: "list-001".into(),
            card_id: taiji_types::card::CardId::new(1),
            seller_id: AgentId::new(),
            base_price: CurrencyAmount::new(100),
            current_price: CurrencyAmount::new(100),
            copy_count: 0,
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        repo.save_listing(&listing).await.unwrap();
        let loaded = repo.load_listing("list-001").await.unwrap().unwrap();
        assert_eq!(loaded.card_id.as_u64(), 1);
    }
}
