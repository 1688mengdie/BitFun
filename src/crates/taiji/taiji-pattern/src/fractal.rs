//! 缠论分型检测
//!
//! 分型定义：
//! - **顶分型**（TopFractal）：`bar[i].high > bar[i-1].high && bar[i].high > bar[i+1].high`
//! - **底分型**（BottomFractal）：`bar[i].low  < bar[i-1].low  && bar[i].low  < bar[i+1].low`
//!
//! 首尾两根 bar 无法形成分型（需要前后邻居）。
//! 参考: 量价时空/Phase-2-派发提示词.md:770 — R-2-501 — taiji-pattern ComputeNode

use taiji_engine::types::bar::RawBar;

/// 分型方向
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FractalDirection {
    Top,    // 顶分型
    Bottom, // 底分型
}

/// 分型
#[derive(Debug, Clone)]
pub struct Fractal {
    /// 在原始 bar 序列中的索引
    pub index: usize,
    /// 分型方向
    pub direction: FractalDirection,
    /// 分型价格（顶分型=high，底分型=low）
    pub price: f64,
    /// 对应的时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 从 K 线序列中检测分型。
///
/// 返回的 Vec 按 index 升序排列。
pub fn detect_fractals(bars: &[RawBar]) -> Vec<Fractal> {
    let n = bars.len();
    if n < 3 {
        return vec![];
    }

    let mut fractals = Vec::new();

    // 首尾 bar 无法形成分型，遍历 1..n-1
    for i in 1..n - 1 {
        let prev = &bars[i - 1];
        let curr = &bars[i];
        let next = &bars[i + 1];

        // 顶分型：curr.high 高于两侧
        if curr.high > prev.high && curr.high > next.high {
            fractals.push(Fractal {
                index: i,
                direction: FractalDirection::Top,
                price: curr.high,
                timestamp: curr.dt,
            });
        }

        // 底分型：curr.low 低于两侧
        if curr.low < prev.low && curr.low < next.low {
            fractals.push(Fractal {
                index: i,
                direction: FractalDirection::Bottom,
                price: curr.low,
                timestamp: curr.dt,
            });
        }
    }

    fractals
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};
    use taiji_engine::types::bar::{Freq, Symbol};

    fn make_bar(high: f64, low: f64, dt: DateTime<Utc>) -> RawBar {
        RawBar {
            symbol: Symbol::from("test"),
            dt,
            freq: Freq::F5,
            id: 0,
            open: (high + low) / 2.0,
            high,
            low,
            close: (high + low) / 2.0,
            vol: 0.0,
            amount: 0.0,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_empty_bars() {
        assert!(detect_fractals(&[]).is_empty());
    }

    #[test]
    fn test_less_than_3_bars() {
        let t = Utc.with_ymd_and_hms(2026, 7, 22, 9, 0, 0).unwrap();
        let bars = vec![make_bar(100.0, 99.0, t)];
        assert!(detect_fractals(&bars).is_empty());
    }

    #[test]
    fn test_detect_top_fractal() {
        // bar[1] high=105 > bar[0].high=100 && > bar[2].high=103 → Top
        let t = |h, m| Utc.with_ymd_and_hms(2026, 7, 22, h, m, 0).unwrap();
        let bars = vec![
            make_bar(100.0, 98.0, t(9, 0)),
            make_bar(105.0, 99.0, t(9, 5)), // Top fractal
            make_bar(103.0, 97.0, t(9, 10)),
        ];
        let f = detect_fractals(&bars);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].direction, FractalDirection::Top);
        assert_eq!(f[0].index, 1);
        assert!((f[0].price - 105.0).abs() < 1e-9);
    }

    #[test]
    fn test_detect_bottom_fractal() {
        // bar[1] low=95 < bar[0].low=98 && < bar[2].low=97 → Bottom
        let t = |h, m| Utc.with_ymd_and_hms(2026, 7, 22, h, m, 0).unwrap();
        let bars = vec![
            make_bar(102.0, 98.0, t(9, 0)),
            make_bar(101.0, 95.0, t(9, 5)), // Bottom fractal
            make_bar(103.0, 97.0, t(9, 10)),
        ];
        let f = detect_fractals(&bars);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].direction, FractalDirection::Bottom);
        assert_eq!(f[0].index, 1);
        assert!((f[0].price - 95.0).abs() < 1e-9);
    }

    #[test]
    fn test_detect_both_types() {
        // bar[1] is top, bar[3] is top, bar[4] is bottom, bar[7] is top
        let t = |h, m| Utc.with_ymd_and_hms(2026, 7, 22, h, m, 0).unwrap();
        let bars = vec![
            make_bar(100.0, 98.0, t(9, 0)),
            make_bar(110.0, 99.0, t(9, 5)),  // top (110 > 100 && 110 > 105)
            make_bar(105.0, 97.0, t(9, 10)),
            make_bar(106.0, 96.0, t(9, 15)), // top (106 > 105 && 106 > 104)
            make_bar(104.0, 92.0, t(9, 20)), // bottom (92 < 96 && 92 < 94)
            make_bar(107.0, 94.0, t(9, 25)),
            make_bar(108.0, 95.0, t(9, 30)),
            make_bar(115.0, 96.0, t(9, 35)), // top (115 > 108 && 115 > 112)
            make_bar(112.0, 97.0, t(9, 40)),
        ];
        let f = detect_fractals(&bars);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].direction, FractalDirection::Top);
        assert_eq!(f[0].index, 1);
        assert_eq!(f[1].direction, FractalDirection::Top);
        assert_eq!(f[1].index, 3);
        assert_eq!(f[2].direction, FractalDirection::Bottom);
        assert_eq!(f[2].index, 4);
        assert_eq!(f[3].direction, FractalDirection::Top);
        assert_eq!(f[3].index, 7);
    }

    #[test]
    fn test_no_fractal_on_flat_top() {
        // bar[1] high == bar[0].high → not strictly greater → no top fractal
        let t = |h, m| Utc.with_ymd_and_hms(2026, 7, 22, h, m, 0).unwrap();
        let bars = vec![
            make_bar(100.0, 98.0, t(9, 0)),
            make_bar(100.0, 99.0, t(9, 5)),
            make_bar(99.0, 97.0, t(9, 10)),
        ];
        let f = detect_fractals(&bars);
        // bar[1].high (100) == bar[0].high (100), not > → no top fractal
        // bar[1].low (99) > bar[0].low (98) and > bar[2].low (97) → no bottom fractal
        assert_eq!(f.len(), 0);
    }
}
