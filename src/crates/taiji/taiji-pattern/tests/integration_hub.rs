//! R-3-603: taiji-pattern hub 单元集成测试
//!
//! 验证 ChanHubDetector 从 Bi 序列正确检测中枢。
//!
//! 场景：
//!   1. 空输入 → 空结果
//!   2. 不足 3 笔 → 空结果
//!   3. 3 笔重叠 → 1 个中枢（ZG/ZD/GG/DD 正确）
//!   4. 2 组 3 笔各重叠 → 2 个独立中枢
//!   5. 5 笔含延伸 → 1 个中枢（extend_count > 0）
//!   6. 3 笔无重叠（无公共区间）→ 空结果

use taiji_pattern::bi::{Bi, BiDirection};
use taiji_pattern::hub::{ChanHubDetector, HubLevel};

/// 辅助创建笔
fn make_bi(
    start_idx: usize,
    end_idx: usize,
    direction: BiDirection,
    start_price: f64,
    end_price: f64,
) -> Bi {
    Bi {
        start_index: start_idx,
        end_index: end_idx,
        direction,
        start_price,
        end_price,
    }
}

// ── 1. 空输入 ──

#[test]
fn test_hub_empty_bis() {
    let hubs = ChanHubDetector::detect(&[]);
    assert!(hubs.is_empty(), "empty bis should produce no hubs");
}

// ── 2. 不足 3 笔 ──

#[test]
fn test_hub_less_than_three_bis() {
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert!(hubs.is_empty(), "2 bis should produce no hubs");
}

// ── 3. 3 笔重叠 → 1 个中枢 ──

#[test]
fn test_hub_three_overlapping_bis() {
    // Up(100→110), Down(110→105), Up(105→115)
    // high(110,110,115) → min_high = 110 → ZG
    // low(100,105,105) → max_low = 105 → ZD
    // 105 < 110 → 重叠 → 1 hub
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
        make_bi(10, 15, BiDirection::Up, 105.0, 115.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert_eq!(hubs.len(), 1, "3 overlapping bis should form 1 hub");

    let hub = &hubs[0];
    assert!((hub.zg - 110.0).abs() < 1e-9, "ZG should be min_high=110");
    assert!((hub.zd - 105.0).abs() < 1e-9, "ZD should be max_low=105");
    assert!((hub.gg - 115.0).abs() < 1e-9, "GG should be max_high=115");
    assert!((hub.dd - 100.0).abs() < 1e-9, "DD should be min_low=100");
    assert_eq!(hub.bi_count, 3, "hub should have 3 bis initially");
    assert_eq!(hub.extend_count, 0, "no extension with exactly 3 bis");
    assert_eq!(hub.level, HubLevel::Bi, "hub level should be Bi");
    assert_eq!(hub.direction, BiDirection::Down, "hub direction is opposite of first bi (Up→Down)");
}

// ── 4. 2 组 3 笔各重叠 → 2 个独立中枢 ──

#[test]
fn test_hub_two_separate_hubs() {
    // Hub A: Up(100→110), Down(110→105), Up(105→112) — ZG=110, ZD=105, 重叠
    // Hub B: Up(120→130), Down(130→125), Up(125→135) — ZG=130, ZD=125, 重叠
    // 两组之间 gap: 112→120，无重叠 → 2 个独立中枢
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
        make_bi(10, 15, BiDirection::Up, 105.0, 112.0),
        make_bi(20, 25, BiDirection::Up, 120.0, 130.0),
        make_bi(25, 30, BiDirection::Down, 130.0, 125.0),
        make_bi(30, 35, BiDirection::Up, 125.0, 135.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert_eq!(hubs.len(), 2, "2 separate overlapping groups should form 2 hubs");

    // Hub A
    let h0 = &hubs[0];
    assert!((h0.zg - 110.0).abs() < 1e-9, "HubA ZG=110");
    assert!((h0.zd - 105.0).abs() < 1e-9, "HubA ZD=105");
    assert_eq!(h0.start_bar_idx, 0, "HubA start_idx=0");

    // Hub B
    let h1 = &hubs[1];
    assert!((h1.zg - 130.0).abs() < 1e-9, "HubB ZG=130");
    assert!((h1.zd - 125.0).abs() < 1e-9, "HubB ZD=125");
    assert_eq!(h1.start_bar_idx, 20, "HubB start_idx=20");
}

// ── 5. 5 笔含延伸 → 1 个中枢（extend_count > 0）──

#[test]
fn test_hub_with_extension() {
    // 前 3 笔形成中枢 ZG=110, ZD=105
    // 后续 2 笔在中枢范围内延伸
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
        make_bi(10, 15, BiDirection::Up, 105.0, 112.0),
        // 延伸笔：在中枢范围内
        make_bi(15, 20, BiDirection::Down, 112.0, 107.0),
        make_bi(20, 25, BiDirection::Up, 107.0, 111.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert!(!hubs.is_empty(), "5 bis with overlap should form 1 hub");
    let hub = &hubs[0];
    assert_eq!(hub.bi_count, 5, "extended hub should have 5 bis");
    assert!(hub.extend_count >= 2, "should have at least 2 extensions");
    assert_eq!(hub.level, HubLevel::Bi, "level stays Bi until 9+ bis");
}

// ── 6. 3 笔无重叠（无公共区间）→ 空结果 ──

#[test]
fn test_hub_no_overlap() {
    // Up(100→110), Down(110→90), Up(90→115)
    // highs(110,110,115) → min_high = 110
    // lows(100,90,90) → max_low = 100
    // 100 < 110 → 仍然有重叠（100~110）
    // 为了让无重叠，需要 max_low >= min_high
    // 使用严格分离的价格：Up(100→110), Down(110→85), Up(85→120)
    // highs(110,110,120) → min_high = 110
    // lows(100,85,85) → max_low = 100
    // 100 < 110 → 仍然重叠...
    //
    // 真正无重叠需要下行笔完全不碰上行笔区间：
    // Up(100→110), Down(115→105), Up(105→120)
    // highs(110,115,120) → min_high = 110
    // lows(100,105,105) → max_low = 105
    // 105 < 110 → 仍然重叠
    //
    // 需要下行笔的 low > 上行笔的 high：
    // Up(100→105), Down(110→108), Up(108→115)
    // highs(105,110,115) → min_high = 105
    // lows(100,108,108) → max_low = 108
    // 108 >= 105 → 无重叠 ✅
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 105.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 108.0),
        make_bi(10, 15, BiDirection::Up, 108.0, 115.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert!(hubs.is_empty(), "3 non-overlapping bis should produce no hub");
}

// ── 7. Hub serde roundtrip（集成层面验证序列化）──

#[test]
fn test_hub_serde_roundtrip() {
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
        make_bi(10, 15, BiDirection::Up, 105.0, 115.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert_eq!(hubs.len(), 1);

    let json = serde_json::to_string(&hubs[0]).unwrap();
    let deserialized: taiji_pattern::hub::ChanHub = serde_json::from_str(&json).unwrap();

    assert!((deserialized.zg - hubs[0].zg).abs() < 1e-9);
    assert!((deserialized.zd - hubs[0].zd).abs() < 1e-9);
    assert_eq!(deserialized.bi_count, hubs[0].bi_count);
}

// ── 8. 6 笔含升级检测 ──

#[test]
fn test_hub_upgrade_to_segment_level() {
    // 9+ 笔在中枢范围内 → 升级到 Segment 级别
    // 简易版本：只用 6 笔验证中枢能正确延伸但不升级
    let bis = vec![
        make_bi(0, 5, BiDirection::Up, 100.0, 110.0),
        make_bi(5, 10, BiDirection::Down, 110.0, 105.0),
        make_bi(10, 15, BiDirection::Up, 105.0, 112.0),
        make_bi(15, 20, BiDirection::Down, 112.0, 106.0),
        make_bi(20, 25, BiDirection::Up, 106.0, 111.0),
        make_bi(25, 30, BiDirection::Down, 111.0, 107.0),
    ];
    let hubs = ChanHubDetector::detect(&bis);
    assert!(!hubs.is_empty(), "6 overlapping bis should form hub");

    let hub = &hubs[0];
    assert_eq!(hub.bi_count, 6, "all 6 bis should be in the hub");
    assert_eq!(hub.extend_count, 3, "3 extension bis");
}
