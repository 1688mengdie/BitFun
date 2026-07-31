//! 境界（Realm）类型 — Agent 成长阶段。
//!
//! 参考源：react-xiuxian-game/types.ts:1-9 RealmType 枚举。

use serde::{Deserialize, Serialize};

/// 修仙境界枚举。
///
/// 按修为从低到高排列，实现 PartialOrd 以支持境界比较。
/// 参考 react-xiuxian-game RealmType（声明在 types.ts:1-9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Realm {
    /// 炼气期
    #[serde(rename = "qi_refining")]
    QiRefining,
    /// 筑基期
    #[serde(rename = "foundation")]
    Foundation,
    /// 金丹期
    #[serde(rename = "golden_core")]
    GoldenCore,
    /// 元婴期
    #[serde(rename = "nascent_soul")]
    NascentSoul,
    /// 化神期
    #[serde(rename = "spirit_severing")]
    SpiritSevering,
    /// 炼虚期
    #[serde(rename = "void_refining")]
    VoidRefining,
    /// 渡劫飞升
    #[serde(rename = "immortal_ascension")]
    ImmortalAscension,
}

impl Realm {
    /// 获取境界的显示名称（中文）。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::QiRefining => "炼气期",
            Self::Foundation => "筑基期",
            Self::GoldenCore => "金丹期",
            Self::NascentSoul => "元婴期",
            Self::SpiritSevering => "化神期",
            Self::VoidRefining => "炼虚期",
            Self::ImmortalAscension => "渡劫飞升",
        }
    }
}
