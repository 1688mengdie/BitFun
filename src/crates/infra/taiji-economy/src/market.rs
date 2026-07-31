//! 坊市服务（卡片复制+版税）。
//!
//! 参考：react-xiuxian-game shopService.ts 模板池+价格计算模式。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::AgentId;
use taiji_types::card::CardId;
use taiji_types::economy::CurrencyAmount;

use crate::stone::StoneManager;
use crate::EconomyError;

/// 坊市挂牌。
#[derive(Debug, Clone)]
pub struct MarketListing {
    pub listing_id: String,
    pub card_id: CardId,
    pub seller_id: AgentId,
    pub base_price: CurrencyAmount,
    pub current_price: CurrencyAmount,
    pub copy_count: u32,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 坊市筛选条件。
#[derive(Debug, Clone, Default)]
pub struct MarketFilter {
    pub max_price: Option<CurrencyAmount>,
    pub seller_id: Option<AgentId>,
    pub sort_by: MarketSortBy,
}

/// 坊市排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketSortBy {
    PriceAsc, PriceDesc,
    #[default]
    Newest, CopyCount,
}

/// 版税记录。
#[derive(Debug, Clone)]
pub struct RoyaltyRecord {
    pub royalty_id: String,
    pub listing_id: String,
    pub card_id: CardId,
    pub seller_id: AgentId,
    pub buyer_id: AgentId,
    pub royalty_amount: CurrencyAmount,
    pub platform_fee: CurrencyAmount,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 卡牌存根 trait（首版简化，卡片系统就绪后迁移到真实 CardManager）。
#[async_trait]
pub trait CardStub: Send + Sync {
    async fn is_soul_card(&self, card_id: &CardId) -> Result<bool, EconomyError>;
    async fn royalty_rate(&self, card_id: &CardId) -> Result<f64, EconomyError>;
}

/// 简化卡牌存根（所有卡牌都是非本命卡，版税率 10%）。
pub struct DefaultCardStub;

#[async_trait]
impl CardStub for DefaultCardStub {
    async fn is_soul_card(&self, _card_id: &CardId) -> Result<bool, EconomyError> {
        Ok(false) // 默认不是本命魂卡
    }

    async fn royalty_rate(&self, _card_id: &CardId) -> Result<f64, EconomyError> {
        Ok(0.1) // 默认版税率 10%
    }
}

/// 坊市服务 trait。
#[async_trait]
pub trait MarketService: Send + Sync {
    async fn list_card(&self, seller: &AgentId, card_id: CardId, price: CurrencyAmount) -> Result<String, EconomyError>;
    async fn unlist_card(&self, listing_id: &str) -> Result<(), EconomyError>;
    async fn copy_card(&self, buyer: &AgentId, listing_id: &str) -> Result<CardId, EconomyError>;
    async fn get_listings(&self, filter: &MarketFilter) -> Result<Vec<MarketListing>, EconomyError>;
    async fn get_listing(&self, listing_id: &str) -> Result<Option<MarketListing>, EconomyError>;
    async fn get_royalty_history(&self, agent_id: &AgentId) -> Result<Vec<RoyaltyRecord>, EconomyError>;
}

/// 坊市服务默认实现。
pub struct MarketServiceImpl<T: StoneManager, U: CardStub> {
    pub stone_mgr: T,
    card_stub: U,
    listings: RwLock<HashMap<String, MarketListing>>,
    royalties: RwLock<Vec<RoyaltyRecord>>,
    platform_fee_percent: u32,
    price_increase_per_copy_percent: u32,
    max_price_multiplier: u32,
}

impl<T: StoneManager, U: CardStub> MarketServiceImpl<T, U> {
    pub fn new(stone_mgr: T, card_stub: U) -> Self {
        Self {
            stone_mgr,
            card_stub,
            listings: RwLock::new(HashMap::new()),
            royalties: RwLock::new(Vec::new()),
            platform_fee_percent: 5,
            price_increase_per_copy_percent: 10,
            max_price_multiplier: 3,
        }
    }

    fn calculate_copy_price(&self, listing: &MarketListing) -> CurrencyAmount {
        let base = listing.base_price.as_u64();
        let max_price = base * self.max_price_multiplier as u64;
        let mut price = base;
        for _ in 0..listing.copy_count {
            price = price * (100 + self.price_increase_per_copy_percent as u64) / 100;
            if price >= max_price {
                return CurrencyAmount::new(max_price);
            }
        }
        CurrencyAmount::new(price)
    }
}

#[async_trait]
impl<T: StoneManager + Send + Sync, U: CardStub + Send + Sync> MarketService for MarketServiceImpl<T, U> {
    async fn list_card(&self, seller: &AgentId, card_id: CardId, price: CurrencyAmount) -> Result<String, EconomyError> {
        if price.is_zero() {
            return Err(EconomyError::InvalidPrice(price));
        }
        let listing_id = uuid::Uuid::new_v4().to_string();
        let listing = MarketListing {
            listing_id: listing_id.clone(),
            card_id,
            seller_id: seller.clone(),
            base_price: price,
            current_price: price,
            copy_count: 0,
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        let mut listings = self.listings.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        listings.insert(listing_id.clone(), listing);
        Ok(listing_id)
    }

    async fn unlist_card(&self, listing_id: &str) -> Result<(), EconomyError> {
        let mut listings = self.listings.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        let listing = listings.get_mut(listing_id).ok_or_else(|| EconomyError::ListingNotFound(listing_id.into()))?;
        listing.is_active = false;
        Ok(())
    }

    async fn copy_card(&self, buyer: &AgentId, listing_id: &str) -> Result<CardId, EconomyError> {
        let listing = {
            let listings = self.listings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
            listings.get(listing_id).cloned().ok_or_else(|| EconomyError::ListingNotFound(listing_id.into()))?
        };

        if !listing.is_active {
            return Err(EconomyError::ListingNotFound(listing_id.into()));
        }

        // 检查本命魂卡
        if self.card_stub.is_soul_card(&listing.card_id).await? {
            return Err(EconomyError::CannotCopySoulCard);
        }

        // 计算当前价格
        let current_price = self.calculate_copy_price(&listing);
        let royalty_rate = self.card_stub.royalty_rate(&listing.card_id).await?;

        // 买方扣款
        self.stone_mgr.withdraw(buyer, current_price).await?;

        // 计算版税和平台费
        let royalty_amount = CurrencyAmount::new(
            (current_price.as_u64() as f64 * royalty_rate) as u64
        );
        let platform_fee = CurrencyAmount::new(
            (current_price.as_u64() as f64 * self.platform_fee_percent as f64 / 100.0) as u64
        );

        // 卖家收入 = 版税 - 平台费
        let seller_net = if royalty_amount.as_u64() > platform_fee.as_u64() {
            CurrencyAmount::new(royalty_amount.as_u64() - platform_fee.as_u64())
        } else {
            CurrencyAmount::new(0)
        };

        // 卖家入账
        if !seller_net.is_zero() {
            self.stone_mgr.deposit(&listing.seller_id, seller_net).await?;
        }

        // 更新挂牌
        {
            let mut listings = self.listings.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
            if let Some(l) = listings.get_mut(listing_id) {
                l.copy_count += 1;
                l.current_price = self.calculate_copy_price(l);
            }
        }

        // 记录版税
        let record = RoyaltyRecord {
            royalty_id: uuid::Uuid::new_v4().to_string(),
            listing_id: listing_id.into(),
            card_id: listing.card_id,
            seller_id: listing.seller_id,
            buyer_id: buyer.clone(),
            royalty_amount,
            platform_fee,
            timestamp: chrono::Utc::now(),
        };
        if let Ok(mut royalties) = self.royalties.write() {
            royalties.push(record);
        }

        Ok(listing.card_id)
    }

    async fn get_listings(&self, filter: &MarketFilter) -> Result<Vec<MarketListing>, EconomyError> {
        let listings = self.listings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        let mut result: Vec<MarketListing> = listings.values()
            .filter(|l| l.is_active)
            .filter(|l| filter.max_price.is_none_or(|mp| l.current_price <= mp))
            .filter(|l| filter.seller_id.as_ref().is_none_or(|sid| l.seller_id == *sid))
            .cloned()
            .collect();

        match filter.sort_by {
            MarketSortBy::PriceAsc => result.sort_by_key(|a| a.current_price),
            MarketSortBy::PriceDesc => result.sort_by_key(|b| std::cmp::Reverse(b.current_price)),
            MarketSortBy::Newest => result.sort_by_key(|b| std::cmp::Reverse(b.created_at)),
            MarketSortBy::CopyCount => result.sort_by_key(|b| std::cmp::Reverse(b.copy_count)),
        }
        Ok(result)
    }

    async fn get_listing(&self, listing_id: &str) -> Result<Option<MarketListing>, EconomyError> {
        let listings = self.listings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(listings.get(listing_id).cloned().filter(|l| l.is_active))
    }

    async fn get_royalty_history(&self, agent_id: &AgentId) -> Result<Vec<RoyaltyRecord>, EconomyError> {
        let royalties = self.royalties.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(royalties.iter().filter(|r| r.seller_id == *agent_id || r.buyer_id == *agent_id).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stone::InMemoryStoneManager;

    #[tokio::test]
    async fn test_list_and_copy() {
        let stone = InMemoryStoneManager::new();
        let card = DefaultCardStub;
        let market = MarketServiceImpl::new(stone, card);

        let seller = AgentId::new();
        let buyer = AgentId::new();
        let card_id = CardId::new(42);

        // 上架
        let listing_id = market.list_card(&seller, card_id, CurrencyAmount::new(100)).await.unwrap();
        assert!(!listing_id.is_empty());

        // 买家充值
        market.stone_mgr.deposit(&buyer, CurrencyAmount::new(1000)).await.unwrap();

        // 复制
        let copied = market.copy_card(&buyer, &listing_id).await.unwrap();
        assert_eq!(copied, card_id);

        // 卖家收到版税
        assert!(market.stone_mgr.get_balance(&seller).await.unwrap().as_u64() > 0);
    }

    #[tokio::test]
    async fn test_unlist() {
        let stone = InMemoryStoneManager::new();
        let card = DefaultCardStub;
        let market = MarketServiceImpl::new(stone, card);

        let seller = AgentId::new();
        let listing_id = market.list_card(&seller, CardId::new(1), CurrencyAmount::new(100)).await.unwrap();
        market.unlist_card(&listing_id).await.unwrap();
        assert!(market.get_listing(&listing_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_price_scarcity() {
        let stone = InMemoryStoneManager::new();
        let card = DefaultCardStub;
        let market = MarketServiceImpl::new(stone, card);

        let seller = AgentId::new();
        let buyer = AgentId::new();
        market.stone_mgr.deposit(&buyer, CurrencyAmount::new(10000)).await.unwrap();

        let listing_id = market.list_card(&seller, CardId::new(1), CurrencyAmount::new(100)).await.unwrap();

        // 复制 3 次，价格应从 100 → 110 → 121 → 133
        for _ in 0..3 {
            market.copy_card(&buyer, &listing_id).await.unwrap();
        }

        let listing = market.get_listing(&listing_id).await.unwrap().unwrap();
        assert_eq!(listing.copy_count, 3);
        // 100 * 1.1^3 = 133.1 → trunc 133
        assert_eq!(listing.current_price.as_u64(), 133);
    }

    #[tokio::test]
    async fn test_list_price_zero_rejected() {
        let market = MarketServiceImpl::new(InMemoryStoneManager::new(), DefaultCardStub);
        let result = market.list_card(&AgentId::new(), CardId::new(1), CurrencyAmount::new(0)).await;
        assert!(matches!(result, Err(EconomyError::InvalidPrice(_))));
    }
}
