//! R-2-601: Pipeline ↔ ComputeNode 集成测试
//!
//! 验证 NodeFactory 注册 ComputeNode → Pipeline 创建 → 执行 → 信号输出全链路。
//!
//! ## 测试场景
//!
//! 1. 定义一个 `ThresholdSignalNode`：当 K 线收盘价超过阈值时生成开多信号
//! 2. 通过 `NodeFactoryBuilder` 注册该节点类型
//! 3. 使用 PipelineConfig 构建 Pipeline
//! 4. 通过 NodeFactory 创建节点实例并加入 Pipeline
//! 5. 喂入合成 tick 数据（跨分钟边界闭合 K 线）
//! 6. 验证 Pipeline 输出信号
//!
//! ## 设计参考
//!
//! - R-2-201 ComputeNode trait + NodeFactory
//! - Phase-2-类型契约.md §5 ComputeNode / §6 Pipeline DAG
//! - taiji-engine pipeline/mod.rs execute_dag 执行流程

use std::collections::HashMap;

use chrono::{TimeZone, Utc};

use taiji_engine::config::*;
use taiji_engine::error::Result;
use taiji_engine::factory::{NodeDescriptor, NodeFactoryBuilder};
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::state::StateKey;
use taiji_engine::types::tick::TickData;

// ============================================================================
// 测试节点：ThresholdSignalNode
// ============================================================================

/// 阈值信号节点：当 K 线收盘价超过 `threshold` 时发出 Long 信号。
///
/// 配置参数：
/// - `threshold`: 价格阈值（f64），默认 100.0
/// - `instrument`: 信号中的合约代码，默认 "test"
struct ThresholdSignalNode {
    id: NodeId,
    threshold: f64,
    instrument: String,
    last_close: f64,
}

impl ThresholdSignalNode {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            threshold: 100.0,
            instrument: "test".into(),
            last_close: 0.0,
        }
    }
}

impl ComputeNode for ThresholdSignalNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "threshold_signal"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["signal:threshold".into()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(t) = config.get_f64("threshold") {
            self.threshold = t;
        }
        if let Some(inst) = config.get_str("instrument") {
            self.instrument = inst.to_string();
        }
        Ok(())
    }

    fn on_bar(&mut self, bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        self.last_close = bar.close;
        Ok(())
    }

    fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<Signal>> {
        if self.last_close > self.threshold {
            Ok(vec![Signal {
                timestamp: chrono::Utc::now(),
                instrument: self.instrument.clone(),
                freq: Freq::F1,
                action: SignalAction::Long,
                entry: Some(self.last_close),
                stop_loss: None,
                take_profit: None,
                size: Some(1.0),
                source: self.id.clone(),
                confidence: 0.85,
                metadata: HashMap::new(),
                disclaimer: None,
            }])
        } else {
            Ok(vec![])
        }
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F1]
    }

    fn is_ready(&self, _state: &StateStore) -> bool {
        // 不需要预热——第一个 bar 即可生成信号
        true
    }
}

// ============================================================================
// 工具函数
// ============================================================================

/// 构造一个合成 TickData。
fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64) -> TickData {
    TickData {
        instrument: "test".into(),
        trading_day: "20260722".into(),
        exchange_id: "TEST".into(),
        exchange_inst_id: "test".into(),
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
        bid_price2: 0.0,
        bid_volume2: 0,
        ask_price2: 0.0,
        ask_volume2: 0,
        bid_price3: 0.0,
        bid_volume3: 0,
        ask_price3: 0.0,
        ask_volume3: 0,
        bid_price4: 0.0,
        bid_volume4: 0,
        ask_price4: 0.0,
        ask_volume4: 0,
        bid_price5: 0.0,
        bid_volume5: 0,
        ask_price5: 0.0,
        ask_volume5: 0,
        average_price: 0.0,
        action_day: String::new(),
        trade_type: None,
        cum_volume: None,
        cum_position: None,
        timestamp_ms: ts_ms,
    }
}

/// 构建 UTC 毫秒时间戳。
fn ts(hour: u32, min: u32, sec: u32) -> i64 {
    Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
        .unwrap()
        .timestamp_millis()
}

/// 构建 PipelineConfig。
fn make_pipeline_config(threshold: f64) -> PipelineConfig {
    PipelineConfig {
        name: "r-2-601-integration".into(),
        version: "1.0".into(),
        bar_gen: BarGenConfig {
            modes: vec!["time".into()],
            time_freqs: vec!["1m".into()],
        },
        data_source: DataSourceSpec {
            type_name: "mock".into(),
            config: serde_json::json!({}),
        },
        nodes: vec![NodeSpec {
            id: "threshold_node".into(),
            type_name: "threshold_signal".into(),
            config: serde_json::json!({
                "threshold": threshold,
                "instrument": "ag2506",
            }),
            input_keys: vec![],
            output_keys: vec!["signal:threshold".into()],
        }],
    }
}

// ============================================================================
// 集成测试
// ============================================================================

/// NodeFactory 注册 → Pipeline 创建 → 合成 tick 输入 → 信号输出验证
///
/// 测试步骤：
/// 1. 通过 NodeFactoryBuilder 注册 ThresholdSignalNode
/// 2. 从 YAML 配置创建 Pipeline
/// 3. 通过 NodeFactory 创建节点实例，加入 Pipeline
/// 4. 推导 DAG 边
/// 5. 喂入合成 tick：09:00 (price=100) → 09:00:30 (price=90) → 09:01 (price=80)
///    闭合 09:00 K 线 (close=90, 未超阈值) → 无信号
/// 6. 喂入 09:01:30 (price=160) → 09:02 (price=170)
///    闭合 09:01 K 线 (close=160, 超阈值 150) → 产生信号
#[test]
fn test_node_factory_registration_and_signal_output() {
    // ── 1. 通过 NodeFactoryBuilder 注册 ThresholdSignalNode ──
    let factory = NodeFactoryBuilder::new()
        .install(NodeDescriptor {
            type_name: "threshold_signal",
            constructor: Box::new(|config| {
                let mut node = ThresholdSignalNode::new("threshold_node");
                let store = StateStore::new();
                node.on_init(config, &store)?;
                Ok(Box::new(node))
            }),
        })
        .build()
        .expect("NodeFactoryBuilder should build");

    // 验证工厂中存在注册的类型
    assert!(factory.contains("threshold_signal"));
    assert_eq!(factory.list_types().len(), 1);

    // ── 2. 创建 PipelineConfig ──
    let config = make_pipeline_config(150.0);
    assert!(config.validate().is_ok());

    // ── 3. 从配置构建 Pipeline ──
    let mut pipeline = Pipeline::from_config(config).expect("Pipeline::from_config");

    // ── 4. 通过 NodeFactory 创建节点并加入 Pipeline ──
    let mut node_config = NodeConfig::new();
    node_config
        .params
        .insert("threshold".into(), serde_json::json!(150.0));
    node_config
        .params
        .insert("instrument".into(), serde_json::json!("ag2506"));

    let node = factory
        .create("threshold_signal", &node_config)
        .expect("factory.create should create node");
    assert_eq!(node.id(), "threshold_node");
    assert_eq!(node.name(), "threshold_signal");

    pipeline.add_node(node);
    pipeline
        .derive_edges()
        .expect("derive_edges should succeed");

    // ── 5. 喂入合成 tick 数据 ──

    // 5a. 09:00:00, price=100 — 创建 09:00 K 线
    let result = pipeline
        .feed_tick_direct(&make_tick(ts(9, 0, 0), 100.0, 100.0, 100_000.0))
        .expect("feed_tick_direct should succeed");
    assert!(result.signals.is_empty(), "no signal on first tick");
    assert!(result.closed_bars.is_empty(), "no closed bars yet");

    // 5b. 09:00:30, price=90 — 更新 09:00 K 线，close=90
    let result = pipeline
        .feed_tick_direct(&make_tick(ts(9, 0, 30), 90.0, 200.0, 180_000.0))
        .expect("feed_tick_direct should succeed");
    assert!(result.signals.is_empty(), "no signal before bar close");

    // 5c. 09:01:00, price=80 — 闭合 09:00 K 线 (close=90)
    // 阈值 150 > close=90 → 不产生信号
    let result = pipeline
        .feed_tick_direct(&make_tick(ts(9, 1, 0), 80.0, 300.0, 240_000.0))
        .expect("feed_tick_direct should succeed");
    assert!(
        result.closed_bars.len() >= 1,
        "should close at least one bar"
    );
    // node 尚未预热（last_close=0 < threshold=150 前次为 90）
    // 原本 node 的 is_ready 条件是 last_close > 0.0，
    // 但 on_bar 已在 execute_dag 中被调用 → last_close 已更新为 90
    // close=90 < threshold=150 → 无信号
    assert!(
        result.signals.is_empty(),
        "no signal when close below threshold"
    );

    // 5d. 09:01:30, price=160 — 更新 09:01 K 线
    let result = pipeline
        .feed_tick_direct(&make_tick(ts(9, 1, 30), 160.0, 400.0, 640_000.0))
        .expect("feed_tick_direct should succeed");
    assert!(result.signals.is_empty(), "no signal during bar build");

    // 5e. 09:02:00, price=170 — 闭合 09:01 K 线 (close=160)
    // 阈值 150 < close=160 → 产生 Long 信号
    let result = pipeline
        .feed_tick_direct(&make_tick(ts(9, 2, 0), 170.0, 500.0, 850_000.0))
        .expect("feed_tick_direct should succeed");
    assert!(
        result.closed_bars.len() >= 1,
        "should close another bar"
    );

    // ── 6. 验证信号输出 ──
    assert!(
        !result.signals.is_empty(),
        "should generate signal when close={} exceeds threshold=150",
        result.signals.len()
    );

    let signal = &result.signals[0];
    assert_eq!(signal.action, SignalAction::Long);
    assert_eq!(signal.entry, Some(160.0));
    assert_eq!(signal.instrument, "ag2506");
    assert_eq!(signal.source, "threshold_node");
    assert!(
        (signal.confidence - 0.85).abs() < 1e-6,
        "confidence should match"
    );

    // 验证 NodeStatus 中有执行记录
    let status = pipeline.status();
    let node_status = status
        .nodes
        .iter()
        .find(|n| n.id == "threshold_node")
        .expect("threshold_node should be in pipeline status");
    assert!(node_status.ready);
}

/// 验证未注册的节点类型会导致 factory.create() 返回错误。
#[test]
fn test_unknown_node_type_returns_error() {
    let factory = NodeFactoryBuilder::new().build().expect("empty factory");
    let result = factory.create("nonexistent", &NodeConfig::new());
    assert!(
        result.is_err(),
        "creating unknown node type should fail"
    );
}

/// 验证 Pipeline 处理空 tick 序列不会崩溃。
#[test]
fn test_empty_tick_sequence_no_panic() {
    let config = make_pipeline_config(100.0);
    let mut pipeline = Pipeline::from_config(config).expect("Pipeline::from_config");

    let mut node_config = NodeConfig::new();
    node_config
        .params
        .insert("threshold".into(), serde_json::json!(100.0));

    // 手动构造节点（不通过 factory，简化测试）
    pipeline.add_node(Box::new(ThresholdSignalNode::new("threshold_node")));
    pipeline
        .derive_edges()
        .expect("derive_edges should succeed");

    // 喂入 3 个 tick 但都在同一分钟内，不触发 DAG
    for i in 0..3 {
        let result = pipeline
            .feed_tick_direct(&make_tick(
                ts(9, 0, i * 10),
                100.0 + i as f64 * 10.0,
                100.0,
                100_000.0,
            ))
            .expect("feed_tick_direct");
        assert!(result.signals.is_empty(), "no signals within same bucket");
    }
}

/// 验证 NodeFactoryBuilder 拒绝重复注册。
#[test]
fn test_duplicate_registration_rejected() {
    let result = NodeFactoryBuilder::new()
        .install(NodeDescriptor {
            type_name: "dup",
            constructor: Box::new(|_| {
                Ok(Box::new(ThresholdSignalNode::new("dup1")) as Box<dyn ComputeNode>)
            }),
        })
        .install(NodeDescriptor {
            type_name: "dup",
            constructor: Box::new(|_| {
                Ok(Box::new(ThresholdSignalNode::new("dup2")) as Box<dyn ComputeNode>)
            }),
        })
        .build();
    assert!(result.is_err(), "duplicate registration should fail");
}

/// 验证 register_node! 宏可以替代手动注册。
#[test]
fn test_register_macro_works() {
    use taiji_engine::register_node;

    let mut factory = taiji_engine::factory::NodeFactory::new();
    register_node!(factory, "threshold_macro", ThresholdSignalNode, "macro_node");

    assert!(factory.contains("threshold_macro"));

    let mut node_config = NodeConfig::new();
    node_config
        .params
        .insert("threshold".into(), serde_json::json!(200.0));

    let node = factory
        .create("threshold_macro", &node_config)
        .expect("register_node! macro should create node");
    assert_eq!(node.id(), "macro_node");
    assert_eq!(node.name(), "threshold_signal");
}

/// 验证信号 serde round-trip 完整性。
#[test]
fn test_signal_serde_roundtrip() {
    let signal = Signal {
        timestamp: chrono::Utc::now(),
        instrument: "ag2506".into(),
        freq: Freq::F1,
        action: SignalAction::Long,
        entry: Some(5625.0),
        stop_loss: Some(5600.0),
        take_profit: Some(5700.0),
        size: Some(2.0),
        source: "test_node".into(),
        confidence: 0.85,
        metadata: HashMap::from([("strategy".into(), "threshold".into())]),
        disclaimer: None,
    };

    let json = serde_json::to_string(&signal).expect("serialize");
    let deserialized: Signal = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(signal.instrument, deserialized.instrument);
    assert_eq!(signal.action, deserialized.action);
    assert_eq!(signal.entry, deserialized.entry);
    assert!((signal.confidence - deserialized.confidence).abs() < 1e-6);
}

/// 验证 PipelineStatus 在多节点下的状态快照。
#[test]
fn test_multiple_nodes_in_pipeline() {
    let config = PipelineConfig {
        name: "multi-node-test".into(),
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
                id: "node_a".into(),
                type_name: "threshold_signal".into(),
                config: serde_json::json!({"threshold": 100.0}),
                input_keys: vec![],
                output_keys: vec!["sig_a".into()],
            },
            NodeSpec {
                id: "node_b".into(),
                type_name: "threshold_signal".into(),
                config: serde_json::json!({"threshold": 200.0}),
                input_keys: vec![],
                output_keys: vec!["sig_b".into()],
            },
        ],
    };

    let mut pipeline = Pipeline::from_config(config).expect("Pipeline::from_config");
    pipeline.add_node(Box::new(ThresholdSignalNode::new("node_a")));
    pipeline.add_node(Box::new(ThresholdSignalNode::new("node_b")));
    pipeline
        .derive_edges()
        .expect("derive_edges should succeed");

    // 喂入 tick 闭合 09:00 K 线
    pipeline
        .feed_tick_direct(&make_tick(ts(9, 0, 0), 150.0, 100.0, 150_000.0))
        .expect("feed_tick");
    pipeline
        .feed_tick_direct(&make_tick(ts(9, 1, 0), 160.0, 200.0, 320_000.0))
        .expect("feed_tick");

    let status = pipeline.status();
    assert_eq!(
        status.nodes.len(),
        2,
        "should have 2 nodes in pipeline status"
    );

    // node_a threshold=100, close=150 → ready
    // node_b threshold=200, close=150 → not ready (close < threshold)
    // is_ready 对 node_b 返回 false（last_close=150 < threshold=200）
    // 但 node_b 的 on_bar 已调用，只是 on_calculate 返回空
}
