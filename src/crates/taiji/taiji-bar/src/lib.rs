//! Tick-to-KLine 聚合引擎 — BarNode 实现 ComputeNode。
//! 从 taiji-engine::pipeline::bar_gen::BarGenerator 迁移到独立 crate。
//! 参考: czsc BarGenerator (Apache 2.0)

use std::sync::Arc;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar, Symbol};
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

// ── PartialBar（内部，不导出）─────────────────────────────────────────

struct PartialBar {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    vol: f64,
    amount: f64,
    open_interest_current: Option<f64>,
    delta_sum: f64,
    start_time: DateTime<Utc>,
    tick_count: u64,
    prev_volume: f64,
    prev_amount: f64,
}

impl PartialBar {
    fn new(price: f64, vol: f64, amount: f64, oi: Option<f64>, time: DateTime<Utc>) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            vol: 0.0,
            amount: 0.0,
            open_interest_current: oi,
            delta_sum: 0.0,
            start_time: time,
            tick_count: 1,
            prev_volume: vol,
            prev_amount: amount,
        }
    }

    fn update(&mut self, price: f64, vol: f64, amount: f64, oi: Option<f64>, delta: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.vol += (vol - self.prev_volume).max(0.0);
        self.amount += (amount - self.prev_amount).max(0.0);
        self.open_interest_current = oi;
        self.delta_sum += delta;
        self.prev_volume = vol;
        self.prev_amount = amount;
        self.tick_count += 1;
    }

    fn finalize(&self, id: i32, symbol: Symbol, freq: Freq, end_time: DateTime<Utc>) -> RawBar {
        RawBar {
            symbol,
            dt: end_time,
            freq,
            id,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            vol: self.vol,
            amount: self.amount,
            open_interest: self.open_interest_current,
            delta: if self.delta_sum != 0.0 {
                Some(self.delta_sum)
            } else {
                None
            },
        }
    }
}

// ── BarNode ───────────────────────────────────────────────────────────

/// Bar 生成节点。
///
/// 实现 `ComputeNode`，通过 `on_tick` 接收逐笔 tick，按时间边界聚合为 `RawBar`，
/// 写入 `StateStore`（key = `"bars:{freq_key}"`，如 `"bars:1m"`）。
///
/// 配置参数（NodeConfig）：
/// - `freq` (str): 周期标识，如 `"1m"`, `"5m"`, `"1h"`, `"1d"`。默认 `"1m"`。
pub struct BarNode {
    id: NodeId,
    freq: Freq,
    current_bar: Option<PartialBar>,
    next_id: i32,
    symbol: Option<Symbol>,
}

impl BarNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            freq: Freq::F1,
            current_bar: None,
            next_id: 0,
            symbol: None,
        }
    }

    fn classify_delta(tick: &TickData) -> f64 {
        if let Some(tt) = tick.trade_type {
            return tt;
        }
        if tick.last_price >= tick.ask_price1 && tick.ask_price1 > 0.0 {
            1.0
        } else if tick.last_price <= tick.bid_price1 && tick.bid_price1 > 0.0 {
            -1.0
        } else {
            0.0
        }
    }

    fn time_bucket(dt: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
        let total_minutes = dt.hour() as i64 * 60 + dt.minute() as i64;
        let bucket_min = (total_minutes / minutes) * minutes;
        let h = (bucket_min / 60) as u32;
        let m = (bucket_min % 60) as u32;
        Utc.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), h, m, 0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
    }

    fn output_key(&self) -> StateKey {
        format!("bars:{}", self.freq.freq_key())
    }

    /// 闭合当前未完成的 bar，写入 StateStore。
    fn close_current_bar(&mut self, bucket: DateTime<Utc>, state: &StateStore) {
        let old = self.current_bar.take().unwrap();
        let sym = self
            .symbol
            .clone()
            .unwrap_or_else(|| Symbol::from("UNKNOWN"));
        let bar = old.finalize(self.next_id, sym, self.freq, bucket);
        self.next_id += 1;

        let key = self.output_key();
        let bars: Arc<Vec<Arc<RawBar>>> = state.get(&key).unwrap_or_else(|| Arc::new(Vec::new()));
        let mut new_bars: Vec<Arc<RawBar>> = (*bars).clone();
        new_bars.push(Arc::new(bar));
        state.set(key, StateValue::Bars(Arc::new(new_bars)), self.id());
    }
}

impl ComputeNode for BarNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "BarNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![self.output_key()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(freq_str) = config.get_str("freq") {
            self.freq = Freq::from_key(freq_str).unwrap_or(Freq::F1);
        }
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        let price = tick.last_price;
        let vol = tick.volume;
        let amount = tick.turnover;
        let oi = if tick.open_interest > 0.0 {
            Some(tick.open_interest)
        } else {
            None
        };
        let delta = Self::classify_delta(tick);

        let dt = Utc.timestamp_millis_opt(tick.timestamp_ms).unwrap();
        let symbol = Symbol::from(tick.instrument.as_str());

        let Some(minutes) = self.freq.minutes() else {
            return Ok(());
        };

        let bucket = Self::time_bucket(dt, minutes);

        // 跨边界 → 闭合旧 bar
        if let Some(ref partial) = self.current_bar {
            if partial.start_time != bucket {
                self.close_current_bar(bucket, state);
            }
        }

        // 更新/创建当前 bar
        if let Some(bar) = self.current_bar.as_mut() {
            bar.update(price, vol, amount, oi, delta);
        } else {
            self.current_bar = Some(PartialBar::new(price, vol, amount, oi, bucket));
            self.symbol = Some(symbol);
        }

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![self.freq]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64, oi: f64) -> TickData {
        TickData {
            instrument: "rb9999".into(),
            timestamp_ms: ts_ms,
            last_price: price,
            volume: vol,
            turnover: amount,
            open_interest: oi,
            ..TickData::default()
        }
    }

    fn ts(hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    fn ts_day(day: u32, hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, day, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    // ── 单 tick 累加 + 边界闭合 ──

    #[test]
    fn test_single_tick_no_close() {
        let mut node = BarNode::new("bar1".into());
        let mut store = StateStore::new();

        node.on_tick(
            &make_tick(ts(9, 1, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &mut store,
        )
        .unwrap();

        // 同一个桶内的 tick 不会闭合 bar
        assert!(store
            .get::<Arc<Vec<Arc<RawBar>>>>(&node.output_key())
            .is_none());
        assert!(node.current_bar.is_some());
    }

    #[test]
    fn test_boundary_close() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let mut store = StateStore::new();

        // 09:01 → 桶 09:00
        node.on_tick(
            &make_tick(ts(9, 1, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &mut store,
        )
        .unwrap();
        // 09:03 → 桶 09:00（同一桶）
        node.on_tick(
            &make_tick(ts(9, 3, 0), 4010.0, 200.0, 802_000.0, 5000.0),
            &mut store,
        )
        .unwrap();
        // 09:05 → 桶 09:05（跨边界）
        node.on_tick(
            &make_tick(ts(9, 5, 0), 4020.0, 300.0, 1_206_000.0, 5100.0),
            &mut store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        let bar = &bars[0];
        assert_eq!(bar.open, 4000.0);
        assert_eq!(bar.high, 4010.0);
        assert_eq!(bar.low, 4000.0);
        assert_eq!(bar.close, 4010.0);
        assert_eq!(bar.vol, 100.0);
        assert_eq!(bar.amount, 402_000.0);
        assert_eq!(bar.open_interest, Some(5000.0));
    }

    #[test]
    fn test_volume_rollback_handling() {
        let mut node = BarNode::new("bar1".into());
        let mut store = StateStore::new();

        // 主力换月导致累计成交量回退：vol 200→100
        node.on_tick(
            &make_tick(ts(9, 0, 0), 4000.0, 200.0, 800_000.0, 5000.0),
            &mut store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 1, 0), 4010.0, 100.0, 400_000.0, 5000.0),
            &mut store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        let bar = &bars[0];
        // vol: 0 + max(0, 100-200) = 0
        assert_eq!(bar.vol, 0.0);
        // amount: 0 + max(0, 400k-800k) = 0
        assert_eq!(bar.amount, 0.0);
    }

    // ── 跨日处理 ──

    #[test]
    fn test_cross_day() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let mut store = StateStore::new();

        // 7/22 23:58 → 桶 23:55 (5min)
        node.on_tick(
            &make_tick(ts_day(22, 23, 58, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &mut store,
        )
        .unwrap();

        // 7/23 00:01 → 桶 00:00，跨日跨桶
        node.on_tick(
            &make_tick(ts_day(23, 0, 1, 0), 4010.0, 200.0, 802_000.0, 5000.0),
            &mut store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        // bar 结束时间 = bucket 边界，即次日 00:00
        assert_eq!(bars[0].dt.day(), 23);
        assert_eq!(bars[0].dt.hour(), 0);
        assert_eq!(bars[0].dt.minute(), 0);
    }

    // ── on_init 读取 freq 配置 ──

    #[test]
    fn test_on_init_custom_freq() {
        let mut node = BarNode::new("bar1".into());
        let mut store = StateStore::new();
        let mut config = NodeConfig::new();
        config
            .params
            .insert("freq".into(), serde_json::Value::String("1h".into()));

        node.on_init(&config, &mut store).unwrap();

        assert_eq!(node.freq, Freq::F60);
        assert_eq!(node.output_key(), "bars:1h");
        assert_eq!(node.subscribed_freqs(), vec![Freq::F60]);
    }

    #[test]
    fn test_on_init_default_freq() {
        let mut node = BarNode::new("bar1".into());
        let mut store = StateStore::new();
        let config = NodeConfig::new();

        node.on_init(&config, &mut store).unwrap();

        assert_eq!(node.freq, Freq::F1);
        assert_eq!(node.output_key(), "bars:1m");
    }

    // ── Delta 分类 ──

    #[test]
    fn test_delta_gm_mode() {
        let mut tick = make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0);
        tick.trade_type = Some(-1.0);
        assert_eq!(BarNode::classify_delta(&tick), -1.0);

        tick.trade_type = Some(2.0);
        assert_eq!(BarNode::classify_delta(&tick), 2.0);
    }

    #[test]
    fn test_delta_ctp_l1() {
        let mut tick = make_tick(ts(9, 0, 0), 4005.0, 100.0, 400_000.0, 5000.0);
        tick.ask_price1 = 4005.0;
        tick.bid_price1 = 4000.0;
        assert_eq!(BarNode::classify_delta(&tick), 1.0);

        tick.last_price = 4000.0;
        assert_eq!(BarNode::classify_delta(&tick), -1.0);

        tick.last_price = 4002.0;
        assert_eq!(BarNode::classify_delta(&tick), 0.0);
    }

    // ── 多 bar 连续闭合 ──

    #[test]
    fn test_multiple_bars() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let mut store = StateStore::new();

        // Bar 1: 09:00-09:04
        node.on_tick(
            &make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &mut store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 4, 59), 4010.0, 200.0, 802_000.0, 5000.0),
            &mut store,
        )
        .unwrap();

        // Bar 2: 09:05-09:09
        node.on_tick(
            &make_tick(ts(9, 5, 0), 4020.0, 300.0, 1_206_000.0, 5100.0),
            &mut store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 9, 59), 4030.0, 400.0, 1_612_000.0, 5200.0),
            &mut store,
        )
        .unwrap();

        // Bar 3: 09:10+
        node.on_tick(
            &make_tick(ts(9, 10, 0), 4040.0, 500.0, 2_020_000.0, 5300.0),
            &mut store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 2);

        // Bar 1
        assert_eq!(bars[0].open, 4000.0);
        assert_eq!(bars[0].close, 4010.0);
        assert_eq!(bars[0].vol, 100.0);
        // Bar 2
        assert_eq!(bars[1].open, 4020.0);
        assert_eq!(bars[1].close, 4030.0);
        assert_eq!(bars[1].vol, 100.0);
    }
}
