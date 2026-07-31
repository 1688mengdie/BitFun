//! event-bus 闆嗘垚娴嬭瘯銆?
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use taiji_infra_event_bus::bus::{EventBus, EventBusConfig};
use taiji_infra_event_bus::codec::{CodecConfig, EventCodec};
use taiji_infra_event_bus::envelope::TaijiEventPriority;
use taiji_infra_event_bus::error::EventBusResult;
use taiji_infra_event_bus::event::TaijiEvent;
use taiji_infra_event_bus::router::{EventRouter, EventSubscriber};
use taiji_infra_event_bus::scheduler::{KpiScheduler, TaskResult};
use taiji_infra_message_bus::bus::MessageBus;
use taiji_infra_message_bus::error::MessageBusError;
use taiji_infra_message_bus::in_memory::InMemoryBus;
use taiji_types::agent::AgentId;
use taiji_types::credit::AgentCredit;
use taiji_types::realm::Realm;
use tokio::sync::mpsc;

// 鈹€鈹€ MockMessageBus锛堢敤浜庢硾鍨嬮獙璇侊級 鈹€鈹€

struct MockMessageBus {
    pub topics: DashMap<String, Vec<mpsc::Sender<Bytes>>>,
}

impl MockMessageBus {
    fn new() -> Self {
        Self {
            topics: DashMap::new(),
        }
    }
}

impl MessageBus for MockMessageBus {
    fn publish(&self, topic: &str, payload: Bytes) -> Result<(), MessageBusError> {
        if let Some(mut entry) = self.topics.get_mut(topic) {
            entry.retain(|tx| tx.try_send(payload.clone()).is_ok());
        }
        Ok(())
    }

    fn subscribe(&self, topic: &str) -> Result<mpsc::Receiver<Bytes>, MessageBusError> {
        let (tx, rx) = mpsc::channel(64);
        self.topics
            .entry(topic.to_string())
            .or_default()
            .push(tx);
        Ok(rx)
    }
}

// 鈹€鈹€ 娴嬭瘯 鈹€鈹€

#[tokio::test]
async fn test_event_codec_roundtrip() {
    let codec = EventCodec::new(CodecConfig::default());
    let event = TaijiEvent::AgentCreated {
        agent_id: AgentId::new(),
        name: "test_agent".into(),
        realm: Realm::QiRefining,
        timestamp: std::time::SystemTime::now(),
    };

    let bytes = codec.serialize(&event).unwrap();
    let deserialized = codec.deserialize(&bytes).unwrap();

    match (&event, &deserialized) {
        (TaijiEvent::AgentCreated { name: n1, .. }, TaijiEvent::AgentCreated { name: n2, .. }) => {
            assert_eq!(n1, n2);
        }
        _ => panic!("event type mismatch after roundtrip"),
    }
}

#[tokio::test]
async fn test_event_bus_pub_sub() {
    let msg_bus = InMemoryBus::new(64);
    let config = EventBusConfig {
        topic_prefix: "test.".into(),
        codec_config: CodecConfig::default(),
    };
    let event_bus = EventBus::new(msg_bus, config);

    let mut rx = event_bus.subscribe("normal").unwrap();

    let agent_id = AgentId::new();
    let event = TaijiEvent::AgentCreated {
        agent_id: agent_id.clone(),
        name: "pub_sub_test".into(),
        realm: Realm::Foundation,
        timestamp: std::time::SystemTime::now(),
    };

    let id = event_bus.publish(event, TaijiEventPriority::Normal).await.unwrap();
    assert!(!id.is_empty());

    let received = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("channel closed");
    match &received {
        TaijiEvent::AgentCreated { name, .. } => {
            assert_eq!(name, "pub_sub_test");
        }
        _ => panic!("unexpected event type"),
    }
}

#[tokio::test]
async fn test_priority_sorting() {
    assert!(TaijiEventPriority::Critical < TaijiEventPriority::High);
    assert!(TaijiEventPriority::High < TaijiEventPriority::Normal);
    assert!(TaijiEventPriority::Normal < TaijiEventPriority::Low);
}

#[test]
fn test_kpi_formula() {
    let scheduler = KpiScheduler::new();
    let credit = AgentCredit {
        score: 0.0,

        success_rate: 1.0,

        review_pass_rate: 1.0,
        rework_rate: 0.0,
        kpi_bonus: 0.0,
    };
    let score = scheduler.calculate(&credit);
    // 1.0 * 0.4 + 1.0 * 0.3 + (1-0.0) * 0.2 + 0.0 * 0.1 = 0.9
    assert!((score - 0.9).abs() < 1e-10);
}

#[test]
fn test_kpi_select_best() {
    let scheduler = KpiScheduler::new();
    let a1 = AgentId::new();
    let a2 = AgentId::new();

    scheduler.set_credit(
        a1.clone(),
        AgentCredit {
            score: 0.0,

            success_rate: 0.9,

            review_pass_rate: 0.8,
            rework_rate: 0.1,
            kpi_bonus: 0.0,
        },
    );
    scheduler.set_credit(
        a2.clone(),
        AgentCredit {
            score: 0.0,

            success_rate: 0.5,

            review_pass_rate: 0.5,
            rework_rate: 0.3,
            kpi_bonus: 0.0,
        },
    );

    let best = scheduler.select_best(&[a1.clone(), a2.clone()]);
    assert_eq!(best, Some(a1));
}

#[test]
fn test_kpi_update_credit() {
    let scheduler = KpiScheduler::new();
    let agent = AgentId::new();

    scheduler.update_credit(
        &agent,
        &TaskResult {
            success: true,
            review_passed: true,
            rework: false,
            kpi_bonus: 0.0,
        },
    );

    let credit = scheduler.get_credit(&agent);
    assert!(credit.score > 0.0);
    assert!(credit.success_rate > 0.0);
}

#[tokio::test]
async fn test_mock_message_bus_generic() {
    let mock = MockMessageBus::new();
    let config = EventBusConfig::default();
    let event_bus = EventBus::new(mock, config);

    let event = TaijiEvent::SystemError {
        agent_id: None,
        error: "test error".into(),
        recoverable: true,
    };

    let result = event_bus.publish(event, TaijiEventPriority::Low).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_subscribers_isolation() {
    let msg_bus = InMemoryBus::new(64);
    let config = EventBusConfig::default();
    let event_bus = EventBus::new(msg_bus, config);

    let mut rx1 = event_bus.subscribe("normal").unwrap();
    let mut rx2 = event_bus.subscribe("normal").unwrap();

    let event = TaijiEvent::ConfigChanged {
        path: "test.path".into(),
        old_value: None,
        new_value: serde_json::json!("new"),
    };

    event_bus.publish(event, TaijiEventPriority::Normal).await.unwrap();

    // 涓や釜璁㈤槄鑰呴兘搴旀敹鍒?    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx1.recv())
        .await
        .expect("subscriber 1 timeout")
        .expect("channel closed");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx2.recv())
        .await
        .expect("subscriber 2 timeout")
        .expect("channel closed");
}

#[tokio::test]
async fn test_event_router_with_failing_subscriber() {
    let router = EventRouter::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // 姝ｅ父璁㈤槄鑰?    let ok_sub = {
        let counter = Arc::clone(&counter);
        struct OkSub(Arc<AtomicUsize>);
        #[async_trait]
        impl EventSubscriber for OkSub {
            async fn on_event(&self, _event: &TaijiEvent) -> EventBusResult<()> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        Arc::new(OkSub(counter))
    };

    // 澶辫触璁㈤槄鑰?    struct FailSub;
    #[async_trait]
    impl EventSubscriber for FailSub {
        async fn on_event(&self, _event: &TaijiEvent) -> EventBusResult<()> {
            Err(taiji_infra_event_bus::error::EventBusError::Internal("fail".into()))
        }
    }

    router.subscribe("ok".into(), ok_sub);
    router.subscribe("fail".into(), Arc::new(FailSub));

    let event = TaijiEvent::SystemError {
        agent_id: None,
        error: "test".into(),
        recoverable: false,
    };

    router.route(&event).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1, "ok subscriber should have been called despite fail subscriber");
}

// 鈹€鈹€ R-1-201: message-bus 鈫?event-bus 闆嗘垚娴嬭瘯 鈹€鈹€

#[tokio::test]
async fn test_at_least_once_delivery_100_events() {
    let msg_bus = InMemoryBus::new(256);
    let config = EventBusConfig {
        topic_prefix: "integ.".into(),
        codec_config: CodecConfig::default(),
    };
    let event_bus = EventBus::new(msg_bus, config);

    // 鍚庡彴 drain 浠诲姟锛氭敹闆嗘墍鏈変簨浠?    let mut rx = event_bus.subscribe("normal").unwrap();
    let count = 100usize;
    let drainer = tokio::spawn(async move {
        let mut received = Vec::with_capacity(count);
        for _ in 0..count {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                rx.recv(),
            )
            .await
            {
                Ok(Some(event)) => received.push(event),
                _ => break,
            }
        }
        received
    });

    // 鍙戝竷 100 涓?AgentCreated 浜嬩欢
    for i in 0..count {
        let event = TaijiEvent::AgentCreated {
            agent_id: AgentId::new(),
            name: format!("agent_{}", i),
            realm: Realm::Foundation,
            timestamp: std::time::SystemTime::now(),
        };
        event_bus
            .publish(event, TaijiEventPriority::Normal)
            .await
            .unwrap();
        tokio::task::yield_now().await;
    }

    let received = drainer.await.unwrap();
    assert_eq!(
        received.len(),
        count,
        "at-least-once delivery: expected {} events, got {}",
        count,
        received.len()
    );

    // 楠岃瘉浜嬩欢鍐呭椤哄簭
    for (i, event) in received.iter().enumerate() {
        match event {
            TaijiEvent::AgentCreated { name, .. } => {
                assert_eq!(name, &format!("agent_{}", i));
            }
            _ => panic!("unexpected event type at index {}", i),
        }
    }
}
