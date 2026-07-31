//! taiji-types 核心类型单元测试。
//!
//! 验证所有公开类型的：
//! - Debug + Clone + PartialEq 派生
//! - Serialize + Deserialize 派生
//! - 默认值正确性
//! - 构造/解析函数

use chrono::Utc;
use taiji_types::agent::{AgentId, SpiritCard, SpiritRoot, Title};
use taiji_types::card::{CardId, SlotCost, SlotType, Tier};
use taiji_types::credit::AgentCredit;
use taiji_types::error::{ErrorKind, LvpaError};
use taiji_types::event::{TaijiEvent, TransportEvent};
use taiji_types::message::{
    ContentBlock, HintBlock, Message, DataBlock, Priority, TextBlock, ThinkingBlock,
    ToolCallBlock, ToolResultBlock,
};
use taiji_types::permission::{
    Action, PermissionBehavior, PermissionDecision, PermissionMode, PermissionRule, ResourceQuota,
};
use taiji_types::realm::Realm;
use taiji_types::shared::{Metadata, Timestamp, Version};
use uuid::Uuid;

// ── AgentId ──

#[test]
fn test_agent_id_round_trip() {
    let id = AgentId::new();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: AgentId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_agent_id_parse() {
    let id = AgentId::new();
    let s = id.to_string();
    let parsed = AgentId::parse(&s).unwrap();
    assert_eq!(id, parsed);
}

// ── SpiritRoot ──

#[test]
fn test_spirit_root_round_trip() {
    let roots = [
        SpiritRoot::Metal,
        SpiritRoot::Wood,
        SpiritRoot::Water,
        SpiritRoot::Fire,
        SpiritRoot::Earth,
    ];
    for root in &roots {
        let json = serde_json::to_string(root).unwrap();
        let parsed: SpiritRoot = serde_json::from_str(&json).unwrap();
        assert_eq!(*root, parsed);
    }
}

// ── SpiritCard ──

#[test]
fn test_spirit_card_construction() {
    let card = SpiritCard {
        card_id: Uuid::now_v7(),
        name: "天机算子".into(),
        tier: Tier::Silver,
        description: "洞悉天机，算无遗策".into(),
        slot_cost: SlotCost(2),
        spirit_root: SpiritRoot::Fire,
    };
    assert_eq!(card.name, "天机算子");
    assert_eq!(card.tier, Tier::Silver);
    assert_eq!(card.description, "洞悉天机，算无遗策");
    assert_eq!(card.slot_cost, SlotCost(2));
    assert_eq!(card.spirit_root, SpiritRoot::Fire);
}

// ── Realm ──

#[test]
fn test_realm_order() {
    assert!(Realm::QiRefining < Realm::Foundation);
    assert!(Realm::Foundation < Realm::GoldenCore);
    assert!(Realm::GoldenCore < Realm::NascentSoul);
    assert!(Realm::NascentSoul < Realm::SpiritSevering);
    assert!(Realm::SpiritSevering < Realm::VoidRefining);
    assert!(Realm::VoidRefining < Realm::ImmortalAscension);
}

#[test]
fn test_realm_display_name() {
    assert_eq!(Realm::QiRefining.display_name(), "炼气期");
    assert_eq!(Realm::Foundation.display_name(), "筑基期");
    assert_eq!(Realm::GoldenCore.display_name(), "金丹期");
    assert_eq!(Realm::NascentSoul.display_name(), "元婴期");
    assert_eq!(Realm::SpiritSevering.display_name(), "化神期");
    assert_eq!(Realm::VoidRefining.display_name(), "炼虚期");
    assert_eq!(Realm::ImmortalAscension.display_name(), "渡劫飞升");
}

// ── AgentCredit ──

#[test]
fn test_credit_defaults() {
    let credit = AgentCredit::default();
    assert_eq!(credit.score, 50.0);
    assert_eq!(credit.success_rate, 1.0);
    assert_eq!(credit.rework_rate, 0.0);
}

#[test]
fn test_credit_round_trip() {
    let credit = AgentCredit {
        score: 85.5,
        contribution: 500.0,
        success_rate: 0.92,
        daoxin: 70,
        review_pass_rate: 0.88,
        rework_rate: 0.05,
        kpi_bonus: 10.0,
    };
    let json = serde_json::to_string(&credit).unwrap();
    let parsed: AgentCredit = serde_json::from_str(&json).unwrap();
    assert_eq!(credit, parsed);
}

// ── ResourceQuota ──

#[test]
fn test_resource_quota_defaults() {
    let quota = ResourceQuota::default();
    assert_eq!(quota.max_rps, 100);
    assert_eq!(quota.max_concurrency, 10);
    assert_eq!(quota.token_budget, 1_000_000);
}

#[test]
fn test_resource_quota_round_trip() {
    let quota = ResourceQuota {
        max_rps: 500,
        max_concurrency: 20,
        token_budget: 5_000_000,
    };
    let json = serde_json::to_string(&quota).unwrap();
    let parsed: ResourceQuota = serde_json::from_str(&json).unwrap();
    assert_eq!(quota, parsed);
    assert_eq!(parsed.max_rps, 500);
    assert_eq!(parsed.token_budget, 5_000_000);
}

// ── Action ──

#[test]
fn test_action_round_trip() {
    let actions = [
        Action::Read("market_data".into()),
        Action::Write("ledger".into()),
        Action::Execute("strategy".into()),
        Action::Admin("system".into()),
    ];
    for action in &actions {
        let json = serde_json::to_string(action).unwrap();
        let parsed: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(*action, parsed);
    }
}

// ── PermissionMode ──

#[test]
fn test_permission_mode_round_trip() {
    let modes = [
        PermissionMode::Default,
        PermissionMode::AcceptEdits,
        PermissionMode::Explore,
        PermissionMode::Bypass,
        PermissionMode::DontAsk,
    ];
    for mode in &modes {
        let json = serde_json::to_string(mode).unwrap();
        let parsed: PermissionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(*mode, parsed);
    }
}

// ── PermissionBehavior ──

#[test]
fn test_permission_behavior_round_trip() {
    let behaviors = [
        PermissionBehavior::Allow,
        PermissionBehavior::Deny,
        PermissionBehavior::Ask,
        PermissionBehavior::Passthrough,
    ];
    for b in &behaviors {
        let json = serde_json::to_string(b).unwrap();
        let parsed: PermissionBehavior = serde_json::from_str(&json).unwrap();
        assert_eq!(*b, parsed);
    }
}

// ── PermissionRule ──

#[test]
fn test_permission_rule_construction() {
    let rule = PermissionRule {
        tool_name: "Bash".into(),
        rule_content: Some("npm install".into()),
        behavior: PermissionBehavior::Allow,
        source: "userSettings".into(),
    };
    assert_eq!(rule.tool_name, "Bash");
    assert_eq!(rule.behavior, PermissionBehavior::Allow);
}

#[test]
fn test_permission_rule_round_trip() {
    let rule = PermissionRule {
        tool_name: "Write".into(),
        rule_content: Some("src/**".into()),
        behavior: PermissionBehavior::Deny,
        source: "projectSettings".into(),
    };
    let json = serde_json::to_string(&rule).unwrap();
    let parsed: PermissionRule = serde_json::from_str(&json).unwrap();
    assert_eq!(rule, parsed);
}

// ── PermissionDecision ──

#[test]
fn test_permission_decision_construction() {
    let decision = PermissionDecision {
        behavior: PermissionBehavior::Deny,
        message: "Blocked by safety rule".into(),
        decision_reason: Some("matches deny pattern".into()),
        bypass_immune: false,
    };
    assert_eq!(decision.behavior, PermissionBehavior::Deny);
    assert_eq!(decision.message, "Blocked by safety rule");
}

// ── ContentBlock ──

#[test]
fn test_content_block_text() {
    let block = ContentBlock::Text(TextBlock {
        text: "hello".into(),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

#[test]
fn test_content_block_thinking() {
    let block = ContentBlock::Thinking(ThinkingBlock {
        thinking: "processing...".into(),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

#[test]
fn test_content_block_hint() {
    let block = ContentBlock::Hint(HintBlock {
        hint: "use caution".into(),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

#[test]
fn test_content_block_data() {
    let block = ContentBlock::Data(DataBlock {
        mime_type: "image/png".into(),
        data: "base64data".into(),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

#[test]
fn test_content_block_tool_call() {
    let block = ContentBlock::ToolCall(ToolCallBlock {
        tool_name: "ping".into(),
        args: serde_json::json!({"host": "localhost"}),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

#[test]
fn test_content_block_tool_result() {
    let block = ContentBlock::ToolResult(ToolResultBlock {
        tool_name: "ping".into(),
        success: true,
        output: serde_json::json!({"latency_ms": 5}),
    });
    let json = serde_json::to_string(&block).unwrap();
    let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, parsed);
}

// ── Message ──

#[test]
fn test_message_construction() {
    let msg = Message {
        name: "test_agent".into(),
        content: vec![ContentBlock::Text(TextBlock {
            text: "hello".into(),
        })],
        role: "user".into(),
        id: Uuid::now_v7(),
        topic: "test.topic".into(),
        priority: Priority::Normal,
        metadata: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    assert_eq!(msg.topic, "test.topic");
    assert_eq!(msg.role, "user");
    assert_eq!(msg.name, "test_agent");
    assert_eq!(msg.priority, Priority::Normal);
}

#[test]
fn test_message_round_trip() {
    let msg = Message {
        name: "assistant".into(),
        content: vec![
            ContentBlock::Text(TextBlock {
                text: "answer".into(),
            }),
            ContentBlock::ToolCall(ToolCallBlock {
                tool_name: "search".into(),
                args: serde_json::json!({"q": "test"}),
            }),
        ],
        role: "assistant".into(),
        id: Uuid::now_v7(),
        topic: "default".into(),
        priority: Priority::High,
        metadata: serde_json::json!({"source": "llm"}),
        created_at: Utc::now(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg.name, parsed.name);
    assert_eq!(msg.content.len(), parsed.content.len());
    assert_eq!(msg.role, parsed.role);
}

// ── TransportEvent ──

#[test]
fn test_transport_event_agent_updated() {
    let event = TransportEvent::AgentUpdated {
        agent_id: AgentId::new(),
        realm: Realm::GoldenCore,
        credit: AgentCredit::default(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: TransportEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn test_transport_event_notification() {
    let event = TransportEvent::Notification {
        level: "info".into(),
        message: "task completed".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: TransportEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

// ── TaijiEvent ──

#[test]
fn test_taiji_event_agent_created() {
    let event = TaijiEvent::AgentCreated {
        agent_id: AgentId::new(),
        name: "散修甲".into(),
        realm: Realm::QiRefining,
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: TaijiEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn test_taiji_event_realm_upgraded() {
    let event = TaijiEvent::RealmUpgraded {
        agent_id: AgentId::new(),
        from: Realm::QiRefining,
        to: Realm::Foundation,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: TaijiEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn test_taiji_event_task_published() {
    let event = TaijiEvent::TaskPublished {
        task_id: Uuid::now_v7(),
        publisher_id: AgentId::new(),
        task_type: "analysis".into(),
        priority: 5,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: TaijiEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

// ── Error ──

#[test]
fn test_error_display() {
    let cases = [
        (LvpaError::Unimplemented("taiji-types".into()), "not implemented: taiji-types"),
        (LvpaError::Config("missing field".into()), "configuration error: missing field"),
        (LvpaError::Serialization("parse error".into()), "serialization error: parse error"),
        (LvpaError::PermissionDenied("no access".into()), "permission denied: no access"),
        (LvpaError::NotFound("agent".into()), "not found: agent"),
        (LvpaError::Internal("oops".into()), "internal error: oops"),
    ];
    for (err, expected) in &cases {
        assert_eq!(err.to_string(), *expected);
    }
}

#[test]
fn test_error_serde_round_trip() {
    let cases = [
        LvpaError::Unimplemented("test".into()),
        LvpaError::Config("config".into()),
        LvpaError::Serialization("ser".into()),
        LvpaError::PermissionDenied("perm".into()),
        LvpaError::NotFound("not_found".into()),
        LvpaError::Internal("internal".into()),
    ];
    for err in &cases {
        let json = serde_json::to_string(err).unwrap();
        let parsed: LvpaError = serde_json::from_str(&json).unwrap();
        assert_eq!(*err, parsed);
    }
}

// ── Priority ──

#[test]
fn test_priority_ordering() {
    assert!(Priority::Low < Priority::Normal);
    assert!(Priority::Normal < Priority::High);
    assert!(Priority::High < Priority::Critical);
}

// ── Card ──

#[test]
fn test_card_id_newtype() {
    let id = CardId::new(42);
    assert_eq!(id.as_u64(), 42);
}

#[test]
fn test_card_id_round_trip() {
    let id = CardId::new(99);
    let json = serde_json::to_string(&id).unwrap();
    let parsed: CardId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_tier_order() {
    assert!(Tier::Blackiron < Tier::Bronze);
    assert!(Tier::Bronze < Tier::Silver);
    assert!(Tier::Silver < Tier::Gold);
    assert!(Tier::Gold < Tier::Jade);
    assert!(Tier::Jade < Tier::Divine);
}

#[test]
fn test_tier_round_trip() {
    let tiers = [
        Tier::Blackiron,
        Tier::Bronze,
        Tier::Silver,
        Tier::Gold,
        Tier::Jade,
        Tier::Divine,
    ];
    for tier in &tiers {
        let json = serde_json::to_string(tier).unwrap();
        let parsed: Tier = serde_json::from_str(&json).unwrap();
        assert_eq!(*tier, parsed);
    }
}

#[test]
fn test_slot_type_round_trip() {
    let slots = [
        SlotType::Main,
        SlotType::Sub,
        SlotType::Passive,
        SlotType::Consumable,
    ];
    for slot in &slots {
        let json = serde_json::to_string(slot).unwrap();
        let parsed: SlotType = serde_json::from_str(&json).unwrap();
        assert_eq!(*slot, parsed);
    }
}

#[test]
fn test_slot_cost_clamp() {
    let cost = SlotCost::new(5);
    assert_eq!(cost.as_u8(), 5);
    let clamped = SlotCost::new(10);
    assert_eq!(clamped.as_u8(), 5); // clamped to max 5
}

#[test]
fn test_slot_cost_default() {
    let cost = SlotCost::default();
    assert_eq!(cost.as_u8(), 1);
}

// ── Title ──

#[test]
fn test_title_construction() {
    let title = Title {
        name: "交易圣手".into(),
        effect: "交易收益率 +5%".into(),
        bonus_value: 0.05,
        bonus_target: "trading_yield".into(),
        condition: "连续50笔盈利".into(),
        condition_type: "count".into(),
        condition_threshold: 50.0,
    };
    assert_eq!(title.name, "交易圣手");
    assert!((title.bonus_value - 0.05).abs() < 1e-9);
    assert_eq!(title.bonus_target, "trading_yield");
}

#[test]
fn test_title_round_trip() {
    let title = Title {
        name: "分析大师".into(),
        effect: "分析速度 +10%".into(),
        bonus_value: 0.10,
        bonus_target: "analysis_speed".into(),
        condition: "连续7天活跃".into(),
        condition_type: "count".into(),
        condition_threshold: 7.0,
    };
    let json = serde_json::to_string(&title).unwrap();
    let parsed: Title = serde_json::from_str(&json).unwrap();
    assert_eq!(title, parsed);
}

// ── ErrorKind ──

#[test]
fn test_error_kind_round_trip() {
    let kinds = [
        ErrorKind::Config,
        ErrorKind::Permission,
        ErrorKind::Serialization,
        ErrorKind::NotFound,
        ErrorKind::Internal,
        ErrorKind::Unimplemented,
    ];
    for kind in &kinds {
        let json = serde_json::to_string(kind).unwrap();
        let parsed: ErrorKind = serde_json::from_str(&json).unwrap();
        assert_eq!(*kind, parsed);
    }
}

// ── Shared type aliases ──

#[test]
fn test_timestamp_round_trip() {
    let ts: Timestamp = Utc::now();
    let json = serde_json::to_string(&ts).unwrap();
    let parsed: Timestamp = serde_json::from_str(&json).unwrap();
    assert_eq!(ts, parsed);
}

#[test]
fn test_version_round_trip() {
    let ver: Version = semver::Version::new(1, 2, 3);
    let json = serde_json::to_string(&ver).unwrap();
    let parsed: Version = serde_json::from_str(&json).unwrap();
    assert_eq!(ver, parsed);
}

#[test]
fn test_metadata_round_trip() {
    let meta: Metadata = serde_json::json!({"key": "value", "count": 42});
    let json = serde_json::to_string(&meta).unwrap();
    let parsed: Metadata = serde_json::from_str(&json).unwrap();
    assert_eq!(meta, parsed);
}
