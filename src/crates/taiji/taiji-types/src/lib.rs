#![doc = "! taiji-types — LVPA 核心类型定义"]
//!
//! 本 crate 是 LVPA 类型系统的根节点，提供所有基础设施模块共享的基础类型。
//! 零外部运行时依赖，仅 serde/uuid/chrono/thiserror。
//!
//! # 设计原则
//!
//! - **零依赖外部运行时**：不依赖 tokio/async-trait 等运行时 crate
//! - **所有公开类型实现 Debug + Clone + PartialEq + Serialize + Deserialize**
//! - **零 unwrap/panic**：所有可能失败的操作返回 Result
//! - **与 BitFun 上游类型隔离**：不依赖 bitfun-core-types/bitfun-events
//!
//! # 参考源索引
//!
//! 各类型的参考外部源码见各模块文档头部注释。
//! 核心参考项目：agentscope (Apache 2.0)、react-xiuxian-game、gbrain (MIT)、BitFun (MIT)。

pub mod error;
pub mod agent;
pub mod realm;
pub mod credit;
pub mod permission;
pub mod harness;
pub mod message;
pub mod event;
pub mod card;
pub mod shared;
pub mod knowledge;
pub mod lvpa_ui;
pub mod economy;
pub mod workshop_dungeon;

#[cfg(test)]
mod tests {
    use crate::agent::*;
    use crate::card::*;
    use crate::credit::*;
    use crate::permission::*;
    use crate::realm::*;
    use crate::harness::*;
    use crate::knowledge::*;
    use crate::economy::*;
    use crate::workshop_dungeon::*;
    use crate::event::TaijiEvent;

    // ===== SpiritRoot =====

    #[test]
    fn test_spirit_root_serde_roundtrip() {
        for root in &[SpiritRoot::Metal, SpiritRoot::Wood, SpiritRoot::Water, SpiritRoot::Fire, SpiritRoot::Earth] {
            let json = serde_json::to_string(root).unwrap();
            let back: SpiritRoot = serde_json::from_str(&json).unwrap();
            assert_eq!(*root, back);
        }
    }

    #[test]
    fn test_spirit_root_display() {
        assert_eq!(format!("{}", SpiritRoot::Metal), "金");
        assert_eq!(format!("{}", SpiritRoot::Wood), "木");
        assert_eq!(format!("{}", SpiritRoot::Water), "水");
        assert_eq!(format!("{}", SpiritRoot::Fire), "火");
        assert_eq!(format!("{}", SpiritRoot::Earth), "土");
    }

    #[test]
    fn test_spirit_root_alias_gold() {
        // Metal 应能反序列化 "gold" 别名
        let json = "\"gold\"";
        let root: SpiritRoot = serde_json::from_str(json).unwrap();
        assert_eq!(root, SpiritRoot::Metal);
    }

    // ===== Realm =====

    #[test]
    fn test_realm_ordering() {
        assert!(Realm::QiRefining < Realm::Foundation);
        assert!(Realm::Foundation < Realm::GoldenCore);
        assert!(Realm::GoldenCore < Realm::NascentSoul);
        assert!(Realm::NascentSoul < Realm::SpiritSevering);
        assert!(Realm::SpiritSevering < Realm::VoidRefining);
        assert!(Realm::VoidRefining < Realm::ImmortalAscension);
    }

    #[test]
    fn test_realm_serde_roundtrip() {
        for realm in &[Realm::QiRefining, Realm::Foundation, Realm::GoldenCore,
                       Realm::NascentSoul, Realm::SpiritSevering, Realm::VoidRefining,
                       Realm::ImmortalAscension] {
            let json = serde_json::to_string(realm).unwrap();
            let back: Realm = serde_json::from_str(&json).unwrap();
            assert_eq!(*realm, back);
        }
    }

    // ===== AgentCredit =====

    #[test]
    fn test_agent_credit_default() {
        let credit = AgentCredit::default();
        assert!((credit.score - 50.0).abs() < 1e-9);
        assert!((credit.success_rate - 1.0).abs() < 1e-9);
        assert_eq!(credit.daoxin, 50);
    }

    #[test]
    fn test_agent_credit_serde_roundtrip() {
        let credit = AgentCredit {
            score: 85.0,
            contribution: 1000.0,
            success_rate: 0.92,
            daoxin: 75,
            review_pass_rate: 0.88,
            rework_rate: 0.05,
            kpi_bonus: 200.0,
        };
        let json = serde_json::to_string(&credit).unwrap();
        let back: AgentCredit = serde_json::from_str(&json).unwrap();
        assert!((back.score - 85.0).abs() < 1e-9);
        assert_eq!(back.daoxin, 75);
    }

    // ===== AgentState / AgentStatus =====

    #[test]
    fn test_agent_status_serde() {
        for status in &[AgentStatus::Idle, AgentStatus::Running, AgentStatus::Sleeping, AgentStatus::Destroyed] {
            let json = serde_json::to_string(status).unwrap();
            let back: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*status, back);
        }
    }

    #[test]
    fn test_agent_state_serde_roundtrip() {
        let state = AgentState {
            session_id: "test-session".into(),
            status: AgentStatus::Running,
            context: vec![],
            summary: Some("test summary".into()),
            cur_iter: 5,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "test-session");
        assert_eq!(back.status, AgentStatus::Running);
        assert_eq!(back.summary, Some("test summary".into()));
        assert_eq!(back.cur_iter, 5);
    }

    // ===== AgentConfig =====

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.model_id, "default");
        assert_eq!(config.max_iters, 50);
    }

    // ===== AuthType =====

    #[test]
    fn test_auth_type_serde() {
        for auth in &[AuthType::ApiKey, AuthType::Jwt, AuthType::Nostr] {
            let json = serde_json::to_string(auth).unwrap();
            let back: AuthType = serde_json::from_str(&json).unwrap();
            assert_eq!(*auth, back);
        }
    }

    // ===== AgentId =====

    #[test]
    fn test_agent_id_new() {
        let id = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id, id2);
    }

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::new();
        let s = format!("{}", id);
        assert_eq!(s.len(), 36); // UUID v7 format
    }

    #[test]
    fn test_agent_id_parse() {
        let id = AgentId::new();
        let s = id.to_string();
        let parsed = AgentId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    // ===== GateCommand =====

    #[test]
    fn test_gate_command_serde_allow() {
        let cmd = GateCommand::Allow;
        let json = serde_json::to_string(&cmd).unwrap();
        let back: GateCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(back, GateCommand::Allow);
    }

    #[test]
    fn test_gate_command_serde_deny() {
        let cmd = GateCommand::Deny;
        let json = serde_json::to_string(&cmd).unwrap();
        let back: GateCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(back, GateCommand::Deny);
    }

    #[test]
    fn test_gate_command_serde_ask() {
        let cmd = GateCommand::Ask {
            suggested_rules: vec![PermissionRule {
                tool_name: "read".into(),
                rule_content: Some("*.rs".into()),
                behavior: PermissionBehavior::Allow,
                source: "test".into(),
            }],
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: GateCommand = serde_json::from_str(&json).unwrap();
        match back {
            GateCommand::Ask { suggested_rules } => {
                assert_eq!(suggested_rules.len(), 1);
                assert_eq!(suggested_rules[0].tool_name, "read");
            }
            _ => panic!("expected Ask variant"),
        }
    }

    // ===== PermissionMode =====

    #[test]
    fn test_permission_mode_serde() {
        for mode in &[PermissionMode::Default, PermissionMode::AcceptEdits,
                      PermissionMode::Explore, PermissionMode::Bypass, PermissionMode::DontAsk] {
            let json = serde_json::to_string(mode).unwrap();
            let back: PermissionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, back);
        }
    }

    // ===== HarnessConfig =====

    #[test]
    fn test_harness_config_default() {
        let config = HarnessConfig::default();
        assert_eq!(config.guard_level, GuardLevel::Default);
        assert!(config.rules.is_empty());
    }

    // ===== GuardLevel alias =====

    #[test]
    fn test_guard_level_alias() {
        let level: GuardLevel = PermissionMode::Explore;
        assert_eq!(level, PermissionMode::Explore);
    }

    // ===== Knowledge / gbrain =====

    #[test]
    fn test_chunk_strategy_default() {
        let strategy = ChunkStrategy::default();
        match strategy {
            ChunkStrategy::Fixed { chunk_size, overlap } => {
                assert_eq!(chunk_size, 512);
                assert_eq!(overlap, 64);
            }
            _ => panic!("default should be Fixed"),
        }
    }

    #[test]
    fn test_chunk_strategy_serde() {
        let strategies = [
            ChunkStrategy::Fixed { chunk_size: 256, overlap: 32 },
            ChunkStrategy::Paragraph,
            ChunkStrategy::Sentence,
        ];
        for s in &strategies {
            let json = serde_json::to_string(s).unwrap();
            let back: ChunkStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn test_search_opts_default() {
        let opts = SearchOpts::default();
        assert_eq!(opts.top_k, 10);
        assert_eq!(opts.min_score, 0.0);
        assert!(opts.use_expansion);
        assert!(opts.use_keyword);
        assert!(!opts.use_graph);
        assert!(opts.source_filter.is_none());
    }

    #[test]
    fn test_search_opts_serde() {
        let opts = SearchOpts {
            top_k: 5,
            min_score: 0.5,
            source_filter: Some(vec!["user:alice".into()]),
            use_expansion: false,
            use_keyword: true,
            use_graph: true,
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: SearchOpts = serde_json::from_str(&json).unwrap();
        assert_eq!(back.top_k, 5);
        assert!((back.min_score - 0.5).abs() < 1e-9);
        assert_eq!(back.source_filter, Some(vec!["user:alice".into()]));
        assert!(!back.use_expansion);
    }

    #[test]
    fn test_gbrain_config_default() {
        let config = GBrainConfig::default();
        assert_eq!(config.engine, "pglite");
        assert_eq!(config.embedding_dimensions, 384);
        assert!(config.database_url.is_none());
    }

    #[test]
    fn test_gbrain_config_serde() {
        let config = GBrainConfig {
            engine: "postgres".into(),
            database_url: Some("postgres://localhost:5432/gbrain".into()),
            database_path: None,
            embedding_model: Some("intfloat/e5-small-v2".into()),
            embedding_dimensions: 384,
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: GBrainConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.engine, "postgres");
        assert_eq!(back.database_url, Some("postgres://localhost:5432/gbrain".into()));
        assert_eq!(back.embedding_model, Some("intfloat/e5-small-v2".into()));
    }

    #[test]
    fn test_page_serde_roundtrip() {
        let page = Page {
            id: "lvpa-architecture".into(),
            title: "LVPA 架构总纲".into(),
            content: "# 架构总纲\n\n三层架构...".into(),
            source_id: "system".into(),
            tags: vec!["architecture".into(), "lvpa".into()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            metadata: serde_json::json!({"version": "2.4"}),
        };
        let json = serde_json::to_string(&page).unwrap();
        let back: Page = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "lvpa-architecture");
        assert_eq!(back.tags.len(), 2);
        assert_eq!(back.metadata["version"], "2.4");
    }

    #[test]
    fn test_chunk_serde_roundtrip() {
        let chunk = Chunk {
            id: "chunk:001".into(),
            page_id: "lvpa-architecture".into(),
            seq: 1,
            text: "三层架构定义...".into(),
            embedding: Some(vec![0.1, 0.2, 0.3]),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: Chunk = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "chunk:001");
        assert_eq!(back.seq, 1);
        assert!(back.embedding.is_some());
    }

    #[test]
    fn test_search_result_serde() {
        let result = SearchResult {
            chunk: Chunk {
                id: "chunk:001".into(),
                page_id: "lvpa-architecture".into(),
                seq: 1,
                text: "三层架构...".into(),
                embedding: None,
            },
            score: 0.95,
            source_id: "system".into(),
            page_title: "LVPA 架构总纲".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert!((back.score - 0.95).abs() < 1e-9);
        assert_eq!(back.page_title, "LVPA 架构总纲");
    }

    #[test]
    fn test_page_input_serde() {
        let input = PageInput {
            title: "新页面".into(),
            content: "页面正文".into(),
            source_id: Some("user:alice".into()),
            tags: vec!["draft".into()],
            metadata: serde_json::Value::Null,
        };
        let json = serde_json::to_string(&input).unwrap();
        let back: PageInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title, "新页面");
        assert_eq!(back.source_id, Some("user:alice".into()));
        assert!(back.tags.contains(&"draft".into()));
    }

    // ===== SpiritCard 类型检查 =====

    #[test]
    fn test_spirit_card_uses_tier_enum() {
        let card = SpiritCard {
            card_id: uuid::Uuid::new_v4(),
            name: "测试本命魂卡".into(),
            tier: Tier::Gold,       // 原 u8，现 Tier 枚举
            description: "测试".into(),
            slot_cost: SlotCost(2), // 原 u8，现 SlotCost
            spirit_root: SpiritRoot::Metal,
        };
        assert_eq!(card.tier, Tier::Gold);
        assert_eq!(card.slot_cost.as_u8(), 2);
    }

    #[test]
    fn test_spirit_card_tier_serde() {
        let card = SpiritCard {
            card_id: uuid::Uuid::new_v4(),
            name: "九转金身诀".into(),
            tier: Tier::Jade,
            description: "顶级功法".into(),
            slot_cost: SlotCost(3),
            spirit_root: SpiritRoot::Fire,
        };
        let json = serde_json::to_string(&card).unwrap();
        let back: SpiritCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tier, Tier::Jade);
        assert_eq!(back.slot_cost.as_u8(), 3);
        assert_eq!(back.spirit_root, SpiritRoot::Fire);
    }

    // ===== Title / 荣誉称号 =====

    #[test]
    fn test_title_serde() {
        let title = Title {
            name: "百战真君".into(),
            effect: "权限等级+1".into(),
            bonus_value: 1.0,
            bonus_target: "permission_level".into(),
            condition: "评分≥90".into(),
            condition_type: "score".into(),
            condition_threshold: 90.0,
        };
        let json = serde_json::to_string(&title).unwrap();
        let back: Title = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "百战真君");
        assert_eq!(back.bonus_target, "permission_level");
    }

    #[test]
    fn test_title_config_default() {
        let cfg = TitleConfig::default();
        assert_eq!(cfg.max_active_titles, 3);
        assert!(cfg.bonus_caps.contains_key("permission_level"));
    }

    // ===== Economy — CurrencyAmount =====

    #[test]
    fn test_currency_amount_new() {
        let ca = CurrencyAmount::new(100);
        assert_eq!(ca.as_u64(), 100);
    }

    #[test]
    fn test_currency_amount_default() {
        let ca = CurrencyAmount::default();
        assert!(ca.is_zero());
    }

    #[test]
    fn test_currency_amount_from_u64() {
        let ca: CurrencyAmount = 42.into();
        assert_eq!(ca.as_u64(), 42);
    }

    #[test]
    fn test_currency_amount_saturating_ops() {
        let a = CurrencyAmount::new(100);
        let b = CurrencyAmount::new(50);
        assert_eq!(a.saturating_add(b).as_u64(), 150);
        assert_eq!(a.saturating_sub(b).as_u64(), 50);
        assert!(CurrencyAmount::new(10).saturating_sub(CurrencyAmount::new(20)).is_zero());
    }

    #[test]
    fn test_currency_amount_serde_roundtrip() {
        let ca = CurrencyAmount::new(999);
        let json = serde_json::to_string(&ca).unwrap();
        let back: CurrencyAmount = serde_json::from_str(&json).unwrap();
        assert_eq!(back.as_u64(), 999);
    }

    // ===== Economy — CurrencyType =====

    #[test]
    fn test_currency_type_serde() {
        for ct in &[CurrencyType::Token, CurrencyType::Stone] {
            let json = serde_json::to_string(ct).unwrap();
            let back: CurrencyType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ct, back);
        }
    }

    // ===== Economy — TransactionType =====

    #[test]
    fn test_transaction_type_serde() {
        for tt in &[TransactionType::Deposit, TransactionType::Withdrawal, TransactionType::Transfer,
                     TransactionType::Exchange, TransactionType::Reward, TransactionType::Royalty,
                     TransactionType::CardCopy, TransactionType::SlotUpgrade] {
            let json = serde_json::to_string(tt).unwrap();
            let back: TransactionType = serde_json::from_str(&json).unwrap();
            assert_eq!(*tt, back);
        }
    }

    // ===== Economy — TransactionRecord =====

    #[test]
    fn test_transaction_record_serde() {
        let tx = TransactionRecord {
            tx_id: "tx-001".into(),
            agent_id: AgentId::new(),
            counterparty: None,
            amount: CurrencyAmount::new(500),
            currency_type: CurrencyType::Stone,
            tx_type: TransactionType::Deposit,
            timestamp: chrono::Utc::now(),
            description: "充值".into(),
            metadata: None,
        };
        let json = serde_json::to_string(&tx).unwrap();
        let back: TransactionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.amount.as_u64(), 500);
        assert_eq!(back.description, "充值");
    }

    // ===== Economy — TokenAccount =====

    #[test]
    fn test_token_account_serde() {
        let acc = TokenAccount {
            agent_id: AgentId::new(),
            total_consumed: CurrencyAmount::new(1000),
            total_subsidized: CurrencyAmount::new(500),
            subsidy_active: true,
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&acc).unwrap();
        let back: TokenAccount = serde_json::from_str(&json).unwrap();
        assert!(back.subsidy_active);
        assert_eq!(back.total_consumed.as_u64(), 1000);
    }

    // ===== Economy — StoneAccount =====

    #[test]
    fn test_stone_account_serde() {
        let acc = StoneAccount {
            agent_id: AgentId::new(),
            balance: CurrencyAmount::new(5000),
            lifetime_earnings: CurrencyAmount::new(10000),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&acc).unwrap();
        let back: StoneAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(back.balance.as_u64(), 5000);
    }

    // ===== Economy — TreasureItem =====

    #[test]
    fn test_treasure_item_rebirth_token_serde() {
        let item = TreasureItem::RebirthToken;
        let json = serde_json::to_string(&item).unwrap();
        let back: TreasureItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TreasureItem::RebirthToken);
    }

    #[test]
    fn test_treasure_item_spirit_stones_serde() {
        let item = TreasureItem::SpiritStones(CurrencyAmount::new(500));
        let json = serde_json::to_string(&item).unwrap();
        let back: TreasureItem = serde_json::from_str(&json).unwrap();
        match back {
            TreasureItem::SpiritStones(amount) => assert_eq!(amount.as_u64(), 500),
            _ => panic!("expected SpiritStones"),
        }
    }

    #[test]
    fn test_treasure_item_stone_equivalent() {
        assert_eq!(TreasureItem::RebirthToken.stone_equivalent().as_u64(), 1000);
        assert_eq!(
            TreasureItem::SpiritStones(CurrencyAmount::new(300)).stone_equivalent().as_u64(),
            300
        );
    }

    #[test]
    fn test_treasure_item_is_stone_cost() {
        assert!(!TreasureItem::RebirthToken.is_stone_cost());
        assert!(TreasureItem::SpiritStones(CurrencyAmount::new(100)).is_stone_cost());
    }

    // ===== Workshop / Dungeon =====

    #[test]
    fn test_workshop_id_serde() {
        let id = WorkshopId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: WorkshopId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_workshop_type_serde() {
        for wt in WorkshopType::all() {
            let json = serde_json::to_string(&wt).unwrap();
            let back: WorkshopType = serde_json::from_str(&json).unwrap();
            assert_eq!(wt, back);
        }
    }

    #[test]
    fn test_workshop_status_serde() {
        for st in &[WorkshopStatus::Active, WorkshopStatus::Paused, WorkshopStatus::Closed] {
            let json = serde_json::to_string(st).unwrap();
            let back: WorkshopStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*st, back);
        }
    }

    #[test]
    fn test_dungeon_id_serde() {
        let id = DungeonId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: DungeonId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_dungeon_status_serde() {
        for st in &[DungeonStatus::Recruiting, DungeonStatus::Ready, DungeonStatus::InProgress,
                     DungeonStatus::Completed, DungeonStatus::Disbanded] {
            let json = serde_json::to_string(st).unwrap();
            let back: DungeonStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*st, back);
        }
    }

    #[test]
    fn test_workshop_output_serde() {
        let output = WorkshopOutput {
            output_id: "out-001".into(),
            workshop_id: WorkshopId::new(),
            node_name: "需求分析".into(),
            produced_by: AgentId::new(),
            data: serde_json::json!({"result": "ok"}),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&output).unwrap();
        let back: WorkshopOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.output_id, "out-001");
        assert_eq!(back.node_name, "需求分析");
    }

    #[test]
    fn test_dungeon_result_serde() {
        let result = DungeonResult {
            dungeon_id: DungeonId::new(),
            member_results: vec![MemberResult {
                agent_id: AgentId::new(),
                contribution_share: 1.0,
                score_delta: 10.0,
                spirit_stones_earned: 100,
            }],
            completed_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: DungeonResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.member_results.len(), 1);
        assert!((back.member_results[0].contribution_share - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_workshop_events_serde() {
        // WorkshopJoined
        let evt = TaijiEvent::WorkshopJoined {
            workshop_id: WorkshopId::new(),
            workshop_type: WorkshopType::Tianji,
            agent_id: AgentId::new(),
            spirit_root: SpiritRoot::Metal,
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: TaijiEvent = serde_json::from_str(&json).unwrap();
        match back {
            TaijiEvent::WorkshopJoined { workshop_type, .. } => {
                assert_eq!(workshop_type, WorkshopType::Tianji);
            }
            _ => panic!("expected WorkshopJoined"),
        }
    }

    #[test]
    fn test_dungeon_events_serde() {
        let dungeon_id = DungeonId::new();
        let evt = TaijiEvent::DungeonPublished {
            dungeon_id,
            name: "测试副本".into(),
            publisher_id: AgentId::new(),
            min_members: 2,
            max_members: 4,
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: TaijiEvent = serde_json::from_str(&json).unwrap();
        match back {
            TaijiEvent::DungeonPublished { name, min_members, .. } => {
                assert_eq!(name, "测试副本");
                assert_eq!(min_members, 2);
            }
            _ => panic!("expected DungeonPublished"),
        }
    }
}
