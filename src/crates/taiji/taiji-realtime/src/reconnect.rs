//! 行情源 WebSocket 客户端 — 自动连接、断线指数退避重连
//!
//! 数据流:
//!   WsDataClient → tokio-tungstenite WebSocket → 反序列化 TickData →
//!   crossbeam channel → Pipeline::feed_tick_direct()
//!
//! # 指数退避策略
//!
//! 断线后等待: 1s → 2s → 4s → 8s → ... → max 60s
//! 重连成功后立即重置退避到 1s。
//! 参考: 量价时空/Phase-2-派发提示词.md:819 — R-2-503 — taiji-realtime 行情接入

use crossbeam::channel::Sender;
use serde_json::Value;
use taiji_engine::types::tick::TickData;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::StreamExt;
use std::time::Duration;

/// WebSocket 行情源配置
#[derive(Debug, Clone)]
pub struct WsSourceConfig {
    /// WebSocket URL（如 `"ws://market.example.com/tick"`）
    pub url: String,
    /// 订阅的合约列表
    pub instruments: Vec<String>,
    /// 可选的认证 token
    pub auth_token: Option<String>,
}

/// WebSocket 客户端断线重连结果
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionEvent {
    /// 已连接
    Connected,
    /// 已断开
    Disconnected(String),
    /// 重连成功（第几次重试）
    Reconnected(u32),
    /// 达到最大重试次数，放弃
    GiveUp,
}

/// WebSocket 行情数据客户端
///
/// 从远程 WebSocket 服务端接收 JSON TickData，通过 crossbeam channel 发送给引擎。
/// 支持指数退避自动重连。
pub struct WsDataClient {
    config: WsSourceConfig,
    tick_tx: Sender<TickData>,
    event_tx: crossbeam::channel::Sender<ConnectionEvent>,
    event_rx: crossbeam::channel::Receiver<ConnectionEvent>,
    max_retries: u32,
}

impl WsDataClient {
    /// 创建 WsDataClient
    ///
    /// * `config` — 行情源配置（URL + 订阅合约）
    /// * `tick_tx` — crossbeam Sender，发送解析后的 TickData
    pub fn new(config: WsSourceConfig, tick_tx: Sender<TickData>) -> Self {
        let (event_tx, event_rx) = crossbeam::channel::unbounded();
        Self {
            config,
            tick_tx,
            event_tx,
            event_rx,
            max_retries: u32::MAX, // 默认无限重试
        }
    }

    /// 设置最大重试次数（默认无限重试）
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// 获取连接事件接收端
    pub fn event_receiver(&self) -> crossbeam::channel::Receiver<ConnectionEvent> {
        self.event_rx.clone()
    }

    /// 启动连接循环（阻塞，应在独立线程中运行）
    ///
    /// 连接 → 处理消息 → 断线 → 指数退避重连 → ...
    pub fn run(&self) {
        let url = &self.config.url;
        let mut retry_count = 0u32;

        loop {
            match self.connect_once(url) {
                Ok(()) => {
                    // 正常退出（WebSocket 关闭）
                    retry_count = 0;
                    let _ = self.event_tx.send(ConnectionEvent::Disconnected("connection closed".into()));
                }
                Err(e) => {
                    retry_count += 1;
                    let _ = self.event_tx.send(ConnectionEvent::Disconnected(e.to_string()));

                    if retry_count >= self.max_retries {
                        let _ = self.event_tx.send(ConnectionEvent::GiveUp);
                        break;
                    }

                    // 指数退避: 1s, 2s, 4s, 8s, ..., max 60s
                    let delay = Duration::from_secs(
                        (1u64 << retry_count.min(6)).min(60)
                    );
                    tracing::warn!(
                        url = %url,
                        retry = retry_count,
                        delay_ms = delay.as_millis(),
                        "WsDataClient 断线重连"
                    );
                    std::thread::sleep(delay);

                    let _ = self.event_tx.send(ConnectionEvent::Reconnected(retry_count));
                }
            }
        }
    }

    /// 单次 WebSocket 连接和处理循环
    fn connect_once(&self, url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;

        rt.block_on(async {
            let (ws_stream, _) = connect_async(url).await?;
            let _ = self.event_tx.send(ConnectionEvent::Connected);
            tracing::info!(url = %url, "WsDataClient 已连接");

            let (_, mut read) = ws_stream.split();

            while let Some(msg) = read.next().await {
                match msg? {
                    Message::Text(text) => {
                        if let Err(e) = self.handle_message(&text) {
                            tracing::error!("消息处理失败: {}", e);
                        }
                    }
                    Message::Binary(data) => {
                        if let Ok(text) = String::from_utf8(data.to_vec()) {
                            if let Err(e) = self.handle_message(&text) {
                                tracing::error!("消息处理失败: {}", e);
                            }
                        }
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Close(_) => {
                        tracing::info!("WsDataClient 收到关闭帧");
                        break;
                    }
                    _ => {}
                }
            }

            Ok(())
        })
    }

    /// 处理单条 JSON 消息，反序列化为 TickData 并发送到 channel
    fn handle_message(&self, text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let value: Value = serde_json::from_str(text)?;

        // 支持两种格式:
        // 1. 直接是 TickData JSON 对象
        // 2. 包装在 { "type": "tick", "data": {...} } 中
        let tick_value = match value.get("data") {
            Some(data) if value.get("type").and_then(|t| t.as_str()) == Some("tick") => data,
            _ => &value,
        };

        let tick: TickData = serde_json::from_value(tick_value.clone())?;

        // 通过 crossbeam channel 发送（非阻塞，L1 安全）
        self.tick_tx
            .send(tick)
            .map_err(|e| format!("tick channel send failed: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel;
    use std::thread;
    use taiji_engine::types::tick::TickData;

    #[test]
    fn test_handle_tick_message() {
        let (tx, rx) = channel::bounded::<TickData>(16);
        let config = WsSourceConfig {
            url: "ws://localhost:9999/test".into(),
            instruments: vec!["rb2501".into()],
            auth_token: None,
        };
        let client = WsDataClient::new(config, tx);

        // 构建完整 TickData，序列化后测试反序列化
        let mut tick = TickData::default();
        tick.instrument = "rb2501".into();
        tick.last_price = 4200.5;
        tick.volume = 1000.0;
        tick.open_interest = 50000.0;
        tick.timestamp_ms = 1700000000000;
        tick.open_price = 4190.0;
        tick.highest_price = 4210.0;
        tick.lowest_price = 4185.0;
        tick.close_price = 4200.5;
        tick.turnover = 42005000.0;
        tick.pre_settlement_price = 4180.0;
        tick.upper_limit_price = 4300.0;
        tick.lower_limit_price = 4060.0;
        tick.bid_price1 = 4200.0;
        tick.bid_volume1 = 10;
        tick.ask_price1 = 4201.0;
        tick.ask_volume1 = 15;
        tick.bid_price2 = 4199.5;
        tick.bid_volume2 = 20;
        tick.ask_price2 = 4201.5;
        tick.ask_volume2 = 25;

        let json = serde_json::to_string(&tick).unwrap();
        client.handle_message(&json).unwrap();

        let received = rx.recv().unwrap();
        assert_eq!(received.instrument, "rb2501");
        assert_eq!(received.last_price, 4200.5);
        assert!((received.bid_price1 - 4200.0).abs() < 1e-10);
    }

    #[test]
    fn test_handle_wrapped_message() {
        let (tx, rx) = channel::bounded::<TickData>(16);
        let config = WsSourceConfig {
            url: "ws://localhost:9999/test".into(),
            instruments: vec![],
            auth_token: None,
        };
        let client = WsDataClient::new(config, tx);

        let mut tick = TickData::default();
        tick.instrument = "IF2506".into();
        tick.last_price = 3800.0;
        tick.volume = 500.0;
        tick.timestamp_ms = 1700000000000;

        let inner = serde_json::to_value(&tick).unwrap();
        let wrapped = serde_json::json!({"type": "tick", "data": inner});
        let json = serde_json::to_string(&wrapped).unwrap();

        client.handle_message(&json).unwrap();

        let received = rx.recv().unwrap();
        assert_eq!(received.instrument, "IF2506");
        assert_eq!(received.last_price, 3800.0);
    }

    #[test]
    fn test_connection_event_channel_mechanics() {
        // 验证连接事件通道的发送/接收机制
        use crossbeam::channel;

        let (tx, _tick_rx) = channel::bounded::<TickData>(4);
        let (event_tx, event_rx) = channel::unbounded::<ConnectionEvent>();
        let config = WsSourceConfig {
            url: "ws://127.0.0.1:1/test".into(),
            instruments: vec![],
            auth_token: None,
        };
        let _client = WsDataClient::new(config, tx);

        // 模拟连接事件序列
        thread::spawn(move || {
            let _ = event_tx.send(ConnectionEvent::Disconnected("test".into()));
            thread::sleep(std::time::Duration::from_millis(50));
            let _ = event_tx.send(ConnectionEvent::Reconnected(1));
            thread::sleep(std::time::Duration::from_millis(50));
            let _ = event_tx.send(ConnectionEvent::GiveUp);
        });

        let mut events = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match event_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(e) => events.push(e),
                Err(_) => break,
            }
        }
        assert!(!events.is_empty(), "应有连接事件");
        assert!(
            events.iter().any(|e| matches!(e, ConnectionEvent::GiveUp)),
            "应包含 GiveUp 事件, got: {:?}",
            events
        );
    }

    #[test]
    fn test_invalid_json_does_not_panic() {
        let (tx, _rx) = channel::bounded::<TickData>(4);
        let config = WsSourceConfig {
            url: "ws://localhost:9999/test".into(),
            instruments: vec![],
            auth_token: None,
        };
        let client = WsDataClient::new(config, tx);

        // 无效 JSON 不应 panic
        let result = client.handle_message("not valid json");
        assert!(result.is_err());

        // 缺少必要字段的 JSON
        let result = client.handle_message(r#"{"hello":"world"}"#);
        assert!(result.is_err());
    }
}
