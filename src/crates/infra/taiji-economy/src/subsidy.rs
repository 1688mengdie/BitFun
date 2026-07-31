//! 新手补贴服务。
//!
//! 炼气~金丹境界自动补贴 Token，突破元婴后停止。

use async_trait::async_trait;

use taiji_types::agent::AgentId;
use taiji_types::realm::Realm;

use crate::token::TokenManager;
use crate::EconomyError;

/// 补贴配置。
#[derive(Debug, Clone)]
pub struct SubsidyConfig {
    pub eligible_realms: Vec<Realm>,
    pub unlimited_tokens: bool,
}

impl Default for SubsidyConfig {
    fn default() -> Self {
        Self {
            eligible_realms: vec![Realm::QiRefining, Realm::Foundation, Realm::GoldenCore],
            unlimited_tokens: true,
        }
    }
}

/// 新手补贴服务 trait。
#[async_trait]
pub trait SubsidyService: Send + Sync {
    async fn is_eligible(&self, realm: &Realm) -> Result<bool, EconomyError>;
    async fn on_realm_upgrade(&self, agent_id: &AgentId, new_realm: &Realm) -> Result<(), EconomyError>;
    async fn get_config(&self) -> Result<SubsidyConfig, EconomyError>;
}

/// 补贴服务默认实现。
pub struct SubsidyServiceImpl<T: TokenManager> {
    pub token_mgr: T,
    config: std::sync::RwLock<SubsidyConfig>,
}

impl<T: TokenManager> SubsidyServiceImpl<T> {
    pub fn new(token_mgr: T) -> Self {
        Self { token_mgr, config: std::sync::RwLock::new(SubsidyConfig::default()) }
    }

    pub fn with_config(token_mgr: T, config: SubsidyConfig) -> Self {
        Self { token_mgr, config: std::sync::RwLock::new(config) }
    }
}

#[async_trait]
impl<T: TokenManager + Send + Sync> SubsidyService for SubsidyServiceImpl<T> {
    async fn is_eligible(&self, realm: &Realm) -> Result<bool, EconomyError> {
        let config = self.config.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(config.eligible_realms.contains(realm))
    }

    async fn on_realm_upgrade(&self, agent_id: &AgentId, new_realm: &Realm) -> Result<(), EconomyError> {
        // 突破元婴（含）以上 → 停止补贴
        let should_stop = *new_realm >= Realm::NascentSoul;
        if should_stop {
            self.token_mgr.set_subsidy_active(agent_id, false).await?;
        }
        Ok(())
    }

    async fn get_config(&self) -> Result<SubsidyConfig, EconomyError> {
        self.config.read().map(|c| c.clone()).map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::InMemoryTokenManager;

    #[tokio::test]
    async fn test_is_eligible_qi_refining() {
        let svc = SubsidyServiceImpl::new(InMemoryTokenManager::new());
        assert!(svc.is_eligible(&Realm::QiRefining).await.unwrap());
        assert!(svc.is_eligible(&Realm::Foundation).await.unwrap());
        assert!(svc.is_eligible(&Realm::GoldenCore).await.unwrap());
        assert!(!svc.is_eligible(&Realm::NascentSoul).await.unwrap());
        assert!(!svc.is_eligible(&Realm::SpiritSevering).await.unwrap());
    }

    #[tokio::test]
    async fn test_on_realm_upgrade_stops_subsidy() {
        let token = InMemoryTokenManager::new();
        let svc = SubsidyServiceImpl::new(token);
        let id = AgentId::new();
        svc.token_mgr.get_or_create_account(&id).await.unwrap();
        assert!(svc.token_mgr.is_subsidy_active(&id).await.unwrap());

        svc.on_realm_upgrade(&id, &Realm::NascentSoul).await.unwrap();
        assert!(!svc.token_mgr.is_subsidy_active(&id).await.unwrap());
    }

    #[tokio::test]
    async fn test_on_realm_upgrade_keeps_subsidy() {
        let token = InMemoryTokenManager::new();
        let svc = SubsidyServiceImpl::new(token);
        let id = AgentId::new();
        svc.token_mgr.get_or_create_account(&id).await.unwrap();
        svc.on_realm_upgrade(&id, &Realm::Foundation).await.unwrap();
        assert!(svc.token_mgr.is_subsidy_active(&id).await.unwrap());
    }
}
