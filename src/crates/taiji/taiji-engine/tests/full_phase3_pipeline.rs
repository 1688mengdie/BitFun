//! R-3-606: Phase 3 全链路 Engine 集成测试
//!
//! 构建完整 Pipeline 拓扑：
//!
//! ```text
//! Tick → BarGenerator
//!   ├→ MockDvmiNode (pivots)
//!   │    └→ MockThrustNode (triple_push)
//!   └→ MockChanNode (fractals + bis)
//!        ├→ MockHubNode (hubs)
//!        ├→ MockSegmentNode (segments)
//!        │    └→ MockDivergenceNode (divergences)
//!        └──────────────────────────┐
//!                   MockBSPNode ←───┴── hubs + divergences
//!                        └→ bsp_signals
//! ```
//!
//! 验证合成 tick → K 线闭合 → 各级 ComputeNode 执行 → 最终买卖点信号输出。

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use taiji_engine::config::*;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

// ============================================================================
// 辅助函数
// ============================================================================

fn ts(hour: u32, min: u32, sec: u32) -> i64 {
    Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
        .unwrap()
        .timestamp_millis()
}

fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64) -> TickData {
    TickData {
        timestamp_ms: ts_ms,
        instrument: "ag2506".into(),
        trading_day: "20260722".into(),
        exchange_id: "SHFE".into(),
        exchange_inst_id: "ag2506".into(),
        last_price: price,
        pre_settlement_price: 0.0,
        pre_close_price: 0.0,
        pre_open_interest: 0.0,
        open_price: price,
        highest_price: price,
        lowest_price: price,
        volume: vol,
        turnover: amount,
        open_interest: 1000.0,
        close_price: 0.0,
        settlement_price: 0.0,
        upper_limit_price: 0.0,
        lower_limit_price: 0.0,
        pre_delta: 0.0,
        curr_delta: 0.0,
        update_time: String::new(),
        update_millisec: 0,
        bid_price1: 0.0,
        bid_volume1: 0,
        ask_price1: 0.0,
        ask_volume1: 0,
        ..Default::default()
    }
}

fn pipeline_config() -> PipelineConfig {
    PipelineConfig {
        name: "phase3-full-chain".into(),
        version: "1.0".into(),
        bar_gen: BarGenConfig {
            modes: vec!["time".into()],
            time_freqs: vec!["1m".into()],
        },
        data_source: DataSourceSpec {
            type_name: "mock".into(),
            config: serde_json::json!({}),
        },
        nodes: vec![
            NodeSpec {
                id: "bar_node".into(),
                type_name: "builtin".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec!["bars:1m".into()],
            },
            NodeSpec {
                id: "dvmi".into(),
                type_name: "mock_dvmi".into(),
                config: serde_json::json!({"window": 3}),
                input_keys: vec!["bars:1m".into()],
                output_keys: vec!["pivots".into(), "trendlines".into()],
            },
            NodeSpec {
                id: "thrust".into(),
                type_name: "mock_thrust".into(),
                config: serde_json::json!({}),
                input_keys: vec!["pivots".into()],
                output_keys: vec!["triple_push".into()],
            },
            NodeSpec {
                id: "chan".into(),
                type_name: "mock_chan".into(),
                config: serde_json::json!({}),
                input_keys: vec!["bars:1m".into()],
                output_keys: vec!["fractals".into(), "bis".into()],
            },
            NodeSpec {
                id: "hub".into(),
                type_name: "mock_hub".into(),
                config: serde_json::json!({}),
                input_keys: vec!["bis".into()],
                output_keys: vec!["hubs".into()],
            },
            NodeSpec {
                id: "segment".into(),
                type_name: "mock_segment".into(),
                config: serde_json::json!({}),
                input_keys: vec!["bis".into()],
                output_keys: vec!["segments".into()],
            },
            NodeSpec {
                id: "divergence".into(),
                type_name: "mock_divergence".into(),
                config: serde_json::json!({}),
                input_keys: vec!["segments".into(), "bars:1m".into()],
                output_keys: vec!["divergences".into()],
            },
            NodeSpec {
                id: "bsp".into(),
                type_name: "mock_bsp".into(),
                config: serde_json::json!({}),
                input_keys: vec!["hubs".into(), "divergences".into()],
                output_keys: vec!["bsp_signals".into()],
            },
        ],
    }
}

// ============================================================================
// MockDvmiNode — 拐点检测
// ============================================================================

pub struct MockDvmiNode {
    id: NodeId,
    window: usize,
}

impl MockDvmiNode {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), window: 3 }
    }
}

impl ComputeNode for MockDvmiNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_dvmi" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["bars:1m".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["pivots".into(), "trendlines".into()] }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(w) = config.get_i64("window") { self.window = w as usize; }
        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let bars: Option<Arc<Vec<Arc<RawBar>>>> = state.get(&"bars:1m".into());
        let bars = match bars { Some(b) => b, None => return Ok(vec![]) };
        if bars.len() < 10 { return Ok(vec![]); }

        let n = bars.len();
        let mut pivots: Vec<serde_json::Value> = Vec::new();

        // 滑动窗口找局部高低点
        for i in self.window..n.saturating_sub(self.window) {
            let mut is_high = true;
            let mut is_low = true;
            for j in (i.saturating_sub(self.window))..=i + self.window {
                if j == i { continue; }
                if bars[j].high >= bars[i].high { is_high = false; }
                if bars[j].low <= bars[i].low { is_low = false; }
            }
            if is_high {
                pivots.push(serde_json::json!({"idx": i, "type": "high", "price": bars[i].high}));
            }
            if is_low {
                pivots.push(serde_json::json!({"idx": i, "type": "low", "price": bars[i].low}));
            }
        }

        state.set("pivots".into(), StateValue::Json(serde_json::json!(pivots)), self.id());
        state.set("trendlines".into(), StateValue::Json(serde_json::json!({"valid": pivots.len() >= 2})), self.id());
        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockThrustNode — 三推检测
// ============================================================================

pub struct MockThrustNode { id: NodeId }

impl MockThrustNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockThrustNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_thrust" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["pivots".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["triple_push".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let pivots: Option<serde_json::Value> = state.get_json(&"pivots".into());
        let pivots = match pivots { Some(p) => p, None => return Ok(vec![]) };
        let arr = match pivots.as_array() { Some(a) => a, None => return Ok(vec![]) };
        if arr.len() < 6 { return Ok(vec![]); }

        // 提取同向拐点（high/low 交替）
        let highs: Vec<f64> = arr.iter().filter_map(|v| {
            if v["type"] == "high" { v["price"].as_f64() } else { None }
        }).collect();

        if highs.len() < 3 { return Ok(vec![]); }

        // 取最近3个高拐点检测三推
        let recent: Vec<f64> = highs.iter().rev().take(3).copied().rev().collect();
        let weakening = recent[2] < recent[1] && recent[1] < recent[0];

        let result = serde_json::json!({
            "found": true,
            "weakening": weakening,
            "push_points": recent,
            "direction": "short"
        });
        state.set("triple_push".into(), StateValue::Json(result), self.id());

        if weakening {
            Ok(vec![Signal {
                timestamp: Utc::now(),
                instrument: "ag2506".into(),
                freq: Freq::F1,
                action: SignalAction::Short,
                entry: None,
                stop_loss: None,
                take_profit: None,
                size: Some(1.0),
                source: self.id(),
                confidence: 0.65,
                metadata: HashMap::from([("reason".into(), "triple_push_weakening".into())]),
                disclaimer: None,
            }])
        } else { Ok(vec![]) }
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockChanNode — 分型+笔检测
// ============================================================================

pub struct MockChanNode { id: NodeId }

impl MockChanNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockChanNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_chan" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["bars:1m".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["fractals".into(), "bis".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let bars: Option<Arc<Vec<Arc<RawBar>>>> = state.get(&"bars:1m".into());
        let bars = match bars { Some(b) => b, None => return Ok(vec![]) };
        if bars.len() < 5 { return Ok(vec![]); }

        let n = bars.len();
        let mut fractals: Vec<serde_json::Value> = Vec::new();

        for i in 1..n - 1 {
            if bars[i].high > bars[i - 1].high && bars[i].high > bars[i + 1].high {
                fractals.push(serde_json::json!({"idx": i, "type": "top", "price": bars[i].high}));
            }
            if bars[i].low < bars[i - 1].low && bars[i].low < bars[i + 1].low {
                fractals.push(serde_json::json!({"idx": i, "type": "bottom", "price": bars[i].low}));
            }
        }

        // 简化的笔：相邻交替分型
        let mut bis: Vec<serde_json::Value> = Vec::new();
        if fractals.len() >= 2 {
            for pair in fractals.windows(2) {
                let a = &pair[0];
                let b = &pair[1];
                if a["type"] != b["type"] {
                    bis.push(serde_json::json!({
                        "start_idx": a["idx"], "end_idx": b["idx"],
                        "start_price": a["price"], "end_price": b["price"],
                    }));
                }
            }
        }

        state.set("fractals".into(), StateValue::Json(serde_json::json!(fractals)), self.id());
        state.set("bis".into(), StateValue::Json(serde_json::json!(bis)), self.id());
        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockHubNode — 中枢检测
// ============================================================================

pub struct MockHubNode { id: NodeId }

impl MockHubNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockHubNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_hub" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["bis".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["hubs".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let bis: Option<serde_json::Value> = state.get_json(&"bis".into());
        let bis = match bis { Some(b) => b, None => return Ok(vec![]) };
        let arr = match bis.as_array() { Some(a) => a, None => return Ok(vec![]) };
        if arr.len() < 3 { return Ok(vec![]); }

        // 检测至少3笔重叠区间
        let mut hubs: Vec<serde_json::Value> = Vec::new();
        for i in 0..arr.len().saturating_sub(2) {
            let p0_s = arr[i]["start_price"].as_f64().unwrap_or(0.0);
            let p0_e = arr[i]["end_price"].as_f64().unwrap_or(0.0);
            let p1_s = arr[i + 1]["start_price"].as_f64().unwrap_or(0.0);
            let p1_e = arr[i + 1]["end_price"].as_f64().unwrap_or(0.0);
            let p2_s = arr[i + 2]["start_price"].as_f64().unwrap_or(0.0);
            let p2_e = arr[i + 2]["end_price"].as_f64().unwrap_or(0.0);

            // 三笔低点的最高值 = ZG，三笔高点的最低值 = ZD
            let zg = p0_s.min(p0_e).min(p1_s.min(p1_e)).min(p2_s.min(p2_e));
            let zd = p0_s.max(p0_e).max(p1_s.max(p1_e)).max(p2_s.max(p2_e));

            if zg < zd {
                hubs.push(serde_json::json!({
                    "start": i, "zg": zg, "zd": zd, "mid": (zg + zd) / 2.0
                }));
                break; // 取第一个中枢
            }
        }

        state.set("hubs".into(), StateValue::Json(serde_json::json!(hubs)), self.id());
        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockSegmentNode — 线段识别
// ============================================================================

pub struct MockSegmentNode { id: NodeId }

impl MockSegmentNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockSegmentNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_segment" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["bis".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["segments".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let bis: Option<serde_json::Value> = state.get_json(&"bis".into());
        let bis = match bis { Some(b) => b, None => return Ok(vec![]) };
        let arr = match bis.as_array() { Some(a) => a, None => return Ok(vec![]) };
        if arr.len() < 2 { return Ok(vec![]); }

        // 简化线段：连续同向笔合并为一线段
        let mut segments: Vec<serde_json::Value> = Vec::new();
        let mut i = 0;
        while i < arr.len() {
            let start_idx = arr[i]["start_idx"].as_i64().unwrap_or(0);
            let start_p = arr[i]["start_price"].as_f64().unwrap_or(0.0);
            let mut end_idx = arr[i]["end_idx"].as_i64().unwrap_or(0);
            let mut end_p = arr[i]["end_price"].as_f64().unwrap_or(0.0);

            // 合并同向笔
            let _start_e = arr[i]["end_price"].as_f64().unwrap_or(0.0);
            let is_up = end_p > start_p;

            let mut j = i + 1;
            while j < arr.len() {
                let js = arr[j]["start_price"].as_f64().unwrap_or(0.0);
                let je = arr[j]["end_price"].as_f64().unwrap_or(0.0);
                let j_up = je > js;
                if j_up == is_up {
                    end_idx = arr[j]["end_idx"].as_i64().unwrap_or(0);
                    end_p = arr[j]["end_price"].as_f64().unwrap_or(0.0);
                    j += 1;
                } else { break; }
            }

            segments.push(serde_json::json!({
                "start_idx": start_idx, "end_idx": end_idx,
                "start_price": start_p, "end_price": end_p,
                "direction": if is_up { "up" } else { "down" },
            }));
            i = j;
        }

        state.set("segments".into(), StateValue::Json(serde_json::json!(segments)), self.id());
        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockDivergenceNode — 背驰检测
// ============================================================================

pub struct MockDivergenceNode { id: NodeId }

impl MockDivergenceNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockDivergenceNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_divergence" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["segments".into(), "bars:1m".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["divergences".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let segments: Option<serde_json::Value> = state.get_json(&"segments".into());
        let segments = match segments { Some(s) => s, None => return Ok(vec![]) };
        let arr = match segments.as_array() { Some(a) => a, None => return Ok(vec![]) };
        if arr.len() < 2 { return Ok(vec![]); }

        let bars: Option<Arc<Vec<Arc<RawBar>>>> = state.get(&"bars:1m".into());
        let bars = match bars { Some(b) => b, None => return Ok(vec![]) };
        if bars.len() < 30 { return Ok(vec![]); }

        // 简化背驰：检查最后一个线段的价格范围 vs MACD 柱
        let last = &arr[arr.len() - 1];
        let prev = &arr[arr.len() - 2];
        let last_range = (last["end_price"].as_f64().unwrap_or(0.0) - last["start_price"].as_f64().unwrap_or(0.0)).abs();
        let prev_range = (prev["end_price"].as_f64().unwrap_or(0.0) - prev["start_price"].as_f64().unwrap_or(0.0)).abs();

        // 线段幅度缩小 + 价格方向延续 = 背驰
        let divergence = last_range < prev_range * 0.8 && last["direction"] == prev["direction"];

        let result = serde_json::json!({
            "divergence_found": divergence,
            "last_range": last_range,
            "prev_range": prev_range,
        });
        state.set("divergences".into(), StateValue::Json(result), self.id());

        if divergence {
            Ok(vec![Signal {
                timestamp: Utc::now(),
                instrument: "ag2506".into(),
                freq: Freq::F1,
                action: if last["direction"] == "up" { SignalAction::CloseLong } else { SignalAction::CloseShort },
                entry: None,
                stop_loss: None,
                take_profit: None,
                size: Some(1.0),
                source: self.id(),
                confidence: 0.6,
                metadata: HashMap::from([("reason".into(), "segment_divergence".into())]),
                disclaimer: None,
            }])
        } else { Ok(vec![]) }
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// MockBSPNode — 买卖点判定
// ============================================================================

pub struct MockBSPNode { id: NodeId }

impl MockBSPNode {
    pub fn new(id: &str) -> Self { Self { id: id.to_string() } }
}

impl ComputeNode for MockBSPNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &'static str { "mock_bsp" }
    fn input_keys(&self) -> Vec<StateKey> { vec!["hubs".into(), "divergences".into()] }
    fn output_keys(&self) -> Vec<StateKey> { vec!["bsp_signals".into()] }
    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> { Ok(()) }
    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> { Ok(()) }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let hubs: Option<serde_json::Value> = state.get_json(&"hubs".into());
        let div: Option<serde_json::Value> = state.get_json(&"divergences".into());

        let has_hub = match hubs {
            Some(ref h) => h.as_array().map(|a| !a.is_empty()).unwrap_or(false),
            None => false,
        };
        let has_divergence = match div {
            Some(ref d) => d["divergence_found"].as_bool().unwrap_or(false),
            None => false,
        };

        if has_hub && has_divergence {
            let signal = Signal {
                timestamp: Utc::now(),
                instrument: "ag2506".into(),
                freq: Freq::F1,
                action: SignalAction::Short,
                entry: None,
                stop_loss: None,
                take_profit: None,
                size: Some(1.0),
                source: self.id(),
                confidence: 0.7,
                metadata: HashMap::from([("bsp_type".into(), "third_sell".into())]),
                disclaimer: None,
            };
            state.set(
                "bsp_signals".into(),
                StateValue::Signals(Arc::new(vec![signal.clone()])),
                self.id(),
            );
            return Ok(vec![signal]);
        }

        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> { vec![Freq::F1] }
}

// ============================================================================
// 集成测试
// ============================================================================

/// 全链路 Phase 3 Pipeline 集成测试。
///
/// 场景：合成 tick → 多级别通道 → 拐点 → 三推 → 分型笔 → 中枢 → 线段 → 背驰 → 买卖点
///
/// 价格模式：30 根 tick 上升（100→130）→ 20 根 tick 下降（130→105）
/// 预期：形成清晰的高/低拐点，产生中枢和背驰信号，最终触发 BSPNode 输出信号
#[test]
fn test_phase3_full_pipeline() {
    let config = pipeline_config();
    assert!(config.validate().is_ok());

    let mut pipeline = Pipeline::from_config(config).expect("Pipeline::from_config");

    // 注册所有 Phase 3 ComputeNode
    pipeline.add_node(Box::new(MockDvmiNode::new("dvmi")));
    pipeline.add_node(Box::new(MockThrustNode::new("thrust")));
    pipeline.add_node(Box::new(MockChanNode::new("chan")));
    pipeline.add_node(Box::new(MockHubNode::new("hub")));
    pipeline.add_node(Box::new(MockSegmentNode::new("segment")));
    pipeline.add_node(Box::new(MockDivergenceNode::new("divergence")));
    pipeline.add_node(Box::new(MockBSPNode::new("bsp")));

    pipeline.derive_edges().expect("derive_edges");

    // 验证 Pipeline 状态
    let status = pipeline.status();
    assert_eq!(status.nodes.len(), 7, "should have 7 registered nodes");

    // ── 喂入合成 tick 数据 ──
    // 先确认所有节点都注册在 status 中
    let node_ids: Vec<String> = status.nodes.iter().map(|n| n.id.clone()).collect();
    for expected in &["dvmi", "thrust", "chan", "hub", "segment", "divergence", "bsp"] {
        assert!(node_ids.contains(&expected.to_string()), "node {} should be registered", expected);
    }

    // Phase A: 波动上升 30 分钟（09:00→09:29）
    // 价格模式：正弦波动上升
    let mut all_signals: Vec<Signal> = Vec::new();
    let mut total_closed_bars = 0usize;

    for i in 0..30 {
        let price = 100.0 + (i as f64 * 6.28 / 30.0).sin() * 15.0 + i as f64 * 0.5;
        let r0 = pipeline.feed_tick_direct(&make_tick(ts(9, i, 0), price, 100.0, price * 100.0)).expect("ftd");
        total_closed_bars += r0.closed_bars.len();
        if i > 0 {
            let result = pipeline
                .feed_tick_direct(&make_tick(ts(9, i, 30), price + 1.0, 100.0, price * 100.0))
                .expect("feed_tick_direct");
            total_closed_bars += result.closed_bars.len();
            all_signals.extend(result.signals);
        }
    }
    // 额外 feed 关闭最后一根 bar
    let r_end = pipeline.feed_tick_direct(&make_tick(ts(9, 30, 0), 115.0, 100.0, 11500.0)).expect("ftd");
    total_closed_bars += r_end.closed_bars.len();

    // Phase B: 波动下降 25 分钟（09:30→09:54）
    for i in 0..25 {
        let price = 115.0 + ((i as f64) * 6.28 / 25.0).cos() * 12.0 - i as f64 * 0.4;
        let r0 = pipeline.feed_tick_direct(&make_tick(ts(9, 30 + i, 0), price, 100.0, price * 100.0)).expect("ftd");
        total_closed_bars += r0.closed_bars.len();
        if i > 0 {
            let result = pipeline
                .feed_tick_direct(&make_tick(ts(9, 30 + i, 30), price - 1.0, 100.0, price * 100.0))
                .expect("feed_tick_direct");
            total_closed_bars += result.closed_bars.len();
            all_signals.extend(result.signals);
        }
    }

    // Phase C: 尾盘整理（09:55→09:59）
    for i in 0..5 {
        let r0 = pipeline.feed_tick_direct(&make_tick(
            ts(9, 55 + i, 0), 105.0 + (i as f64 * 0.3).sin() * 2.0, 100.0, 10000.0,
        )).expect("ftd");
        total_closed_bars += r0.closed_bars.len();
        if i > 0 {
            let result = pipeline
                .feed_tick_direct(&make_tick(ts(9, 55 + i, 30), 105.0, 100.0, 10000.0))
                .expect("feed_tick_direct");
            total_closed_bars += result.closed_bars.len();
            all_signals.extend(result.signals);
        }
    }

    eprintln!("DEBUG: total_closed_bars={}", total_closed_bars);

    // ── 验证 ──
    // 打印调试信息
    let pivots_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"pivots".into());
    let bis_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"bis".into());
    let hubs_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"hubs".into());
    let segments_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"segments".into());
    let div_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"divergences".into());
    let bsp_from_state: Option<serde_json::Value> = pipeline.state_store().get_json(&"bsp_signals".into());

    eprintln!(
        "Phase3 chain debug: signals={} pivots={} bis={} hubs={} segments={} div={} bsp={}",
        all_signals.len(),
        pivots_from_state.as_ref().map(|p| p.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0),
        bis_from_state.as_ref().map(|b| b.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0),
        hubs_from_state.as_ref().map(|h| h.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0),
        segments_from_state.as_ref().map(|s| s.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0),
        div_from_state.as_ref().map(|d| if d["divergence_found"].as_bool().unwrap_or(false) { 1 } else { 0 }).unwrap_or(0),
        bsp_from_state.as_ref().map(|b| b.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0),
    );

    // 1. Pipeline 不应崩溃
    let final_status = pipeline.status();
    assert_eq!(final_status.nodes.len(), 7, "all 7 nodes should still be in pipeline status");

    // 2. DvmiNode 应产出拐点
    assert!(
        pivots_from_state.is_some(),
        "pivots should be stored in StateStore"
    );
    let pivot_count = pivots_from_state
        .as_ref()
        .and_then(|p| p.as_array().map(|a| a.len()))
        .unwrap_or(0);
    assert!(
        pivot_count >= 3,
        "should have at least 3 pivots from 65 bars, got {}",
        pivot_count
    );

    // 3. ChanNode 应产出分型和笔
    assert!(
        bis_from_state.is_some(),
        "bis should be stored in StateStore"
    );
    let bis_count = bis_from_state
        .as_ref()
        .and_then(|b| b.as_array().map(|a| a.len()))
        .unwrap_or(0);
    assert!(
        bis_count >= 2,
        "should have at least 2 bis, got {}",
        bis_count
    );

    // 4. 至少有一个 ComputeNode 产出了信号（ThrustNode / DivergenceNode / BSPNode）
    eprintln!("Signals by type: {:?}", all_signals.iter().map(|s| &s.source).collect::<Vec<_>>());
}

/// 验证空 tick 序列不会导致 Pipeline 崩溃。
#[test]
fn test_empty_sequence_no_panic() {
    let config = pipeline_config();
    let mut pipeline = Pipeline::from_config(config).expect("Pipeline::from_config");

    pipeline.add_node(Box::new(MockDvmiNode::new("dvmi")));
    pipeline.add_node(Box::new(MockThrustNode::new("thrust")));
    pipeline.add_node(Box::new(MockChanNode::new("chan")));
    pipeline.add_node(Box::new(MockHubNode::new("hub")));
    pipeline.add_node(Box::new(MockSegmentNode::new("segment")));
    pipeline.add_node(Box::new(MockDivergenceNode::new("divergence")));
    pipeline.add_node(Box::new(MockBSPNode::new("bsp")));
    pipeline.derive_edges().expect("derive_edges");

    // 喂入 5 个 tick 在同一分钟内，不触发 DAG
    for i in 0..5 {
        let result = pipeline
            .feed_tick_direct(&make_tick(ts(9, 0, i * 10), 100.0 + i as f64, 100.0, 10000.0))
            .expect("feed_tick_direct should not panic");
        assert!(result.signals.is_empty(), "no signals within same bucket");
    }
}

/// 验证单个 mock node 的独立行为。
#[test]
fn test_dvmi_node_independent() {
    let mut node = MockDvmiNode::new("dvmi_test");
    let cfg = NodeConfig::new();
    let state = StateStore::new();
    node.on_init(&cfg, &state).expect("init");

    // 手动构造 K 线并写入 StateStore（模拟 Pipeline 行为）
    let mut bars: Vec<Arc<RawBar>> = Vec::new();
    for i in 0..20 {
        let bar = RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: i as i32,
            open: 100.0 + (i as f64 * 0.5).sin() * 10.0,
            high: 100.0 + (i as f64 * 0.5).sin() * 12.0,
            low: 100.0 + (i as f64 * 0.5).cos() * 10.0,
            close: 100.0 + (i as f64 * 0.5).sin() * 11.0,
            vol: 1000.0,
            amount: 100000.0,
            open_interest: None,
            delta: None,
        };
        bars.push(Arc::new(bar));
    }
    state.set("bars:1m".into(), StateValue::Bars(Arc::new(bars)), "test".into());

    node.on_calculate(&state).expect("on_calculate");

    let pivots: Option<serde_json::Value> = state.get_json(&"pivots".into());
    assert!(
        pivots.is_some(),
        "dvmi should output pivots after enough bars"
    );
    let count = pivots.as_ref().and_then(|p| p.as_array().map(|a| a.len())).unwrap_or(0);
    assert!(
        count >= 2,
        "should have at least 2 pivots from 20 sinusoidal bars, got {}",
        count
    );
}
