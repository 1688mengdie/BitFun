//! 兑换服务（灵石→灵力单向兑换）。

use async_trait::async_trait;

use taiji_types::agent::AgentId;
use taiji_types::economy::CurrencyAmount;

use crate::token::TokenManager;
use crate::stone::StoneManager;
use crate::EconomyError;

/// 汇率（有理数，避免 f64 精度问题）。
#[derive(Debug, Clone, Copy)]
pub struct ExchangeRate {
    pub stones_per_token_numerator: u64,
    pub stones_per_token_denominator: u64,
}

impl ExchangeRate {
    pub fn new(numerator: u64, denominator: u64) -> Self {
        Self { stones_per_token_numerator: numerator, stones_per_token_denominator: denominator }
    }

    /// 计算指定灵石能兑换多少 Token。
    pub fn calculate_tokens(&self, stones_amount: CurrencyAmount) -> Result<CurrencyAmount, EconomyError> {
        if self.stones_per_token_numerator == 0 || self.stones_per_token_denominator == 0 {
            return Err(EconomyError::InvalidExchangeRate("rate numerator/denominator cannot be zero".into()));
        }
        let tokens = (stones_amount.as_u64() as u128)
            * (self.stones_per_token_denominator as u128)
            / (self.stones_per_token_numerator as u128);
        if tokens > u64::MAX as u128 {
            return Err(EconomyError::ConsumptionOverflow);
        }
        Ok(CurrencyAmount::new(tokens as u64))
    }
}

impl Default for ExchangeRate {
    fn default() -> Self { Self::new(2, 1) }
}

/// 兑换服务 trait。
#[async_trait]
pub trait ExchangeService: Send + Sync {
    async fn exchange(&self, agent_id: &AgentId, stones_amount: CurrencyAmount) -> Result<CurrencyAmount, EconomyError>;
    async fn get_rate(&self) -> Result<ExchangeRate, EconomyError>;
    async fn update_rate(&self, rate: ExchangeRate) -> Result<(), EconomyError>;
}

/// 兑换服务默认实现。
pub struct ExchangeServiceImpl<T: StoneManager, U: TokenManager> {
    pub stone_mgr: T,
    pub token_mgr: U,
    rate: std::sync::RwLock<ExchangeRate>,
}

impl<T: StoneManager, U: TokenManager> ExchangeServiceImpl<T, U> {
    pub fn new(stone_mgr: T, token_mgr: U) -> Self {
        Self { stone_mgr, token_mgr, rate: std::sync::RwLock::new(ExchangeRate::default()) }
    }

    pub fn with_rate(stone_mgr: T, token_mgr: U, rate: ExchangeRate) -> Self {
        Self { stone_mgr, token_mgr, rate: std::sync::RwLock::new(rate) }
    }
}

#[async_trait]
impl<T: StoneManager + Send + Sync, U: TokenManager + Send + Sync> ExchangeService for ExchangeServiceImpl<T, U> {
    async fn exchange(&self, agent_id: &AgentId, stones_amount: CurrencyAmount) -> Result<CurrencyAmount, EconomyError> {
        if stones_amount.is_zero() { return Ok(CurrencyAmount::new(0)); }
        let rate = *self.rate.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        let tokens = rate.calculate_tokens(stones_amount)?;
        self.stone_mgr.withdraw(agent_id, stones_amount).await?;
        self.token_mgr.record_consumption(agent_id, tokens).await?;
        Ok(tokens)
    }

    async fn get_rate(&self) -> Result<ExchangeRate, EconomyError> {
        self.rate.read().map(|r| *r).map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))
    }

    async fn update_rate(&self, rate: ExchangeRate) -> Result<(), EconomyError> {
        let mut r = self.rate.write().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        *r = rate; Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::InMemoryTokenManager;
    use crate::stone::InMemoryStoneManager;

    #[test]
    fn test_rate_calculate() {
        let r = ExchangeRate::new(2, 1);
        assert_eq!(r.calculate_tokens(CurrencyAmount::new(100)).unwrap().as_u64(), 50);
    }

    #[test]
    fn test_rate_custom() {
        let r = ExchangeRate::new(5, 2);
        assert_eq!(r.calculate_tokens(CurrencyAmount::new(100)).unwrap().as_u64(), 40);
    }

    #[test]
    fn test_rate_zero_numerator() {
        assert!(matches!(ExchangeRate::new(0, 1).calculate_tokens(CurrencyAmount::new(100)),
            Err(EconomyError::InvalidExchangeRate(_))));
    }

    #[tokio::test]
    async fn test_exchange_flow() {
        let stone = InMemoryStoneManager::new();
        let token = InMemoryTokenManager::new();
        let svc = ExchangeServiceImpl::with_rate(stone, token, ExchangeRate::new(2, 1));
        let id = AgentId::new();
        svc.stone_mgr.deposit(&id, CurrencyAmount::new(1000)).await.unwrap();
        svc.token_mgr.set_subsidy_active(&id, false).await.unwrap();
        let tokens = svc.exchange(&id, CurrencyAmount::new(100)).await.unwrap();
        assert_eq!(tokens.as_u64(), 50);
        assert_eq!(svc.stone_mgr.get_balance(&id).await.unwrap().as_u64(), 900);
    }

    #[tokio::test]
    async fn test_exchange_insufficient() {
        let stone = InMemoryStoneManager::new();
        let token = InMemoryTokenManager::new();
        let svc = ExchangeServiceImpl::new(stone, token);
        let id = AgentId::new();
        svc.stone_mgr.deposit(&id, CurrencyAmount::new(50)).await.unwrap();
        assert!(matches!(svc.exchange(&id, CurrencyAmount::new(100)).await,
            Err(EconomyError::InsufficientBalance(..))));
    }
}
