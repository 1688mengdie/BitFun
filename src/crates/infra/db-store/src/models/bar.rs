//! K 线数据模型
//!
//! 来源: czsc-core/src/objects/bar.rs:34-52 (Apache 2.0)
//! 来源: czsc-core/src/objects/freq.rs:40-104 (Apache 2.0)
//! 来源: modules/db-store/接口设计.md:126-170 — Freq + RawBar

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 时间周期（LVPA: K 线聚合的时间单位）
///
/// 来源: czsc-core/src/objects/freq.rs:40-104
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Freq {
    Tick,
    F1, F2, F3, F4, F5, F6,
    F10, F12, F15, F20, F30, F60, F120, F240, F360,
    /// 日线
    D,
    /// 周线
    W,
    /// 月线
    M,
    /// 季线
    S,
    /// 年线
    Y,
}

impl Freq {
    /// 判断是否为分钟级别
    ///
    /// 来源: freq.rs:112-133
    pub fn is_minute_freq(&self) -> bool {
        matches!(self, Freq::F1 | Freq::F2 | Freq::F3 | Freq::F4 | Freq::F5 | Freq::F6
            | Freq::F10 | Freq::F12 | Freq::F15 | Freq::F20 | Freq::F30
            | Freq::F60 | Freq::F120 | Freq::F240 | Freq::F360)
    }

    /// 获取对应的分钟数
    ///
    /// 来源: freq.rs:136-155
    pub fn minutes(&self) -> Option<i64> {
        match self {
            Freq::F1 => Some(1),    Freq::F2 => Some(2),
            Freq::F3 => Some(3),    Freq::F4 => Some(4),
            Freq::F5 => Some(5),    Freq::F6 => Some(6),
            Freq::F10 => Some(10),  Freq::F12 => Some(12),
            Freq::F15 => Some(15),  Freq::F20 => Some(20),
            Freq::F30 => Some(30),  Freq::F60 => Some(60),
            Freq::F120 => Some(120), Freq::F240 => Some(240),
            Freq::F360 => Some(360),
            _ => None,
        }
    }

    /// 获取 SQLite 表名后缀（用于分表）
    ///
    /// 来源: modules/db-store/接口设计.md:160-169 — table_suffix
    pub fn table_suffix(&self) -> &'static str {
        match self {
            Freq::F1 => "1min",    Freq::F5 => "5min",
            Freq::F15 => "15min",  Freq::F30 => "30min",
            Freq::F60 => "60min",  Freq::D  => "day",
            Freq::W  => "week",    Freq::M  => "month",
            _ => "other",
        }
    }

    /// 生成 K 线表名
    ///
    /// 格式: klines_{freq_suffix}_{yyyymm}
    pub fn kline_table_name(&self, yyyymm: &str) -> String {
        format!("klines_{}_{}", self.table_suffix(), yyyymm)
    }
}

/// 原始 K 线元素
///
/// LVPA: 灵脉中存储的基础市场数据单元。
/// 来源: czsc-core/src/objects/bar.rs:34-52
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBar {
    /// 合约代码
    pub symbol: String,
    /// 时间戳
    pub dt: DateTime<Utc>,
    /// 周期
    pub freq: Freq,
    /// 升序 ID
    pub id: i32,
    /// 开盘价
    pub open: f64,
    /// 收盘价
    pub close: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 成交量
    pub vol: f64,
    /// 成交额
    pub amount: f64,
    /// 持仓量（期货）
    ///
    /// v2.4 新增字段
    pub open_interest: Option<f64>,
    /// 成交笔数
    ///
    /// v2.4 新增字段
    pub trade_count: Option<u64>,
}

impl RawBar {
    /// 上影线
    ///
    /// 来源: bar.rs:56-58
    pub fn upper(&self) -> f64 {
        self.high - self.open.max(self.close)
    }

    /// 下影线
    ///
    /// 来源: bar.rs:61-63
    pub fn lower(&self) -> f64 {
        self.open.min(self.close) - self.low
    }

    /// 实体
    ///
    /// 来源: bar.rs:66-68
    pub fn solid(&self) -> f64 {
        (self.open - self.close).abs()
    }
}

/// K 线更新事件（用于 L1→L2 落盘通道）
///
/// 来源: modules/db-store/接口设计.md:550-557 — BarUpdate
#[derive(Debug, Clone)]
pub struct BarUpdate {
    pub symbol: String,
    pub freq: Freq,
    pub bars: Vec<RawBar>,
    pub batch_id: u64,
}

/// 缓冲统计
///
/// 来源: modules/db-store/接口设计.md:539-548 — BufferStats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    pub total_entries: usize,
    pub capacity: usize,
    pub push_count: u64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub flush_pending: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_conversion() {
        assert!(Freq::F1.is_minute_freq());
        assert!(!Freq::D.is_minute_freq());
        assert_eq!(Freq::F30.minutes(), Some(30));
        assert_eq!(Freq::D.minutes(), None);
    }

    #[test]
    fn test_table_suffix() {
        assert_eq!(Freq::F1.table_suffix(), "1min");
        assert_eq!(Freq::D.table_suffix(), "day");
        assert_eq!(Freq::W.table_suffix(), "week");
    }

    #[test]
    fn test_kline_table_name() {
        let name = Freq::F1.kline_table_name("202607");
        assert_eq!(name, "klines_1min_202607");
    }

    #[test]
    fn test_raw_bar_upper_lower_solid() {
        let bar = RawBar {
            symbol: "BTC-USDT".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 0,
            open: 10.0,
            close: 12.0,
            high: 15.0,
            low: 8.0,
            vol: 100.0,
            amount: 1000.0,
            open_interest: None,
            trade_count: None,
        };
        assert!((bar.upper() - 3.0).abs() < f64::EPSILON);
        assert!((bar.lower() - 2.0).abs() < f64::EPSILON);
        assert!((bar.solid() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bar_update() {
        let update = BarUpdate {
            symbol: "RB".into(),
            freq: Freq::F5,
            bars: vec![],
            batch_id: 1,
        };
        assert_eq!(update.symbol, "RB");
        assert_eq!(update.freq, Freq::F5);
    }

    #[test]
    fn test_buffer_stats() {
        let stats = BufferStats {
            total_entries: 100,
            capacity: 1000,
            push_count: 500,
            hit_count: 400,
            miss_count: 100,
            flush_pending: 0,
        };
        assert_eq!(stats.total_entries, 100);
        assert_eq!(stats.capacity, 1000);
    }
}
