//! 经济系统全链路集成测试（R-EC-501）。

use taiji_economy::token::InMemoryTokenManager;
use taiji_economy::token::TokenManager;
use taiji_economy::stone::InMemoryStoneManager;
use taiji_economy::stone::StoneManager;
use taiji_economy::exchange::{ExchangeService, ExchangeServiceImpl, ExchangeRate};
use taiji_economy::subsidy::{SubsidyService, SubsidyServiceImpl};
use taiji_economy::market::{DefaultCardStub, MarketService, MarketServiceImpl};
use taiji_economy::realm_gate::{DomainId, RealmGateService, RealmGateServiceImpl};
use taiji_economy::bankruptcy::{BankruptcyService, BankruptcyServiceImpl, BankruptcyVerdict, demote_realm};
use taiji_economy::EconomyError;

use taiji_types::agent::AgentId;
use taiji_types::card::CardId;
use taiji_types::economy::CurrencyAmount;
use taiji_types::realm::Realm;

/// 场景 01：新手 Agent → Token 消耗不计入成本
#[tokio::test]
async fn test_novice_token_consumption_free() {
    let token = InMemoryTokenManager::new();
    let id = AgentId::new();
    token.get_or_create_account(&id).await.unwrap();
    // 默认 subsidy_active = true
    token.record_consumption(&id, CurrencyAmount::new(100)).await.unwrap();
    let stats = token.get_stats(&id).await.unwrap();
    assert_eq!(stats.total_consumed.as_u64(), 0); // 补贴期不计入
    assert!(stats.subsidy_active);
}

/// 场景 02：突破元婴 → Token 消耗开始计入成本
#[tokio::test]
async fn test_realm_breakthrough_stops_subsidy() {
    let token = InMemoryTokenManager::new();
    let subsidy = SubsidyServiceImpl::new(token);
    let id = AgentId::new();
    subsidy.token_mgr.get_or_create_account(&id).await.unwrap();

    // 突破元婴
    subsidy.on_realm_upgrade(&id, &Realm::NascentSoul).await.unwrap();
    assert!(!subsidy.token_mgr.is_subsidy_active(&id).await.unwrap());

    // 现在消耗应该计入
    subsidy.token_mgr.record_consumption(&id, CurrencyAmount::new(100)).await.unwrap();
    let stats = subsidy.token_mgr.get_stats(&id).await.unwrap();
    assert_eq!(stats.total_consumed.as_u64(), 100);
}

/// 场景 03：灵石 CRUD（充值→消费→查询余额→交易历史）
#[tokio::test]
async fn test_stone_crud() {
    let stone = InMemoryStoneManager::new();
    let id = AgentId::new();
    stone.deposit(&id, CurrencyAmount::new(5000)).await.unwrap();
    assert_eq!(stone.get_balance(&id).await.unwrap().as_u64(), 5000);

    stone.withdraw(&id, CurrencyAmount::new(1200)).await.unwrap();
    assert_eq!(stone.get_balance(&id).await.unwrap().as_u64(), 3800);

    let history = stone.get_transaction_history(&id, 10).await.unwrap();
    assert_eq!(history.len(), 2); // deposit + withdrawal
}

/// 场景 04：灵石→Token 兑换全流程
#[tokio::test]
async fn test_exchange_full_flow() {
    let stone = InMemoryStoneManager::new();
    let token = InMemoryTokenManager::new();
    let svc = ExchangeServiceImpl::with_rate(stone, token, ExchangeRate::new(10, 1)); // 10 灵石 = 1 灵力

    let id = AgentId::new();
    svc.stone_mgr.deposit(&id, CurrencyAmount::new(1000)).await.unwrap();
    svc.token_mgr.set_subsidy_active(&id, false).await.unwrap();

    let tokens = svc.exchange(&id, CurrencyAmount::new(200)).await.unwrap();
    assert_eq!(tokens.as_u64(), 20); // 200 / 10 = 20
    assert_eq!(svc.stone_mgr.get_balance(&id).await.unwrap().as_u64(), 800);
}

/// 场景 05：坊市全链路（上架→复制→版税）
#[tokio::test]
async fn test_market_full_flow() {
    let stone = InMemoryStoneManager::new();
    let market = MarketServiceImpl::new(stone, DefaultCardStub);

    let seller = AgentId::new();
    let buyer = AgentId::new();
    let card_id = CardId::new(99);

    let listing_id = market.list_card(&seller, card_id, CurrencyAmount::new(100)).await.unwrap();
    market.stone_mgr.deposit(&buyer, CurrencyAmount::new(500)).await.unwrap();

    let copied = market.copy_card(&buyer, &listing_id).await.unwrap();
    assert_eq!(copied, card_id);

    // 卖家有版税收入
    let seller_balance = market.stone_mgr.get_balance(&seller).await.unwrap();
    assert!(seller_balance.as_u64() > 0);

    // 买家灵石减少
    assert_eq!(market.stone_mgr.get_balance(&buyer).await.unwrap().as_u64(), 400); // 500 - 100

    // 版税历史
    let royalties = market.get_royalty_history(&seller).await.unwrap();
    assert_eq!(royalties.len(), 1);
}

/// 场景 06：余额不足时拒绝
#[tokio::test]
async fn test_insufficient_balance_rejected() {
    let stone = InMemoryStoneManager::new();
    let id = AgentId::new();
    // 不充值直接消费
    let result = stone.withdraw(&id, CurrencyAmount::new(100)).await;
    assert!(matches!(result, Err(EconomyError::AccountNotFound(_))));
}

/// 场景 07：飞升门控（越域→拒绝→升级→允许）
#[tokio::test]
async fn test_realm_gate_flow() {
    let gate = RealmGateServiceImpl::new();
    let id = AgentId::new();
    let trade_domain = DomainId("domain:trade".into());

    // 炼气不能访问交易域
    assert!(!gate.can_access(&id, &Realm::QiRefining, &trade_domain).await.unwrap());

    // 金丹可以
    assert!(gate.can_access(&id, &Realm::GoldenCore, &trade_domain).await.unwrap());
}

/// 场景 08：价格稀缺性递增
#[tokio::test]
async fn test_scarcity_pricing() {
    let stone = InMemoryStoneManager::new();
    let market = MarketServiceImpl::new(stone, DefaultCardStub);

    let seller = AgentId::new();
    let buyer = AgentId::new();
    market.stone_mgr.deposit(&buyer, CurrencyAmount::new(10000)).await.unwrap();

    let listing_id = market.list_card(&seller, CardId::new(1), CurrencyAmount::new(100)).await.unwrap();

    // 复制 5 次，每次价格递增 10%
    for _ in 0..5 {
        market.copy_card(&buyer, &listing_id).await.unwrap();
    }

    let listing = market.get_listing(&listing_id).await.unwrap().unwrap();
    assert_eq!(listing.copy_count, 5);
    // 100 * 1.1^5 = 160 (integer truncation)
    assert_eq!(listing.current_price.as_u64(), 160);
}

/// 场景 09：空列表查询不崩溃
#[tokio::test]
async fn test_empty_listings() {
    let market = MarketServiceImpl::new(InMemoryStoneManager::new(), DefaultCardStub);
    let listings = market.get_listings(&Default::default()).await.unwrap();
    assert!(listings.is_empty());
}

/// 场景 10：境界补贴检查边界
#[tokio::test]
async fn test_subsidy_boundaries() {
    let token = InMemoryTokenManager::new();
    let svc = SubsidyServiceImpl::new(token);

    assert!(svc.is_eligible(&Realm::QiRefining).await.unwrap());
    assert!(svc.is_eligible(&Realm::Foundation).await.unwrap());
    assert!(svc.is_eligible(&Realm::GoldenCore).await.unwrap());
    assert!(!svc.is_eligible(&Realm::NascentSoul).await.unwrap());
    assert!(!svc.is_eligible(&Realm::ImmortalAscension).await.unwrap());
}

/// 场景 11：入不敷出判定 — 灵石严重不足时触发 Critical（境界掉落）
#[tokio::test]
async fn test_bankruptcy_critical_triggers_demotion() {
    let token = InMemoryTokenManager::new();
    let stone = InMemoryStoneManager::new();
    let id = AgentId::new();

    // 通过公共 API 设置账户
    token.get_or_create_account(&id).await.unwrap();
    token.set_subsidy_active(&id, false).await.unwrap();
    stone.deposit(&id, CurrencyAmount::new(100)).await.unwrap();
    token.record_consumption(&id, CurrencyAmount::new(500)).await.unwrap();

    // 低阈值方便测试：消耗/余额 >= 3 即 Critical
    let svc = BankruptcyServiceImpl::with_ratios(token, stone, 2.0, 3.0);
    let verdict = svc.assess(&id, &Realm::NascentSoul).await.unwrap();
    assert!(matches!(verdict, BankruptcyVerdict::Critical { .. }));

    if let BankruptcyVerdict::Critical { current_realm, demoted_to } = verdict {
        assert_eq!(current_realm, Realm::NascentSoul);
        assert_eq!(demoted_to, Realm::GoldenCore);
    }

    // execute_demotion 验证境界降级
    let new_realm = svc.execute_demotion(&id, &Realm::NascentSoul).await.unwrap();
    assert_eq!(new_realm, Realm::GoldenCore);
}

/// 场景 12：入不敷出判定 — 新手保护（炼气~金丹不触发）
#[tokio::test]
async fn test_bankruptcy_novice_protection() {
    let token = InMemoryTokenManager::new();
    let stone = InMemoryStoneManager::new();
    let svc = BankruptcyServiceImpl::new(token, stone);
    let id = AgentId::new();

    // 炼气期即使大量消耗也不触发
    let verdict = svc.assess(&id, &Realm::QiRefining).await.unwrap();
    assert_eq!(verdict, BankruptcyVerdict::Solvent);
}

/// 场景 13：入不敷出判定 — Warning（消耗偏高但未达临界）
#[tokio::test]
async fn test_bankruptcy_warning() {
    let token = InMemoryTokenManager::new();
    let stone = InMemoryStoneManager::new();
    let id = AgentId::new();

    // 通过公共 API 设置账户
    token.get_or_create_account(&id).await.unwrap();
    token.set_subsidy_active(&id, false).await.unwrap();
    stone.deposit(&id, CurrencyAmount::new(100)).await.unwrap();
    token.record_consumption(&id, CurrencyAmount::new(300)).await.unwrap();

    let svc = BankruptcyServiceImpl::with_ratios(token, stone, 2.0, 10.0);
    let verdict = svc.assess(&id, &Realm::NascentSoul).await.unwrap();
    assert!(matches!(verdict, BankruptcyVerdict::Warning { .. }));
}

/// 场景 14：demote_realm 映射验证
#[test]
fn test_demote_realm_mapping() {
    assert_eq!(demote_realm(&Realm::NascentSoul), Some(Realm::GoldenCore));
    assert_eq!(demote_realm(&Realm::SpiritSevering), Some(Realm::NascentSoul));
    assert_eq!(demote_realm(&Realm::VoidRefining), Some(Realm::SpiritSevering));
    assert_eq!(demote_realm(&Realm::ImmortalAscension), Some(Realm::VoidRefining));
    assert_eq!(demote_realm(&Realm::QiRefining), None);
    assert_eq!(demote_realm(&Realm::GoldenCore), None);
    assert_eq!(demote_realm(&Realm::Foundation), None);
}
