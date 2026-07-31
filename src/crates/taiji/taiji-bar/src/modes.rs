//! AggMode — 三种聚合模式。
//!
//! 参考: taiji-engine/pipeline/bar_gen.rs:16-20 AggMode 枚举
//! 参考: dvmi_source.txt 时间/量/幅度三种模式描述
//! 参考: 量价时空/Phase-2-派发提示词.md:429 — R-2-203 — BarGenerator tick→K线

/// K 线聚合模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggMode {
    /// 时间模式 — 按固定时间窗口聚合（1min/5min/1h 等）。
    Time,
    /// 成交量模式 — 每累计 N 手成交闭合一根 K 线。
    Volume,
    /// 价格幅度模式 — 每波动 N 个 tick/点闭合一根 K 线。
    Range,
}

/// 聚合参数 — 配合 AggMode 使用。
#[derive(Debug, Clone, Copy)]
pub struct AggParams {
    /// 成交量模式：触发聚合的累计成交量阈值。
    pub volume_threshold: f64,
    /// 幅度模式：触发聚合的价格变动阈值。
    pub range_threshold: f64,
}

impl Default for AggParams {
    fn default() -> Self {
        Self {
            volume_threshold: 1000.0,
            range_threshold: 10.0,
        }
    }
}
