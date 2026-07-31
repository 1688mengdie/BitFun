//! transport crate 集成测试
//!
//! 覆盖 TransportAdapter trait 的全链路场景：
//! - MockTransportAdapter 接口完整性
//! - WsTransportAdapter send → broadcast → receive 全链路
//! - TransportMessage 序列化 roundtrip
//! - 多消费者场景
//! - trait object 多态使用

use serde_json::json;
use taiji_infra_transport::{
    MockTransportAdapter, TransportAdapter, TransportMessage, WsTransportAdapter,
};

/// 验证 MockTransportAdapter 完整接口
#[tokio::test]
async fn test_mock_adapter_full_flow() {
    let adapter = MockTransportAdapter::new();
    assert_eq!(adapter.sent_count(), 0);

    let msg = TransportMessage::new("test:event", json!({"seq": 1}));
    adapter.send(msg).await.unwrap();
    assert_eq!(adapter.sent_count(), 1);

    adapter.clear();
    assert_eq!(adapter.sent_count(), 0);
}

/// 验证 WsTransportAdapter → broadcast → receive 全链路
#[tokio::test]
async fn test_ws_adapter_broadcast_flow() {
    let (adapter, mut rx) = WsTransportAdapter::new();

    let msg = TransportMessage::new("broadcast:test", json!({"msg": "hello"}));
    adapter.send(msg.clone()).await.unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received, msg);
}

/// 验证 WsTransportAdapter 多消费者场景
#[tokio::test]
async fn test_ws_adapter_multiple_consumers() {
    let (adapter, mut rx1) = WsTransportAdapter::new();
    let mut rx2 = adapter.subscribe();

    let msg = TransportMessage::new("multi:consumer", json!({"count": 3}));
    adapter.send(msg.clone()).await.unwrap();

    assert_eq!(rx1.recv().await.unwrap(), msg);
    assert_eq!(rx2.recv().await.unwrap(), msg);
}

/// 验证 TransportMessage serde 序列化 roundtrip
#[test]
fn test_transport_message_serde_roundtrip() {
    let msg = TransportMessage::new("agent:credit_changed", json!({"score": 85.0}));
    let json_str = serde_json::to_string(&msg).unwrap();
    let deserialized: TransportMessage = serde_json::from_str(&json_str).unwrap();
    assert_eq!(msg, deserialized);
}

/// 验证 TransportAdapter trait 对象多态性
#[tokio::test]
async fn test_trait_object_polymorphism() {
    let (_rx1, _rx2) = {
        let (adapter1, rx1) = WsTransportAdapter::new();
        let (adapter2, rx2) = WsTransportAdapter::new();
        let adapters: Vec<Box<dyn TransportAdapter>> = vec![
            Box::new(MockTransportAdapter::new()),
            Box::new(adapter1),
            Box::new(adapter2),
        ];

        let msg = TransportMessage::new("poly:test", json!({"type": "polymorphism"}));
        for adapter in &adapters {
            adapter.send(msg.clone()).await.unwrap();
        }
        (rx1, rx2)
    };
    // 保持 receiver 存活直到 send 完成
    drop(_rx1);
    drop(_rx2);
}

/// 验证 with_capacity 构造
#[tokio::test]
async fn test_ws_adapter_with_capacity() {
    let capacity = 64;
    let (adapter, _rx) = WsTransportAdapter::with_capacity(capacity);
    assert_eq!(adapter.capacity(), capacity);
}

/// 验证顺序一致性
#[tokio::test]
async fn test_ws_adapter_message_order() {
    let (adapter, mut rx) = WsTransportAdapter::new();

    for i in 0..5 {
        let msg = TransportMessage::new("ordered", json!({"i": i}));
        adapter.send(msg).await.unwrap();
    }

    for i in 0..5 {
        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, json!({"i": i}));
    }
}
