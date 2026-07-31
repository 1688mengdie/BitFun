//! 经济系统错误类型。

use thiserror::Error;

use taiji_types::agent::AgentId;
use taiji_types::economy::CurrencyAmount;

/// 经济系统错误。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EconomyError {
    #[error("余额不足: 需要 {0}，实际 {1}")]
    InsufficientBalance(CurrencyAmount, CurrencyAmount),

    #[error("账户不存在: {0}")]
    AccountNotFound(AgentId),

    #[error("兑换汇率无效: {0}")]
    InvalidExchangeRate(String),

    #[error("卡牌上架价格无效: {0}")]
    InvalidPrice(CurrencyAmount),

    #[error("挂牌不存在: {0}")]
    ListingNotFound(String),

    #[error("本命魂卡不可复制")]
    CannotCopySoulCard,

    #[error("灵域访问被拒绝: {0} 无法访问 {1}")]
    DomainAccessDenied(AgentId, String),

    #[error("消费溢出")]
    ConsumptionOverflow,

    #[error("数据存储错误: {0}")]
    StorageError(String),

    #[error("未实现: {0}")]
    Unimplemented(String),

    #[error("未知经济错误: {0}")]
    Unknown(String),
}
