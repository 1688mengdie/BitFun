#![allow(unused)]
#![doc = "taiji-infra-transport — 传音阵（Layer 3 连接层 IPC）"]

//! LVPA Layer 3 连接层组件：统一 IPC 传输抽象。
//!
//! 提供 `TransportAdapter` trait，为前端提供 `TransportMessage` 序列化 + 发送能力。
//! 适配器包括 `WsTransportAdapter`（WebSocket）和 `TauriTransportAdapter`（Tauri emitter）。
//!
//! # 架构定位
//!
//! transport 层是 Layer 3 用户交互层的连接层组件（v2.3 重分类），
//! **不属于**基础设施模块。详见架构总纲 §1.1。
//!
//! # 设计原则
//!
//! - **不感知语义**：仅传递序列化后的 `TransportMessage(event_name + payload)`
//! - **可插拔后端**：通过 `TransportAdapter` trait 支持多种传输方式
//! - **Layer 3 边界**：不参与后端事件总线，仅作为数据输出通道
//!
//! # 模块结构
//!
//! | 模块 | 类型 | 说明 |
//! |:-----|:-----|:------|
//! | `message` | `TransportMessage` | 传输消息结构体（event_name + payload） |
//! | `adapter` | `TransportAdapter` trait | 核心传输接口 |
//! | `adapter` | `MockTransportAdapter` | 测试用模拟适配器（嵌入 adapter.rs） |
//! | `ws_adapter` | `WsTransportAdapter` | 基于 tokio::sync::broadcast 的实现 |
//! | `tauri_adapter` | `TauriTransportAdapter` | 基于 Tauri Emitter 的实现（feature gate） |
//!
//! # 快速开始
//!
//! ```rust
//! use taiji_infra_transport::{TransportMessage, WsTransportAdapter, TransportAdapter};
//! use serde_json::json;
//!
//! let (adapter, mut rx) = WsTransportAdapter::new();
//! let msg = TransportMessage::new("hello", json!({"msg": "world"}));
//!
//! let rt = tokio::runtime::Runtime::new().unwrap();
//! rt.block_on(async {
//!     adapter.send(msg.clone()).await.unwrap();
//!     let received = rx.recv().await.unwrap();
//!     assert_eq!(received, msg);
//! });
//! ```
//!
//! # R-ID 映射
//!
//! 对应 R-1-103，子任务分解：
//! - R-1-103-01: TransportMessage 类型 (`message.rs`)
//! - R-1-103-02: TransportAdapter trait (`adapter.rs`)
//! - R-1-103-03: WsTransportAdapter (`ws_adapter.rs`)
//! - R-1-103-04: TauriTransportAdapter (`tauri_adapter.rs`)
//! - R-1-103-05: lib.rs 导出
//! - R-1-103-06: 测试全覆盖

pub mod message;
pub mod adapter;
pub mod ws_adapter;

#[cfg(feature = "tauri")]
pub mod tauri_adapter;

// — 重新导出核心类型 —

pub use message::TransportMessage;
pub use adapter::{MockTransportAdapter, TransportAdapter};
pub use ws_adapter::WsTransportAdapter;

#[cfg(feature = "tauri")]
pub use tauri_adapter::TauriTransportAdapter;
