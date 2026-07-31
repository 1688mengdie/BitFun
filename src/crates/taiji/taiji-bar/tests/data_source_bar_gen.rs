//! R-2-602: DataSource → BarGenerator 集成测试
//!
//! 验证完整数据通路：
//!   MockDataSource (RawTick) → Pipeline.feed_tick()
//!     → SchemaAdapter → TickData → BarGenerator → closed bars
//!
//! 测试场景：
//! 1. 单周期 1m 时间聚合：5 分钟 ticks → 5 根 1m K 线
//! 2. 多周期 1m+5m 时间聚合：10 分钟 ticks → 10 根 1m + 2 根 5m K 线
//! 3. 成交量聚合：tick 累计量 → 按阈值闭合 K 线
//! 4. 无数据源空跑：Pipeline.feed_tick() 返回 empty

use std::collections::HashMap;

use chrono::TimeZone;
use taiji_engine::config::{BarGenConfig, DataSourceSpec, NodeSpec, PipelineConfig};
use taiji_engine::error::Result as TaijiResult;
use taiji_engine::pipeline::Pipeline;
use taiji_engine::source::datasource::{DataSource, DataSourceConfig, FieldDef, SourceHealth};
use taiji_engine::types::tick::RawTick;

// ── Mock DataSource ──────────────────────────────────────────────────────

/// 模拟行情源：生成连续的 RawTick 序列。
/// 每笔 tick 写入 price / cum_volume / cum_amount 等字段，
/// 模拟真实 CTP 快照行情格式。
struct MockTickSource {
    /// 已取走的 tick 数
    consumed: usize,
    /// 预设的 tick 数据: (price, cum_vol, cum_amount, ts_ms, trade_type)
    ticks: Vec<(f64, f64, f64, i64, f64)>,
    /// 品种代码
    instrument: String,
}

impl MockTickSource {
    fn new(instrument: &str, ticks: Vec<(f64, f64, f64, i64, f64)>) -> Self {
        Self {
            consumed: 0,
            ticks,
            instrument: instrument.to_string(),
        }
    }

    /// Helper: 生成一组等间隔 tick（每秒一笔）用于时间聚合测试。
    fn make_time_ticks(
        _instrument: &str,
        start_ts_ms: i64,
        num_ticks: usize,
        interval_ms: i64,
        start_price: f64,
        price_step: f64,
        base_vol: f64,
        base_amount: f64,
    ) -> Vec<(f64, f64, f64, i64, f64)> {
        let mut ticks = Vec::with_capacity(num_ticks);
        for i in 0..num_ticks {
            let ts = start_ts_ms + i as i64 * interval_ms;
            let price = start_price + i as f64 * price_step;
            let cum_vol = base_vol * (i + 1) as f64;
            let cum_amount = base_amount * (i + 1) as f64;
            let delta = if i % 2 == 0 { 1.0 } else { -1.0 };
            ticks.push((price, cum_vol, cum_amount, ts, delta));
        }
        ticks
    }
}

impl DataSource for MockTickSource {
    fn name(&self) -> &'static str {
        "mock_tick_source"
    }

    fn schema(&self) -> Vec<FieldDef> {
        vec![]
    }

    fn connect(&mut self, _config: &DataSourceConfig) -> TaijiResult<()> {
        self.consumed = 0;
        Ok(())
    }

    fn disconnect(&mut self) -> TaijiResult<()> {
        Ok(())
    }

    fn subscribe(&mut self, _instruments: &[&str]) -> TaijiResult<()> {
        Ok(())
    }

    fn next_raw(&mut self) -> TaijiResult<Option<RawTick>> {
        if self.consumed >= self.ticks.len() {
            return Ok(None);
        }
        let (price, cum_vol, cum_amount, ts_ms, trade_type) = self.ticks[self.consumed];
        self.consumed += 1;

        let mut fields = HashMap::new();
        fields.insert("price".into(), price);
        fields.insert("cum_volume".into(), cum_vol);
        fields.insert("cum_amount".into(), cum_amount);
        fields.insert("open_interest".into(), 50000.0);
        fields.insert("trade_type".into(), trade_type);
        fields.insert("open".into(), price - 0.5);
        fields.insert("high".into(), price + 0.5);
        fields.insert("low".into(), price - 1.0);
        // Also set bid/ask for delta classification (not needed since trade_type is set)

        Ok(Some(RawTick {
            instrument: self.instrument.clone(),
            source_id: "mock:001".into(),
            fields,
            timestamp: ts_ms,
            sequence: Some(self.consumed as u64),
        }))
    }

    fn health_check(&self) -> SourceHealth {
        SourceHealth::Healthy
    }
}

// ── 辅助函数 ─────────────────────────────────────────────────────────────

/// 构建一个带假节点的最小 Pipeline，注入 MockTickSource。
fn build_pipeline(
    modes: Vec<&str>,
    time_freqs: Vec<&str>,
    data_source: Box<dyn DataSource>,
) -> Pipeline {
    let config = PipelineConfig {
        name: "bar_gen_test".into(),
        version: "1.0".into(),
        bar_gen: BarGenConfig {
            modes: modes.into_iter().map(|s| s.to_string()).collect(),
            time_freqs: time_freqs.into_iter().map(|s| s.to_string()).collect(),
        },
        data_source: DataSourceSpec {
            type_name: "mock".into(),
            config: serde_json::json!({}),
        },
        nodes: vec![NodeSpec {
            id: "dummy".into(),
            type_name: "dummy".into(),
            config: serde_json::json!({}),
            input_keys: vec![],
            output_keys: vec![],
        }],
    };

    let mut pipeline = Pipeline::from_config(config).expect("from_config");

    // Register and add dummy node so derive_edges() passes
    pipeline.register_node_type(
        "dummy",
        Box::new(|_: &taiji_engine::node::NodeConfig| {
            Ok(Box::new(DummyNode::default()))
        }),
    );
    pipeline.add_node(Box::new(DummyNode::default()));
    pipeline.derive_edges().expect("derive_edges");

    pipeline.set_data_source(data_source);
    pipeline
}

/// 什么都不做的虚拟节点，仅用于通过配置校验。
#[derive(Default)]
struct DummyNode;

impl taiji_engine::node::ComputeNode for DummyNode {
    fn id(&self) -> taiji_engine::types::NodeId {
        "dummy".into()
    }
    fn name(&self) -> &'static str {
        "dummy"
    }
    fn input_keys(&self) -> Vec<taiji_engine::types::state::StateKey> {
        vec![]
    }
    fn output_keys(&self) -> Vec<taiji_engine::types::state::StateKey> {
        vec![]
    }
    fn on_init(
        &mut self,
        _config: &taiji_engine::node::NodeConfig,
        _state: &taiji_engine::store::StateStore,
    ) -> TaijiResult<()> {
        Ok(())
    }
    fn on_bar(
        &mut self,
        _bar: &taiji_engine::types::bar::RawBar,
        _period: taiji_engine::types::bar::Freq,
        _state: &taiji_engine::store::StateStore,
    ) -> TaijiResult<()> {
        Ok(())
    }
    fn is_ready(&self, _state: &taiji_engine::store::StateStore) -> bool {
        true
    }
    fn subscribed_freqs(&self) -> Vec<taiji_engine::types::bar::Freq> {
        vec![]
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

/// 边界情况：空数据源 → feed_tick 直接返回 empty TickResult
#[test]
fn test_empty_mock_source() {
    let ds = MockTickSource::new("rb9999", vec![]);
    let mut pipeline = build_pipeline(vec!["time"], vec!["1m"], Box::new(ds));
    let result = pipeline.feed_tick().expect("feed_tick with empty source");
    assert!(result.closed_bars.is_empty(), "no ticks → no bars");
    assert!(result.signals.is_empty());
}

// ── 场景 1: 单周期 1m 时间聚合 ──

#[test]
fn test_ds_to_bar_1min_time_agg() {
    // 每 1 秒一笔 tick，持续 6 分钟（360 笔）
    // 09:00:00 开始，bar 在整分钟边界闭合
    // tick 60 (09:01:00) 闭合 bar#0，tick 300 (09:05:00) 闭合 bar#4 = 5 根
    let start_ts = chrono::Utc
        .with_ymd_and_hms(2026, 7, 22, 9, 0, 0)
        .unwrap()
        .timestamp_millis();
    let ticks = MockTickSource::make_time_ticks(
        "rb9999", start_ts, 360, // 6min × 60s = 360 ticks
        1000,                     // 每秒一笔
        4000.0,                   // 起始价
        0.1,                      // 每笔步长
        100.0,                    // base vol
        400_000.0,                // base amount
    );
    let n_ticks = ticks.len();
    let ds = MockTickSource::new("rb9999", ticks);
    let mut pipeline = build_pipeline(vec!["time"], vec!["1m"], Box::new(ds));

    let mut total_closed = 0;
    let mut bar_count_by_freq: HashMap<String, usize> = HashMap::new();

    for _ in 0..n_ticks {
        let result = pipeline.feed_tick().expect("feed_tick");
        for (freq, _bar) in &result.closed_bars {
            let key = freq.freq_key().to_string();
            *bar_count_by_freq.entry(key).or_default() += 1;
        }
        total_closed += result.closed_bars.len();
    }

    // 耗尽后的 feed_tick 应返回 empty
    let final_result = pipeline.feed_tick().expect("feed_tick after exhaustion");
    assert!(final_result.closed_bars.is_empty(), "no more ticks");
    assert!(final_result.signals.is_empty());

    // 6 分钟：09:01~09:05 各闭合一根 = 5 根
    assert_eq!(
        bar_count_by_freq.get("1m").copied().unwrap_or(0),
        5,
        "6 minutes should produce 5 × 1m bars"
    );
    assert_eq!(total_closed, 5, "total closed bars should be 5");
}

// ── 场景 2: 多周期 1m+5m 时间聚合 ──

#[test]
fn test_ds_to_bar_multi_freq_time_agg() {
    // 每 0.5 秒一笔 tick，持续 11 分钟（1320 笔）
    // 应产生 10 根 1m K 线 + 2 根 5m K 线
    let start_ts = chrono::Utc
        .with_ymd_and_hms(2026, 7, 22, 9, 0, 0)
        .unwrap()
        .timestamp_millis();
    let ticks = MockTickSource::make_time_ticks(
        "rb9999", start_ts, 1320, // 11min × 60s × 2 = 1320
        500,
        5000.0, 0.05, 200.0, 500_000.0,
    );
    let n_ticks = ticks.len();
    let ds = MockTickSource::new("rb9999", ticks);
    let mut pipeline = build_pipeline(vec!["time"], vec!["1m", "5m"], Box::new(ds));

    let mut bar_count: HashMap<String, usize> = HashMap::new();

    for _ in 0..n_ticks {
        let result = pipeline.feed_tick().expect("feed_tick");
        for (freq, bar) in &result.closed_bars {
            let key = freq.freq_key().to_string();
            *bar_count.entry(key).or_default() += 1;
            // 基础 OHLCV 合理性检查
            assert!(
                bar.high >= bar.low,
                "OHLC violated: high ({}) < low ({})",
                bar.high,
                bar.low
            );
            assert!(
                bar.high >= bar.close,
                "close ({}) > high ({})",
                bar.close,
                bar.high
            );
            assert!(
                bar.low <= bar.close,
                "close ({}) < low ({})",
                bar.close,
                bar.low
            );
        }
    }

    // 10 分钟 → 10 根 1m + 2 根 5m
    assert_eq!(
        bar_count.get("1m").copied().unwrap_or(0),
        10,
        "10 min → 10 × 1m bars"
    );
    assert_eq!(
        bar_count.get("5m").copied().unwrap_or(0),
        2,
        "10 min → 2 × 5m bars"
    );
}

// ── 场景 3: 长序列 OHLCV 正确性 ──

#[test]
fn test_ds_to_bar_ohlcv_integrity() {
    // 生成 11 分钟 ticks（660 笔），验证每条闭合 K 线的 OHLCV 一致性
    let start_ts = chrono::Utc
        .with_ymd_and_hms(2026, 7, 22, 9, 0, 0)
        .unwrap()
        .timestamp_millis();
    let ticks = MockTickSource::make_time_ticks(
        "rb9999", start_ts, 660, 1000, 4000.0, 0.2, 200.0, 500_000.0,
    );
    let n_ticks = ticks.len();
    let ds = MockTickSource::new("rb9999", ticks);
    let mut pipeline = build_pipeline(vec!["time"], vec!["1m"], Box::new(ds));

    let mut all_bars = Vec::new();
    for _ in 0..n_ticks {
        let result = pipeline.feed_tick().expect("feed_tick");
        all_bars.extend(result.closed_bars);
    }

    // OHLCV 完整性检查
    for (_freq, bar) in &all_bars {
        assert!(bar.open.is_finite(), "open should be finite");
        assert!(
            bar.high.is_finite() && bar.high >= bar.low,
            "high/low corrupted: high={}, low={}",
            bar.high,
            bar.low
        );
        assert!(bar.close.is_finite(), "close should be finite");
        assert!(bar.vol >= 0.0, "vol should be non-negative, got {}", bar.vol);
        assert!(
            bar.amount >= 0.0,
            "amount should be non-negative, got {}",
            bar.amount
        );
    }

    // 11 分钟 → 10 根 1m bar（09:01~09:10）
    assert_eq!(
        all_bars.len(),
        10,
        "11 minutes should produce 10 × 1m bars"
    );
}
