//! BarGenerator — tick→K 线多周期聚合器。
//!
//! 支持时间/成交量/价格幅度三种聚合模式（AggMode）。
//! 非 async，适用于 Layer 1 实时计算。
//!
//! 参考: taiji-engine/pipeline/bar_gen.rs:103-239 BarGenerator
//! 参考: czsc bar_generator.rs (Apache 2.0) Tick→Bar 聚合算法
//! 参考: 量价时空/Phase-2-派发提示词.md:429 — R-2-203 — BarGenerator tick→K线

use std::collections::HashMap;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use taiji_engine::types::bar::{Freq, RawBar, Symbol};
use taiji_engine::types::tick::TickData;

use crate::modes::{AggMode, AggParams};

// ── PartialBar ───────────────────────────────────────────────────────────

/// 当前正在构建的、尚未闭合的 K 线。
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
        if !price.is_finite() || !vol.is_finite() || !amount.is_finite() || !delta.is_finite() {
            return;
        }
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
            delta: if self.delta_sum != 0.0 { Some(self.delta_sum) } else { None },
        }
    }
}

/// 键类型：(频次) — 单 symbol 下用 Freq 即可区分
type PartialsMap = HashMap<Freq, PartialBar>;
type CompletedMap = HashMap<Freq, Vec<RawBar>>;

/// tick→K 线多周期聚合器。
pub struct BarGenerator {
    symbol: Symbol,
    partials: PartialsMap,
    completed: CompletedMap,
    modes: Vec<AggMode>,
    time_freqs: Vec<Freq>,
    agg_params: AggParams,
    next_id: i32,
    #[allow(dead_code)]
    last_cum_vol: Option<f64>,
    #[allow(dead_code)]
    last_price: Option<f64>,
}

impl BarGenerator {
    pub fn new(symbol: Symbol, modes: Vec<AggMode>, time_freqs: Vec<Freq>) -> Self {
        Self {
            symbol,
            partials: HashMap::new(),
            completed: HashMap::new(),
            modes,
            time_freqs,
            agg_params: AggParams::default(),
            next_id: 0,
            last_cum_vol: None,
            last_price: None,
        }
    }

    pub fn new_with_params(
        symbol: Symbol,
        modes: Vec<AggMode>,
        time_freqs: Vec<Freq>,
        agg_params: AggParams,
    ) -> Self {
        Self {
            symbol,
            partials: HashMap::new(),
            completed: HashMap::new(),
            modes,
            time_freqs,
            agg_params,
            next_id: 0,
            last_cum_vol: None,
            last_price: None,
        }
    }

    /// 处理一笔 tick，返回所有被此 tick 闭合的 (Freq, RawBar)。
    pub fn update_tick(&mut self, tick: &TickData) -> Vec<(Freq, RawBar)> {
        let price = tick.last_price;
        let vol = tick.volume;
        let amount = tick.turnover;
        if !price.is_finite() || !vol.is_finite() || !amount.is_finite() {
            return Vec::new();
        }
        let oi = if tick.open_interest > 0.0 && tick.open_interest.is_finite() {
            Some(tick.open_interest)
        } else {
            None
        };

        let delta = classify_delta(tick);
        let ts_ms = tick.timestamp_ms;
        let dt = Utc.timestamp_millis_opt(ts_ms).single().unwrap_or(Utc::now());
        let mut closed = Vec::new();

        // 分离 mode 判断以避免借用冲突
        let has_time = self.modes.contains(&AggMode::Time);
        let has_volume = self.modes.contains(&AggMode::Volume);
        let has_range = self.modes.contains(&AggMode::Range);

        if has_time {
            closed.extend(self.update_time_mode(price, vol, amount, oi, delta, dt));
        }
        if has_volume {
            closed.extend(self.update_volume_mode(price, vol, amount, oi, delta, dt));
        }
        if has_range {
            closed.extend(self.update_range_mode(price, vol, amount, oi, delta, dt));
        }

        self.last_cum_vol = Some(vol);
        self.last_price = Some(price);
        closed
    }

    fn update_time_mode(
        &mut self,
        price: f64,
        vol: f64,
        amount: f64,
        oi: Option<f64>,
        delta: f64,
        dt: DateTime<Utc>,
    ) -> Vec<(Freq, RawBar)> {
        let mut closed = Vec::new();
        for &freq in &self.time_freqs {
            let Some(minutes) = freq.minutes() else { continue };
            let bucket = time_bucket(dt, minutes);

            if let Some(partial) = self.partials.get(&freq) {
                if partial.start_time != bucket {
                    let old = self.partials.remove(&freq).unwrap();
                    let bar = old.finalize(self.next_id, self.symbol.clone(), freq, bucket);
                    self.next_id += 1;
                    self.completed.entry(freq).or_default().push(bar.clone());
                    closed.push((freq, bar));
                }
            }

            let entry = self.partials.entry(freq).or_insert_with(|| {
                PartialBar::new(price, vol, amount, oi, bucket)
            });
            entry.update(price, vol, amount, oi, delta);
        }
        closed
    }

    fn update_volume_mode(
        &mut self,
        price: f64,
        vol: f64,
        amount: f64,
        oi: Option<f64>,
        delta: f64,
        dt: DateTime<Utc>,
    ) -> Vec<(Freq, RawBar)> {
        let mut closed = Vec::new();
        let tag: Freq = Freq::F1;
        let entry = self.partials.entry(tag).or_insert_with(|| {
            PartialBar::new(price, vol, amount, oi, dt)
        });
        entry.update(price, vol, amount, oi, delta);

        if entry.vol >= self.agg_params.volume_threshold {
            let bar = entry.finalize(self.next_id, self.symbol.clone(), tag, dt);
            self.next_id += 1;
            self.completed.entry(tag).or_default().push(bar.clone());
            closed.push((tag, bar));
            self.partials.remove(&tag);
        }
        closed
    }

    fn update_range_mode(
        &mut self,
        price: f64,
        vol: f64,
        amount: f64,
        oi: Option<f64>,
        delta: f64,
        dt: DateTime<Utc>,
    ) -> Vec<(Freq, RawBar)> {
        let mut closed = Vec::new();
        let tag: Freq = Freq::F1;
        let entry = self.partials.entry(tag).or_insert_with(|| {
            PartialBar::new(price, vol, amount, oi, dt)
        });
        entry.update(price, vol, amount, oi, delta);

        let range = (entry.high - entry.low).abs();
        if range >= self.agg_params.range_threshold {
            let bar = entry.finalize(self.next_id, self.symbol.clone(), tag, dt);
            self.next_id += 1;
            self.completed.entry(tag).or_default().push(bar.clone());
            closed.push((tag, bar));
            self.partials.remove(&tag);
        }
        closed
    }

    pub fn force_close(&mut self, freq: Freq) -> Option<RawBar> {
        self.partials.remove(&freq).map(|partial| {
            let now = Utc::now();
            let bar = partial.finalize(self.next_id, self.symbol.clone(), freq, now);
            self.next_id += 1;
            self.completed.entry(freq).or_default().push(bar.clone());
            bar
        })
    }

    pub fn bars(&self, freq: &Freq) -> &[RawBar] {
        self.completed.get(freq).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn clear_bars(&mut self, freq: &Freq) {
        self.completed.remove(freq);
    }

    pub fn configured_freqs(&self) -> &[Freq] {
        &self.time_freqs
    }
}

// ── 辅助函数 ──

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

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64, oi: f64) -> TickData {
        TickData {
            instrument: "rb9999".into(),
            trading_day: "20260722".into(),
            exchange_id: "SHFE".into(),
            exchange_inst_id: "rb9999".into(),
            last_price: price,
            volume: vol,
            turnover: amount,
            open_interest: oi,
            timestamp_ms: ts_ms,
            ..TickData::default()
        }
    }

    fn ts(hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn test_time_agg_1min() {
        let symbol = Symbol::from("rb9999");
        let mut gen = BarGenerator::new(symbol, vec![AggMode::Time], vec![Freq::F1]);
        let start = ts(9, 0, 0);
        // 121 ticks at 500ms: 09:00:00 → 09:01:00 (cross 1min boundary)
        let mut total_closed = 0;
        for i in 0..121 {
            let t = start + i as i64 * 500;
            let closed = gen.update_tick(&make_tick(t, 4000.0 + i as f64 * 0.1, 100.0 * (i + 1) as f64, 400_000.0, 5000.0));
            total_closed += closed.len();
            // i=120 (t=09:01:00.000) should close 09:00 bar
            if i == 120 {
                assert_eq!(closed.len(), 1, "should close at 1min boundary");
                let (_f, bar) = &closed[0];
                assert!((bar.open - 4000.0).abs() < 0.01);
            }
        }
        assert!(total_closed >= 1);
    }

    #[test]
    fn test_time_agg_multi_freq() {
        let symbol = Symbol::from("rb9999");
        let mut gen = BarGenerator::new(symbol, vec![AggMode::Time], vec![Freq::F1, Freq::F5]);
        for min in 0..5 {
            let base_vol = 100.0 * (min * 2 + 1) as f64;
            gen.update_tick(&make_tick(ts(9, min, 0), 4000.0 + min as f64, base_vol, 400_000.0, 5000.0));
            gen.update_tick(&make_tick(ts(9, min, 30), 4000.0 + min as f64, base_vol + 50.0, 400_000.0, 5000.0));
        }
        // 09:05 → crosses both F1 and F5 boundaries
        let closed = gen.update_tick(&make_tick(ts(9, 5, 0), 4010.0, 600.0, 400_000.0, 5000.0));
        assert_eq!(closed.len(), 2, "should close F1 and F5 bars");
    }

    #[test]
    fn test_volume_agg() {
        let symbol = Symbol::from("rb9999");
        let params = AggParams { volume_threshold: 500.0, ..AggParams::default() };
        let mut gen = BarGenerator::new_with_params(symbol, vec![AggMode::Volume], vec![], params);
        // 累计成交量从 0 开始递增，每笔 +100
        // tick 0: cum_vol=0 建立基线 → bar.vol=0
        // tick 5: cum_vol=500 → bar.vol=500 闭合
        for i in 0..6 {
            let cum_vol = 100.0 * i as f64;
            let closed = gen.update_tick(&make_tick(ts(9, 0, i as u32), 4000.0, cum_vol, 400_000.0, 5000.0));
            if i < 5 {
                assert_eq!(closed.len(), 0, "i={}: no close yet (vol={})", i, cum_vol);
            } else {
                assert_eq!(closed.len(), 1, "i={}: should close at vol 500", i);
                let (_f, bar) = &closed[0];
                assert!((bar.vol - 500.0).abs() < 0.01, "bar vol should be 500, got {}", bar.vol);
            }
        }
    }

    #[test]
    fn test_range_agg() {
        let symbol = Symbol::from("rb9999");
        let params = AggParams { range_threshold: 10.0, ..AggParams::default() };
        let mut gen = BarGenerator::new_with_params(symbol, vec![AggMode::Range], vec![], params);
        let c1 = gen.update_tick(&make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0));
        assert_eq!(c1.len(), 0);
        let c2 = gen.update_tick(&make_tick(ts(9, 0, 1), 4010.0, 100.0, 400_000.0, 5000.0));
        assert_eq!(c2.len(), 1);
        assert!((c2[0].1.high - c2[0].1.low - 10.0).abs() < 0.01, "range should be 10");
    }

    #[test]
    fn test_force_close() {
        let symbol = Symbol::from("rb9999");
        let mut gen = BarGenerator::new(symbol, vec![AggMode::Time], vec![Freq::F1]);
        gen.update_tick(&make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0));
        let bar = gen.force_close(Freq::F1);
        assert!(bar.is_some());
        assert!((bar.unwrap().open - 4000.0).abs() < 0.01);
        assert!(gen.force_close(Freq::F1).is_none());
    }

    #[test]
    fn test_cross_day() {
        let symbol = Symbol::from("rb9999");
        let mut gen = BarGenerator::new(symbol, vec![AggMode::Time], vec![Freq::F5]);
        let t1 = Utc.with_ymd_and_hms(2026, 7, 22, 23, 58, 0).unwrap().timestamp_millis();
        gen.update_tick(&make_tick(t1, 4000.0, 100.0, 400_000.0, 5000.0));
        let t2 = Utc.with_ymd_and_hms(2026, 7, 23, 0, 1, 0).unwrap().timestamp_millis();
        let closed = gen.update_tick(&make_tick(t2, 4010.0, 200.0, 802_000.0, 5000.0));
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].1.dt.day(), 23);
    }

    #[test]
    fn test_volume_rollback() {
        let symbol = Symbol::from("rb9999");
        let mut gen = BarGenerator::new(symbol, vec![AggMode::Time], vec![Freq::F1]);
        // 主力换月：cum_vol 从 200 回退到 100 → bar vol = max(0,100-200) = 0
        // 基线 tick 在 09:00，第二笔在 09:01 跨边界闭合 bar
        gen.update_tick(&make_tick(ts(9, 0, 0), 4000.0, 200.0, 800_000.0, 5000.0));
        gen.update_tick(&make_tick(ts(9, 1, 0), 4010.0, 100.0, 400_000.0, 5000.0));
        let bars = gen.bars(&Freq::F1);
        assert_eq!(bars.len(), 1);
        // 首笔基线 vol=0，第二笔增量 max(0,100-200)=0
        assert_eq!(bars[0].vol, 0.0, "rollback should yield 0 vol");
    }
}
