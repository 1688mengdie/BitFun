//! 云条件单降级模块 — 架构总纲 §0.7-8
//!
//! 本地延迟时自动切换为券商云端执行。
//!
//! # 设计
//!
//! ```text
//! [LatencyMonitor] ──测量──→ [AutoSwitcher] ──切换──→ [CloudConditionalBridge]
//!      ↑                        │                          │
//!      └── 周期性 ping 检测      │  本地延迟 > 阈值 → 云     │
//!                               │  本地延迟恢复 → 切回     │
//!                               └──────────────────────────┘
//! ```
//!
//! # 券商适配器
//!
//! - [`CtpCloudBridge`]: 基于 openctp TTS 的条件单（止损/止盈）
//! - [`JqCloudBridge`]: 基于掘金 (myquant) REST API 的条件单
//! - [`DefaultCloudBridge`]: 通用 HTTP API 实现（回退）
//!
//! # 参考
//!
//! 架构总纲 §0.7: 时间就是金钱 — 实时交易不允许任何代码层阻塞。做不到毫秒级则云条件单降级
//! 架构总纲 §0.8: 不够快就上云条件单 — 本地延迟时自动切换为券商云端执行
//! 参考: openctp (BSD) td_demo.py ReqOrderInsert + TTS REST API
//! 参考: 掘金 myquant gm SDK REST API

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::types::OrderRequest;

// =============================================================================
// 券商枚举 + 工厂
// =============================================================================

/// 支持的云条件单券商。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudBrokerKind {
    /// openctp / CTP (TTS 仿真或生产环境)
    Ctp,
    /// 掘金 myquant
    Jq,
    /// 通用 HTTP API（默认）
    Default,
}

/// 创建指定类型的云条件单桥接。
///
/// # 参数
///
/// - `kind`: 券商类型
/// - `api_base_url`: API 基础 URL
/// - `api_key`: API 密钥 / Token
/// - `account_id`: 账户 ID
/// - `broker_id`: 券商 ID（CTP 专用，非 CTP 券商可传空字符串）
pub fn create_cloud_bridge(
    kind: CloudBrokerKind,
    api_base_url: &str,
    api_key: &str,
    account_id: &str,
    broker_id: &str,
) -> Arc<dyn CloudConditionalBridge> {
    match kind {
        CloudBrokerKind::Ctp => Arc::new(CtpCloudBridge::new(api_base_url, api_key, account_id, broker_id)),
        CloudBrokerKind::Jq => Arc::new(JqCloudBridge::new(api_base_url, api_key, account_id)),
        CloudBrokerKind::Default => Arc::new(DefaultCloudBridge::new(api_base_url, api_key, account_id)),
    }
}

// =============================================================================
// 延迟检测
// =============================================================================

/// 延迟统计快照。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LatencySnapshot {
    /// 最近一次测量的 RTT（微秒）。
    pub last_rtt_us: u64,
    /// 最近 N 次的平均延迟（微秒）。
    pub avg_rtt_us: f64,
    /// 最近 N 次的最大延迟（微秒）。
    pub max_rtt_us: u64,
    /// 样本数量。
    pub sample_count: u64,
}

/// 延迟检测触发器 — 定时测量 execution bridge 的 RTT。
///
/// # Layer 1 合规
///
/// LatencyMonitor 运行在 L1 IO 线程或独立的检测线程中，
/// 不阻塞 L1 Compute Thread。测量结果通过原子变量共享。
pub struct LatencyMonitor {
    /// 延迟测量回调（向 bridge 发送 ping 并测量 RTT）。
    measure_fn: Box<dyn Fn() -> Result<Duration, String> + Send + Sync>,
    /// 检测间隔。
    interval: Duration,
    /// 最新 RTT 快照（原子共享）。
    last_rtt_us: Arc<AtomicU64>,
    /// 滑动窗口内历史 RTT 值（μs）。
    history: Arc<std::sync::Mutex<Vec<u64>>>,
    /// 窗口大小。
    window_size: usize,
}

impl LatencyMonitor {
    /// 创建新的延迟监测器。
    ///
    /// `measure_fn` 返回一次 RTT 测量结果。
    /// `interval` 是测量间隔。
    /// `window_size` 是滑动平均窗口大小（默认 10）。
    pub fn new(
        measure_fn: Box<dyn Fn() -> Result<Duration, String> + Send + Sync>,
        interval: Duration,
        window_size: usize,
    ) -> Self {
        Self {
            measure_fn,
            interval,
            last_rtt_us: Arc::new(AtomicU64::new(0)),
            history: Arc::new(std::sync::Mutex::new(Vec::with_capacity(window_size))),
            window_size,
        }
    }

    /// 获取最近一次测量的 RTT（微秒）。
    pub fn last_rtt_us(&self) -> u64 {
        self.last_rtt_us.load(AtomicOrdering::Acquire)
    }

    /// 获取当前延迟快照。
    pub async fn snapshot(&self) -> LatencySnapshot {
        let history = self.history.lock().unwrap();
        let len = history.len() as u64;
        let avg = if len > 0 {
            history.iter().copied().map(|v| v as f64).sum::<f64>() / len as f64
        } else {
            0.0
        };
        let max = history.iter().copied().max().unwrap_or(0);

        LatencySnapshot {
            last_rtt_us: self.last_rtt_us(),
            avg_rtt_us: avg,
            max_rtt_us: max,
            sample_count: len,
        }
    }

    /// 执行一次延迟测量（同步，适合在 IO 线程调用）。
    pub fn measure_once(&self) -> u64 {
        let us = match (self.measure_fn)() {
            Ok(duration) => {
                let us = duration.as_micros() as u64;
                self.last_rtt_us.store(us, AtomicOrdering::Release);
                us
            }
            Err(e) => {
                warn!("Latency measure failed: {}", e);
                let us = u64::MAX;
                self.last_rtt_us.store(us, AtomicOrdering::Release);
                us
            }
        };
        if let Ok(mut history) = self.history.lock() {
            history.push(us);
            if history.len() > self.window_size {
                history.remove(0);
            }
        }
        us
    }

    /// 循环执行延迟检测（异步任务）。
    pub async fn run_loop(&self, shutdown_rx: &mut mpsc::Receiver<()>) {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.measure_once();
                }
                _ = shutdown_rx.recv() => {
                    info!("LatencyMonitor shutdown");
                    break;
                }
            }
        }
    }
}

// =============================================================================
// 云条件单桥接
// =============================================================================

/// 条件单类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionalOrderType {
    /// 止损单（价格达到触发价后以市价成交）。
    Stop,
    /// 止盈单（价格达到触发价后以限价成交）。
    Limit,
    /// OTO（一单触发另一单）。
    Oto,
    /// OCO（一单成交则取消另一单）。
    Oco,
    /// 追踪止损（价格向有利方向移动时自动调整触发价）。
    TrailingStop,
}

/// 云条件单请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConditionalOrder {
    /// 券商端订单 ID。
    pub cloud_order_id: Option<String>,
    /// 合约代码。
    pub instrument: String,
    /// 方向（Buy/Sell）。
    pub direction: String,
    /// 开平标志（Open/Close）。
    pub offset: String,
    /// 触发价格（条件单触发价）。
    pub trigger_price: f64,
    /// 委托价格（触发后的限价，市价单填 0）。
    pub limit_price: f64,
    /// 数量。
    pub volume: u32,
    /// 条件单类型。
    pub order_type: ConditionalOrderType,
    /// 有效期限（秒，0 为当日有效）。
    pub ttl_secs: u64,
    /// 附加参数（券商特定扩展）。
    pub extra: std::collections::HashMap<String, String>,
}

/// 云条件单响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConditionalResponse {
    /// 券商端订单 ID。
    pub cloud_order_id: String,
    /// 是否成功提交。
    pub success: bool,
    /// 错误消息（如有）。
    pub error: Option<String>,
}

/// 云条件单 API 桥接 trait。
///
/// 实现将信号作为条件单提交到券商云端执行。
#[async_trait]
pub trait CloudConditionalBridge: Send + Sync {
    /// 提交一个云条件单。
    async fn submit_conditional(&self, order: CloudConditionalOrder) -> Result<CloudConditionalResponse, String>;

    /// 取消一个云条件单。
    async fn cancel_conditional(&self, cloud_order_id: &str) -> Result<bool, String>;

    /// 查询云条件单状态。
    async fn query_conditional(&self, cloud_order_id: &str) -> Result<CloudConditionalStatus, String>;

    /// 检查云 API 连接状态。
    async fn check_connectivity(&self) -> Result<Duration, String>;
}

/// 云条件单状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConditionalStatus {
    pub cloud_order_id: String,
    pub status: ConditionalStatus,
    pub filled_volume: u32,
    pub filled_price: Option<f64>,
    pub error: Option<String>,
}

/// 云条件单生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionalStatus {
    /// 已提交。
    Submitted,
    /// 已激活（条件监控中）。
    Active,
    /// 已触发。
    Triggered,
    /// 部分成交。
    PartiallyFilled,
    /// 全部成交。
    Filled,
    /// 已取消。
    Cancelled,
    /// 已过期。
    Expired,
    /// 已拒绝。
    Rejected,
}

// =============================================================================
// 券商适配器 1: CtpCloudBridge — 基于 openctp TTS 的条件单
// =============================================================================

/// openctp / CTP 条件单桥接。
///
/// 通过 openctp TTS REST API 提交条件单（止损/止盈）。
/// 参考 openctp td_demo.py + TTS HTTP API 规范。
///
/// CTP 条件单通过 OrderRef + 扩展字段实现：
/// - Stop: 触发价低于当前价时以市价卖出 / 触发价高于当前价时以市价买入
/// - Limit: 触发价达到时以限价委托
pub struct CtpCloudBridge {
    api_base_url: String,
    api_key: String,
    account_id: String,
    broker_id: String,
    client: reqwest::Client,
}

impl CtpCloudBridge {
    /// 创建 CTP 云桥接。
    ///
    /// `broker_id`: CTP 券商 ID（如 "9999" = 模拟仿真）。
    pub fn new(api_base_url: &str, api_key: &str, account_id: &str, broker_id: &str) -> Self {
        Self {
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            account_id: account_id.to_string(),
            broker_id: broker_id.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// 根据条件单类型确定 CTP 报单价格类型。
    /// 止损 → 市价单触发；止盈 → 限价单触发
    fn map_order_price_type(order_type: ConditionalOrderType) -> &'static str {
        match order_type {
            ConditionalOrderType::Stop => "1",    // 任意价（市价）
            ConditionalOrderType::Limit => "2",    // 限价
            _ => "2",                              // 默认限价
        }
    }

    /// 构建 CTP 格式的插入报单请求体。
    fn build_ctp_order(&self, order: &CloudConditionalOrder) -> serde_json::Value {
        serde_json::json!({
            "BrokerID": self.broker_id,
            "InvestorID": self.account_id,
            "InstrumentID": order.instrument,
            "OrderRef": order.cloud_order_id.as_deref().unwrap_or(""),
            "Direction": if order.direction.eq_ignore_ascii_case("buy") { "0" } else { "1" },
            "CombOffsetFlag": match order.offset.to_lowercase().as_str() {
                "open" => "0",
                "close" => "1",
                "closetoday" => "3",
                _ => "0",
            },
            "CombHedgeFlag": "1",
            "LimitPrice": order.limit_price,
            "VolumeTotalOriginal": order.volume,
            "OrderPriceType": Self::map_order_price_type(order.order_type),
            "TimeCondition": if order.ttl_secs == 0 { "1" } else { "2" }, // GFD/GTD
            "GTDDate": "",
            "VolumeCondition": "1",
            "MinVolume": 1,
            "ContingentCondition": match order.order_type {
                ConditionalOrderType::Stop => "3".to_string(),     // 止损
                ConditionalOrderType::Limit => "2".to_string(),    // 触价
                _ => "1".to_string(),                               // 立即
            },
            "StopPrice": order.trigger_price,
            "ForceCloseReason": "0",
            "IsAutoSuspend": 0,
            "UserID": self.account_id,
        })
    }
}

#[async_trait]
impl CloudConditionalBridge for CtpCloudBridge {
    async fn submit_conditional(&self, order: CloudConditionalOrder) -> Result<CloudConditionalResponse, String> {
        let ctp_order = self.build_ctp_order(&order);
        let url = format!("{}/tts/api/order_insert", self.api_base_url);

        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&ctp_order)
            .send()
            .await
            .map_err(|e| format!("CTP order insert HTTP failed: {}", e))?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("CTP order insert parse failed: {}", e))?;

        let order_ref = body["OrderRef"].as_str()
            .or_else(|| body["order_ref"].as_str())
            .unwrap_or("unknown")
            .to_string();

        if status.is_success() {
            Ok(CloudConditionalResponse {
                cloud_order_id: order_ref,
                success: true,
                error: None,
            })
        } else {
            let err_msg = body["ErrorMsg"].as_str()
                .or_else(|| body["error"].as_str())
                .unwrap_or("CTP order rejected")
                .to_string();
            Ok(CloudConditionalResponse {
                cloud_order_id: order_ref,
                success: false,
                error: Some(err_msg),
            })
        }
    }

    async fn cancel_conditional(&self, cloud_order_id: &str) -> Result<bool, String> {
        let url = format!("{}/tts/api/order_action", self.api_base_url);
        let cancel_req = serde_json::json!({
            "BrokerID": self.broker_id,
            "InvestorID": self.account_id,
            "OrderRef": cloud_order_id,
            "ActionFlag": "0", // 删除
        });

        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&cancel_req)
            .send()
            .await
            .map_err(|e| format!("CTP order cancel HTTP failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn query_conditional(&self, cloud_order_id: &str) -> Result<CloudConditionalStatus, String> {
        let url = format!("{}/tts/api/order_query", self.api_base_url);
        let qry_req = serde_json::json!({
            "BrokerID": self.broker_id,
            "InvestorID": self.account_id,
            "OrderRef": cloud_order_id,
        });

        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&qry_req)
            .send()
            .await
            .map_err(|e| format!("CTP order query HTTP failed: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("CTP order query parse failed: {}", e))?;

        let order_status = body["OrderStatus"].as_str().unwrap_or("0");
        let status = match order_status {
            "0" => ConditionalStatus::Submitted,    // AllTraded
            "1" => ConditionalStatus::PartiallyFilled,
            "2" => ConditionalStatus::Filled,
            "3" => ConditionalStatus::Cancelled,
            "4" => ConditionalStatus::Rejected,
            "5" => ConditionalStatus::Active,        // 未成交还在队列中
            "a" => ConditionalStatus::Expired,
            _ => ConditionalStatus::Submitted,
        };

        Ok(CloudConditionalStatus {
            cloud_order_id: cloud_order_id.to_string(),
            status,
            filled_volume: body["VolumeTraded"].as_u64().unwrap_or(0) as u32,
            filled_price: body["PriceTraded"].as_f64(),
            error: body["StatusMsg"].as_str().map(|s| s.to_string()),
        })
    }

    async fn check_connectivity(&self) -> Result<Duration, String> {
        let start = Instant::now();
        let url = format!("{}/tts/api/health", self.api_base_url);

        let response = self
            .client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("CTP API unreachable: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("CTP API returned status: {}", response.status()));
        }

        Ok(start.elapsed())
    }
}

// =============================================================================
// 券商适配器 2: JqCloudBridge — 基于掘金 (myquant) REST API
// =============================================================================

/// 掘金 myquant 条件单桥接。
///
/// 通过掘金 REST API 提交条件单。
/// 参考掘金 gm SDK: https://www.myquant.cn/docs/python/python_trade_api
pub struct JqCloudBridge {
    api_base_url: String,
    token: String,
    account_id: String,
    client: reqwest::Client,
}

impl JqCloudBridge {
    /// 创建掘金云桥接。
    ///
    /// `api_key`: 掘金 API Token（gm token）。
    pub fn new(api_base_url: &str, token: &str, account_id: &str) -> Self {
        Self {
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            account_id: account_id.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// 构建掘金格式的条件单请求体。
    fn build_jq_order(&self, order: &CloudConditionalOrder) -> serde_json::Value {
        // 掘金条件单参数
        // order_type: order_type 决定策略
        let strategy = match order.order_type {
            ConditionalOrderType::Stop => "stop_loss",
            ConditionalOrderType::Limit => "take_profit",
            ConditionalOrderType::TrailingStop => "trailing_stop",
            _ => "stop_loss",
        };

        serde_json::json!({
            "account_id": self.account_id,
            "symbol": order.instrument,
            "side": order.direction.to_lowercase(),
            "order_type": strategy,
            "volume": order.volume,
            "trigger_price": order.trigger_price,
            "order_price": if order.limit_price > 0.0 { serde_json::Value::from(order.limit_price) } else { serde_json::Value::Null },
            "offset": match order.offset.to_lowercase().as_str() {
                "open" => "open",
                "close" => "close",
                _ => "open",
            },
            "expire": if order.ttl_secs > 0 { order.ttl_secs } else { 86400 },
        })
    }
}

#[async_trait]
impl CloudConditionalBridge for JqCloudBridge {
    async fn submit_conditional(&self, order: CloudConditionalOrder) -> Result<CloudConditionalResponse, String> {
        let jq_order = self.build_jq_order(&order);
        let url = format!("{}/v1/conditional-orders", self.api_base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&jq_order)
            .send()
            .await
            .map_err(|e| format!("Jq order insert HTTP failed: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Jq order insert parse failed: {}", e))?;

        let order_id = body["order_id"].as_str()
            .or_else(|| body["id"].as_str())
            .unwrap_or("unknown")
            .to_string();
        let is_ok = body["status"].as_str().map(|s| s == "accepted" || s == "ok").unwrap_or(false);

        Ok(CloudConditionalResponse {
            cloud_order_id: order_id,
            success: is_ok,
            error: body["error"].as_str().map(|s| s.to_string()),
        })
    }

    async fn cancel_conditional(&self, cloud_order_id: &str) -> Result<bool, String> {
        let url = format!("{}/v1/conditional-orders/{}", self.api_base_url, cloud_order_id);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Jq order cancel HTTP failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn query_conditional(&self, cloud_order_id: &str) -> Result<CloudConditionalStatus, String> {
        let url = format!("{}/v1/conditional-orders/{}", self.api_base_url, cloud_order_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Jq order query HTTP failed: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Jq order query parse failed: {}", e))?;

        let jq_status = body["status"].as_str().unwrap_or("unknown");
        let status = match jq_status {
            "submitted" => ConditionalStatus::Submitted,
            "active" | "watching" => ConditionalStatus::Active,
            "triggered" => ConditionalStatus::Triggered,
            "partial_filled" => ConditionalStatus::PartiallyFilled,
            "filled" | "completed" => ConditionalStatus::Filled,
            "cancelled" | "canceled" => ConditionalStatus::Cancelled,
            "expired" => ConditionalStatus::Expired,
            "rejected" | "failed" => ConditionalStatus::Rejected,
            _ => ConditionalStatus::Submitted,
        };

        Ok(CloudConditionalStatus {
            cloud_order_id: cloud_order_id.to_string(),
            status,
            filled_volume: body["filled_volume"].as_u64().unwrap_or(0) as u32,
            filled_price: body["filled_avg_price"].as_f64().or_else(|| body["filled_price"].as_f64()),
            error: body["error"].as_str().map(|s| s.to_string()),
        })
    }

    async fn check_connectivity(&self) -> Result<Duration, String> {
        let start = Instant::now();
        let url = format!("{}/v1/health", self.api_base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Jq API unreachable: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Jq API returned status: {}", response.status()));
        }

        Ok(start.elapsed())
    }
}

// =============================================================================
// 券商适配器 3: DefaultCloudBridge — 通用 HTTP API（fallback）
// =============================================================================

/// 默认的云条件单桥接实现（通用 HTTP API）。
///
/// 通过 REST API 将条件单提交到通用云平台。
pub struct DefaultCloudBridge {
    api_base_url: String,
    api_key: String,
    account_id: String,
    client: reqwest::Client,
}

impl DefaultCloudBridge {
    /// 创建新的默认云桥接。
    pub fn new(api_base_url: &str, api_key: &str, account_id: &str) -> Self {
        Self {
            api_base_url: api_base_url.to_string(),
            api_key: api_key.to_string(),
            account_id: account_id.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

#[async_trait]
impl CloudConditionalBridge for DefaultCloudBridge {
    async fn submit_conditional(&self, order: CloudConditionalOrder) -> Result<CloudConditionalResponse, String> {
        if self.check_connectivity().await.is_err() {
            return Err("Cloud API not reachable".into());
        }

        let payload = serde_json::json!({
            "account_id": self.account_id,
            "instrument": order.instrument,
            "direction": order.direction,
            "offset": order.offset,
            "trigger_price": order.trigger_price,
            "limit_price": order.limit_price,
            "volume": order.volume,
            "order_type": serde_json::to_value(order.order_type).unwrap_or_default(),
            "ttl_secs": order.ttl_secs,
            "extra": order.extra,
        });

        let response = self
            .client
            .post(format!("{}/api/v1/conditional-orders", self.api_base_url))
            .header("X-API-Key", &self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(CloudConditionalResponse {
            cloud_order_id: body["order_id"].as_str().unwrap_or("unknown").to_string(),
            success: body["success"].as_bool().unwrap_or(false),
            error: body["error"].as_str().map(|s| s.to_string()),
        })
    }

    async fn cancel_conditional(&self, cloud_order_id: &str) -> Result<bool, String> {
        let response = self
            .client
            .delete(format!(
                "{}/api/v1/conditional-orders/{}",
                self.api_base_url, cloud_order_id
            ))
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        Ok(response.status().is_success())
    }

    async fn query_conditional(&self, cloud_order_id: &str) -> Result<CloudConditionalStatus, String> {
        let response = self
            .client
            .get(format!(
                "{}/api/v1/conditional-orders/{}",
                self.api_base_url, cloud_order_id
            ))
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let body: CloudConditionalStatus = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(body)
    }

    async fn check_connectivity(&self) -> Result<Duration, String> {
        let start = Instant::now();
        let response = self
            .client
            .get(format!("{}/api/v1/health", self.api_base_url))
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("Cloud API unreachable: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Cloud API returned status: {}", response.status()));
        }

        Ok(start.elapsed())
    }
}

// =============================================================================
// 自动切换逻辑
// =============================================================================

/// 执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// 本地执行（低延迟，正常模式）。
    Local,
    /// 云条件单执行（本地延迟过高时降级）。
    CloudConditional,
}

impl ExecutionMode {
    pub fn is_local(&self) -> bool {
        matches!(self, ExecutionMode::Local)
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self, ExecutionMode::CloudConditional)
    }
}

/// 降级策略配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationConfig {
    /// 延迟阈值（微秒）。超过此值触发云条件单降级。
    pub latency_threshold_us: u64,
    /// 恢复阈值（微秒）。延迟低于此值且持续恢复稳定期后切回本地。
    pub recovery_threshold_us: u64,
    /// 触发降级所需的连续超标次数。
    pub trigger_consecutive_count: u32,
    /// 切回本地前需要连续保持低延迟的检测次数（恢复稳定期）。
    pub recovery_stable_count: u32,
    /// 检测间隔（秒）。
    pub check_interval_secs: u64,
    /// 云 API 基础 URL。
    pub cloud_api_base_url: String,
    /// 云 API 密钥。
    pub cloud_api_key: String,
    /// 账户 ID。
    pub account_id: String,
}

impl Default for DegradationConfig {
    fn default() -> Self {
        Self {
            // 默认延迟阈值 500μs（0.5ms）
            latency_threshold_us: 500,
            // 默认恢复阈值 200μs（延迟降到 0.2ms 以下才切回）
            recovery_threshold_us: 200,
            // 连续 3 次超标触发降级
            trigger_consecutive_count: 3,
            // 连续 5 次达标才切回本地
            recovery_stable_count: 5,
            // 每 1 秒检测一次
            check_interval_secs: 1,
            // 以下需运行时配置
            cloud_api_base_url: String::new(),
            cloud_api_key: String::new(),
            account_id: String::new(),
        }
    }
}

/// 自动切换器 — 根据延迟数据和策略自动切换执行模式。
///
/// # 状态机
///
/// ```text
///              ┌──────────────────────────────┐
///              │                              │
///              ▼                              │
///   [Local] ──→ 延迟超标计数 ──→ [CloudConditional]
///              ↑                              │
///              │                              │
///              └── 恢复稳定期 ────────────────┘
/// ```
pub struct AutoSwitcher {
    /// 当前执行模式。
    mode: Arc<RwLock<ExecutionMode>>,
    /// 降级配置。
    config: DegradationConfig,
    /// 延迟监测器。
    monitor: Arc<LatencyMonitor>,
    /// 云桥接。
    cloud_bridge: Arc<dyn CloudConditionalBridge>,
    /// 连续超标计数。
    consecutive_high: Arc<AtomicU64>,
    /// 连续达标计数（用于恢复判定）。
    consecutive_low: Arc<AtomicU64>,
    /// 运行标志。
    running: Arc<AtomicBool>,
    /// 延迟快照广播。
    snapshot_tx: tokio::sync::broadcast::Sender<LatencySnapshot>,
    /// 模式变更广播。
    mode_change_tx: tokio::sync::broadcast::Sender<ExecutionMode>,
}

impl AutoSwitcher {
    /// 创建自动切换器。
    pub fn new(
        config: DegradationConfig,
        monitor: Arc<LatencyMonitor>,
        cloud_bridge: Arc<dyn CloudConditionalBridge>,
    ) -> Self {
        let (snapshot_tx, _) = tokio::sync::broadcast::channel(64);
        let (mode_change_tx, _) = tokio::sync::broadcast::channel(64);
        Self {
            mode: Arc::new(RwLock::new(ExecutionMode::Local)),
            config,
            monitor,
            cloud_bridge,
            consecutive_high: Arc::new(AtomicU64::new(0)),
            consecutive_low: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            snapshot_tx,
            mode_change_tx,
        }
    }

    /// 获取当前执行模式。
    pub async fn current_mode(&self) -> ExecutionMode {
        *self.mode.read().await
    }

    /// 获取运行状态。
    pub fn is_running(&self) -> bool {
        self.running.load(AtomicOrdering::Acquire)
    }

    /// 获取延迟快照接收端（用于监控展示）。
    pub fn subscribe_snapshot(&self) -> tokio::sync::broadcast::Receiver<LatencySnapshot> {
        self.snapshot_tx.subscribe()
    }

    /// 获取模式变更通知接收端。
    pub fn subscribe_mode_change(&self) -> tokio::sync::broadcast::Receiver<ExecutionMode> {
        self.mode_change_tx.subscribe()
    }

    /// 切换执行模式。
    async fn switch_mode(&self, new_mode: ExecutionMode) {
        let mut mode = self.mode.write().await;
        if *mode != new_mode {
            info!(
                "Execution mode switch: {:?} → {:?}",
                *mode, new_mode
            );
            *mode = new_mode;
            let _ = self.mode_change_tx.send(new_mode);
        }
    }

    /// 执行模式决策逻辑。
    async fn decide(&self) {
        let snapshot = self.monitor.snapshot().await;
        let _ = self.snapshot_tx.send(snapshot);

        let current_mode = *self.mode.read().await;
        let rtt = self.monitor.last_rtt_us();

        match current_mode {
            ExecutionMode::Local => {
                if rtt >= self.config.latency_threshold_us || rtt == u64::MAX {
                    let count = self.consecutive_high.fetch_add(1, AtomicOrdering::AcqRel) + 1;
                    self.consecutive_low.store(0, AtomicOrdering::Release);

                    if count >= self.config.trigger_consecutive_count as u64 {
                        // 检查云 API 连通性
                        match self.cloud_bridge.check_connectivity().await {
                            Ok(cloud_rtt) => {
                                info!(
                                    "Switching to cloud conditional (local={}μs, cloud={:?})",
                                    rtt, cloud_rtt
                                );
                                self.switch_mode(ExecutionMode::CloudConditional).await;
                                self.consecutive_high.store(0, AtomicOrdering::Release);
                            }
                            Err(e) => {
                                warn!("Cloud API unreachable, staying local: {}", e);
                                self.consecutive_high.store(0, AtomicOrdering::Release);
                            }
                        }
                    }
                } else {
                    self.consecutive_high.store(0, AtomicOrdering::Release);
                }
            }
            ExecutionMode::CloudConditional => {
                if rtt < self.config.recovery_threshold_us {
                    let count = self.consecutive_low.fetch_add(1, AtomicOrdering::AcqRel) + 1;
                    if count >= self.config.recovery_stable_count as u64 {
                        info!(
                            "Latency recovered ({}μs), switching back to local",
                            rtt
                        );
                        self.switch_mode(ExecutionMode::Local).await;
                        self.consecutive_low.store(0, AtomicOrdering::Release);
                    }
                } else {
                    self.consecutive_low.store(0, AtomicOrdering::Release);
                }
            }
        }
    }

    /// 启动自动切换循环（异步任务）。
    pub async fn run(&self, shutdown_rx: &mut mpsc::Receiver<()>) {
        self.running.store(true, AtomicOrdering::Release);
        info!(
            "AutoSwitcher started (threshold={}μs, recovery={}μs)",
            self.config.latency_threshold_us, self.config.recovery_threshold_us
        );

        let mut interval = tokio::time::interval(Duration::from_secs(self.config.check_interval_secs));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.decide().await;
                }
                _ = shutdown_rx.recv() => {
                    info!("AutoSwitcher shutdown");
                    self.running.store(false, AtomicOrdering::Release);
                    break;
                }
            }
        }
    }
}

// =============================================================================
// 集成辅助：降级订单路由
// =============================================================================

/// 将 OrderRequest 转换为 CloudConditionalOrder。
pub fn order_to_conditional(
    order: OrderRequest,
    order_type: ConditionalOrderType,
    trigger_price: f64,
) -> CloudConditionalOrder {
    CloudConditionalOrder {
        cloud_order_id: None,
        instrument: order.instrument,
        direction: match order.direction {
            crate::types::Direction::Buy => "Buy".to_string(),
            crate::types::Direction::Sell => "Sell".to_string(),
        },
        offset: match order.offset {
            crate::types::Offset::Open => "Open".to_string(),
            crate::types::Offset::Close => "Close".to_string(),
            crate::types::Offset::CloseToday => "CloseToday".to_string(),
        },
        trigger_price,
        limit_price: order.price,
        volume: order.volume,
        order_type,
        ttl_secs: 86400, // 默认当天有效
        extra: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ── 模拟桥接 ──

    struct MockCloudBridge {
        should_fail: bool,
        simulate_latency: Duration,
    }

    impl MockCloudBridge {
        fn new() -> Self {
            Self {
                should_fail: false,
                simulate_latency: Duration::from_micros(100),
            }
        }

        fn with_latency(latency: Duration) -> Self {
            Self {
                should_fail: false,
                simulate_latency: latency,
            }
        }

        fn failing() -> Self {
            Self {
                should_fail: true,
                simulate_latency: Duration::from_micros(100),
            }
        }
    }

    #[async_trait]
    impl CloudConditionalBridge for MockCloudBridge {
        async fn submit_conditional(&self, _order: CloudConditionalOrder) -> Result<CloudConditionalResponse, String> {
            if self.should_fail {
                return Err("Cloud API unavailable".into());
            }
            Ok(CloudConditionalResponse {
                cloud_order_id: format!("cloud-{}", uuid::Uuid::new_v4()),
                success: true,
                error: None,
            })
        }

        async fn cancel_conditional(&self, _cloud_order_id: &str) -> Result<bool, String> {
            Ok(!self.should_fail)
        }

        async fn query_conditional(&self, _cloud_order_id: &str) -> Result<CloudConditionalStatus, String> {
            Ok(CloudConditionalStatus {
                cloud_order_id: "mock".into(),
                status: ConditionalStatus::Active,
                filled_volume: 0,
                filled_price: None,
                error: None,
            })
        }

        async fn check_connectivity(&self) -> Result<Duration, String> {
            if self.should_fail {
                Err("Mock: API unavailable".into())
            } else {
                Ok(self.simulate_latency)
            }
        }
    }

    // ── LatencyMonitor tests ──

    #[test]
    fn test_latency_monitor_measure() {
        let monitor = LatencyMonitor::new(
            Box::new(|| Ok(Duration::from_micros(150))),
            Duration::from_secs(1),
            10,
        );

        let us = monitor.measure_once();
        assert_eq!(us, 150);
        assert_eq!(monitor.last_rtt_us(), 150);
    }

    #[test]
    fn test_latency_monitor_measure_failure_sets_high() {
        let monitor = LatencyMonitor::new(
            Box::new(|| Err("timeout".into())),
            Duration::from_secs(1),
            10,
        );

        let us = monitor.measure_once();
        assert_eq!(us, u64::MAX);
        assert_eq!(monitor.last_rtt_us(), u64::MAX);
    }

    #[tokio::test]
    async fn test_latency_monitor_snapshot() {
        let monitor = LatencyMonitor::new(
            Box::new(|| Ok(Duration::from_micros(200))),
            Duration::from_secs(1),
            10,
        );

        monitor.measure_once();
        monitor.measure_once();
        monitor.measure_once();

        let snapshot = monitor.snapshot().await;
        assert_eq!(snapshot.last_rtt_us, 200);
        assert!((snapshot.avg_rtt_us - 200.0).abs() < 0.1);
        assert_eq!(snapshot.max_rtt_us, 200);
        assert_eq!(snapshot.sample_count, 3);
    }

    // ── CloudBridge tests ──

    #[tokio::test]
    async fn test_mock_cloud_bridge_submit_success() {
        let bridge = MockCloudBridge::new();
        let order = CloudConditionalOrder {
            cloud_order_id: None,
            instrument: "ag2506".into(),
            direction: "Buy".into(),
            offset: "Open".into(),
            trigger_price: 5600.0,
            limit_price: 5625.0,
            volume: 2,
            order_type: ConditionalOrderType::Stop,
            ttl_secs: 86400,
            extra: std::collections::HashMap::new(),
        };

        let response = bridge.submit_conditional(order).await.unwrap();
        assert!(response.success);
        assert!(response.cloud_order_id.starts_with("cloud-"));
    }

    #[tokio::test]
    async fn test_mock_cloud_bridge_connectivity_check() {
        let bridge = MockCloudBridge::new();
        let rtt = bridge.check_connectivity().await.unwrap();
        assert!(rtt > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_mock_cloud_bridge_failure() {
        let bridge = MockCloudBridge::failing();
        let result = bridge.check_connectivity().await;
        assert!(result.is_err());
    }

    // ── AutoSwitcher tests ──

    #[tokio::test]
    async fn test_auto_switcher_starts_in_local_mode() {
        let monitor = Arc::new(LatencyMonitor::new(
            Box::new(|| Ok(Duration::from_micros(50))),
            Duration::from_secs(1),
            5,
        ));

        let bridge = Arc::new(MockCloudBridge::new());

        let config = DegradationConfig {
            latency_threshold_us: 500,
            recovery_threshold_us: 200,
            trigger_consecutive_count: 2,
            recovery_stable_count: 3,
            check_interval_secs: 1,
            ..Default::default()
        };

        let switcher = AutoSwitcher::new(config, monitor, bridge);
        let mode = switcher.current_mode().await;
        assert_eq!(mode, ExecutionMode::Local);
    }

    #[test]
    fn test_order_to_conditional_conversion() {
        let order = OrderRequest {
            order_id: "exec-000001".into(),
            instrument: "ag2506".into(),
            direction: crate::types::Direction::Buy,
            offset: crate::types::Offset::Open,
            price: 5625.0,
            volume: 2,
            order_type: crate::types::OrderType::Limit,
        };

        let conditional = order_to_conditional(order, ConditionalOrderType::Stop, 5600.0);
        assert_eq!(conditional.instrument, "ag2506");
        assert_eq!(conditional.direction, "Buy");
        assert_eq!(conditional.trigger_price, 5600.0);
        assert_eq!(conditional.limit_price, 5625.0);
        assert_eq!(conditional.volume, 2);
    }

    #[test]
    fn test_execution_mode_helpers() {
        assert!(ExecutionMode::Local.is_local());
        assert!(!ExecutionMode::Local.is_cloud());
        assert!(ExecutionMode::CloudConditional.is_cloud());
        assert!(!ExecutionMode::CloudConditional.is_local());
    }

    #[test]
    fn test_conditional_order_type_serde() {
        let types = vec![
            ConditionalOrderType::Stop,
            ConditionalOrderType::Limit,
            ConditionalOrderType::Oto,
            ConditionalOrderType::Oco,
            ConditionalOrderType::TrailingStop,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let back: ConditionalOrderType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, back);
        }
    }

    #[test]
    fn test_degradation_config_default() {
        let config = DegradationConfig::default();
        assert_eq!(config.latency_threshold_us, 500);
        assert_eq!(config.recovery_threshold_us, 200);
        assert_eq!(config.trigger_consecutive_count, 3);
        assert_eq!(config.recovery_stable_count, 5);
    }

    #[tokio::test]
    async fn test_auto_switcher_subscribe_snapshot() {
        let monitor = Arc::new(LatencyMonitor::new(
            Box::new(|| Ok(Duration::from_micros(100))),
            Duration::from_secs(1),
            5,
        ));

        let bridge = Arc::new(MockCloudBridge::new());

        let config = DegradationConfig {
            latency_threshold_us: 1000,
            ..Default::default()
        };

        let switcher = AutoSwitcher::new(config, monitor, bridge);
        let rx = switcher.subscribe_snapshot();
        drop(rx);
    }

    // ── CtpCloudBridge tests ──

    #[test]
    fn test_ctp_bridge_order_price_type_mapping() {
        assert_eq!(CtpCloudBridge::map_order_price_type(ConditionalOrderType::Stop), "1");
        assert_eq!(CtpCloudBridge::map_order_price_type(ConditionalOrderType::Limit), "2");
    }

    #[test]
    fn test_ctp_bridge_build_ctp_order_stop() {
        let bridge = CtpCloudBridge::new("http://localhost:8080", "test-key", "test-acc", "9999");
        let order = CloudConditionalOrder {
            cloud_order_id: Some("ref-001".into()),
            instrument: "ag2506".into(),
            direction: "Buy".into(),
            offset: "Open".into(),
            trigger_price: 5600.0,
            limit_price: 0.0,
            volume: 2,
            order_type: ConditionalOrderType::Stop,
            ttl_secs: 0,
            extra: std::collections::HashMap::new(),
        };
        let ctp = bridge.build_ctp_order(&order);
        assert_eq!(ctp["InstrumentID"], "ag2506");
        assert_eq!(ctp["Direction"], "0"); // Buy
        assert_eq!(ctp["OrderPriceType"], "1"); // 市价
        assert_eq!(ctp["ContingentCondition"], "3"); // 止损
        assert_eq!(ctp["StopPrice"], 5600.0);
        assert_eq!(ctp["VolumeTotalOriginal"], 2);
    }

    #[test]
    fn test_ctp_bridge_build_ctp_order_limit() {
        let bridge = CtpCloudBridge::new("http://localhost:8080", "test-key", "test-acc", "9999");
        let order = CloudConditionalOrder {
            cloud_order_id: None,
            instrument: "rb2501".into(),
            direction: "Sell".into(),
            offset: "Close".into(),
            trigger_price: 3800.0,
            limit_price: 3780.0,
            volume: 5,
            order_type: ConditionalOrderType::Limit,
            ttl_secs: 86400,
            extra: std::collections::HashMap::new(),
        };
        let ctp = bridge.build_ctp_order(&order);
        assert_eq!(ctp["InstrumentID"], "rb2501");
        assert_eq!(ctp["Direction"], "1"); // Sell
        assert_eq!(ctp["CombOffsetFlag"], "1"); // Close
        assert_eq!(ctp["OrderPriceType"], "2"); // 限价
        assert_eq!(ctp["ContingentCondition"], "2"); // 触价
        assert_eq!(ctp["StopPrice"], 3800.0);
        assert_eq!(ctp["LimitPrice"], 3780.0);
    }

    // ── JqCloudBridge tests ──

    #[test]
    fn test_jq_bridge_build_jq_order_stop() {
        let bridge = JqCloudBridge::new("http://localhost:8080", "test-token", "test-acc");
        let order = CloudConditionalOrder {
            cloud_order_id: None,
            instrument: "ag2506".into(),
            direction: "Buy".into(),
            offset: "Open".into(),
            trigger_price: 5600.0,
            limit_price: 0.0,
            volume: 2,
            order_type: ConditionalOrderType::Stop,
            ttl_secs: 0,
            extra: std::collections::HashMap::new(),
        };
        let jq = bridge.build_jq_order(&order);
        assert_eq!(jq["symbol"], "ag2506");
        assert_eq!(jq["side"], "buy");
        assert_eq!(jq["order_type"], "stop_loss");
        assert_eq!(jq["trigger_price"], 5600.0);
        assert_eq!(jq["volume"], 2);
    }

    #[test]
    fn test_jq_bridge_build_jq_order_take_profit() {
        let bridge = JqCloudBridge::new("http://localhost:8080", "test-token", "test-acc");
        let order = CloudConditionalOrder {
            cloud_order_id: None,
            instrument: "rb2501".into(),
            direction: "Sell".into(),
            offset: "Close".into(),
            trigger_price: 3800.0,
            limit_price: 3780.0,
            volume: 5,
            order_type: ConditionalOrderType::Limit,
            ttl_secs: 86400,
            extra: std::collections::HashMap::new(),
        };
        let jq = bridge.build_jq_order(&order);
        assert_eq!(jq["order_type"], "take_profit");
        assert_eq!(jq["order_price"], 3780.0);
        assert_eq!(jq["side"], "sell");
    }

    // ── CloudBrokerKind + factory tests ──

    #[test]
    fn test_cloud_broker_kind_serde() {
        for kind in &[CloudBrokerKind::Ctp, CloudBrokerKind::Jq, CloudBrokerKind::Default] {
            let json = serde_json::to_string(kind).unwrap();
            let back: CloudBrokerKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*kind, back);
        }
    }

    #[tokio::test]
    async fn test_create_ctp_bridge() {
        let bridge = create_cloud_bridge(
            CloudBrokerKind::Ctp,
            "http://localhost:8080",
            "key",
            "acc",
            "9999",
        );
        // 验证桥接创建不崩溃（预期失败，因无真实服务）
        let result = bridge.check_connectivity().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_jq_bridge() {
        let bridge = create_cloud_bridge(
            CloudBrokerKind::Jq,
            "http://localhost:8080",
            "token",
            "acc",
            "",
        );
        let result = bridge.check_connectivity().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_default_bridge() {
        let bridge = create_cloud_bridge(
            CloudBrokerKind::Default,
            "http://localhost:8080",
            "key",
            "acc",
            "",
        );
        let result = bridge.check_connectivity().await;
        assert!(result.is_err());
    }
}
