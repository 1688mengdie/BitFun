//! 传输消息类型 — TransportMessage
//!
//! transport 层的唯一消息类型。不携带任何语义类型信息。
//! 仅承载 event_name + 已序列化的 JSON payload。
//!
//! # 设计原则
//! - 不感知事件语义（不引用 TaijiEvent/AgenticEvent）
//! - event_name 供前端路由（如 "agent:updated"、"tool:completed"）
//! - payload 为已序列化的 JSON，前端直接消费
//!
//! # 参考来源
//! - H8: modules/transport/接口设计.md:22-28 — TransportMessage 结构体定义

use serde::{Deserialize, Serialize};

/// 传输消息 — transport 层的唯一消息类型。
///
/// 由 event-bus 在调用 `send()` 前通过 `EventCodec` 完成序列化组装，
/// transport 层只负责传递，不感知消息语义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransportMessage {
    /// 前端友好事件名称（如 "agent:credit_changed"）。
    pub event_name: String,
    /// 已序列化的 JSON 负载。
    pub payload: serde_json::Value,
}

impl TransportMessage {
    /// 创建 TransportMessage。
    ///
    /// # 示例
    /// ```
    /// use taiji_infra_transport::TransportMessage;
    /// use serde_json::json;
    ///
    /// let msg = TransportMessage::new("agent:credit_changed", json!({"score": 85.0}));
    /// assert_eq!(msg.event_name, "agent:credit_changed");
    /// ```
    pub fn new(event_name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event_name: event_name.into(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new() {
        let msg = TransportMessage::new("test:event", json!({"key": "value"}));
        assert_eq!(msg.event_name, "test:event");
        assert_eq!(msg.payload, json!({"key": "value"}));
    }

    #[test]
    fn test_serde_roundtrip() {
        let msg = TransportMessage::new("agent:credit_changed", json!({"score": 85.0, "delta": 5.0}));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: TransportMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_serde_roundtrip_empty_payload() {
        let msg = TransportMessage::new("heartbeat", json!(null));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: TransportMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_serde_roundtrip_nested_payload() {
        let msg = TransportMessage::new(
            "tool:completed",
            json!({
                "tool_name": "fetch_kline",
                "duration_ms": 42,
                "result": {"open": 5000.0, "high": 5050.0, "low": 4980.0, "close": 5020.0}
            }),
        );
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: TransportMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_event_name_from_string_slice() {
        let msg = TransportMessage::new("simple_event", json!({}));
        assert_eq!(msg.event_name, "simple_event");
    }

    #[test]
    fn test_event_name_from_string() {
        let msg = TransportMessage::new(String::from("owned_event"), json!({}));
        assert_eq!(msg.event_name, "owned_event");
    }
}
