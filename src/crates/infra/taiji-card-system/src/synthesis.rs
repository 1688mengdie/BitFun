//! # 卡片合成系统 — 合成配方 + 品质晋升 + 分解回收
//!
//! 提供卡牌的合成（低品质→高品质）与分解（卡牌→材料）机制。
//!
//! ## 品质晋升路线（6 阶）
//!
//! ```text
//! Blackiron → Bronze → Silver → Gold → Jade → Divine
//!  (Common)   (Uncommon) (Rare) (Epic) (Legendary) (Mythic)
//! ```
//!
//! - 合成：低品质 ×2~3 + 灵石 → 高品质 ×1（100% 成功率）
//! - 分解：卡牌 → 对应品质材料（数量取决于原品质）
//!
//! ## 参考
//!
//! - 架构总纲 §5.3 卡片系统 — 品质体系、合成经济
//! - godot-skill-system (MIT) — skillManager.gd 合成/分解模式
//! - modules/card-system/实现参考.rs — 卡片生命周期

use crate::card_manager::CardManager;
use taiji_types::card::{
    can_equip_tier, Card, CardError, CardId, CardType, Tier,
};
use taiji_types::realm::Realm;

// =============================================================================
// 基础类型
// =============================================================================

/// 卡牌规格 — 用于合成配方的输入/输出描述。
#[derive(Debug, Clone)]
pub struct CardSpec {
    /// 要求的最低品质
    pub tier: Tier,
    /// 卡牌类型要求（None = 任意类型）
    pub card_type: Option<CardType>,
    /// 卡牌名称模式（None = 任意名称）
    pub name_pattern: Option<String>,
}

impl CardSpec {
    /// 检查一张卡牌是否匹配本规格。
    pub fn matches(&self, card: &Card) -> bool {
        if card.tier != self.tier {
            return false;
        }
        if let Some(ref ct) = self.card_type {
            if card.card_type != *ct {
                return false;
            }
        }
        if let Some(ref pattern) = self.name_pattern {
            if !card.name.contains(pattern) {
                return false;
            }
        }
        true
    }
}

/// 合成配方 — 将 N 张输入卡牌合成为 1 张输出卡牌。
#[derive(Debug, Clone)]
pub struct SynthesisRecipe {
    /// 配方唯一 ID
    pub recipe_id: String,
    /// 配方名称
    pub name: String,
    /// 配方描述
    pub description: String,
    /// 输入规格（数量和顺序）
    pub inputs: Vec<CardSpec>,
    /// 输出规格
    pub output: CardSpec,
    /// 消耗灵石数量
    pub cost: u64,
}

/// 分解材料奖励。
#[derive(Debug, Clone)]
pub struct MaterialReward {
    /// 产出的材料卡牌
    pub material: Card,
    /// 数量
    pub quantity: u32,
}

/// 合成专用错误。
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SynthesisError {
    #[error("recipe not found: {0}")]
    RecipeNotFound(String),

    #[error("input card count mismatch: expected {expected}, got {got}")]
    InputCountMismatch { expected: usize, got: usize },

    #[error("input card {index} does not match recipe spec: required tier {tier:?}")]
    InputSpecMismatch { index: usize, tier: Tier },

    #[error("insufficient spirit stones: need {required}, have {available}")]
    InsufficientSpiritStones { required: u64, available: u64 },

    #[error("output tier {tier:?} exceeds realm limit for realm {realm:?}")]
    OutputTierLocked { tier: Tier, realm: Realm },

    #[error("duplicate card used as input: {0:?}")]
    DuplicateInput(CardId),

    #[error("card is equipped and cannot be used in synthesis: {0:?}")]
    CardEquipped(CardId),

    #[error("card system error: {0}")]
    CardSystem(CardError),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<CardError> for SynthesisError {
    fn from(e: CardError) -> Self {
        SynthesisError::CardSystem(e)
    }
}

// =============================================================================
// 合成引擎
// =============================================================================

/// 合成引擎 — 管理配方、执行合成与分解。
pub struct SynthesisEngine {
    /// 可用配方列表
    recipes: Vec<SynthesisRecipe>,
}

impl SynthesisEngine {
    /// 创建新的合成引擎。
    pub fn new() -> Self {
        Self { recipes: Vec::new() }
    }

    /// 创建带有默认品质晋升配方的合成引擎。
    ///
    /// 生成 5 个标准晋升配方（Blackiron→Bronze→Silver→Gold→Jade→Divine）。
    pub fn with_default_recipes() -> Self {
        let mut engine = Self::new();
        engine.add_default_recipes();
        engine
    }

    /// 注册一个配方。
    pub fn add_recipe(&mut self, recipe: SynthesisRecipe) {
        self.recipes.push(recipe);
    }

    /// 批量注册配方。
    pub fn add_recipes(&mut self, recipes: Vec<SynthesisRecipe>) {
        self.recipes.extend(recipes);
    }

    /// 获取所有配方。
    pub fn recipes(&self) -> &[SynthesisRecipe] {
        &self.recipes
    }

    /// 按 ID 查找配方。
    pub fn find_recipe(&self, recipe_id: &str) -> Option<&SynthesisRecipe> {
        self.recipes.iter().find(|r| r.recipe_id == recipe_id)
    }

    /// ====================================================================
    /// 合成执行
    /// ====================================================================
    ///
    /// 将 N 张输入卡牌按配方合成为一张新卡。
    ///
    /// # 流程
    ///
    /// 1. 校验配方存在
    /// 2. 校验输入卡牌数量匹配
    /// 3. 校验每张输入卡牌匹配对应规格
    /// 4. 校验无重复卡牌
    /// 5. 校验灵石充足
    /// 6. 创建输出卡牌
    /// 7. 通过 CardManager 移除输入卡牌、添加输出卡牌
    ///
    /// # 参数
    ///
    /// * `recipe_id` — 配方 ID
    /// * `input_cards` — 输入卡牌列表（顺序需与配方 inputs 一致）
    /// * `owner_realm` — 卡牌拥有者的当前境界（用于输出品质锁定校验）
    /// * `spirit_stones` — 当前持有的灵石数量
    /// * `card_manager` — 卡片管理器（用于持久化操作）
    pub async fn synthesize(
        &self,
        recipe_id: &str,
        input_cards: &[Card],
        owner_realm: &Realm,
        spirit_stones: u64,
        card_manager: &dyn CardManager,
        owner_id: &taiji_types::agent::AgentId,
    ) -> Result<Card, SynthesisError> {
        // 1. 查找配方
        let recipe = self
            .find_recipe(recipe_id)
            .ok_or_else(|| SynthesisError::RecipeNotFound(recipe_id.to_string()))?;

        // 2. 校验数量
        if input_cards.len() != recipe.inputs.len() {
            return Err(SynthesisError::InputCountMismatch {
                expected: recipe.inputs.len(),
                got: input_cards.len(),
            });
        }

        // 3. 校验每张卡牌匹配规格
        for (i, (card, spec)) in input_cards.iter().zip(recipe.inputs.iter()).enumerate() {
            if !spec.matches(card) {
                return Err(SynthesisError::InputSpecMismatch {
                    index: i,
                    tier: spec.tier,
                });
            }
        }

        // 4. 校验无重复卡牌
        let mut seen = std::collections::HashSet::new();
        for card in input_cards {
            if !seen.insert(card.card_id) {
                return Err(SynthesisError::DuplicateInput(card.card_id));
            }
        }

        // 5. 校验灵石充足
        if spirit_stones < recipe.cost {
            return Err(SynthesisError::InsufficientSpiritStones {
                required: recipe.cost,
                available: spirit_stones,
            });
        }

        // 6. 校验输出品质不被境界锁定
        let output_tier = recipe.output.tier;
        // 如果 owner 境界低于输出卡牌所需最低境界，禁止合成
        if !can_equip_tier(*owner_realm, output_tier) {
            return Err(SynthesisError::OutputTierLocked {
                tier: output_tier,
                realm: *owner_realm,
            });
        }

        // 7. 创建输出卡牌
        let output_card_type = recipe
            .output
            .card_type
            .unwrap_or(CardType::Material);
        let output_name = format!(
            "{} ({:?})",
            recipe.name,
            output_tier
        );

        let mut output_card = Card::new(output_name, output_card_type, output_tier, output_tier);

        // 继承输入卡牌的平均等级（取整）
        let avg_level: u32 = if input_cards.is_empty() {
            1
        } else {
            let sum: u32 = input_cards.iter().map(|c| c.level).sum();
            (sum / input_cards.len() as u32).max(1)
        };
        output_card.level = avg_level;
        output_card.card_id = CardId::from(uuid::Uuid::new_v4());

        // 8. 通过 CardManager 执行持久化操作
        // 先移除输入卡牌
        for card in input_cards {
            card_manager
                .remove_card(owner_id, &card.card_id)
                .await?;
        }

        // 添加输出卡牌
        card_manager.add_card(owner_id, output_card.clone()).await?;

        Ok(output_card)
    }

    /// ====================================================================
    /// 卡牌分解
    /// ====================================================================
    ///
    /// 将一张卡牌分解为对应品质的材料卡。
    ///
    /// # 分解产出规则
    ///
    /// | 原品质 | 产出材料品质 | 数量 |
    /// |--------|-------------|:----:|
    /// | Blackiron | Blackiron Essence | 1 |
    /// | Bronze | Bronze Essence | 1~2 |
    /// | Silver | Silver Essence | 1~3 |
    /// | Gold | Gold Essence | 2~3 |
    /// | Jade | Jade Essence | 2~4 |
    /// | Divine | Divine Essence | 3~5 |
    ///
    /// 另外返还部分灵石（原合成 cost 的 50%）。
    pub fn decompose(&self, card: &Card) -> Result<(Vec<MaterialReward>, u64), SynthesisError> {
        let quantity = match card.tier {
            Tier::Blackiron => 1,
            Tier::Bronze => 1,
            Tier::Silver => 2,
            Tier::Gold => 2,
            Tier::Jade => 3,
            Tier::Divine => 4,
        };

        // 额外随机增量（模拟波动）
        let bonus = tier_decompose_bonus(card.tier);
        let total_qty = quantity + bonus;

        // 创建材料卡
        let material_name = format!("{:?} Essence", card.tier);
        let mut material = Card::new(
            material_name,
            CardType::Material,
            card.tier,
            card.tier,
        );
        material.card_id = CardId::from(uuid::Uuid::new_v4());
        // 材料卡基础属性标记分解来源
        material
            .base_stats
            .insert("decompose_yield".into(), total_qty as f64);

        // 灵石返还（上级合成 cost 的 50%）
        let refund = tier_synthesis_cost(next_tier(card.tier)) / 2;

        Ok((
            vec![MaterialReward {
                material,
                quantity: total_qty,
            }],
            refund,
        ))
    }
}

impl Default for SynthesisEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 默认配方生成
// =============================================================================

impl SynthesisEngine {
    /// 添加 5 个标准品质晋升配方。
    fn add_default_recipes(&mut self) {
        let recipes = vec![
            SynthesisRecipe {
                recipe_id: "upgrade_blackiron_to_bronze".into(),
                name: "黑铁铸青铜".into(),
                description: "3张黑铁卡 + 100灵石 → 1张青铜卡".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Blackiron,
                        card_type: None,
                        name_pattern: None,
                    };
                    3
                ],
                output: CardSpec {
                    tier: Tier::Bronze,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 100,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_bronze_to_silver".into(),
                name: "青铜锻白银".into(),
                description: "3张青铜卡 + 200灵石 → 1张白银卡".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Bronze,
                        card_type: None,
                        name_pattern: None,
                    };
                    3
                ],
                output: CardSpec {
                    tier: Tier::Silver,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 200,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_silver_to_gold".into(),
                name: "白银炼黄金".into(),
                description: "3张白银卡 + 500灵石 → 1张黄金卡".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Silver,
                        card_type: None,
                        name_pattern: None,
                    };
                    3
                ],
                output: CardSpec {
                    tier: Tier::Gold,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 500,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_gold_to_jade".into(),
                name: "黄金琢玉髓".into(),
                description: "3张黄金卡 + 1000灵石 → 1张玉髓卡".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Gold,
                        card_type: None,
                        name_pattern: None,
                    };
                    3
                ],
                output: CardSpec {
                    tier: Tier::Jade,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 1000,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_jade_to_divine".into(),
                name: "玉髓化神话".into(),
                description: "3张玉髓卡 + 2000灵石 → 1张神话卡".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Jade,
                        card_type: None,
                        name_pattern: None,
                    };
                    3
                ],
                output: CardSpec {
                    tier: Tier::Divine,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 2000,
            },
            // ── 2 卡加速配方（节省一张卡，但 cost 更高） ──
            SynthesisRecipe {
                recipe_id: "upgrade_blackiron_to_bronze_fast".into(),
                name: "黑铁铸青铜·速".into(),
                description: "2张黑铁卡 + 200灵石 → 1张青铜卡（加速配方）".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Blackiron,
                        card_type: None,
                        name_pattern: None,
                    };
                    2
                ],
                output: CardSpec {
                    tier: Tier::Bronze,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 200,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_bronze_to_silver_fast".into(),
                name: "青铜锻白银·速".into(),
                description: "2张青铜卡 + 400灵石 → 1张白银卡（加速配方）".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Bronze,
                        card_type: None,
                        name_pattern: None,
                    };
                    2
                ],
                output: CardSpec {
                    tier: Tier::Silver,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 400,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_silver_to_gold_fast".into(),
                name: "白银炼黄金·速".into(),
                description: "2张白银卡 + 1000灵石 → 1张黄金卡（加速配方）".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Silver,
                        card_type: None,
                        name_pattern: None,
                    };
                    2
                ],
                output: CardSpec {
                    tier: Tier::Gold,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 1000,
            },
            SynthesisRecipe {
                recipe_id: "upgrade_gold_to_jade_fast".into(),
                name: "黄金琢玉髓·速".into(),
                description: "2张黄金卡 + 2000灵石 → 1张玉髓卡（加速配方）".into(),
                inputs: vec![
                    CardSpec {
                        tier: Tier::Gold,
                        card_type: None,
                        name_pattern: None,
                    };
                    2
                ],
                output: CardSpec {
                    tier: Tier::Jade,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 2000,
            },
        ];

        self.recipes = recipes;
    }
}

// =============================================================================
// 辅助函数
// =============================================================================

/// 返回下一个品质等级（None = 已是最高）。
pub fn next_tier(tier: Tier) -> Option<Tier> {
    match tier {
        Tier::Blackiron => Some(Tier::Bronze),
        Tier::Bronze => Some(Tier::Silver),
        Tier::Silver => Some(Tier::Gold),
        Tier::Gold => Some(Tier::Jade),
        Tier::Jade => Some(Tier::Divine),
        Tier::Divine => None,
    }
}

/// 返回指定品质对应的最高可装备境界（用于合成校验）。
pub fn tier_to_max_realm(tier: Tier) -> Realm {
    match tier {
        Tier::Blackiron => Realm::QiRefining,
        Tier::Bronze => Realm::Foundation,
        Tier::Silver => Realm::GoldenCore,
        Tier::Gold => Realm::NascentSoul,
        Tier::Jade => Realm::SpiritSevering,
        Tier::Divine => Realm::VoidRefining,
    }
}

/// 获取指定品质的合成 cost（用于分解返还计算）。
pub fn tier_synthesis_cost(tier: Option<Tier>) -> u64 {
    match tier {
        None => 0,
        Some(Tier::Blackiron) => 0,
        Some(Tier::Bronze) => 100,
        Some(Tier::Silver) => 200,
        Some(Tier::Gold) => 500,
        Some(Tier::Jade) => 1000,
        Some(Tier::Divine) => 2000,
    }
}

/// 分解时额外产出数量（基于品质的固定加成，不引入随机）。
fn tier_decompose_bonus(tier: Tier) -> u32 {
    match tier {
        Tier::Blackiron => 0,
        Tier::Bronze => 0,
        Tier::Silver => 1,
        Tier::Gold => 1,
        Tier::Jade => 2,
        Tier::Divine => 2,
    }
}

/// 6 阶品质名称映射（文档对照，不产生 runtime 开销）。
#[allow(dead_code)]
const TIER_TO_QUALITY_NAME: &[(Tier, &str)] = &[
    (Tier::Blackiron, "Common"),
    (Tier::Bronze, "Uncommon"),
    (Tier::Silver, "Rare"),
    (Tier::Gold, "Epic"),
    (Tier::Jade, "Legendary"),
    (Tier::Divine, "Mythic"),
];

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::agent::AgentId;

    // ── Mock CardManager ──

    struct MockCardManager {
        cards: std::sync::Mutex<Vec<Card>>,
    }

    impl MockCardManager {
        fn new() -> Self {
            Self {
                cards: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl CardManager for MockCardManager {
        async fn get_card(&self, card_id: &CardId) -> Result<Card, CardError> {
            let cards = self.cards.lock().unwrap();
            cards
                .iter()
                .find(|c| c.card_id == *card_id)
                .cloned()
                .ok_or(CardError::CardNotFound(*card_id))
        }

        async fn list_cards(&self, _owner_id: &AgentId) -> Result<Vec<Card>, CardError> {
            let cards = self.cards.lock().unwrap();
            Ok(cards.clone())
        }

        async fn add_card(&self, _owner_id: &AgentId, card: Card) -> Result<(), CardError> {
            let mut cards = self.cards.lock().unwrap();
            cards.push(card);
            Ok(())
        }

        async fn remove_card(&self, _owner_id: &AgentId, card_id: &CardId) -> Result<(), CardError> {
            let mut cards = self.cards.lock().unwrap();
            let len_before = cards.len();
            cards.retain(|c| c.card_id != *card_id);
            if cards.len() < len_before {
                Ok(())
            } else {
                Err(CardError::CardNotFound(*card_id))
            }
        }

        async fn equip_card(
            &self,
            _owner_id: &AgentId,
            _card_id: &CardId,
            _slot_type: taiji_types::card::SlotType,
            _slot_index: u32,
            _current_realm: &Realm,
        ) -> Result<(), CardError> {
            Ok(())
        }

        async fn unequip_card(
            &self,
            _owner_id: &AgentId,
            _slot_type: taiji_types::card::SlotType,
            _slot_index: u32,
        ) -> Result<(), CardError> {
            Ok(())
        }

        async fn get_equipped_slots(
            &self,
            _owner_id: &AgentId,
        ) -> Result<Vec<taiji_types::card::CardSlot>, CardError> {
            Ok(Vec::new())
        }

        async fn get_slot_config(
            &self,
            _owner_id: &AgentId,
        ) -> Result<taiji_types::card::SlotConfig, CardError> {
            Ok(taiji_types::card::SlotConfig::default())
        }

        async fn after_realm_upgrade(
            &self,
            _owner_id: &AgentId,
            _new_realm: &Realm,
        ) -> Result<Vec<CardId>, CardError> {
            Ok(Vec::new())
        }

        async fn unlock_slot(
            &self,
            _owner_id: &AgentId,
            _slot_type: taiji_types::card::SlotType,
            _slot_index: u32,
        ) -> Result<(), CardError> {
            Ok(())
        }
    }

    fn make_card(id: u64, tier: Tier, card_type: CardType) -> Card {
        let mut card = Card::new(format!("card_{}", id), card_type, tier, tier);
        card.card_id = CardId::new(id);
        card
    }

    // ── CardSpec 测试 ──

    #[test]
    fn test_card_spec_matches_tier() {
        let spec = CardSpec {
            tier: Tier::Blackiron,
            card_type: None,
            name_pattern: None,
        };
        let card = make_card(1, Tier::Blackiron, CardType::Spell);
        assert!(spec.matches(&card));
    }

    #[test]
    fn test_card_spec_rejects_wrong_tier() {
        let spec = CardSpec {
            tier: Tier::Blackiron,
            card_type: None,
            name_pattern: None,
        };
        let card = make_card(1, Tier::Bronze, CardType::Spell);
        assert!(!spec.matches(&card));
    }

    #[test]
    fn test_card_spec_matches_card_type() {
        let spec = CardSpec {
            tier: Tier::Silver,
            card_type: Some(CardType::Weapon),
            name_pattern: None,
        };
        let card = make_card(1, Tier::Silver, CardType::Weapon);
        assert!(spec.matches(&card));
    }

    #[test]
    fn test_card_spec_rejects_wrong_card_type() {
        let spec = CardSpec {
            tier: Tier::Silver,
            card_type: Some(CardType::Weapon),
            name_pattern: None,
        };
        let card = make_card(1, Tier::Silver, CardType::Spell);
        assert!(!spec.matches(&card));
    }

    #[test]
    fn test_card_spec_matches_name_pattern() {
        let spec = CardSpec {
            tier: Tier::Gold,
            card_type: None,
            name_pattern: Some("九转".into()),
        };
        let mut card = make_card(1, Tier::Gold, CardType::Spell);
        card.name = "九转金身诀".into();
        assert!(spec.matches(&card));
    }

    // ── SynthesisEngine 测试 ──

    #[test]
    fn test_engine_new_has_no_recipes() {
        let engine = SynthesisEngine::new();
        assert!(engine.recipes().is_empty());
    }

    #[test]
    fn test_engine_with_default_recipes() {
        let engine = SynthesisEngine::with_default_recipes();
        assert_eq!(engine.recipes().len(), 9); // 5 standard + 4 fast
    }

    #[test]
    fn test_find_recipe_exists() {
        let engine = SynthesisEngine::with_default_recipes();
        assert!(engine.find_recipe("upgrade_blackiron_to_bronze").is_some());
    }

    #[test]
    fn test_find_recipe_not_found() {
        let engine = SynthesisEngine::with_default_recipes();
        assert!(engine.find_recipe("nonexistent").is_none());
    }

    #[test]
    fn test_next_tier_basic() {
        assert_eq!(next_tier(Tier::Blackiron), Some(Tier::Bronze));
        assert_eq!(next_tier(Tier::Bronze), Some(Tier::Silver));
        assert_eq!(next_tier(Tier::Silver), Some(Tier::Gold));
        assert_eq!(next_tier(Tier::Gold), Some(Tier::Jade));
        assert_eq!(next_tier(Tier::Jade), Some(Tier::Divine));
        assert_eq!(next_tier(Tier::Divine), None);
    }

    #[test]
    fn test_tier_to_max_realm() {
        assert_eq!(tier_to_max_realm(Tier::Blackiron), Realm::QiRefining);
        assert_eq!(tier_to_max_realm(Tier::Bronze), Realm::Foundation);
        assert_eq!(tier_to_max_realm(Tier::Silver), Realm::GoldenCore);
        assert_eq!(tier_to_max_realm(Tier::Gold), Realm::NascentSoul);
        assert_eq!(tier_to_max_realm(Tier::Jade), Realm::SpiritSevering);
        assert_eq!(tier_to_max_realm(Tier::Divine), Realm::VoidRefining);
    }

    #[test]
    fn test_tier_synthesis_cost_values() {
        assert_eq!(tier_synthesis_cost(Some(Tier::Blackiron)), 0);
        assert_eq!(tier_synthesis_cost(Some(Tier::Bronze)), 100);
        assert_eq!(tier_synthesis_cost(Some(Tier::Silver)), 200);
        assert_eq!(tier_synthesis_cost(Some(Tier::Gold)), 500);
        assert_eq!(tier_synthesis_cost(Some(Tier::Jade)), 1000);
        assert_eq!(tier_synthesis_cost(Some(Tier::Divine)), 2000);
        assert_eq!(tier_synthesis_cost(None), 0);
    }

    #[test]
    fn test_tier_decompose_bonus() {
        assert_eq!(tier_decompose_bonus(Tier::Blackiron), 0);
        assert_eq!(tier_decompose_bonus(Tier::Bronze), 0);
        assert_eq!(tier_decompose_bonus(Tier::Silver), 1);
        assert_eq!(tier_decompose_bonus(Tier::Gold), 1);
        assert_eq!(tier_decompose_bonus(Tier::Jade), 2);
        assert_eq!(tier_decompose_bonus(Tier::Divine), 2);
    }

    // ── 合成测试 ──

    #[tokio::test]
    async fn test_synthesize_success() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card1 = make_card(1, Tier::Blackiron, CardType::Spell);
        let card2 = make_card(2, Tier::Blackiron, CardType::Weapon);
        let card3 = make_card(3, Tier::Blackiron, CardType::Armor);

        mgr.add_card(&owner_id, card1.clone()).await.unwrap();
        mgr.add_card(&owner_id, card2.clone()).await.unwrap();
        mgr.add_card(&owner_id, card3.clone()).await.unwrap();

        let inputs = vec![card1, card2, card3];
        // QiRefining 无法装备 Bronze（输出 Tier），改用 Foundation
        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &inputs,
                &Realm::Foundation,
                200,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(result.is_ok(), "synthesize failed: {:?}", result.err());
        let output = result.unwrap();
        assert_eq!(output.tier, Tier::Bronze);
        // 输入卡已移除
        assert!(mgr.get_card(&CardId::new(1)).await.is_err());
        assert!(mgr.get_card(&CardId::new(2)).await.is_err());
        assert!(mgr.get_card(&CardId::new(3)).await.is_err());
    }

    #[tokio::test]
    async fn test_synthesize_recipe_not_found() {
        let engine = SynthesisEngine::new();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let result = engine
            .synthesize(
                "nonexistent",
                &[],
                &Realm::QiRefining,
                0,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::RecipeNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_synthesize_input_count_mismatch() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card = make_card(1, Tier::Blackiron, CardType::Spell);
        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &[card],
                &Realm::QiRefining,
                200,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::InputCountMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn test_synthesize_input_spec_mismatch() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card1 = make_card(1, Tier::Bronze, CardType::Spell); // Wrong tier
        let card2 = make_card(2, Tier::Bronze, CardType::Weapon);
        let card3 = make_card(3, Tier::Bronze, CardType::Armor);

        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &[card1, card2, card3],
                &Realm::QiRefining,
                200,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::InputSpecMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn test_synthesize_insufficient_spirit_stones() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card1 = make_card(1, Tier::Blackiron, CardType::Spell);
        let card2 = make_card(2, Tier::Blackiron, CardType::Weapon);
        let card3 = make_card(3, Tier::Blackiron, CardType::Armor);

        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &[card1, card2, card3],
                &Realm::QiRefining,
                50, // 不够 100
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::InsufficientSpiritStones { .. })
        ));
    }

    #[tokio::test]
    async fn test_synthesize_output_tier_locked() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card1 = make_card(1, Tier::Blackiron, CardType::Spell);
        let card2 = make_card(2, Tier::Blackiron, CardType::Weapon);
        let card3 = make_card(3, Tier::Blackiron, CardType::Armor);

        // 炼气无法装备青铜卡，因此合成 Bronze 应该被锁定
        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &[card1, card2, card3],
                &Realm::QiRefining,
                200,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::OutputTierLocked { .. })
        ));
    }

    #[tokio::test]
    async fn test_synthesize_duplicate_input() {
        let engine = SynthesisEngine::with_default_recipes();
        let mgr = MockCardManager::new();
        let owner_id = AgentId::new();

        let card1 = make_card(1, Tier::Blackiron, CardType::Spell);
        let card2 = make_card(1, Tier::Blackiron, CardType::Spell); // Same ID

        // 需要 3 张，补第 3 张
        let card3 = make_card(3, Tier::Blackiron, CardType::Armor);

        let result = engine
            .synthesize(
                "upgrade_blackiron_to_bronze",
                &[card1, card2, card3],
                &Realm::QiRefining,
                200,
                &mgr,
                &owner_id,
            )
            .await;

        assert!(matches!(
            result,
            Err(SynthesisError::DuplicateInput(_))
        ));
    }

    // ── 分解测试 ──

    #[test]
    fn test_decompose_blackiron() {
        let engine = SynthesisEngine::new();
        let card = make_card(1, Tier::Blackiron, CardType::Spell);
        let (rewards, refund) = engine.decompose(&card).unwrap();

        assert_eq!(rewards.len(), 1);
        assert_eq!(rewards[0].quantity, 1);
        assert_eq!(rewards[0].material.tier, Tier::Blackiron);
        assert_eq!(rewards[0].material.card_type, CardType::Material);
        // Blackiron 分解无上层合成，refund = tier_synthesis_cost(Some(Bronze)) / 2 = 50
        assert_eq!(refund, 50);
    }

    #[test]
    fn test_decompose_silver() {
        let engine = SynthesisEngine::new();
        let card = make_card(1, Tier::Silver, CardType::Spell);
        let (rewards, refund) = engine.decompose(&card).unwrap();

        assert_eq!(rewards.len(), 1);
        // Silver: quantity=2 + bonus=1 = 3
        assert_eq!(rewards[0].quantity, 3);
        assert_eq!(rewards[0].material.tier, Tier::Silver);
        // Silver 合成 cost = tier_synthesis_cost(Some(Gold)) / 2 = 250
        assert_eq!(refund, 250);
    }

    #[test]
    fn test_decompose_divine() {
        let engine = SynthesisEngine::new();
        let card = make_card(1, Tier::Divine, CardType::Spell);
        let (rewards, refund) = engine.decompose(&card).unwrap();

        assert_eq!(rewards.len(), 1);
        // Divine: quantity=4 + bonus=2 = 6
        assert_eq!(rewards[0].quantity, 6);
        assert_eq!(rewards[0].material.tier, Tier::Divine);
        // Divine 无上层合成，refund = tier_synthesis_cost(None) / 2 = 0
        assert_eq!(refund, 0);
    }

    // ── Custom recipe ──

    #[test]
    fn test_add_custom_recipe() {
        let mut engine = SynthesisEngine::new();
        let recipe = SynthesisRecipe {
            recipe_id: "custom_test".into(),
            name: "测试配方".into(),
            description: "".into(),
            inputs: vec![CardSpec {
                tier: Tier::Blackiron,
                card_type: Some(CardType::Material),
                name_pattern: None,
            }],
            output: CardSpec {
                tier: Tier::Bronze,
                card_type: Some(CardType::Material),
                name_pattern: None,
            },
            cost: 50,
        };
        engine.add_recipe(recipe);
        assert_eq!(engine.recipes().len(), 1);
        assert!(engine.find_recipe("custom_test").is_some());
    }

    #[test]
    fn test_add_recipes_batch() {
        let mut engine = SynthesisEngine::new();
        let recipes = vec![
            SynthesisRecipe {
                recipe_id: "r1".into(),
                name: "R1".into(),
                description: "".into(),
                inputs: vec![],
                output: CardSpec {
                    tier: Tier::Blackiron,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 0,
            },
            SynthesisRecipe {
                recipe_id: "r2".into(),
                name: "R2".into(),
                description: "".into(),
                inputs: vec![],
                output: CardSpec {
                    tier: Tier::Bronze,
                    card_type: None,
                    name_pattern: None,
                },
                cost: 0,
            },
        ];
        engine.add_recipes(recipes);
        assert_eq!(engine.recipes().len(), 2);
    }

    // ── SynthesisError display ──

    #[test]
    fn test_synthesis_error_display() {
        let err = SynthesisError::RecipeNotFound("test".into());
        assert_eq!(err.to_string(), "recipe not found: test");

        let err = SynthesisError::InputCountMismatch {
            expected: 3,
            got: 1,
        };
        assert!(err.to_string().contains("expected 3"));
    }
}
