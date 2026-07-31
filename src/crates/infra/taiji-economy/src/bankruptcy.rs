//! 入不敷出判定（R-EC-503）。
//!
//! 高手（元婴+）自负盈亏。当 Token 消耗量 >> 灵石余额时，触发境界掉落警告。
//! 境界掉落通过 event-bus 广播给 agent-system 执行。

use async_trait::async_trait;

use taiji_types::agent::AgentId;
use taiji_types::economy::CurrencyAmount;
use taiji_types::realm::Realm;

use crate::token::TokenManager;
use crate::stone::StoneManager;
use crate::EconomyError;

/// 入不敷出判定结果。
#[derive(Debug, Clone, PartialEq)]
pub enum BankruptcyVerdict {
    /// 财务健康
    Solvent,
    /// 警告：建议减少 Token 消耗
    Warning { ratio: f64 },
    /// 濒临破产：触发境界掉落
    Critical { current_realm: Realm, demoted_to: Realm },
}

/// 境界降级映射（元婴→金丹，化神→元婴，炼虚→化神，飞升→炼虚）。
pub fn demote_realm(realm: &Realm) -> Option<Realm> {
    match realm {
        Realm::NascentSoul => Some(Realm::GoldenCore),
        Realm::SpiritSevering => Some(Realm::NascentSoul),
        Realm::VoidRefining => Some(Realm::SpiritSevering),
        Realm::ImmortalAscension => Some(Realm::VoidRefining),
        _ => None, // 炼气~金丹不降级（新手保护）
    }
}

/// 入不敷出判定服务。
#[async_trait]
pub trait BankruptcyService: Send + Sync {
    /// 评估 Agent 财务状况。
    async fn assess(&self, agent_id: &AgentId, realm: &Realm) -> Result<BankruptcyVerdict, EconomyError>;

    /// 触发境界掉落（返回新境界）。
    async fn execute_demotion(&self, agent_id: &AgentId, realm: &Realm) -> Result<Realm, EconomyError>;
}

/// 入不敷出判定默认实现。
pub struct BankruptcyServiceImpl<T: TokenManager, U: StoneManager> {
    token_mgr: T,
    stone_mgr: U,
    /// Token 消耗与灵石余额比率上限（默认 10.0 = 消耗量是余额 10 倍时警告）
    warning_ratio: f64,
    /// 临界比率（默认 50.0 = 消耗量是余额 50 倍时触发降级）
    critical_ratio: f64,
}

impl<T: TokenManager, U: StoneManager> BankruptcyServiceImpl<T, U> {
    pub fn new(token_mgr: T, stone_mgr: U) -> Self {
        Self {
            token_mgr,
            stone_mgr,
            warning_ratio: 10.0,
            critical_ratio: 50.0,
        }
    }

    pub fn with_ratios(token_mgr: T, stone_mgr: U, warning_ratio: f64, critical_ratio: f64) -> Self {
        Self { token_mgr, stone_mgr, warning_ratio, critical_ratio }
    }
}

#[async_trait]
impl<T: TokenManager + Send + Sync, U: StoneManager + Send + Sync> BankruptcyService for BankruptcyServiceImpl<T, U> {
    async fn assess(&self, agent_id: &AgentId, realm: &Realm) -> Result<BankruptcyVerdict, EconomyError> {
        // 新手保护：炼气~金丹不判定
        if *realm <= Realm::GoldenCore {
            return Ok(BankruptcyVerdict::Solvent);
        }

        let stats = match self.token_mgr.get_stats(agent_id).await {
            Ok(s) => s,
            Err(_) => return Ok(BankruptcyVerdict::Solvent), // 无账户 = 无消耗
        };

        if stats.net_self_paid.is_zero() {
            return Ok(BankruptcyVerdict::Solvent);
        }

        let balance = match self.stone_mgr.get_balance(agent_id).await {
            Ok(b) => b,
            Err(_) => CurrencyAmount::new(0),
        };

        if balance.is_zero() {
            // 余额为 0 但消耗很大 → 临界
            if stats.net_self_paid.as_u64() > 1000 {
                if let Some(demoted) = demote_realm(realm) {
                    return Ok(BankruptcyVerdict::Critical { current_realm: *realm, demoted_to: demoted });
                }
            }
            return Ok(BankruptcyVerdict::Warning { ratio: f64::MAX });
        }

        let ratio = stats.net_self_paid.as_u64() as f64 / balance.as_u64() as f64;

        if ratio >= self.critical_ratio {
            if let Some(demoted) = demote_realm(realm) {
                return Ok(BankruptcyVerdict::Critical { current_realm: *realm, demoted_to: demoted });
            }
            Ok(BankruptcyVerdict::Warning { ratio })
        } else if ratio >= self.warning_ratio {
            Ok(BankruptcyVerdict::Warning { ratio })
        } else {
            Ok(BankruptcyVerdict::Solvent)
        }
    }

    async fn execute_demotion(&self, agent_id: &AgentId, realm: &Realm) -> Result<Realm, EconomyError> {
        demote_realm(realm).ok_or_else(|| EconomyError::Unknown(format!(
            "agent {} at realm {:?} cannot be demoted further", agent_id, realm
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::InMemoryTokenManager;
    use crate::stone::InMemoryStoneManager;

    #[tokio::test]
    async fn test_novice_protection() {
        let token = InMemoryTokenManager::new();
        let stone = InMemoryStoneManager::new();
        let svc = BankruptcyServiceImpl::new(token, stone);
        let id = AgentId::new();

        // 炼气期不判定
        let verdict = svc.assess(&id, &Realm::QiRefining).await.unwrap();
        assert_eq!(verdict, BankruptcyVerdict::Solvent);
    }

    #[tokio::test]
    async fn test_solvent() {
        let token = InMemoryTokenManager::new();
        let stone = InMemoryStoneManager::new();
        let svc = BankruptcyServiceImpl::new(token, stone);
        let id = AgentId::new();

        svc.token_mgr.get_or_create_account(&id).await.unwrap();
        svc.token_mgr.set_subsidy_active(&id, false).await.unwrap();
        svc.stone_mgr.deposit(&id, CurrencyAmount::new(10000)).await.unwrap();
        svc.token_mgr.record_consumption(&id, CurrencyAmount::new(100)).await.unwrap();

        let verdict = svc.assess(&id, &Realm::NascentSoul).await.unwrap();
        assert_eq!(verdict, BankruptcyVerdict::Solvent);
    }

    #[tokio::test]
    async fn test_warning() {
        let token = InMemoryTokenManager::new();
        let stone = InMemoryStoneManager::new();
        let svc = BankruptcyServiceImpl::with_ratios(token, stone, 2.0, 10.0); // 低阈值方便测试
        let id = AgentId::new();

        svc.token_mgr.get_or_create_account(&id).await.unwrap();
        svc.token_mgr.set_subsidy_active(&id, false).await.unwrap();
        svc.stone_mgr.deposit(&id, CurrencyAmount::new(100)).await.unwrap();
        svc.token_mgr.record_consumption(&id, CurrencyAmount::new(300)).await.unwrap(); // ratio = 3.0

        let verdict = svc.assess(&id, &Realm::NascentSoul).await.unwrap();
        assert!(matches!(verdict, BankruptcyVerdict::Warning { .. }));
    }

    #[tokio::test]
    async fn test_critical_triggers_demotion() {
        let token = InMemoryTokenManager::new();
        let stone = InMemoryStoneManager::new();
        let svc = BankruptcyServiceImpl::with_ratios(token, stone, 2.0, 3.0);
        let id = AgentId::new();

        svc.token_mgr.get_or_create_account(&id).await.unwrap();
        svc.token_mgr.set_subsidy_active(&id, false).await.unwrap();
        svc.stone_mgr.deposit(&id, CurrencyAmount::new(100)).await.unwrap();
        svc.token_mgr.record_consumption(&id, CurrencyAmount::new(500)).await.unwrap(); // ratio = 5.0

        let verdict = svc.assess(&id, &Realm::NascentSoul).await.unwrap();
        assert!(matches!(verdict, BankruptcyVerdict::Critical { .. }));
        if let BankruptcyVerdict::Critical { current_realm, demoted_to } = verdict {
            assert_eq!(current_realm, Realm::NascentSoul);
            assert_eq!(demoted_to, Realm::GoldenCore);
        }
    }

    #[test]
    fn test_demote_realm() {
        assert_eq!(demote_realm(&Realm::NascentSoul), Some(Realm::GoldenCore));
        assert_eq!(demote_realm(&Realm::SpiritSevering), Some(Realm::NascentSoul));
        assert_eq!(demote_realm(&Realm::VoidRefining), Some(Realm::SpiritSevering));
        assert_eq!(demote_realm(&Realm::ImmortalAscension), Some(Realm::VoidRefining));
        assert_eq!(demote_realm(&Realm::QiRefining), None); // 新手保护
        assert_eq!(demote_realm(&Realm::GoldenCore), None);
    }
}
