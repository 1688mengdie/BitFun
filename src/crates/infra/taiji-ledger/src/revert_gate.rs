//! 夺舍门禁（LedgerRevertGate）— 转世重生/夺舍操作的消耗品门控。
//!
//! 架构总纲 §5.1：
//! - 转世重生 = git checkout（消耗天材地宝）
//! - 夺舍 = git revert（消耗天材地宝）
//!
//! 本模块提供带消耗品检查的 revert 操作门禁。
//! 通过 TreasureSpender trait 解耦，不直接依赖经济系统 crate。

use async_trait::async_trait;
use taiji_types::agent::AgentId;
use taiji_types::economy::TreasureItem;

use crate::error::LedgerError;
use crate::git_ops::GitLedger;

/// 天材地宝消耗接口 — 供 LedgerRevertGate 调用扣减道具。
///
/// 由上层（经济系统或应用层）注入实现。
#[async_trait]
pub trait TreasureSpender: Send + Sync {
    /// 检查 Agent 是否持有足够的天材地宝。
    async fn has_sufficient(&self, agent_id: &AgentId, item: &TreasureItem) -> Result<bool, LedgerError>;

    /// 消耗天材地宝（扣减余额/库存）。
    async fn consume(&self, agent_id: &AgentId, item: TreasureItem, reason: &str) -> Result<(), LedgerError>;
}

/// 夺舍门禁 — 消耗天材地宝后执行 git revert/reincarnate。
///
/// # 数据流
/// 1. 检查 Agent 是否持有足够道具（TreasureSpender::has_sufficient）
/// 2. 消耗道具（TreasureSpender::consume）
/// 3. 执行 git reincarnate（hard reset 到目标 commit）
/// 4. 返回 commit hash
pub struct LedgerRevertGate {
    git_ledger: GitLedger,
    spender: Box<dyn TreasureSpender>,
}

impl LedgerRevertGate {
    /// 创建夺舍门禁。
    pub fn new(git_ledger: GitLedger, spender: Box<dyn TreasureSpender>) -> Self {
        Self { git_ledger, spender }
    }

    /// 带消耗品检查的转世重生（git checkout / hard reset）。
    ///
    /// 消耗指定天材地宝后，将 Agent 状态回滚到目标 commit。
    ///
    /// # 参数
    /// - `agent_id`: Agent 道号
    /// - `target_commit`: 目标 commit hash
    /// - `cost`: 消耗的天材地宝（RebirthToken 或 SpiritStones）
    ///
    /// # 返回
    /// - `Ok(())`: 成功回滚
    /// - `Err(LedgerError)`: 余额不足/道具不足/git 错误
    pub async fn revert_with_cost(
        &self,
        agent_id: &AgentId,
        target_commit: &str,
        cost: TreasureItem,
    ) -> Result<(), LedgerError> {
        // Step 1: 检查 Agent 是否持有足够道具
        let has_sufficient = self.spender.has_sufficient(agent_id, &cost).await?;
        if !has_sufficient {
            let equivalent = cost.stone_equivalent();
            return Err(LedgerError::InsufficientTreasure {
                agent_id: agent_id.clone(),
                required: equivalent,
                item: cost.clone(),
            });
        }

        // Step 2: 消耗道具（通过 spender 扣减）
        let reason = match &cost {
            TreasureItem::RebirthToken => "转世重生：消耗重生符".to_string(),
            TreasureItem::SpiritStones(amount) => {
                format!("夺舍重生：消耗灵石 {}", amount)
            }
        };
        self.spender.consume(agent_id, cost, &reason).await?;

        // Step 3: 执行 git reincarnate（hard reset 到目标 commit）
        self.git_ledger.reincarnate(target_commit).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::RwLock;
    use taiji_types::economy::CurrencyAmount;

    /// 测试用 TreasureSpender — 模拟灵石余额检查与扣减。
    struct MockTreasureSpender {
        balance: RwLock<CurrencyAmount>,
    }

    impl MockTreasureSpender {
        fn new(balance: u64) -> Self {
            Self {
                balance: RwLock::new(CurrencyAmount::new(balance)),
            }
        }
    }

    #[async_trait]
    impl TreasureSpender for MockTreasureSpender {
        async fn has_sufficient(&self, _agent_id: &AgentId, item: &TreasureItem) -> Result<bool, LedgerError> {
            let balance = self.balance.read().map_err(|e| {
                LedgerError::Internal(format!("lock error: {}", e))
            })?;
            Ok(*balance >= item.stone_equivalent())
        }

        async fn consume(&self, _agent_id: &AgentId, item: TreasureItem, _reason: &str) -> Result<(), LedgerError> {
            let cost = item.stone_equivalent();
            let mut balance = self.balance.write().map_err(|e| {
                LedgerError::Internal(format!("lock error: {}", e))
            })?;
            if *balance < cost {
                return Err(LedgerError::InsufficientTreasure {
                    agent_id: AgentId::new(),
                    required: cost,
                    item,
                });
            }
            *balance = balance.saturating_sub(cost);
            Ok(())
        }
    }

    fn setup_git_ledger(name: &str) -> GitLedger {
        let dir = env::temp_dir().join(format!("taiji_revert_test_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        GitLedger::new(&dir).unwrap()
    }

    #[tokio::test]
    async fn test_revert_with_cost_ok() {
        let git_ledger = setup_git_ledger("revert_ok");
        let agent = AgentId::new();

        // 先创建一个 commit
        let h1 = git_ledger.commit(&agent, "v1").await.unwrap();
        git_ledger.commit(&agent, "v2").await.unwrap();

        // 余额足够
        let spender = Box::new(MockTreasureSpender::new(2000));
        let gate = LedgerRevertGate::new(git_ledger, spender);

        gate.revert_with_cost(&agent, &h1, TreasureItem::RebirthToken).await.unwrap();
    }

    #[tokio::test]
    async fn test_revert_insufficient_balance() {
        let git_ledger = setup_git_ledger("revert_insufficient");
        let agent = AgentId::new();

        let h1 = git_ledger.commit(&agent, "v1").await.unwrap();

        // 余额不足（只有 500，需要 1000）
        let spender = Box::new(MockTreasureSpender::new(500));
        let gate = LedgerRevertGate::new(git_ledger, spender);

        let result = gate.revert_with_cost(&agent, &h1, TreasureItem::RebirthToken).await;
        assert!(result.is_err());
        match result {
            Err(LedgerError::InsufficientTreasure { .. }) => {} // expected
            _ => panic!("expected InsufficientTreasure, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_revert_with_spirit_stones() {
        let git_ledger = setup_git_ledger("revert_stones");
        let agent = AgentId::new();

        let h1 = git_ledger.commit(&agent, "v1").await.unwrap();
        git_ledger.commit(&agent, "v2").await.unwrap();

        // 用灵石消耗
        let spender = Box::new(MockTreasureSpender::new(500));
        let gate = LedgerRevertGate::new(git_ledger, spender);

        gate.revert_with_cost(&agent, &h1, TreasureItem::SpiritStones(CurrencyAmount::new(300)))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_revert_invalid_commit() {
        let git_ledger = setup_git_ledger("revert_invalid");
        let agent = AgentId::new();

        git_ledger.commit(&agent, "v1").await.unwrap();

        let spender = Box::new(MockTreasureSpender::new(5000));
        let gate = LedgerRevertGate::new(git_ledger, spender);

        // 非法 commit hash
        let result = gate
            .revert_with_cost(&agent, "not_a_valid_commit_hash_1234567", TreasureItem::RebirthToken)
            .await;
        assert!(result.is_err());
    }
}
