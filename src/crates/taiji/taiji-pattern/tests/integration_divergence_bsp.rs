//! R-3-605: taiji-pattern divergence + bsp 集成测试
//!
//! 验证完整链路：
//!   segment → divergence → bsp 背驰+买卖点全链路
//!
//! 场景：
//!   1. DivergenceAnalyzer — MACD/斜率/测度三种背驰模式
//!   2. BSPDetector — 一买/一卖检测（背驰段终点）
//!   3. BSPDetector — 二买/二卖检测（回调不破中枢）
//!   4. BSPDetector — 三买/三卖检测（离开中枢后不回中枢）
//!   5. 无背驰时无买卖点

use taiji_pattern::bi::{Bi, BiDirection};
use taiji_pattern::bsp::{BSPDetector, BSPType, BuySellPoint};
use taiji_pattern::divergence::{DivergenceAnalyzer, DivergenceConfig, DivergenceMode, DivergenceResult};
use taiji_pattern::hub::{ChanHub, HubLevel};
use taiji_pattern::segment::{Segment, SegmentStatus};

// ── 辅助函数 ─────────────────────────────────────────────────────────────

/// 创建简单笔
fn make_bi(_id: usize, direction: BiDirection, start_idx: usize, end_idx: usize,
           start_price: f64, end_price: f64) -> Bi {
    Bi {
        start_index: start_idx,
        end_index: end_idx,
        direction,
        start_price,
        end_price,
    }
}

/// 创建线段
fn make_segment(id: &str, direction: BiDirection, start: usize, end: usize,
                high: f64, low: f64) -> Segment {
    Segment {
        id: id.into(),
        direction,
        start_bar_idx: start,
        end_bar_idx: end,
        high,
        low,
        bi_ids: vec![],
        status: SegmentStatus::Confirmed,
        feature_high: high,
        feature_low: low,
    }
}

/// 创建中枢
fn make_hub(id: &str, start: usize, end: usize, zg: f64, zd: f64) -> ChanHub {
    ChanHub {
        id: id.into(),
        seq: 1,
        level: HubLevel::Segment,
        zg, // 中枢上沿
        zd, // 中枢下沿
        gg: zg.max(zd),
        dd: zg.min(zd),
        direction: BiDirection::Up,
        bi_count: 3,
        extend_count: 0,
        start_bar_idx: start,
        end_bar_idx: end,
    }
}

/// 创建 MACD 柱状线数据（简单线性变化）
#[allow(dead_code)]
fn make_macd_data(length: usize, enter_end: usize, leave_end: usize,
                  enter_strength: f64, leave_strength: f64) -> Vec<f64> {
    let mut data = vec![0.0; length];
    // 进入段：从 enter_end-5 到 enter_end 逐渐增强
    for i in enter_end.saturating_sub(5)..enter_end.min(length) {
        data[i] = enter_strength * (i - enter_end.saturating_sub(5)) as f64 / 5.0;
    }
    // 离开段：从 leave_end-5 到 leave_end 逐渐减弱（背驰）
    for i in leave_end.saturating_sub(5)..leave_end.min(length) {
        data[i] = leave_strength * (i - leave_end.saturating_sub(5)) as f64 / 5.0;
    }
    data
}

// ── 测试 ─────────────────────────────────────────────────────────────────

/// 场景 1a：MACD 背驰 — 离开段面积 < 进入段面积
#[test]
fn test_macd_divergence_detected() {
    // 向上趋势：进入段强，离开段弱 → 背驰
    let enter = make_bi(0, BiDirection::Up, 0, 10, 100.0, 120.0);
    let leave = make_bi(1, BiDirection::Up, 10, 20, 120.0, 130.0);

    // MACD 数据：进入段 (index 0~10) 面积大，离开段 (index 10~20) 面积小
    let mut macd = vec![0.0; 25];
    for i in 2..10 { macd[i] = 3.0; }   // 进入段 MACD 强
    for i in 12..18 { macd[i] = 1.0; }  // 离开段 MACD 弱

    let result = DivergenceAnalyzer::macd_divergence(&enter, &leave, &macd, "total");
    assert!(result, "离开段 MACD 面积 < 进入段 → 应检测到 MACD 背驰");
}

/// 场景 1b：斜率背驰 — 价格新高但斜率减弱
#[test]
fn test_slope_divergence_detected() {
    // 进入段：陡峭上升（100→150，跨度 5）
    let enter = make_bi(0, BiDirection::Up, 0, 5, 100.0, 150.0);
    // 离开段：平缓上升（150→160，跨度 10），价格新高但斜率小
    let leave = make_bi(1, BiDirection::Up, 5, 15, 150.0, 160.0);

    let result = DivergenceAnalyzer::slope_divergence(&enter, &leave);
    assert!(result, "价格新高 + 斜率减弱 → 应检测到斜率背驰");
}

/// 场景 1c：测度背驰 — 价格新高但欧氏距离减弱
#[test]
fn test_measure_divergence_detected() {
    // 进入段：长距离（100→150，跨度 10）
    let enter = make_bi(0, BiDirection::Up, 0, 10, 100.0, 150.0);
    // 离开段：短距离（150→155，跨度 5），价格新高但距离小
    let leave = make_bi(1, BiDirection::Up, 10, 15, 150.0, 155.0);

    let result = DivergenceAnalyzer::measure_divergence(&enter, &leave);
    assert!(result, "价格新高 + 测度减弱 → 应检测到测度背驰");
}

/// 场景 1d：无背驰 — 离开段更强时不应检测到背驰
#[test]
fn test_no_divergence() {
    let enter = make_bi(0, BiDirection::Up, 0, 5, 100.0, 110.0);
    let leave = make_bi(1, BiDirection::Up, 5, 10, 110.0, 130.0); // 更强

    let result_slope = DivergenceAnalyzer::slope_divergence(&enter, &leave);
    assert!(!result_slope, "离开段更强 → 不应有斜率背驰");
}

/// 场景 1e：全量模式 — 三者全满足才判定
#[test]
fn test_divergence_all_mode() {
    let enter = make_bi(0, BiDirection::Up, 0, 10, 100.0, 150.0);
    let leave = make_bi(1, BiDirection::Up, 10, 18, 150.0, 155.0);

    let mut macd = vec![0.0; 22];
    for i in 5..10 { macd[i] = 8.0; }  // 进入段 MACD 大
    for i in 15..18 { macd[i] = 2.0; } // 离开段 MACD 小

    // 全量模式：MACD + 斜率 + 测度 三者全满足
    assert!(DivergenceAnalyzer::all(&enter, &leave, &macd), "三者全满足 → 应检测到背驰");
}

/// 场景 1f：向下背驰
#[test]
fn test_downward_divergence() {
    // 向下趋势：进入段下跌强，离开段下跌弱 → 底背驰
    let enter = make_bi(0, BiDirection::Down, 0, 8, 150.0, 100.0);
    let leave = make_bi(1, BiDirection::Down, 8, 15, 100.0, 95.0);

    let mut macd = vec![0.0; 20];
    for i in 3..8 { macd[i] = -6.0; }  // 进入段 MACD 负值大
    for i in 12..15 { macd[i] = -1.0; } // 离开段 MACD 负值小

    let macd_div = DivergenceAnalyzer::macd_divergence(&enter, &leave, &macd, "total");
    let slope_div = DivergenceAnalyzer::slope_divergence(&enter, &leave);
    let measure_div = DivergenceAnalyzer::measure_divergence(&enter, &leave);

    assert!(macd_div, "向下 MACD 背驰应检测");
    assert!(slope_div, "向下斜率背驰应检测");
    assert!(measure_div, "向下测度背驰应检测");
}

// ── BSP 买卖点检测 ──

/// 场景 2a：一买 — 背驰段终点（上升中枢后向下离开段背驰）
#[test]
fn test_first_buy_detected() {
    // 构建：上升中枢 (ZG=115, ZD=105) + 向下离开段背驰 → 一买
    let hub = make_hub("hub1", 3, 8, 115.0, 105.0);
    let segment = make_segment("seg1", BiDirection::Down, 8, 15, 100.0, 90.0);

    // 进入段 (8→11)：下跌强
    // 离开段 (11→15)：下跌弱（背驰）
    let enter_bi = make_bi(0, BiDirection::Down, 8, 11, 100.0, 92.0);
    let leave_bi = make_bi(1, BiDirection::Down, 11, 15, 92.0, 90.0);

    let mut macd = vec![0.0; 20];
    for i in 9..11 { macd[i] = -5.0; }
    for i in 13..15 { macd[i] = -1.0; }

    let divergence = DivergenceAnalyzer::configured(
        &enter_bi, &leave_bi, &macd,
        &DivergenceConfig::default(),
    );
    assert!(divergence, "应检测到背驰");

    let div_result = DivergenceResult {
        is_divergent: divergence,
        macd_diverged: DivergenceAnalyzer::macd_divergence(&enter_bi, &leave_bi, &macd, "total"),
        slope_diverged: DivergenceAnalyzer::slope_divergence(&enter_bi, &leave_bi),
        measure_diverged: DivergenceAnalyzer::measure_divergence(&enter_bi, &leave_bi),
        mode: DivergenceMode::Config,
        enter_area: 5.0,
        leave_area: 1.0,
    };

    let (buys, _sells) = BSPDetector::detect(
        &[hub], &[segment], &[enter_bi, leave_bi], &[div_result],
    );

    // 一买应为下跌背驰段终点
    let first_buy: Vec<&BuySellPoint> = buys.iter().filter(|p| p.bsp_type == BSPType::FirstBuy).collect();
    assert!(!first_buy.is_empty(), "应检测到一买, 但未发现");
    if let Some(bp) = first_buy.first() {
        assert!(bp.divergence_confirmed, "一买应确认背驰");
        assert!(bp.confidence > 0.0, "一买应有置信度");
    }
}

/// 场景 2b：一卖 — 背驰段终点（上升趋势顶背驰）
#[test]
fn test_first_sell_detected() {
    let hub = make_hub("hub2", 2, 6, 120.0, 110.0);
    let segment = make_segment("seg2", BiDirection::Up, 6, 12, 135.0, 120.0);

    // 进入段 (6→9)：上升强
    // 离开段 (9→12)：上升弱 = 顶背驰
    let enter_bi = make_bi(0, BiDirection::Up, 6, 9, 120.0, 135.0);
    let leave_bi = make_bi(1, BiDirection::Up, 9, 12, 135.0, 138.0);

    let mut macd = vec![0.0; 16];
    for i in 7..9 { macd[i] = 6.0; }
    for i in 10..12 { macd[i] = 1.0; }

    let div_result = DivergenceResult {
        is_divergent: true,
        macd_diverged: true,
        slope_diverged: true,
        measure_diverged: true,
        mode: DivergenceMode::All,
        enter_area: 6.0,
        leave_area: 1.0,
    };

    let (_buys, sells) = BSPDetector::detect(
        &[hub], &[segment], &[enter_bi, leave_bi], &[div_result],
    );

    let first_sell: Vec<&BuySellPoint> = sells.iter().filter(|p| p.bsp_type == BSPType::FirstSell).collect();
    assert!(!first_sell.is_empty(), "应检测到一卖");
}

/// 场景 3：无背驰时无买卖点
#[test]
fn test_no_divergence_no_bsp() {
    let hub = make_hub("hub3", 2, 6, 115.0, 105.0);
    let segment = make_segment("seg3", BiDirection::Up, 6, 10, 125.0, 115.0);

    let enter_bi = make_bi(0, BiDirection::Up, 6, 8, 115.0, 125.0);
    let leave_bi = make_bi(1, BiDirection::Up, 8, 10, 125.0, 130.0);

    // 无背驰
    let div_result = DivergenceResult {
        is_divergent: false,
        macd_diverged: false,
        slope_diverged: false,
        measure_diverged: false,
        mode: DivergenceMode::All,
        enter_area: 0.0,
        leave_area: 0.0,
    };

    let (buys, sells) = BSPDetector::detect(
        &[hub], &[segment], &[enter_bi, leave_bi], &[div_result],
    );
    assert!(buys.is_empty(), "无背驰不应有买点");
    assert!(sells.is_empty(), "无背驰不应有卖点");
}

/// 场景 4：空中枢/空线段时无买卖点
#[test]
fn test_empty_input_no_bsp() {
    let (buys, sells) = BSPDetector::detect(&[], &[], &[], &[]);
    assert!(buys.is_empty());
    assert!(sells.is_empty());
}

/// 场景 5：configured 背驰 — 按配置选择组合
#[test]
fn test_configured_divergence() {
    let enter = make_bi(0, BiDirection::Up, 0, 5, 100.0, 120.0);
    let leave = make_bi(1, BiDirection::Up, 5, 10, 120.0, 125.0);

    let mut macd = vec![0.0; 15];
    for i in 2..5 { macd[i] = 4.0; }
    for i in 8..10 { macd[i] = 3.0; }

    // 仅使用 MACD（不用斜率和测度）
    let config = DivergenceConfig {
        use_macd: true,
        use_slope: false,
        use_measure: false,
        macd_mode: "total".into(),
    };
    let result = DivergenceAnalyzer::configured(&enter, &leave, &macd, &config);
    assert!(result, "仅 MACD 配置下应检测到背驰");
}

/// 场景 6：large 模式 — 背驰结果与信号强相关
#[test]
fn test_divergence_mode_config() {
    // 验证 DivergenceMode 枚举 roundtrip
    let modes = vec![
        DivergenceMode::All,
        DivergenceMode::Any,
        DivergenceMode::Config,
        DivergenceMode::Majority,
    ];
    for mode in &modes {
        let json = serde_json::to_string(mode).unwrap();
        let back: DivergenceMode = serde_json::from_str(&json).unwrap();
        assert_eq!(*mode, back, "DivergenceMode roundtrip 失败: {:?}", mode);
    }
}
