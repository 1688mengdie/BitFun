//! 卡片系统类型 — 本命魂卡品质/插槽/消耗/卡牌定义。
//!
//! 核心概念：品质锁定境界、卡槽限制卡片数量、消耗决定策略代价。
//!
//! 参考源：
//! - godot-skill-system scripts/core/skills/skill.gd (MIT) — CardType/Card 数据模型
//! - godot-skill-system scripts/core/skills/skillModifier.gd (MIT) — Modifier 词条系统
//! - godot-skill-system scripts/core/skills/skillManager.gd (MIT) — CardManager 架构
//! - EGamePlay Ability/AbilityEffect/BuffComponent (MIT) — GAS 四层架构
//! - modules/card-system/实现参考.rs — Rust 翻译参考（351 行）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::agent::SpiritRoot;
use crate::realm::Realm;

// =============================================================================
// 1. 基础类型（已有，保持兼容）
// =============================================================================

/// 卡牌唯一标识（u64 newtype）。
///
/// 可通过 `From<uuid::Uuid>` 从 UUID 转换（取低 64 位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(u64);

impl CardId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<uuid::Uuid> for CardId {
    fn from(u: uuid::Uuid) -> Self {
        // 取 UUID 低 64 位作为 CardId
        let (_, low) = u.as_u64_pair();
        Self(low)
    }
}

/// 卡牌品质（6 阶，从低到高）。
///
/// 同时承担两种语义：
/// 1. Card.tier — 境界锁定位阶（Agent 境界决定可装备的最高 tier）
/// 2. Card.quality — 卡牌自身稀有度（数值倍率参数）
///
/// 境界锁定绑定：
/// - 炼气：Blackiron
/// - 筑基：Blackiron + Bronze
/// - 金丹：Bronze + Silver
/// - 元婴：Silver + Gold
/// - 化神：Gold + Jade
/// - 炼虚/飞升：Jade + Divine
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    /// 黑铁 — 基础品质
    #[serde(rename = "blackiron")]
    Blackiron,
    /// 青铜
    #[serde(rename = "bronze")]
    Bronze,
    /// 白银
    #[serde(rename = "silver")]
    Silver,
    /// 黄金
    #[serde(rename = "gold")]
    Gold,
    /// 玉髓
    #[serde(rename = "jade")]
    Jade,
    /// 神话 — 最高品质
    #[serde(rename = "divine")]
    Divine,
}

impl Tier {
    /// 返回 Tier 对应的卡槽占用数。
    /// Blackiron=1, Bronze=1, Silver=2, Gold=2, Jade=3, Divine=3
    pub fn slot_cost(&self) -> SlotCost {
        match self {
            Self::Blackiron | Self::Bronze => SlotCost(1),
            Self::Silver | Self::Gold => SlotCost(2),
            Self::Jade | Self::Divine => SlotCost(3),
        }
    }

    /// 品质倍率（用于 scaled_stat 计算）。
    /// Blackiron=1.0, Bronze=1.2, Silver=1.5, Gold=2.0, Jade=3.0, Divine=5.0
    pub fn stat_multiplier(&self) -> f64 {
        match self {
            Self::Blackiron => 1.0,
            Self::Bronze => 1.2,
            Self::Silver => 1.5,
            Self::Gold => 2.0,
            Self::Jade => 3.0,
            Self::Divine => 5.0,
        }
    }
}

/// 卡槽类型（4 种）。
///
/// Main = 本命魂卡（固定 1，不可拆卸）
/// Sub = 普通卡片（默认 3，可灵石拓展到 5）
/// Passive = 被动心法槽（初始 0）
/// Consumable = 消耗品槽（丹药/符箓，初始 0）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotType {
    /// 主卡槽 — 本命魂卡唯一位置
    #[serde(rename = "main")]
    Main,
    /// 副卡槽 — 普通卡片位置
    #[serde(rename = "sub")]
    Sub,
    /// 被动卡槽 — 被动效果卡片
    #[serde(rename = "passive")]
    Passive,
    /// 消耗卡槽 — 一次性效果卡片
    #[serde(rename = "consumable")]
    Consumable,
}

/// 卡槽占用数（newtype）。
///
/// 高品质卡片占用更多卡槽。
/// 基础占用：Blackiron=1, Bronze=1, Silver=2, Gold=2, Jade=3, Divine=3。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotCost(pub u8);

impl SlotCost {
    pub fn new(cost: u8) -> Self {
        Self(cost.min(5))
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for SlotCost {
    fn default() -> Self {
        Self(1)
    }
}

// =============================================================================
// 2. 新增类型
// =============================================================================

/// 卡牌类型（9 种）。
///
/// 参考：godot-skill-system scripts/core/skills/skill.gd:4-11 (MIT)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    /// 功法 — 主动技能，需消耗灵力施放
    #[serde(rename = "spell")]
    Spell,
    /// 心法 — 被动技能，常驻效果
    #[serde(rename = "passive")]
    Passive,
    /// 法宝 — 武器，提供主动能力
    #[serde(rename = "weapon")]
    Weapon,
    /// 法衣 — 防具，提供防御属性
    #[serde(rename = "armor")]
    Armor,
    /// 饰品 — 提供特殊效果
    #[serde(rename = "accessory")]
    Accessory,
    /// 丹药 — 一次性消耗品
    #[serde(rename = "potion")]
    Potion,
    /// 符箓 — 一次性技能
    #[serde(rename = "talisman")]
    Talisman,
    /// 秘籍 — 升级技能用
    #[serde(rename = "manual")]
    Manual,
    /// 材料 — 合成/升级用
    #[serde(rename = "material")]
    Material,
}

impl CardType {
    /// 是否为可装备类（可放入槽位）
    pub fn is_equippable(&self) -> bool {
        matches!(self, Self::Spell | Self::Passive | Self::Weapon | Self::Armor | Self::Accessory)
    }

    /// 是否为消耗类
    pub fn is_consumable(&self) -> bool {
        matches!(self, Self::Potion | Self::Talisman)
    }
}

/// 卡牌核心数据模型。
///
/// 参考：godot-skill-system scripts/core/skills/skill.gd:13-42 (MIT)
///       架构总纲 §5.3 — 卡片定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    // ── 基础信息 ──
    pub card_id: CardId,
    pub name: String,
    pub description: String,
    pub card_type: CardType,

    // ── 品质体系 ──
    /// 境界锁定位阶（Agent 境界决定可装备的最高 tier）
    pub tier: Tier,
    /// 卡牌自身稀有度（控制数值倍率，与 tier 同枚举）
    pub quality: Tier,

    // ── 灵根要求 ──
    /// 灵根要求（None = 无限制）
    pub class_requirements: Option<Vec<SpiritRoot>>,

    // ── 属性 ──
    pub base_stats: HashMap<String, f64>,
    pub cooldown_secs: f64,
    pub resource_cost: u32,

    // ── 词条 ──
    pub available_modifiers: Vec<Modifier>,
    pub active_modifier: Option<CardId>,

    // ── 成长 ──
    pub level: u32,
    pub max_level: u32,
    pub experience: u64,

    // ── 套装 ──
    pub set_id: Option<String>,

    // ── 经济 ──
    /// 版税率（0.0-1.0，坊市复制时卖方收益）
    pub royalty_rate: f64,

    // ── 状态效果 ──
    pub applies_status_effects: Vec<StatusEffect>,
    pub applies_self_effects: Vec<StatusEffect>,
}

impl Card {
    /// 创建卡牌实例（新卡初始等级 1，最大等级 10）。
    pub fn new(name: String, card_type: CardType, tier: Tier, quality: Tier) -> Self {
        Self {
            card_id: CardId(0), // 调用者应在后续设置唯一 ID
            name,
            description: String::new(),
            card_type,
            tier,
            quality,
            class_requirements: None,
            base_stats: HashMap::new(),
            cooldown_secs: 0.0,
            resource_cost: 0,
            available_modifiers: Vec::new(),
            active_modifier: None,
            level: 1,
            max_level: 10,
            experience: 0,
            set_id: None,
            royalty_rate: 0.0,
            applies_status_effects: Vec::new(),
            applies_self_effects: Vec::new(),
        }
    }

    /// 按品质倍率缩放后的基础属性。
    /// 倍率映射见 Tier::stat_multiplier()。
    pub fn scaled_stat(&self, stat_name: &str) -> f64 {
        let base = self.base_stats.get(stat_name).copied().unwrap_or(0.0);
        base * self.quality.stat_multiplier()
    }

    /// 获取卡牌当前等级的某项属性基础值（每级 +10%）。
    pub fn get_stat(&self, stat_name: &str) -> f64 {
        let base = self.scaled_stat(stat_name);
        let level_mult = 1.0 + (self.level as f64 - 1.0) * 0.1;
        base * level_mult
    }

    /// 获取冷却时间（受激活词条影响）。
    pub fn get_cooldown(&self) -> f64 {
        let mut cd = self.cooldown_secs;
        if let Some(ref mod_id) = self.active_modifier {
            if let Some(m) = self.find_modifier(mod_id) {
                cd *= m.stat_multipliers.get("cooldown").copied().unwrap_or(1.0);
            }
        }
        cd
    }

    /// 获取灵力消耗（受激活词条影响，最小 1）。
    pub fn get_resource_cost(&self) -> u32 {
        let mut cost = self.resource_cost as f64;
        if let Some(ref mod_id) = self.active_modifier {
            if let Some(m) = self.find_modifier(mod_id) {
                cost *= m.stat_multipliers.get("cost").copied().unwrap_or(1.0);
            }
        }
        (cost.max(1.0)) as u32
    }

    /// 按 ID 查找可用词条。
    pub fn find_modifier(&self, modifier_id: &CardId) -> Option<&Modifier> {
        self.available_modifiers.iter().find(|m| m.modifier_id == *modifier_id)
    }
}

/// 词条修饰器。
///
/// 参考：godot-skill-system scripts/core/skills/skillModifier.gd:1-164 (MIT)
///       modules/card-system/实现参考.rs:146-188
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modifier {
    pub modifier_id: CardId,
    pub name: String,
    pub description: String,

    // ── 属性乘区 ──
    /// 属性倍率（"damage" → 1.5 表示 1.5 倍伤害）
    pub stat_multipliers: HashMap<String, f64>,
    /// 固定加成（"damage" → 50.0 表示 +50 伤害）
    pub flat_bonuses: HashMap<String, f64>,

    // ── 特殊行为 ──
    pub additional_hits: u32,
    pub area_multiplier: f64,
    pub range_multiplier: f64,

    // ── 状态效果 ──
    pub additional_status_effects: Vec<StatusEffect>,
    pub status_effect_duration_multiplier: f64,

    // ── 特殊标记 ──
    pub is_channeled: bool,
    pub channel_duration: f64,
    pub is_charged: bool,
    pub max_charge_time: f64,
    pub charge_damage_multiplier: f64,
}

impl Modifier {
    /// 计算修正后的伤害（含蓄力加成）。
    ///
    /// 公式：base × multiplier + flat_bonus，再叠乘蓄力加成。
    pub fn calculate_damage(&self, base_damage: f64, charge_percent: f64) -> f64 {
        let mult = self.stat_multipliers.get("damage").copied().unwrap_or(1.0);
        let bonus = self.flat_bonuses.get("damage").copied().unwrap_or(0.0);
        let mut damage = base_damage * mult + bonus;

        if self.is_charged && charge_percent > 0.0 {
            let charge_bonus = 1.0 + (self.charge_damage_multiplier - 1.0) * charge_percent;
            damage *= charge_bonus;
        }

        damage
    }
}

/// 卡槽实例。
///
/// 参考：godot-skill-system scripts/core/skills/skillSlot.gd (MIT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSlot {
    pub slot_type: SlotType,
    pub slot_index: u32,
    pub equipped_card: Option<CardId>,
    pub is_locked: bool,
}

/// 卡槽配置方案。
///
/// 架构总纲 §5.3 — 本命魂卡×1 + 普通卡槽默认 3，可花灵石拓展到 5。
/// 参考：godot-skill-system scripts/core/skills/skillManager.gd:23-42 (MIT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotConfig {
    /// 本命魂卡槽（固定 1，不可增减）
    pub soulbound_slots: u32,
    /// 普通卡槽（默认 3，可花灵石拓展到 5）
    pub normal_slots: u32,
    /// 被动卡槽
    pub passive_slots: u32,
    /// 消耗卡槽
    pub consumable_slots: u32,
}

impl Default for SlotConfig {
    fn default() -> Self {
        Self {
            soulbound_slots: 1,
            normal_slots: 3,
            passive_slots: 0,
            consumable_slots: 0,
        }
    }
}

impl SlotConfig {
    /// 普通卡槽最大可能值（灵石拓展上限）
    pub const MAX_NORMAL_SLOTS: u32 = 5;

    /// 获取指定 SlotType 的槽位数。
    pub fn count_for_type(&self, slot_type: &SlotType) -> u32 {
        match slot_type {
            SlotType::Main => self.soulbound_slots,
            SlotType::Sub => self.normal_slots,
            SlotType::Passive => self.passive_slots,
            SlotType::Consumable => self.consumable_slots,
        }
    }

    /// 拓展一个普通卡槽（消耗灵石）。
    pub fn expand_normal(&mut self) -> Result<(), CardError> {
        if self.normal_slots >= Self::MAX_NORMAL_SLOTS {
            return Err(CardError::SlotCapacityExceeded(
                Self::MAX_NORMAL_SLOTS,
                self.normal_slots,
            ));
        }
        self.normal_slots += 1;
        Ok(())
    }

    /// 总槽位数（不含本命魂卡槽）。
    pub fn total_slots(&self) -> u32 {
        self.normal_slots + self.passive_slots + self.consumable_slots
    }
}

/// 状态效果。
///
/// 参考：EGamePlay (MIT) GAS 四层架构·AbilityEffect
///       modules/card-system/实现参考.rs:242-250
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffect {
    pub effect_id: CardId,
    pub name: String,
    pub description: String,
    pub base_duration: f64,
    pub time_remaining: f64,
    pub apply_chance: f64,
}

/// 持续性效果（Buff）。
///
/// 参考：EGamePlay (MIT) GAS 四层架构·BuffComponent.cs
///       modules/card-system/实现参考.rs:253-262
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buff {
    pub buff_id: CardId,
    pub name: String,
    pub duration: f64,
    pub remaining: f64,
    pub stat_modifications: HashMap<String, f64>,
    pub stack_limit: u32,
    pub current_stacks: u32,
}

/// 套装定义。
///
/// 参考：modules/card-system/实现参考.rs:269-285
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefinition {
    pub set_id: String,
    pub name: String,
    pub description: String,
    pub pieces: Vec<SetPiece>,
    pub bonuses: Vec<SetBonus>,
}

/// 套装部件条件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPiece {
    pub card_type: CardType,
    pub min_tier: Tier,
    pub name_pattern: Option<String>,
}

/// 套装奖励。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBonus {
    pub pieces_required: u32,
    pub stat_bonuses: HashMap<String, f64>,
    pub special_effect: Option<String>,
}

// =============================================================================
// 3. 错误类型
// =============================================================================

/// 卡片系统专用错误类型。
///
/// 参考：godot-skill-system (MIT) + modules/card-system/实现参考.rs:337-351
///       plus LVPA 特有扩展（RealmLock/SpiritCardSoulbound 等）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum CardError {
    #[error("card not found: {0:?}")]
    CardNotFound(CardId),

    #[error("slot occupied: {0:?} idx={1}")]
    SlotOccupied(SlotType, u32),

    #[error("card type {0:?} cannot be equipped to slot {1:?}")]
    InvalidSlot(CardType, SlotType),

    #[error("modifier not found: {0:?}")]
    ModifierNotFound(CardId),

    #[error("set not found: {0}")]
    SetNotFound(String),

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("realm lock: agent realm {0:?} cannot equip tier {1:?}")]
    RealmLockTier(Realm, Tier),

    #[error("slot capacity exceeded: have {1} slots but cost {0}")]
    SlotCapacityExceeded(u32, u32),

    #[error("class requirement: card needs {0:?} but agent has {1:?}")]
    ClassRequirement(SpiritRoot, SpiritRoot),

    #[error("upgrade conditions not met: score {0} < required {1}")]
    UpgradeConditions(f64, f64),

    #[error("spirit card is soulbound, cannot be removed")]
    SpiritCardSoulbound,

    #[error("card is currently equipped: {0:?}")]
    CardEquipped(CardId),

    #[error("spirit card already at max tier")]
    SpiritCardAlreadyMaxTier,

    #[error("event bus error: {0}")]
    EventBus(String),

    #[error("internal error: {0}")]
    Internal(String),
}

// =============================================================================
// 4. 境界锁定映射（realm.rs 的辅助函数）
// =============================================================================

/// 返回境界允许装备的 Tier 白名单。
pub fn realm_allowed_tiers(realm: Realm) -> Vec<Tier> {
    match realm {
        Realm::QiRefining => vec![Tier::Blackiron],
        Realm::Foundation => vec![Tier::Blackiron, Tier::Bronze],
        Realm::GoldenCore => vec![Tier::Bronze, Tier::Silver],
        Realm::NascentSoul => vec![Tier::Silver, Tier::Gold],
        Realm::SpiritSevering => vec![Tier::Gold, Tier::Jade],
        Realm::VoidRefining | Realm::ImmortalAscension => vec![Tier::Jade, Tier::Divine],
    }
}

/// 检查境界是否允许装备指定 Tier 的卡牌。
pub fn can_equip_tier(realm: Realm, tier: Tier) -> bool {
    realm_allowed_tiers(realm).contains(&tier)
}

/// 构造境界锁定错误。
pub fn locked_tier_error(realm: Realm, tier: Tier) -> CardError {
    CardError::RealmLockTier(realm, tier)
}

// =============================================================================
// 5. 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::Realm;

    // ── 基础类型 serde ──

    #[test]
    fn test_card_id_new() {
        let id = CardId::new(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn test_card_id_from_uuid() {
        let u = uuid::Uuid::new_v4();
        let id = CardId::from(u);
        // 只验证转换不崩溃
        assert!(id.as_u64() > 0 || id.as_u64() == 0);
    }

    #[test]
    fn test_tier_serde_roundtrip() {
        for tier in &[Tier::Blackiron, Tier::Bronze, Tier::Silver, Tier::Gold, Tier::Jade, Tier::Divine] {
            let json = serde_json::to_string(tier).unwrap();
            let back: Tier = serde_json::from_str(&json).unwrap();
            assert_eq!(*tier, back);
        }
    }

    #[test]
    fn test_tier_ordering() {
        assert!(Tier::Blackiron < Tier::Bronze);
        assert!(Tier::Bronze < Tier::Silver);
        assert!(Tier::Silver < Tier::Gold);
        assert!(Tier::Gold < Tier::Jade);
        assert!(Tier::Jade < Tier::Divine);
    }

    #[test]
    fn test_tier_slot_cost() {
        assert_eq!(Tier::Blackiron.slot_cost().as_u8(), 1);
        assert_eq!(Tier::Bronze.slot_cost().as_u8(), 1);
        assert_eq!(Tier::Silver.slot_cost().as_u8(), 2);
        assert_eq!(Tier::Gold.slot_cost().as_u8(), 2);
        assert_eq!(Tier::Jade.slot_cost().as_u8(), 3);
        assert_eq!(Tier::Divine.slot_cost().as_u8(), 3);
    }

    #[test]
    fn test_tier_stat_multiplier() {
        assert!((Tier::Blackiron.stat_multiplier() - 1.0).abs() < 1e-9);
        assert!((Tier::Bronze.stat_multiplier() - 1.2).abs() < 1e-9);
        assert!((Tier::Silver.stat_multiplier() - 1.5).abs() < 1e-9);
        assert!((Tier::Gold.stat_multiplier() - 2.0).abs() < 1e-9);
        assert!((Tier::Jade.stat_multiplier() - 3.0).abs() < 1e-9);
        assert!((Tier::Divine.stat_multiplier() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_slot_type_serde_roundtrip() {
        for st in &[SlotType::Main, SlotType::Sub, SlotType::Passive, SlotType::Consumable] {
            let json = serde_json::to_string(st).unwrap();
            let back: SlotType = serde_json::from_str(&json).unwrap();
            assert_eq!(*st, back);
        }
    }

    #[test]
    fn test_slot_cost_new() {
        assert_eq!(SlotCost::new(3).as_u8(), 3);
        assert_eq!(SlotCost::new(7).as_u8(), 5); // min(7, 5) = 5
    }

    // ── CardType ──

    #[test]
    fn test_card_type_serde_roundtrip() {
        for ct in &[CardType::Spell, CardType::Passive, CardType::Weapon, CardType::Armor,
                     CardType::Accessory, CardType::Potion, CardType::Talisman, CardType::Manual, CardType::Material] {
            let json = serde_json::to_string(ct).unwrap();
            let back: CardType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ct, back);
        }
    }

    #[test]
    fn test_card_type_equippable() {
        assert!(CardType::Spell.is_equippable());
        assert!(CardType::Weapon.is_equippable());
        assert!(!CardType::Potion.is_equippable());
        assert!(!CardType::Material.is_equippable());
    }

    #[test]
    fn test_card_type_consumable() {
        assert!(CardType::Potion.is_consumable());
        assert!(CardType::Talisman.is_consumable());
        assert!(!CardType::Spell.is_consumable());
    }

    // ── Card ──

    #[test]
    fn test_card_new() {
        let card = Card::new("九转金身诀".into(), CardType::Spell, Tier::Silver, Tier::Silver);
        assert_eq!(card.name, "九转金身诀");
        assert_eq!(card.tier, Tier::Silver);
        assert_eq!(card.level, 1);
        assert_eq!(card.max_level, 10);
    }

    #[test]
    fn test_card_get_stat_with_level() {
        let mut card = Card::new("测试".into(), CardType::Spell, Tier::Blackiron, Tier::Blackiron);
        card.base_stats.insert("attack".into(), 100.0);
        card.level = 5;
        let val = card.get_stat("attack");
        // scaled = 100 * 1.0 = 100, level mult = 1 + 4*0.1 = 1.4, total = 140
        assert!((val - 140.0).abs() < 1e-9);
    }

    #[test]
    fn test_card_get_stat_with_quality_mult() {
        let mut card = Card::new("测试".into(), CardType::Spell, Tier::Silver, Tier::Gold);
        card.base_stats.insert("attack".into(), 100.0);
        card.level = 1;
        let val = card.get_stat("attack");
        // scaled = 100 * 2.0 (Gold mult) = 200, level mult = 1.0, total = 200
        assert!((val - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_card_get_cooldown_with_modifier() {
        let mut card = Card::new("测试".into(), CardType::Spell, Tier::Blackiron, Tier::Blackiron);
        card.cooldown_secs = 10.0;
        let mod_id = CardId(1);
        let modifier = Modifier {
            modifier_id: mod_id,
            name: "急速".into(),
            description: "冷却缩减20%".into(),
            stat_multipliers: [("cooldown".into(), 0.8)].into(),
            flat_bonuses: HashMap::new(),
            additional_hits: 0,
            area_multiplier: 1.0,
            range_multiplier: 1.0,
            additional_status_effects: Vec::new(),
            status_effect_duration_multiplier: 1.0,
            is_channeled: false,
            channel_duration: 0.0,
            is_charged: false,
            max_charge_time: 0.0,
            charge_damage_multiplier: 2.0,
        };
        card.available_modifiers.push(modifier);
        card.active_modifier = Some(mod_id);
        assert!((card.get_cooldown() - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_card_serde_roundtrip() {
        let card = Card::new("九转金身诀".into(), CardType::Spell, Tier::Gold, Tier::Gold);
        let json = serde_json::to_string(&card).unwrap();
        let back: Card = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "九转金身诀");
        assert_eq!(back.card_type, CardType::Spell);
        assert_eq!(back.tier, Tier::Gold);
    }

    // ── Modifier ──

    #[test]
    fn test_modifier_serde_roundtrip() {
        let m = Modifier {
            modifier_id: CardId(1),
            name: "破甲".into(),
            description: "破甲伤害+50%".into(),
            stat_multipliers: [("damage".into(), 1.5)].into(),
            flat_bonuses: [("damage".into(), 50.0)].into(),
            additional_hits: 0,
            area_multiplier: 1.0,
            range_multiplier: 1.0,
            additional_status_effects: Vec::new(),
            status_effect_duration_multiplier: 1.0,
            is_channeled: false,
            channel_duration: 0.0,
            is_charged: false,
            max_charge_time: 0.0,
            charge_damage_multiplier: 2.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: Modifier = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "破甲");
        assert!((back.stat_multipliers.get("damage").unwrap() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_modifier_calculate_damage() {
        let m = Modifier {
            modifier_id: CardId(1),
            name: "破甲".into(),
            description: "".into(),
            stat_multipliers: [("damage".into(), 1.5)].into(),
            flat_bonuses: [("damage".into(), 50.0)].into(),
            additional_hits: 0,
            area_multiplier: 1.0,
            range_multiplier: 1.0,
            additional_status_effects: Vec::new(),
            status_effect_duration_multiplier: 1.0,
            is_channeled: false,
            channel_duration: 0.0,
            is_charged: false,
            max_charge_time: 0.0,
            charge_damage_multiplier: 2.0,
        };
        let dmg = m.calculate_damage(100.0, 0.0);
        assert!((dmg - 200.0).abs() < 1e-9); // 100*1.5 + 50 = 200
    }

    #[test]
    fn test_modifier_charged_damage() {
        let m = Modifier {
            modifier_id: CardId(1),
            name: "蓄力一击".into(),
            description: "".into(),
            stat_multipliers: HashMap::new(),
            flat_bonuses: HashMap::new(),
            additional_hits: 0,
            area_multiplier: 1.0,
            range_multiplier: 1.0,
            additional_status_effects: Vec::new(),
            status_effect_duration_multiplier: 1.0,
            is_channeled: false,
            channel_duration: 0.0,
            is_charged: true,
            max_charge_time: 3.0,
            charge_damage_multiplier: 2.0,
        };
        // 50% 蓄力: 100 * (1 + (2-1)*0.5) = 150
        let dmg = m.calculate_damage(100.0, 0.5);
        assert!((dmg - 150.0).abs() < 1e-9);
    }

    // ── SlotConfig ──

    #[test]
    fn test_slot_config_default() {
        let cfg = SlotConfig::default();
        assert_eq!(cfg.soulbound_slots, 1);
        assert_eq!(cfg.normal_slots, 3);
        assert_eq!(cfg.passive_slots, 0);
        assert_eq!(cfg.consumable_slots, 0);
    }

    #[test]
    fn test_slot_config_count_for_type() {
        let cfg = SlotConfig::default();
        assert_eq!(cfg.count_for_type(&SlotType::Main), 1);
        assert_eq!(cfg.count_for_type(&SlotType::Sub), 3);
        assert_eq!(cfg.count_for_type(&SlotType::Passive), 0);
    }

    #[test]
    fn test_slot_config_expand_normal() {
        let mut cfg = SlotConfig::default();
        assert_eq!(cfg.normal_slots, 3);
        assert!(cfg.expand_normal().is_ok());
        assert_eq!(cfg.normal_slots, 4);
        assert!(cfg.expand_normal().is_ok());
        assert_eq!(cfg.normal_slots, 5);
        // 超出上限
        assert!(cfg.expand_normal().is_err());
    }

    #[test]
    fn test_slot_config_serde() {
        let cfg = SlotConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SlotConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.normal_slots, 3);
    }

    // ── CardSlot ──

    #[test]
    fn test_card_slot_serde() {
        let slot = CardSlot {
            slot_type: SlotType::Sub,
            slot_index: 0,
            equipped_card: Some(CardId(42)),
            is_locked: false,
        };
        let json = serde_json::to_string(&slot).unwrap();
        let back: CardSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.slot_type, SlotType::Sub);
        assert_eq!(back.equipped_card.unwrap(), CardId(42));
    }

    // ── Set / StatusEffect / Buff ──

    #[test]
    fn test_set_definition_serde() {
        let set = SetDefinition {
            set_id: "vermilion_bird".into(),
            name: "朱雀七宿".into(),
            description: "朱雀套装".into(),
            pieces: vec![SetPiece {
                card_type: CardType::Weapon,
                min_tier: Tier::Silver,
                name_pattern: Some("朱雀*".into()),
            }],
            bonuses: vec![SetBonus {
                pieces_required: 2,
                stat_bonuses: [("attack".into(), 50.0)].into(),
                special_effect: None,
            }],
        };
        let json = serde_json::to_string(&set).unwrap();
        let back: SetDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "朱雀七宿");
    }

    #[test]
    fn test_status_effect_serde() {
        let effect = StatusEffect {
            effect_id: CardId(1),
            name: "灼烧".into(),
            description: "每回合损失HP".into(),
            base_duration: 10.0,
            time_remaining: 10.0,
            apply_chance: 0.8,
        };
        let json = serde_json::to_string(&effect).unwrap();
        let back: StatusEffect = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "灼烧");
    }

    #[test]
    fn test_buff_serde() {
        let buff = Buff {
            buff_id: CardId(1),
            name: "护盾".into(),
            duration: 30.0,
            remaining: 30.0,
            stat_modifications: [("defense".into(), 100.0)].into(),
            stack_limit: 3,
            current_stacks: 1,
        };
        let json = serde_json::to_string(&buff).unwrap();
        let back: Buff = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "护盾");
    }

    // ── CardError ──

    #[test]
    fn test_card_error_display() {
        let err = CardError::CardNotFound(CardId(42));
        assert!(err.to_string().contains("card not found"));

        let err = CardError::SpiritCardSoulbound;
        assert_eq!(err.to_string(), "spirit card is soulbound, cannot be removed");
    }

    // ── 境界锁定 ──

    #[test]
    fn test_realm_allowed_tiers_qi_refining() {
        let tiers = realm_allowed_tiers(Realm::QiRefining);
        assert_eq!(tiers, vec![Tier::Blackiron]);
    }

    #[test]
    fn test_realm_allowed_tiers_foundation() {
        let tiers = realm_allowed_tiers(Realm::Foundation);
        assert_eq!(tiers, vec![Tier::Blackiron, Tier::Bronze]);
    }

    #[test]
    fn test_can_equip_tier_allow() {
        assert!(can_equip_tier(Realm::GoldenCore, Tier::Silver));
    }

    #[test]
    fn test_can_equip_tier_deny() {
        assert!(!can_equip_tier(Realm::QiRefining, Tier::Silver));
    }

    #[test]
    fn test_locked_tier_error() {
        let err = locked_tier_error(Realm::QiRefining, Tier::Silver);
        assert!(matches!(err, CardError::RealmLockTier(..)));
    }
}
