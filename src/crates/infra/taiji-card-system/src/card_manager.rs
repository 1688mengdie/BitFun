//! # CardManager — 卡片管理器 trait + 装备/境界校验（Wave 1）
//!
//! 提供卡牌 CRUD、装备/卸装、境界锁定校验，以及境界掉落后的卡牌降级处理。
//!
//! 参考：
//! - godot-skill-system scripts/core/skills/skillManager.gd (MIT)
//! - modules/card-system/实现参考.rs:293-331
//! - taiji-types/src/card.rs `can_equip_tier` / `realm_allowed_tiers`

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

use taiji_types::agent::AgentId;
use taiji_types::card::{
    can_equip_tier, Card, CardError, CardId, CardSlot, SlotConfig, SlotType,
};
use taiji_types::realm::Realm;

// =============================================================================
// CardManager trait
// =============================================================================

/// 卡片管理器 — 卡牌 CRUD + 装备管理 + 境界锁定 + 境界掉落处理。
#[async_trait]
pub trait CardManager: Send + Sync {
    // ── 卡牌操作 ──
    async fn get_card(&self, card_id: &CardId) -> Result<Card, CardError>;
    async fn list_cards(&self, owner_id: &AgentId) -> Result<Vec<Card>, CardError>;
    async fn add_card(&self, owner_id: &AgentId, card: Card) -> Result<(), CardError>;
    async fn remove_card(&self, owner_id: &AgentId, card_id: &CardId) -> Result<(), CardError>;

    // ── 装备/卸装 ──
    async fn equip_card(
        &self,
        owner_id: &AgentId,
        card_id: &CardId,
        slot_type: SlotType,
        slot_index: u32,
        current_realm: &Realm,
    ) -> Result<(), CardError>;
    async fn unequip_card(
        &self,
        owner_id: &AgentId,
        slot_type: SlotType,
        slot_index: u32,
    ) -> Result<(), CardError>;
    async fn get_equipped_slots(&self, owner_id: &AgentId) -> Result<Vec<CardSlot>, CardError>;
    async fn get_slot_config(&self, owner_id: &AgentId) -> Result<SlotConfig, CardError>;

    // ── 境界处理 ──
    /// Agent 境界变化（升级或降级）后的处理。
    ///
    /// 境界降级时，检查所有已装备卡牌的 tier 是否仍被新境界允许。
    /// 不被允许的卡牌槽位被标记为 `is_locked = true`。
    /// 返回受影响的卡牌 ID 列表。
    async fn after_realm_upgrade(
        &self,
        owner_id: &AgentId,
        new_realm: &Realm,
    ) -> Result<Vec<CardId>, CardError>;

    /// 解锁指定槽位（消耗道具或满足条件后调用）。
    async fn unlock_slot(
        &self,
        owner_id: &AgentId,
        slot_type: SlotType,
        slot_index: u32,
    ) -> Result<(), CardError>;
}

// =============================================================================
// InMemoryCardManager
// =============================================================================

/// 内存版卡片管理器。
pub struct InMemoryCardManager {
    /// owner_id → Vec<Card>
    cards: Arc<DashMap<AgentId, Vec<Card>>>,
    /// owner_id → Vec<CardSlot>
    slots: Arc<DashMap<AgentId, Vec<CardSlot>>>,
    /// owner_id → SlotConfig
    configs: Arc<DashMap<AgentId, SlotConfig>>,
}

impl InMemoryCardManager {
    pub fn new() -> Self {
        Self {
            cards: Arc::new(DashMap::new()),
            slots: Arc::new(DashMap::new()),
            configs: Arc::new(DashMap::new()),
        }
    }

    /// 创建默认槽位列表。
    fn default_slots(config: &SlotConfig) -> Vec<CardSlot> {
        let mut slots = Vec::new();
        // 本命魂卡槽（固定 1）
        for i in 0..config.soulbound_slots {
            slots.push(CardSlot {
                slot_type: SlotType::Main,
                slot_index: i,
                equipped_card: None,
                is_locked: false,
            });
        }
        // 普通卡槽
        for i in 0..config.normal_slots {
            slots.push(CardSlot {
                slot_type: SlotType::Sub,
                slot_index: i,
                equipped_card: None,
                is_locked: false,
            });
        }
        // 被动卡槽
        for i in 0..config.passive_slots {
            slots.push(CardSlot {
                slot_type: SlotType::Passive,
                slot_index: i,
                equipped_card: None,
                is_locked: false,
            });
        }
        // 消耗卡槽
        for i in 0..config.consumable_slots {
            slots.push(CardSlot {
                slot_type: SlotType::Consumable,
                slot_index: i,
                equipped_card: None,
                is_locked: false,
            });
        }
        slots
    }
}

impl Default for InMemoryCardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CardManager for InMemoryCardManager {
    async fn get_card(&self, card_id: &CardId) -> Result<Card, CardError> {
        for entry in self.cards.iter() {
            if let Some(card) = entry.value().iter().find(|c| c.card_id == *card_id) {
                return Ok(card.clone());
            }
        }
        Err(CardError::CardNotFound(*card_id))
    }

    async fn list_cards(&self, owner_id: &AgentId) -> Result<Vec<Card>, CardError> {
        Ok(self.cards.get(owner_id).map(|e| e.clone()).unwrap_or_default())
    }

    async fn add_card(&self, owner_id: &AgentId, card: Card) -> Result<(), CardError> {
        self.cards
            .entry(owner_id.clone())
            .or_default()
            .push(card);
        Ok(())
    }

    async fn remove_card(&self, owner_id: &AgentId, card_id: &CardId) -> Result<(), CardError> {
        let mut removed = false;
        if let Some(mut entry) = self.cards.get_mut(owner_id) {
            let len_before = entry.len();
            entry.retain(|c| c.card_id != *card_id);
            removed = entry.len() < len_before;
        }
        if removed {
            Ok(())
        } else {
            Err(CardError::CardNotFound(*card_id))
        }
    }

    async fn equip_card(
        &self,
        owner_id: &AgentId,
        card_id: &CardId,
        slot_type: SlotType,
        slot_index: u32,
        current_realm: &Realm,
    ) -> Result<(), CardError> {
        // 查找卡牌
        let card = self.get_card(card_id).await?;

        // 境界锁定检查
        if !can_equip_tier(*current_realm, card.tier) {
            return Err(CardError::RealmLockTier(*current_realm, card.tier));
        }

        // 检查目标槽位是否可用
        let mut slots = self.slots.entry(owner_id.clone()).or_insert_with(|| {
            let config = self.configs.get(owner_id).map(|c| c.clone()).unwrap_or_default();
            Self::default_slots(&config)
        });

        let target = slots.iter_mut().find(|s| s.slot_type == slot_type && s.slot_index == slot_index);
        match target {
            Some(slot) if slot.is_locked => {
                return Err(CardError::RealmLockTier(*current_realm, card.tier));
            }
            Some(slot) => {
                slot.equipped_card = Some(*card_id);
                slot.is_locked = false;
                Ok(())
            }
            None => Err(CardError::SlotOccupied(slot_type, slot_index)),
        }
    }

    async fn unequip_card(
        &self,
        owner_id: &AgentId,
        slot_type: SlotType,
        slot_index: u32,
    ) -> Result<(), CardError> {
        let mut slots = self.slots.entry(owner_id.clone()).or_default();
        let target = slots.iter_mut().find(|s| s.slot_type == slot_type && s.slot_index == slot_index);
        match target {
            Some(slot) => {
                slot.equipped_card = None;
                slot.is_locked = false; // 卸装时自动解锁
                Ok(())
            }
            None => Err(CardError::SlotOccupied(slot_type, slot_index)),
        }
    }

    async fn get_equipped_slots(&self, owner_id: &AgentId) -> Result<Vec<CardSlot>, CardError> {
        Ok(self.slots.get(owner_id).map(|e| e.clone()).unwrap_or_default())
    }

    async fn get_slot_config(&self, owner_id: &AgentId) -> Result<SlotConfig, CardError> {
        Ok(self.configs.get(owner_id).map(|e| e.clone()).unwrap_or_default())
    }

    async fn after_realm_upgrade(
        &self,
        owner_id: &AgentId,
        new_realm: &Realm,
    ) -> Result<Vec<CardId>, CardError> {
        let mut affected = Vec::new();

        let mut slots = self.slots.entry(owner_id.clone()).or_default();
        for slot in slots.iter_mut() {
            if let Some(card_id) = slot.equipped_card {
                // 从卡牌库查找 tier
                let tier_opt = self.cards.get(owner_id).and_then(|cards| {
                    cards.iter().find(|c| c.card_id == card_id).map(|c| c.tier)
                });
                if let Some(tier) = tier_opt {
                    if !can_equip_tier(*new_realm, tier) {
                        slot.is_locked = true;
                        affected.push(card_id);
                    }
                }
            }
        }

        Ok(affected)
    }

    async fn unlock_slot(
        &self,
        owner_id: &AgentId,
        slot_type: SlotType,
        slot_index: u32,
    ) -> Result<(), CardError> {
        let mut slots = self.slots.entry(owner_id.clone()).or_default();
        let target = slots.iter_mut().find(|s| s.slot_type == slot_type && s.slot_index == slot_index);
        match target {
            Some(slot) => {
                slot.is_locked = false;
                Ok(())
            }
            None => Err(CardError::SlotOccupied(slot_type, slot_index)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::card::CardType;
    use taiji_types::card::Tier;

    fn make_test_card(id: u64, tier: Tier) -> Card {
        let mut card = Card::new(format!("card_{}", id), CardType::Spell, tier, Tier::Blackiron);
        card.card_id = CardId::new(id);
        card
    }

    #[tokio::test]
    async fn test_add_and_get_card() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        let card = make_test_card(1, Tier::Blackiron);
        mgr.add_card(&id, card.clone()).await.unwrap();
        let got = mgr.get_card(&CardId::new(1)).await.unwrap();
        assert_eq!(got.name, card.name);
    }

    #[tokio::test]
    async fn test_list_cards() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        mgr.add_card(&id, make_test_card(1, Tier::Blackiron)).await.unwrap();
        mgr.add_card(&id, make_test_card(2, Tier::Bronze)).await.unwrap();
        let cards = mgr.list_cards(&id).await.unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[tokio::test]
    async fn test_equip_realm_lock() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        let card = make_test_card(1, Tier::Gold); // 黄金卡, card_id = 1
        mgr.add_card(&id, card).await.unwrap();
        // 炼气无法装备黄金卡
        let result = mgr.equip_card(&id, &CardId::new(1), SlotType::Sub, 0, &Realm::QiRefining).await;
        assert!(matches!(result, Err(CardError::RealmLockTier(..))));
    }

    #[tokio::test]
    async fn test_equip_success() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        let card = make_test_card(1, Tier::Blackiron); // card_id = 1
        mgr.add_card(&id, card).await.unwrap();
        mgr.equip_card(&id, &CardId::new(1), SlotType::Sub, 0, &Realm::QiRefining).await.unwrap();
        let slots = mgr.get_equipped_slots(&id).await.unwrap();
        assert!(slots.iter().any(|s| s.equipped_card == Some(CardId::new(1))));
    }

    #[tokio::test]
    async fn test_after_realm_upgrade_locks_over_tier() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        // 添加一张白银卡(card_id=1)和一张青铜卡(card_id=2)
        let silver_card = make_test_card(1, Tier::Silver);
        let bronze_card = make_test_card(2, Tier::Bronze);
        mgr.add_card(&id, silver_card).await.unwrap();
        mgr.add_card(&id, bronze_card).await.unwrap();
        // 在金丹境界装备（金丹允许 Bronze + Silver）
        mgr.equip_card(&id, &CardId::new(1), SlotType::Sub, 0, &Realm::GoldenCore).await.unwrap();
        mgr.equip_card(&id, &CardId::new(2), SlotType::Sub, 1, &Realm::GoldenCore).await.unwrap();

        // 降级到筑基（只能 Blackiron + Bronze）
        let affected = mgr.after_realm_upgrade(&id, &Realm::Foundation).await.unwrap();
        assert_eq!(affected.len(), 1); // 只有 Silver 卡被锁
        assert_eq!(affected[0], CardId::new(1));

        let slots = mgr.get_equipped_slots(&id).await.unwrap();
        let silver_slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(1))).unwrap();
        assert!(silver_slot.is_locked);
        let bronze_slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(2))).unwrap();
        assert!(!bronze_slot.is_locked);
    }

    #[tokio::test]
    async fn test_after_realm_upgrade_noop_when_no_cards() {
        let mgr = InMemoryCardManager::new();
        let id = AgentId::new();
        let affected = mgr.after_realm_upgrade(&id, &Realm::Foundation).await.unwrap();
        assert!(affected.is_empty());
    }
}
