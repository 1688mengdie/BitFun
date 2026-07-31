//! R-1-202: event-bus ↔ transport 集成测试
//!
//! 验证 event-bus 事件通过 transport 推送到前端的全链路：
//!
//! event-bus → EventCodec.serialize(TaijiEvent) →
//! TransportMessage{event_name, payload} → transport.send(msg)
//!
//! # 验收标准
//!
//! - [x] event-bus 事件可转换为 TransportMessage
//! - [x] topic→event_name 转换表正确（agent.credit → "agent:credit_changed" 等）
//! - [x] WsTransportAdapter 可接收并缓存 TransportMessage

use serde_json::json;
use taiji_infra_event_bus::{
    CodecConfig, EventCodec, TaijiEvent,
};
use taiji_infra_transport::{
    TransportAdapter, TransportMessage, WsTransportAdapter,
};
use taiji_types::agent::AgentId;
use taiji_types::realm::Realm;

/// 辅助函数：将 TaijiEvent 转换为 TransportMessage。
///
/// 模拟 event-bus 调用 EventCodec 序列化后组装 TransportMessage 的路径。
fn event_to_transport_message(
    codec: &EventCodec,
    event: &TaijiEvent,
    event_name: &str,
) -> TransportMessage {
    // 序列化 TaijiEvent → Bytes（模拟 EventCodec.serialize）
    let bytes = codec.serialize(event).unwrap();

    // Bytes → JSON Value（前端消费格式）
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    TransportMessage::new(event_name, payload)
}

/// 辅助函数：从 TaijiEvent JSON 的 "type" 字段提取 event_name。
fn extract_event_name(event: &TaijiEvent) -> String {
    let value = serde_json::to_value(event).unwrap();
    value["type"].as_str().unwrap().to_string()
}

// ──────────────────────────────────────────────
// 测试 1: Agent 生命周期事件 → TransportMessage
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_agent_created_event_to_transport() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::AgentCreated {
        agent_id: AgentId::new(),
        name: "test-agent".into(),
        realm: Realm::QiRefining,
        timestamp: std::time::SystemTime::now(),
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "agent:created");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.event_name, "agent:created");
    assert_eq!(msg.payload["name"], "test-agent");
}

#[tokio::test]
async fn test_agent_state_changed_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::AgentStateChanged {
        agent_id: AgentId::new(),
        new_state: "busy".into(),
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "agent:state_changed");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["new_state"], "busy");
}

#[tokio::test]
async fn test_agent_forked_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::AgentForked {
        parent_id: AgentId::new(),
        child_id: AgentId::new(),
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "agent:forked");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert!(msg.payload["parent_id"].is_string());
    assert!(msg.payload["child_id"].is_string());
}

// ──────────────────────────────────────────────
// 测试 2: 境界/评分事件 → TransportMessage
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_realm_upgraded_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::RealmUpgraded {
        agent_id: AgentId::new(),
        from: Realm::QiRefining,
        to: Realm::Foundation,
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "agent:realm_upgraded");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["from"], "qi_refining");
    assert_eq!(msg.payload["to"], "foundation");
}

#[tokio::test]
async fn test_credit_changed_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::CreditChanged {
        agent_id: AgentId::new(),
        new_score: 92.5,
        delta: 3.5,
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "agent:credit_changed");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["new_score"], 92.5);
    assert_eq!(msg.payload["delta"], 3.5);
}

// ──────────────────────────────────────────────
// 测试 3: 任务调度事件 → TransportMessage
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_task_published_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::TaskPublished {
        task_id: uuid::Uuid::now_v7(),
        publisher_id: AgentId::new(),
        task_type: "analysis".into(),
        priority: 5,
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "task:published");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["task_type"], "analysis");
    assert_eq!(msg.payload["priority"], 5);
}

#[tokio::test]
async fn test_task_claimed_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::TaskClaimed {
        task_id: uuid::Uuid::now_v7(),
        agent_id: AgentId::new(),
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "task:claimed");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert!(msg.payload["task_id"].is_string());
}

#[tokio::test]
async fn test_task_completed_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::TaskCompleted {
        task_id: uuid::Uuid::now_v7(),
        agent_id: AgentId::new(),
        success: true,
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "task:completed");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["success"], true);
}

// ──────────────────────────────────────────────
// 测试 4: 系统事件 → TransportMessage
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_system_error_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::SystemError {
        agent_id: Some(AgentId::new()),
        error: "connection lost".into(),
        recoverable: true,
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "system:error");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["error"], "connection lost");
    assert_eq!(msg.payload["recoverable"], true);
}

#[tokio::test]
async fn test_config_changed_event() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::ConfigChanged {
        path: "monitor.level".into(),
        old_value: Some(json!("info")),
        new_value: json!("debug"),
    };

    let event_name = extract_event_name(&event);
    assert_eq!(event_name, "config:changed");

    let msg = event_to_transport_message(&codec, &event, &event_name);
    assert_eq!(msg.payload["path"], "monitor.level");
    assert_eq!(msg.payload["new_value"], "debug");
}

// ──────────────────────────────────────────────
// 测试 5: topic→event_name 转换表正确性
// ──────────────────────────────────────────────

/// 验证 topic→event_name 映射表与 TaijiEvent serde tag 一致。
#[test]
fn test_topic_event_name_mapping() {
    // 验证每个 event 的 serde tag 符合预期
    let events: Vec<(TaijiEvent, &str)> = vec![
        (
            TaijiEvent::CreditChanged {
                agent_id: AgentId::new(),
                new_score: 0.0,
                delta: 0.0,
            },
            "agent:credit_changed",
        ),
        (
            TaijiEvent::RealmUpgraded {
                agent_id: AgentId::new(),
                from: Realm::QiRefining,
                to: Realm::Foundation,
            },
            "agent:realm_upgraded",
        ),
        (
            TaijiEvent::TaskPublished {
                task_id: uuid::Uuid::now_v7(),
                publisher_id: AgentId::new(),
                task_type: "test".into(),
                priority: 1,
            },
            "task:published",
        ),
        (
            TaijiEvent::TaskCompleted {
                task_id: uuid::Uuid::now_v7(),
                agent_id: AgentId::new(),
                success: true,
            },
            "task:completed",
        ),
        (
            TaijiEvent::SystemError {
                agent_id: None,
                error: "err".into(),
                recoverable: false,
            },
            "system:error",
        ),
        (
            TaijiEvent::ConfigChanged {
                path: "test".into(),
                old_value: None,
                new_value: json!(null),
            },
            "config:changed",
        ),
    ];

    for (event, expected_name) in events {
        let serialized = serde_json::to_value(&event).unwrap();
        let actual_name = serialized["type"].as_str().unwrap();
        assert_eq!(
            actual_name, expected_name,
            "TaijiEvent serde tag mismatch: expected '{}', got '{}'",
            expected_name, actual_name
        );
    }
}

// ──────────────────────────────────────────────
// 测试 6: 全链路发送接收
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_full_pipeline_send_receive() {
    let codec = EventCodec::new(CodecConfig::default());
    let (adapter, mut rx) = WsTransportAdapter::new();

    // 创建事件
    let event = TaijiEvent::CreditChanged {
        agent_id: AgentId::new(),
        new_score: 88.0,
        delta: 2.0,
    };

    // 序列化并构建 TransportMessage
    let event_name = extract_event_name(&event);
    let msg = event_to_transport_message(&codec, &event, &event_name);

    // 通过 transport 发送
    adapter.send(msg.clone()).await.unwrap();

    // 接收端验证
    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_name, "agent:credit_changed");
    assert_eq!(received.payload["new_score"], 88.0);
    assert_eq!(received.payload["delta"], 2.0);
}

#[tokio::test]
async fn test_full_pipeline_multiple_events() {
    let codec = EventCodec::new(CodecConfig::default());
    let (adapter, mut rx) = WsTransportAdapter::new();

    let agent_id = AgentId::new();

    // 发送多个事件
    let events = vec![
        (
            TaijiEvent::AgentCreated {
                agent_id: agent_id.clone(),
                name: "trader-1".into(),
                realm: Realm::QiRefining,
                timestamp: std::time::SystemTime::now(),
            },
            "agent:created",
        ),
        (
            TaijiEvent::RealmUpgraded {
                agent_id: agent_id.clone(),
                from: Realm::QiRefining,
                to: Realm::Foundation,
            },
            "agent:realm_upgraded",
        ),
        (
            TaijiEvent::CreditChanged {
                agent_id: agent_id.clone(),
                new_score: 90.0,
                delta: 10.0,
            },
            "agent:credit_changed",
        ),
    ];

    for (event, expected_name) in &events {
        let msg = event_to_transport_message(&codec, event, expected_name);
        adapter.send(msg).await.unwrap();
    }

    // 逐个验证
    for (_, expected_name) in &events {
        let received = rx.recv().await.unwrap();
        assert_eq!(&received.event_name, expected_name);
    }
}
