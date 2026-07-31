//! BarComposer — 低→高周期 K 线合成。
//!
//! 参考: czsc 多周期合成算法 + taiji-engine/pipeline/bar_gen.rs:211-220 time_bucket()
//! 参考: 量价时空/Phase-2-派发提示词.md:429 — R-2-203 — BarGenerator tick→K线
//!
//! 输入低周期 bar（如 1min），输出高周期 bar（如 5min/15min/60min）。
//! OHLCV 一致性：open=低周期第一根 open，high=所有低周期 high 最大值，
//! low=最小值，close=最后一根 close，vol/amount 累加。

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use taiji_engine::types::bar::{Freq, RawBar};

/// 低→高周期 K 线合成器。
pub struct BarComposer {
    /// 目标高周期。
    target_freq: Freq,
    /// 已缓存的低周期 bars。
    buffer: Vec<RawBar>,
    /// 上一个合成完成的高周期 bar 的结束时间。
    last_composed: Option<DateTime<Utc>>,
}

impl BarComposer {
    pub fn new(target_freq: Freq) -> Self {
        Self {
            target_freq,
            buffer: Vec::new(),
            last_composed: None,
        }
    }

    /// 喂入一根低周期 bar，返回新合成的高周期 bars。
    pub fn feed(&mut self, bar: RawBar) -> Vec<RawBar> {
        let mut composed = Vec::new();
        let Some(minutes) = self.target_freq.minutes() else {
            return composed;
        };

        let bucket = time_bucket(bar.dt, minutes);

        // 检查是否跨桶：buffer 中的 bar 属于不同桶时合成
        if let Some(first) = self.buffer.first() {
            let first_bucket = time_bucket(first.dt, minutes);
            if first_bucket != bucket {
                if let Some(composed_bar) = self.compose_current() {
                    composed.push(composed_bar);
                }
            }
        }

        self.buffer.push(bar);
        composed
    }

    /// 强制合成当前 buffer。
    pub fn flush(&mut self) -> Option<RawBar> {
        if self.buffer.is_empty() {
            return None;
        }
        self.compose_current()
    }

    fn compose_current(&mut self) -> Option<RawBar> {
        if self.buffer.is_empty() {
            return None;
        }

        let first = self.buffer.first().unwrap();
        let last = self.buffer.last().unwrap();

        let high = self.buffer.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let low = self.buffer.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let vol: f64 = self.buffer.iter().map(|b| b.vol).sum();
        let amount: f64 = self.buffer.iter().map(|b| b.amount).sum();
        let delta: f64 = self.buffer.iter().filter_map(|b| b.delta).sum();

        // 高周期 bar 的 dt = 桶边界
        let minutes = self.target_freq.minutes()?;
        let dt = time_bucket(last.dt, minutes);

        let composed_bar = RawBar {
            symbol: first.symbol.clone(),
            dt,
            freq: self.target_freq,
            id: first.id,
            open: first.open,
            high,
            low,
            close: last.close,
            vol,
            amount,
            open_interest: last.open_interest,
            delta: if delta != 0.0 { Some(delta) } else { None },
        };

        self.buffer.clear();
        self.last_composed = Some(dt);
        Some(composed_bar)
    }

    pub fn target_freq(&self) -> Freq {
        self.target_freq
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

/// 时间桶边界（与 bargen.rs 中一致）。
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use taiji_engine::types::bar::Symbol;

    fn make_bar(
        dt: DateTime<Utc>,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        vol: f64,
        amount: f64,
    ) -> RawBar {
        RawBar {
            symbol: Symbol::from("rb9999"),
            dt,
            freq: Freq::F1,
            id: 0,
            open,
            high,
            low,
            close,
            vol,
            amount,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_1min_to_5min() {
        let mut composer = BarComposer::new(Freq::F5);
        let start = Utc.with_ymd_and_hms(2026, 7, 22, 9, 0, 0).unwrap();

        // 喂 3 根 1min bars（09:00, 09:01, 09:02）— 同属 5min 桶 09:00
        let b1 = make_bar(start, 4000.0, 4010.0, 3990.0, 4005.0, 100.0, 400_000.0);
        let r1 = composer.feed(b1);
        assert!(r1.is_empty(), "first bar no compose yet");

        let b2 = make_bar(
            start + chrono::Duration::minutes(1),
            4005.0, 4020.0, 4000.0, 4015.0, 200.0, 800_000.0,
        );
        let r2 = composer.feed(b2);
        assert!(r2.is_empty(), "second bar no compose yet");

        // 09:03 bar → 仍在 09:00 桶内
        let b3 = make_bar(
            start + chrono::Duration::minutes(3),
            4015.0, 4025.0, 4010.0, 4020.0, 150.0, 600_000.0,
        );
        let r3 = composer.feed(b3);
        assert!(r3.is_empty(), "third bar still same bucket");

        // 09:05 bar → 跨桶，触发合成
        let b4 = make_bar(
            start + chrono::Duration::minutes(5),
            4020.0, 4030.0, 4015.0, 4025.0, 180.0, 720_000.0,
        );
        let r4 = composer.feed(b4);
        assert_eq!(r4.len(), 1, "should compose 1 bar at bucket boundary");
        let composed = &r4[0];
        assert_eq!(composed.freq, Freq::F5);
        assert!((composed.open - 4000.0).abs() < 0.01, "open should be first bar's open");
        assert!((composed.high - 4025.0).abs() < 0.01, "high should be max");
        assert!((composed.low - 3990.0).abs() < 0.01, "low should be min");
        assert!((composed.close - 4020.0).abs() < 0.01, "close should be last bar's close");
        assert!((composed.vol - 450.0).abs() < 0.01, "vol should be sum");
    }

    #[test]
    fn test_flush() {
        let mut composer = BarComposer::new(Freq::F5);
        let start = Utc.with_ymd_and_hms(2026, 7, 22, 9, 0, 0).unwrap();

        composer.feed(make_bar(start, 4000.0, 4010.0, 3990.0, 4005.0, 100.0, 400_000.0));
        composer.feed(make_bar(
            start + chrono::Duration::minutes(1),
            4005.0, 4020.0, 4000.0, 4015.0, 200.0, 800_000.0,
        ));

        let bar = composer.flush();
        assert!(bar.is_some());
        let bar = bar.unwrap();
        assert!((bar.open - 4000.0).abs() < 0.01);
        assert!((bar.vol - 300.0).abs() < 0.01);

        // flush 后 buffer 为空
        assert!(composer.flush().is_none());
    }
}
