//! 经济系统共享类型 — 灵力（Token）/灵石（Stone）/交易记录。
//!
//! 核心概念：双币分离（Token 不计余额，Stone 计余额）、单向兑换。
//!
//! 参考源：
//! - react-xiuxian-game types.ts（PlayerStats spiritStones 字段）
//! - 架构总纲 §6（经济系统全文）+ §5.3（坊市版税）

use serde::{Deserialize, Serialize};

use crate::agent::AgentId;

// =============================================================================
// HT-1: 基础货币类型
// =============================================================================

/// 货币面额（通用类型，同时用于灵石和灵力）。
///
/// newtype over u64，避免原始 u64 歧义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct CurrencyAmount(u64);

impl CurrencyAmount {
    pub const fn new(amount: u64) -> Self {
        Self(amount)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn checked_mul(self, factor: u64) -> Option<Self> {
        self.0.checked_mul(factor).map(Self)
    }
}

impl From<u64> for CurrencyAmount {
    fn from(amount: u64) -> Self {
        Self(amount)
    }
}

impl std::fmt::Display for CurrencyAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// HT-2: 货币类型枚举
// =============================================================================

/// 货币类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CurrencyType {
    /// 灵力（Token）— 燃烧的算力
    #[serde(rename = "token")]
    Token,
    /// 灵石（Spirit Stone）— 法币
    #[serde(rename = "stone")]
    Stone,
}

// =============================================================================
// HT-B: 天材地宝道具类型
// =============================================================================

/// 天材地宝道具 — 用于转世重生/夺舍等特殊操作消耗。
///
/// 参考架构总纲 §5.1 转世重生（git revert 消耗天材地宝）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TreasureItem {
    /// 转世重生符 — 一次性道具，消耗后执行 git revert（夺舍）
    #[serde(rename = "rebirth_token")]
    RebirthToken,
    /// 灵石 — 也可以直接消耗灵石替代道具
    #[serde(rename = "spirit_stones")]
    SpiritStones(CurrencyAmount),
}

impl TreasureItem {
    /// 获取该道具消耗所需的灵石等价金额（用于余额检查）。
    /// RebirthToken = 1000 灵石等价。
    pub fn stone_equivalent(&self) -> CurrencyAmount {
        match self {
            TreasureItem::RebirthToken => CurrencyAmount::new(1000),
            TreasureItem::SpiritStones(amount) => *amount,
        }
    }

    /// 是否为灵石直接消耗。
    pub fn is_stone_cost(&self) -> bool {
        matches!(self, TreasureItem::SpiritStones(_))
    }
}

// =============================================================================
// HT-3: 交易方向
// =============================================================================

/// 交易类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransactionType {
    /// 充值
    #[serde(rename = "deposit")]
    Deposit,
    /// 消费
    #[serde(rename = "withdrawal")]
    Withdrawal,
    /// 转账
    #[serde(rename = "transfer")]
    Transfer,
    /// 兑换（灵石→灵力）
    #[serde(rename = "exchange")]
    Exchange,
    /// 任务奖励
    #[serde(rename = "reward")]
    Reward,
    /// 版税收入
    #[serde(rename = "royalty")]
    Royalty,
    /// 卡牌复制支出
    #[serde(rename = "card_copy")]
    CardCopy,
    /// 卡槽拓展
    #[serde(rename = "slot_upgrade")]
    SlotUpgrade,
}

// =============================================================================
// HT-4: 交易记录
// =============================================================================

/// 交易记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    /// UUID v7
    pub tx_id: String,
    /// 交易主体
    pub agent_id: AgentId,
    /// 对手方（转账/版税时为对方）
    pub counterparty: Option<AgentId>,
    pub amount: CurrencyAmount,
    pub currency_type: CurrencyType,
    pub tx_type: TransactionType,
    pub timestamp: crate::shared::Timestamp,
    pub description: String,
    /// 额外信息（如 listing_id）
    pub metadata: Option<serde_json::Value>,
}

// =============================================================================
// HT-5: Token 账户
// =============================================================================

/// 灵力账户（TokenAccount）。
///
/// Token 是消耗品，不设"余额"。只记录消耗量和补贴量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAccount {
    pub agent_id: AgentId,
    /// 总消耗灵力（补贴期内不计入）
    pub total_consumed: CurrencyAmount,
    /// 总获得补贴量
    pub total_subsidized: CurrencyAmount,
    /// 补贴是否生效（false = 进入自负盈亏模式）
    pub subsidy_active: bool,
    /// 最后更新时间
    pub updated_at: crate::shared::Timestamp,
}

/// Token 统计快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStats {
    pub agent_id: AgentId,
    pub total_consumed: CurrencyAmount,
    pub total_subsidized: CurrencyAmount,
    /// total_consumed - total_subsidized
    pub net_self_paid: CurrencyAmount,
    pub subsidy_active: bool,
}

// =============================================================================
// HT-6: 灵石账户
// =============================================================================

/// 灵石账户（StoneAccount）。
///
/// 灵石是资产，有余额。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoneAccount {
    pub agent_id: AgentId,
    pub balance: CurrencyAmount,
    /// 累计总收入
    pub lifetime_earnings: CurrencyAmount,
    pub updated_at: crate::shared::Timestamp,
}
