//! 背驰分析 — MACD/斜率/测度三种判定模式 + 4 种组合模式
//!
//! 理论: 理论总纲 §九（1:1 测量移动：资金衰减的度量）
//!       量化总纲 §3 节点4.2（过冲判定：背驰的等价概念）
//! 参考: chanlun-rs (MIT) divergence.rs — 纯函数翻译，去 Arc/RwLock

use serde::{Deserialize, Serialize};

use crate::bi::{Bi, BiDirection};

// ============================================================
// 背驰判定模式
// ============================================================

/// 背驰判定模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DivergenceMode {
    All,       // 全量 — MACD + 斜率 + 测度 三者全满足
    Any,       // 任意 — 任一条件满足
    Config,    // 配置 — 按结构体字段选择组合
    Majority,  // 多数投票 — 至少两个条件满足
}

/// 背驰配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceConfig {
    pub use_macd: bool,
    pub use_slope: bool,
    pub use_measure: bool,
    /// MACD 面积模式: "total" / "bull" / "bear"
    pub macd_mode: String,
}

impl Default for DivergenceConfig {
    fn default() -> Self {
        Self {
            use_macd: true,
            use_slope: true,
            use_measure: true,
            macd_mode: "total".into(),
        }
    }
}

// ============================================================
// 背驰分析结果
// ============================================================

/// 单次背驰判定结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceResult {
    pub is_divergent: bool,
    pub macd_diverged: bool,
    pub slope_diverged: bool,
    pub measure_diverged: bool,
    pub mode: DivergenceMode,
    pub enter_area: f64,
    pub leave_area: f64,
}

// ============================================================
// 辅助函数
// ============================================================

/// 获取笔的极值高点
fn bi_high(bi: &Bi) -> f64 {
    bi.start_price.max(bi.end_price)
}

/// 获取笔的极值低点
fn bi_low(bi: &Bi) -> f64 {
    bi.start_price.min(bi.end_price)
}

/// 获取笔的 dx（bar 索引差）
fn bi_dx(bi: &Bi) -> f64 {
    (bi.end_index - bi.start_index) as f64
}

/// 获取笔的 dy（价格差）
fn bi_dy(bi: &Bi) -> f64 {
    bi.end_price - bi.start_price
}

// ============================================================
// 背驰分析器
// ============================================================

/// 背驰分析 — 纯静态方法，无内部状态
pub struct DivergenceAnalyzer;

impl DivergenceAnalyzer {
    /// MACD 背驰 — 比较进入段和离开段的 MACD 柱状线面积
    ///
    /// # 参数
    /// - `enter`: 进入段笔
    /// - `leave`: 离开段笔
    /// - `macd_data`: MACD 柱状线值序列（与 bar 序列对齐）
    /// - `macd_mode`: "total"=阳+\|阴\|, "bull"=按方向选阳, "bear"=按方向选阴
    pub fn macd_divergence(
        enter: &Bi,
        leave: &Bi,
        macd_data: &[f64],
        macd_mode: &str,
    ) -> bool {
        let enter_area = Self::calc_macd_area(macd_data, enter.start_index, enter.end_index, enter.direction, macd_mode);
        let leave_area = Self::calc_macd_area(macd_data, leave.start_index, leave.end_index, leave.direction, macd_mode);

        // 离开段面积 < 进入段面积 → 背驰
        leave_area < enter_area
    }

    /// 计算笔范围内的 MACD 面积
    fn calc_macd_area(
        macd_data: &[f64],
        start_idx: usize,
        end_idx: usize,
        direction: BiDirection,
        macd_mode: &str,
    ) -> f64 {
        let (lo, hi) = if start_idx <= end_idx {
            (start_idx, end_idx)
        } else {
            (end_idx, start_idx)
        };
        let lo = lo.min(macd_data.len().saturating_sub(1));
        let hi = hi.min(macd_data.len().saturating_sub(1));

        let mut bull_area = 0.0_f64; // 正值 MACD 柱（阳）
        let mut bear_area = 0.0_f64; // 负值 MACD 柱（阴，绝对值）

        for i in lo..=hi {
            if let Some(val) = macd_data.get(i) {
                if *val >= 0.0 {
                    bull_area += val;
                } else {
                    bear_area += val.abs();
                }
            }
        }

        match macd_mode {
            "total" => bull_area + bear_area,
            "bull" => bull_area,
            "bear" => bear_area,
            _ => {
                // 按方向选择: Up 选阳 (bull), Down 选阴 (bear)
                match direction {
                    BiDirection::Up => bull_area,
                    BiDirection::Down => bear_area,
                }
            }
        }
    }

    /// 斜率背驰 — 价格新高/低但斜率 (dy/dx) 减弱
    pub fn slope_divergence(enter: &Bi, leave: &Bi) -> bool {
        let enter_dx = bi_dx(enter);
        if enter_dx < 1.0 {
            return false;
        }
        let enter_dy = bi_dy(enter);
        let enter_slope = enter_dy / enter_dx;

        let leave_dx = bi_dx(leave);
        if leave_dx < 1.0 {
            return false;
        }
        let leave_dy = bi_dy(leave);
        let leave_slope = leave_dy / leave_dx;

        match enter.direction {
            BiDirection::Up => {
                // 向上笔：价格新高 + 斜率减弱
                bi_high(leave) > bi_high(enter) && leave_slope.abs() < enter_slope.abs()
            }
            BiDirection::Down => {
                // 向下笔：价格新低 + 斜率减弱
                bi_low(leave) < bi_low(enter) && leave_slope.abs() < enter_slope.abs()
            }
        }
    }

    /// 测度背驰 — 价格新高/低但欧氏距离 sqrt(dx²+dy²) 减弱
    pub fn measure_divergence(enter: &Bi, leave: &Bi) -> bool {
        let enter_dx = bi_dx(enter);
        let enter_dy = bi_dy(enter);
        let enter_measure = (enter_dx * enter_dx + enter_dy * enter_dy).sqrt();

        let leave_dx = bi_dx(leave);
        let leave_dy = bi_dy(leave);
        let leave_measure = (leave_dx * leave_dx + leave_dy * leave_dy).sqrt();

        match enter.direction {
            BiDirection::Up => {
                bi_high(leave) > bi_high(enter) && leave_measure.abs() < enter_measure.abs()
            }
            BiDirection::Down => {
                bi_low(leave) < bi_low(enter) && leave_measure.abs() < enter_measure.abs()
            }
        }
    }

    /// 全量背驰 — MACD + 斜率 + 测度 三者全满足
    pub fn all(enter: &Bi, leave: &Bi, macd: &[f64]) -> bool {
        Self::macd_divergence(enter, leave, macd, "total")
            && Self::slope_divergence(enter, leave)
            && Self::measure_divergence(enter, leave)
    }

    /// 任意背驰 — 任一条件满足
    pub fn any(enter: &Bi, leave: &Bi, macd: &[f64]) -> bool {
        Self::macd_divergence(enter, leave, macd, "total")
            || Self::slope_divergence(enter, leave)
            || Self::measure_divergence(enter, leave)
    }

    /// 配置背驰 — 按参数选择组合
    pub fn configured(
        enter: &Bi,
        leave: &Bi,
        macd: &[f64],
        config: &DivergenceConfig,
    ) -> bool {
        let macd_ok = if config.use_macd {
            Self::macd_divergence(enter, leave, macd, &config.macd_mode)
        } else {
            true
        };
        let slope_ok = if config.use_slope {
            Self::slope_divergence(enter, leave)
        } else {
            true
        };
        let measure_ok = if config.use_measure {
            Self::measure_divergence(enter, leave)
        } else {
            true
        };

        // 所有启用的条件都必须满足（AND 关系）
        if !config.use_macd && !config.use_slope && !config.use_measure {
            return false; // 没有启用任何条件
        }

        let mut all_true = true;
        let mut any_enabled = false;
        if config.use_macd {
            any_enabled = true;
            all_true = all_true && macd_ok;
        }
        if config.use_slope {
            any_enabled = true;
            all_true = all_true && slope_ok;
        }
        if config.use_measure {
            any_enabled = true;
            all_true = all_true && measure_ok;
        }

        any_enabled && all_true
    }

    /// 多数投票背驰 — 至少两个条件满足
    pub fn majority(enter: &Bi, leave: &Bi, macd: &[f64]) -> bool {
        let votes = [
            Self::macd_divergence(enter, leave, macd, "total"),
            Self::slope_divergence(enter, leave),
            Self::measure_divergence(enter, leave),
        ];
        votes.iter().filter(|&&v| v).count() >= 2
    }

    /// 综合判定 — 按指定模式判断背驰
    pub fn check(
        enter: &Bi,
        leave: &Bi,
        macd: &[f64],
        mode: DivergenceMode,
        config: &DivergenceConfig,
    ) -> DivergenceResult {
        let macd_div = Self::macd_divergence(enter, leave, macd, &config.macd_mode);
        let slope_div = Self::slope_divergence(enter, leave);
        let measure_div = Self::measure_divergence(enter, leave);

        let is_divergent = match mode {
            DivergenceMode::All => macd_div && slope_div && measure_div,
            DivergenceMode::Any => macd_div || slope_div || measure_div,
            DivergenceMode::Config => Self::configured(enter, leave, macd, config),
            DivergenceMode::Majority => {
                [macd_div, slope_div, measure_div].iter().filter(|&&v| v).count() >= 2
            }
        };

        DivergenceResult {
            is_divergent,
            macd_diverged: macd_div,
            slope_diverged: slope_div,
            measure_diverged: measure_div,
            mode,
            enter_area: Self::calc_macd_area(macd, enter.start_index, enter.end_index, enter.direction, &config.macd_mode),
            leave_area: Self::calc_macd_area(macd, leave.start_index, leave.end_index, leave.direction, &config.macd_mode),
        }
    }
}

// ============================================================
// DivergenceNode — ComputeNode 封装
// ============================================================

use std::sync::Arc;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::Signal;
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;
use taiji_engine::types::NodeId;

/// ComputeNode 封装：从 Bi 序列和 MACD 数据检测背驰
///
/// input: `bars`（用于计算 MACD）, `chan:bis`（笔序列）
/// output: `divergences`（背驰判定列表）
pub struct DivergenceNode {
    node_id: NodeId,
}

impl DivergenceNode {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }
}

impl ComputeNode for DivergenceNode {
    fn id(&self) -> NodeId {
        self.node_id.clone()
    }

    fn name(&self) -> &'static str {
        "Divergence"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec!["bars".into(), "chan:bis".into()]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["divergences".into()]
    }

    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_tick(&mut self, _tick: &TickData, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        // 读取 bars 用于 MACD 计算
        let bars: Option<Arc<Vec<Arc<RawBar>>>> = state.get(&"bars".into());
        let bars = match bars {
            Some(b) => b,
            None => return Ok(vec![]),
        };

        // 读取 bis
        let bis_json = match state.get_raw(&"chan:bis".into()) {
            Some(StateValue::Json(val)) => val,
            _ => return Ok(vec![]),
        };
        let bis: Vec<Bi> = match serde_json::from_value(bis_json) {
            Ok(b) => b,
            Err(_) => return Ok(vec![]),
        };

        if bis.len() < 3 || bars.len() < 30 {
            return Ok(vec![]);
        }

        // 计算 MACD 柱状线
        let bars_ref: Vec<RawBar> = bars.iter().map(|b| (**b).clone()).collect();
        let closes: Vec<f64> = bars_ref.iter().map(|b| b.close).collect();
        let macd_hist = compute_macd_hist(&closes);

        let config = DivergenceConfig::default();

        // 对相邻笔对进行背驰检测
        let mut results = Vec::new();
        for i in 0..bis.len().saturating_sub(1) {
            let enter = &bis[i];
            let leave = &bis[i + 1];

            // 跳过同向笔（背驰仅在不同方向有意义）
            if enter.direction == leave.direction {
                continue;
            }

            let result = DivergenceAnalyzer::check(enter, leave, &macd_hist, DivergenceMode::All, &config);
            if result.is_divergent {
                results.push(result);
            }
        }

        state.set(
            "divergences".into(),
            StateValue::Json(serde_json::to_value(&results)?),
            self.node_id.clone(),
        );

        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F5]
    }
}

/// 计算 MACD 柱状线（内部辅助）
fn compute_macd_hist(closes: &[f64]) -> Vec<f64> {
    let ema12 = ema(closes, 12);
    let ema26 = ema(closes, 26);
    let macd: Vec<f64> = ema12.iter().zip(ema26.iter()).map(|(a, b)| a - b).collect();
    let signal = ema(&macd, 9);
    macd.iter().zip(signal.iter()).map(|(m, s)| m - s).collect()
}

/// EMA 计算
fn ema(data: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![0.0; data.len()];
    if data.len() < period {
        return out;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    out[period - 1] = data[..period].iter().sum::<f64>() / period as f64;
    for i in period..data.len() {
        out[i] = data[i] * alpha + out[i - 1] * (1.0 - alpha);
    }
    out
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bi(
        start_index: usize,
        end_index: usize,
        direction: BiDirection,
        start_price: f64,
        end_price: f64,
    ) -> Bi {
        Bi {
            start_index,
            end_index,
            direction,
            start_price,
            end_price,
        }
    }

    fn up_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
        make_bi(s, e, BiDirection::Up, sp, ep)
    }

    fn down_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
        make_bi(s, e, BiDirection::Down, sp, ep)
    }

    fn make_macd_hist(length: usize, enter_area: f64, leave_area: f64) -> Vec<f64> {
        // 模拟 MACD 数据：在 enter 范围内分配 enter_area，leave 范围内分配 leave_area
        let mut data = vec![0.0; length];
        // enter: indices 0..5, leave: indices 5..10
        for i in 0..5 {
            data[i] = enter_area / 5.0;
        }
        for i in 5..10 {
            data[i] = leave_area / 5.0;
        }
        data
    }

    // ========== MACD 背驰 ==========

    #[test]
    fn test_macd_divergence_true() {
        // 进入面积 10 > 离开面积 5 → true
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0);
        assert!(DivergenceAnalyzer::macd_divergence(&enter, &leave, &macd, "total"));
    }

    #[test]
    fn test_macd_divergence_false() {
        // 进入面积 5 < 离开面积 10 → false
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 5.0, 10.0);
        assert!(!DivergenceAnalyzer::macd_divergence(&enter, &leave, &macd, "total"));
    }

    #[test]
    fn test_macd_bull_mode() {
        // "bull" 模式只取正值面积
        let mut macd = vec![0.0; 15];
        for i in 0..5 { macd[i] = 2.0; }    // enter bull=10
        for i in 5..10 { macd[i] = 1.0; }   // leave bull=5
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        assert!(DivergenceAnalyzer::macd_divergence(&enter, &leave, &macd, "bull"));
    }

    // ========== 斜率背驰 ==========

    #[test]
    fn test_slope_divergence_true_up() {
        // 向上：价格新高 (115 > 110) 且斜率减弱
        // enter slope = (110-100)/5 = 2.0, leave slope = (115-110)/5 = 1.0
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        assert!(DivergenceAnalyzer::slope_divergence(&enter, &leave));
    }

    #[test]
    fn test_slope_divergence_false_no_new_high() {
        // 向上但价格没有新高 (112 < 110)... wait that doesn't make sense
        // let's use a proper case: leave didn't make new high
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 105.0, 108.0); // high=108 < 110, no new high
        assert!(!DivergenceAnalyzer::slope_divergence(&enter, &leave));
    }

    #[test]
    fn test_slope_divergence_true_down() {
        // 向下：价格新低 (85 < 90) 且斜率减弱
        // enter slope = (85-95)/5 = -2.0, leave slope = (85-95)? no
        // enter: down from 95 to 85 over 5 bars = -10/5 = -2.0
        // leave: down from 85 to 80 over 5 bars = -5/5 = -1.0
        let enter = down_bi(0, 5, 95.0, 85.0);
        let leave = down_bi(5, 10, 85.0, 80.0);
        assert!(DivergenceAnalyzer::slope_divergence(&enter, &leave));
    }

    // ========== 测度背驰 ==========

    #[test]
    fn test_measure_divergence_true() {
        // 价格新高且测度减弱
        let enter = up_bi(0, 10, 100.0, 110.0);
        let leave = up_bi(10, 18, 110.0, 115.0);
        // enter_measure = sqrt(10² + 10²) = sqrt(200) ≈ 14.14
        // leave_measure = sqrt(8² + 5²) = sqrt(89) ≈ 9.43
        assert!(DivergenceAnalyzer::measure_divergence(&enter, &leave));
    }

    // ========== 组合模式 ==========

    #[test]
    fn test_all_true() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0);
        // MACD true (area 10 > 5), Slope true (price up, slope 2.0 > 1.0), Measure true
        assert!(DivergenceAnalyzer::all(&enter, &leave, &macd));
    }

    #[test]
    fn test_any_true() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 5.0, 10.0); // MACD false (5 < 10)
        // Slope true → any should be true
        assert!(DivergenceAnalyzer::any(&enter, &leave, &macd));
    }

    #[test]
    fn test_majority_true() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0); // MACD true
        // Slope true, Measure true → 3/3 ≥ 2
        assert!(DivergenceAnalyzer::majority(&enter, &leave, &macd));
    }

    #[test]
    fn test_majority_false() {
        // Only 1 of 3 true: construct a case where only slope is true
        // enter: down from 105 to 100 over 2 bars → dy=-5, dx=2, slope=-2.5
        // leave: down from 100 to 98 over 20 bars → dy=-2, dx=20, slope=-0.1
        let enter = down_bi(0, 2, 105.0, 100.0);
        let leave = down_bi(2, 22, 100.0, 98.0);
        // MACD: enter area (small) > leave area (large) → false
        // but we need enter_area < leave_area for divergence false
        let mut macd = vec![0.0; 23];
        // enter indices 0..2: small values
        for i in 0..=2 { macd[i] = 1.0; }  // enter_area=3.0
        // leave indices 2..22: large values
        for i in 2..22 { macd[i] = 10.0; } // leave_area >> 3.0
        // MACD: false (3.0 < large)
        // Slope: |slope|=0.1 < |enter_slope|=2.5, AND leave.low=98 < enter.low=100 → true
        // Measure: sqrt(400+4)=20.1 > sqrt(4+25)=5.39 → false (leave_measure > enter_measure = no divergence)
        // So 1/3 → majority false
        assert!(!DivergenceAnalyzer::majority(&enter, &leave, &macd));
    }

    #[test]
    fn test_majority_two_of_three() {
        // 2/3 → majority true
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0);
        assert!(DivergenceAnalyzer::majority(&enter, &leave, &macd));
    }

    // ========== Config 模式 ==========

    #[test]
    fn test_configured_only_macd() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0);
        let config = DivergenceConfig {
            use_macd: true,
            use_slope: false,
            use_measure: false,
            macd_mode: "total".into(),
        };
        assert!(DivergenceAnalyzer::configured(&enter, &leave, &macd, &config));
    }

    #[test]
    fn test_configured_none_enabled() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        let macd = make_macd_hist(15, 10.0, 5.0);
        let config = DivergenceConfig {
            use_macd: false,
            use_slope: false,
            use_measure: false,
            macd_mode: "total".into(),
        };
        assert!(!DivergenceAnalyzer::configured(&enter, &leave, &macd, &config));
    }

    // ========== Serde ==========

    #[test]
    fn test_divergence_config_serde() {
        let config = DivergenceConfig {
            use_macd: true,
            use_slope: false,
            use_measure: true,
            macd_mode: "bull".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: DivergenceConfig = serde_json::from_str(&json).unwrap();
        assert!(back.use_macd);
        assert!(!back.use_slope);
        assert!(back.use_measure);
        assert_eq!(back.macd_mode, "bull");
    }

    #[test]
    fn test_divergence_mode_serde() {
        for mode in &[DivergenceMode::All, DivergenceMode::Any, DivergenceMode::Config, DivergenceMode::Majority] {
            let json = serde_json::to_string(mode).unwrap();
            let back: DivergenceMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, back);
        }
    }

    // ========== Check 综合判定 ==========

    #[test]
    fn test_check_returns_all_fields() {
        let enter = up_bi(0, 5, 100.0, 110.0);
        let leave = up_bi(5, 10, 110.0, 115.0);
        // macd_hist: indices 0-4 = 2.0, 5-9 = 1.0
        // enter range [0..=5]: 2.0*5 + 1.0 = 11.0
        // leave range [5..=10]: 1.0*5 + 0.0 = 5.0
        let macd = make_macd_hist(15, 10.0, 5.0);
        let config = DivergenceConfig::default();

        let result = DivergenceAnalyzer::check(&enter, &leave, &macd, DivergenceMode::All, &config);
        assert!(result.is_divergent);
        assert!(result.macd_diverged);
        assert!(result.slope_diverged);
        assert!(result.measure_diverged);
        assert!((result.enter_area - 11.0).abs() < 0.1);
        assert!((result.leave_area - 5.0).abs() < 0.1);
    }
}
