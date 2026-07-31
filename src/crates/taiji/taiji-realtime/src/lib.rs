//! taiji-realtime — Real-time market data hub.
//!
//! ## 模块
//! - `channel`  — crossbeam SPSC 通道封装（TickChannel）
//! - `datasource` — CtpDataSource，实现 DataSource trait
//! - `ws_bridge` — axum WebSocket 服务器，JSON 推送 TickData
//! - `reconnect` — WebSocket 客户端自动重连（指数退避）
//! 参考: 量价时空/Phase-2-派发提示词.md:819 — R-2-503 — taiji-realtime 行情接入

pub mod channel;
pub mod datasource;
pub mod reconnect;
pub mod ws_bridge;

pub use channel::TickChannel;
pub use datasource::CtpDataSource;
pub use reconnect::{ConnectionEvent, WsDataClient, WsSourceConfig};
pub use ws_bridge::WsBridge;
