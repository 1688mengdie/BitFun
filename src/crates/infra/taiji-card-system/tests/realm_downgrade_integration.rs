//! # 全链路事件链集成测试 — 评分下降→境界掉落→卡片降级
//!
//! 验证场景：
//! 1. Agent 境界降级（如金丹→筑基）后，超限卡牌被正确锁定
//! 2. 低阶卡牌不受影响
//! 3. 无卡牌时不崩溃
//! 4. 解锁后恢复装备能力

use taiji_card_system::card_manager::{CardManager, InMemoryCardManager};
use taiji_types::agent::AgentId;
use taiji_types::card::{can_equip_tier, Card, CardId, CardType, SlotType, Tier};
use taiji_types::realm::Realm;

/// 辅助：创建测试用卡牌
fn make_card(id: u64, name: &str, tier: Tier) -> Card {
    let mut card = Card::new(name.into(), CardType::Spell, tier, Tier::Blackiron);
    card.card_id = CardId::new(id);
    card
}

/// 场景 01：金丹→筑基降级，Gold 卡被锁定，Blackiron 卡正常
#[tokio::test]
async fn test_downgrade_locks_high_tier_only() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    // 添加卡牌：1 张 Silver + 1 张 Bronze
    mgr.add_card(&agent, make_card(1, "九转金身诀", Tier::Silver)).await.unwrap();
    mgr.add_card(&agent, make_card(2, "铁布衫", Tier::Bronze)).await.unwrap();

    // 在金丹境界装备两张卡（金丹允许 Bronze + Silver）
    mgr.equip_card(&agent, &CardId::new(1), SlotType::Sub, 0, &Realm::GoldenCore).await.unwrap();
    mgr.equip_card(&agent, &CardId::new(2), SlotType::Sub, 1, &Realm::GoldenCore).await.unwrap();

    // 降级到筑基（筑基允许 Blackiron + Bronze）
    let affected = mgr.after_realm_upgrade(&agent, &Realm::Foundation).await.unwrap();
    assert_eq!(affected.len(), 1, "只有 Silver 卡应被锁定");
    assert_eq!(affected[0], CardId::new(1), "Silver 卡应被锁定");

    let slots = mgr.get_equipped_slots(&agent).await.unwrap();

    let silver_slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(1))).unwrap();
    assert!(silver_slot.is_locked, "Silver 卡槽应锁定");

    let bronze_slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(2))).unwrap();
    assert!(!bronze_slot.is_locked, "Bronze 卡槽应正常");
}

/// 场景 02：降级到炼气，所有高于 Blackiron 的卡都被锁定
#[tokio::test]
async fn test_downgrade_to_qi_refining_locks_all() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    // Silver, Gold 各一张（元婴可装备两者）
    mgr.add_card(&agent, make_card(1, "白银剑", Tier::Silver)).await.unwrap();
    mgr.add_card(&agent, make_card(2, "黄金甲", Tier::Gold)).await.unwrap();

    // 在元婴境界装备（元婴允许 Silver + Gold）
    mgr.equip_card(&agent, &CardId::new(1), SlotType::Sub, 0, &Realm::NascentSoul).await.unwrap();
    mgr.equip_card(&agent, &CardId::new(2), SlotType::Sub, 1, &Realm::NascentSoul).await.unwrap();

    // 降级到炼气（只有 Blackiron）
    let affected = mgr.after_realm_upgrade(&agent, &Realm::QiRefining).await.unwrap();
    assert_eq!(affected.len(), 2, "全部卡牌应被锁定");

    let slots = mgr.get_equipped_slots(&agent).await.unwrap();
    for slot in &slots {
        if slot.equipped_card.is_some() {
            assert!(slot.is_locked, "所有装备卡槽应锁定");
        }
    }
}

/// 场景 03：无卡牌时 after_realm_upgrade 不崩溃
#[tokio::test]
async fn test_downgrade_no_cards_noop() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    let affected = mgr.after_realm_upgrade(&agent, &Realm::Foundation).await.unwrap();
    assert!(affected.is_empty(), "无卡牌时应返回空列表");
}

/// 场景 04：无装备卡时 after_realm_upgrade 不崩溃
#[tokio::test]
async fn test_downgrade_no_equipped_cards() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    mgr.add_card(&agent, make_card(1, "闲置金卡", Tier::Gold)).await.unwrap();

    let affected = mgr.after_realm_upgrade(&agent, &Realm::Foundation).await.unwrap();
    assert!(affected.is_empty(), "未装备卡牌时应返回空列表");
}

/// 场景 05：解锁后 can_equip_tier 恢复正常
#[tokio::test]
async fn test_unlock_after_downgrade() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    mgr.add_card(&agent, make_card(1, "白银剑", Tier::Silver)).await.unwrap();
    mgr.equip_card(&agent, &CardId::new(1), SlotType::Sub, 0, &Realm::GoldenCore).await.unwrap();

    // 降级到筑基 → Silver 被锁定（筑基只能 Blackiron + Bronze）
    let affected = mgr.after_realm_upgrade(&agent, &Realm::Foundation).await.unwrap();
    assert_eq!(affected.len(), 1);

    // 解锁
    mgr.unlock_slot(&agent, SlotType::Sub, 0).await.unwrap();
    let slots = mgr.get_equipped_slots(&agent).await.unwrap();
    let slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(1))).unwrap();
    assert!(!slot.is_locked, "解锁后 is_locked 应为 false");
    assert!(slot.equipped_card.is_some(), "解锁后卡牌应仍装备");
}

/// 场景 06：境界升级时（同境界或更高境界）不影响任何卡牌
#[tokio::test]
async fn test_upgrade_does_not_lock() {
    let mgr = InMemoryCardManager::new();
    let agent = AgentId::new();

    mgr.add_card(&agent, make_card(1, "白银剑", Tier::Silver)).await.unwrap();
    mgr.equip_card(&agent, &CardId::new(1), SlotType::Sub, 0, &Realm::GoldenCore).await.unwrap();

    // 升级到元婴（Silver 仍然允许）
    let affected = mgr.after_realm_upgrade(&agent, &Realm::NascentSoul).await.unwrap();
    assert!(affected.is_empty(), "升级不应锁定任何卡牌");

    let slots = mgr.get_equipped_slots(&agent).await.unwrap();
    let slot = slots.iter().find(|s| s.equipped_card == Some(CardId::new(1))).unwrap();
    assert!(!slot.is_locked, "升级后卡槽不应锁定");
}

/// 场景 07：验证 can_equip_tier 映射正确性（防御性）
#[test]
fn test_can_equip_tier_mapping() {
    // 炼气只能 Blackiron
    assert!(can_equip_tier(Realm::QiRefining, Tier::Blackiron));
    assert!(!can_equip_tier(Realm::QiRefining, Tier::Bronze));
    // 筑基可用 Blackiron + Bronze
    assert!(can_equip_tier(Realm::Foundation, Tier::Blackiron));
    assert!(can_equip_tier(Realm::Foundation, Tier::Bronze));
    assert!(!can_equip_tier(Realm::Foundation, Tier::Silver));
    // 金丹可用 Bronze + Silver
    assert!(can_equip_tier(Realm::GoldenCore, Tier::Bronze));
    assert!(can_equip_tier(Realm::GoldenCore, Tier::Silver));
    assert!(!can_equip_tier(Realm::GoldenCore, Tier::Blackiron));
    // 元婴可用 Silver + Gold
    assert!(can_equip_tier(Realm::NascentSoul, Tier::Silver));
    assert!(can_equip_tier(Realm::NascentSoul, Tier::Gold));
    assert!(!can_equip_tier(Realm::NascentSoul, Tier::Jade));
    // 化神可用 Gold + Jade
    assert!(can_equip_tier(Realm::SpiritSevering, Tier::Jade));
    assert!(!can_equip_tier(Realm::SpiritSevering, Tier::Divine));
    // 炼虚可用 Jade + Divine
    assert!(can_equip_tier(Realm::VoidRefining, Tier::Divine));
    // 飞升可用 Divine
    assert!(can_equip_tier(Realm::ImmortalAscension, Tier::Divine));
}
