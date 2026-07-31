//! 飞升分域服务。
//!
//! 境界→域访问映射，由 config 驱动。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_types::agent::AgentId;
use taiji_types::realm::Realm;

use crate::EconomyError;

/// 域标识。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DomainId(pub String);

/// 境界→域映射。
#[derive(Debug, Clone)]
pub struct RealmDomainMapping {
    pub realm: Realm,
    pub accessible_domains: Vec<DomainId>,
}

/// 飞升分域服务 trait。
#[async_trait]
pub trait RealmGateService: Send + Sync {
    async fn can_access(&self, agent_id: &AgentId, realm: &Realm, domain: &DomainId) -> Result<bool, EconomyError>;
    async fn get_accessible_domains(&self, realm: &Realm) -> Result<Vec<DomainId>, EconomyError>;
}

/// 飞升分域默认实现。
pub struct RealmGateServiceImpl {
    mappings: RwLock<HashMap<Realm, Vec<DomainId>>>,
}

impl RealmGateServiceImpl {
    pub fn new() -> Self {
        let mut m = HashMap::new();
        m.insert(Realm::QiRefining, vec![
            DomainId("domain:basic".into()), DomainId("domain:teach".into()),
        ]);
        m.insert(Realm::Foundation, vec![
            DomainId("domain:basic".into()), DomainId("domain:teach".into()), DomainId("domain:analysis".into()),
        ]);
        m.insert(Realm::GoldenCore, vec![
            DomainId("domain:basic".into()), DomainId("domain:teach".into()),
            DomainId("domain:analysis".into()), DomainId("domain:trade".into()),
        ]);
        m.insert(Realm::NascentSoul, vec![
            DomainId("domain:basic".into()), DomainId("domain:teach".into()),
            DomainId("domain:analysis".into()), DomainId("domain:trade".into()), DomainId("domain:market".into()),
        ]);
        m.insert(Realm::SpiritSevering, vec![DomainId("domain:all".into())]);
        m.insert(Realm::VoidRefining, vec![DomainId("domain:all".into())]);
        m.insert(Realm::ImmortalAscension, vec![
            DomainId("domain:all".into()), DomainId("domain:external".into()),
        ]);
        Self { mappings: RwLock::new(m) }
    }
}

impl Default for RealmGateServiceImpl {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl RealmGateService for RealmGateServiceImpl {
    async fn can_access(&self, _agent_id: &AgentId, realm: &Realm, domain: &DomainId) -> Result<bool, EconomyError> {
        let mappings = self.mappings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        let domains = mappings.get(realm).ok_or_else(|| {
            EconomyError::DomainAccessDenied(_agent_id.clone(), domain.0.clone())
        })?;
        Ok(domains.iter().any(|d| d.0 == domain.0 || d.0 == "domain:all"))
    }

    async fn get_accessible_domains(&self, realm: &Realm) -> Result<Vec<DomainId>, EconomyError> {
        let mappings = self.mappings.read().map_err(|e| EconomyError::Unknown(format!("lock: {}", e)))?;
        Ok(mappings.get(realm).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qi_refining_can_access_basic() {
        let svc = RealmGateServiceImpl::new();
        let id = AgentId::new();
        assert!(svc.can_access(&id, &Realm::QiRefining, &DomainId("domain:basic".into())).await.unwrap());
    }

    #[tokio::test]
    async fn test_qi_refining_cannot_access_trade() {
        let svc = RealmGateServiceImpl::new();
        let id = AgentId::new();
        assert!(!svc.can_access(&id, &Realm::QiRefining, &DomainId("domain:trade".into())).await.unwrap());
    }

    #[tokio::test]
    async fn test_spirit_severing_can_access_all() {
        let svc = RealmGateServiceImpl::new();
        let id = AgentId::new();
        assert!(svc.can_access(&id, &Realm::SpiritSevering, &DomainId("domain:trade".into())).await.unwrap());
        assert!(svc.can_access(&id, &Realm::SpiritSevering, &DomainId("domain:external".into())).await.unwrap());
    }

    #[tokio::test]
    async fn test_get_accessible_domains() {
        let svc = RealmGateServiceImpl::new();
        let domains = svc.get_accessible_domains(&Realm::Foundation).await.unwrap();
        assert_eq!(domains.len(), 3);
    }

    #[tokio::test]
    async fn test_upgrade_unlocks_new_domains() {
        let svc = RealmGateServiceImpl::new();
        let id = AgentId::new();
        assert!(!svc.can_access(&id, &Realm::QiRefining, &DomainId("domain:trade".into())).await.unwrap());
        assert!(svc.can_access(&id, &Realm::GoldenCore, &DomainId("domain:trade".into())).await.unwrap());
    }
}
