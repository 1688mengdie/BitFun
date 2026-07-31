//! EventCodec — TaijiEvent ↔ Bytes 序列化桥梁。
//!
//! 参考: modules/event-bus/接口设计.md §三（v2.4）
//! 当前仅支持 JSON，未来可扩展 MessagePack / BSON。

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::error::{EventBusError, EventBusResult};
use crate::event::TaijiEvent;

/// 序列化格式。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SerializationFormat {
    #[serde(rename = "json")]
    Json,
}

/// Codec 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecConfig {
    pub format: SerializationFormat,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::Json,
        }
    }
}

/// 序列化桥梁：TaijiEvent ↔ Bytes。
pub struct EventCodec {
    pub config: CodecConfig,
}

impl EventCodec {
    pub fn new(config: CodecConfig) -> Self {
        Self { config }
    }

    /// 将 TaijiEvent 序列化为 Bytes。
    pub fn serialize(&self, event: &TaijiEvent) -> EventBusResult<Bytes> {
        match self.config.format {
            SerializationFormat::Json => serde_json::to_vec(event)
                .map(Bytes::from)
                .map_err(|e| EventBusError::Serialization(e.to_string())),
        }
    }

    /// 将 Bytes 反序列化为 TaijiEvent。
    pub fn deserialize(&self, data: &[u8]) -> EventBusResult<TaijiEvent> {
        match self.config.format {
            SerializationFormat::Json => serde_json::from_slice(data)
                .map_err(|e| EventBusError::Serialization(e.to_string())),
        }
    }
}
