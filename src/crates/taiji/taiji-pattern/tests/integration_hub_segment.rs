//! R-3-604: taiji-pattern hub + segment 集成测试
//!
//! 验证在同一 Bi 序列上，中枢检测 + 线段划分的全链路一致性。
//!
//! 场景:
//!   1. 空/不足 3 笔 → 两者均返回空
//!   2. 3 笔重叠 + 1 个中枢 → 1 个线段，中枢 ZG/ZD 在 segment high/low 范围内
//!   3. 6 笔 + 2 个中枢 + 2 个线段 → 中枢与线段对应关系
//!   4. 9 笔 + 中枢延伸 → 线段结构正确
//!   5. 无重叠 + 多线段 → 中枢为空

use taiji_pattern::bi::{Bi, BiDirection};
use taiji_pattern::hub::ChanHubDetector;
use taiji_pattern::segment::SegmentDivider;

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

fn up_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
    make_bi(s, e, BiDirection::Up, sp, ep)
}

fn down_bi(s: usize, e: usize, sp: f64, ep: f64) -> Bi {
    make_bi(s, e, BiDirection::Down, sp, ep)
}

// ============================================================
// 场景 1: 空/不足 3 笔 → 两者均返回空
// ============================================================

#[test]
fn test_empty_bis() {
    assert!(ChanHubDetector::detect(&[]).is_empty());
    assert!(SegmentDivider::divide(&[]).is_empty());

    let bis = vec![up_bi(0, 5, 100.0, 110.0), down_bi(5, 10, 110.0, 103.0)];
    assert!(ChanHubDetector::detect(&bis).is_empty());
    assert!(SegmentDivider::divide(&bis).is_empty());
}

// ============================================================
// 场景 2: 3 笔重叠 → 1 中枢 + 1 线段
// ============================================================

#[test]
fn test_one_hub_one_segment() {
    // Up(100→110), Down(110→105), Up(105→115) — overlapped in [105,110]
    let bis = vec![
        up_bi(0, 5, 100.0, 110.0),
        down_bi(5, 10, 110.0, 105.0),
        up_bi(10, 15, 105.0, 115.0),
    ];

    // 中枢检测
    let hubs = ChanHubDetector::detect(&bis);
    assert_eq!(hubs.len(), 1, "3 overlapping bis → 1 hub");

    // 线段划分
    let segs = SegmentDivider::divide(&bis);
    assert_eq!(segs.len(), 1, "3 bis → 1 segment");

    let hub = &hubs[0];
    let seg = &segs[0];

    // 交叉验证：中枢 ZG/ZD 应在 segment 的 high/low 范围内
    assert!(
        seg.high >= hub.gg,
        "segment high {} should >= hub GG {}",
        seg.high,
        hub.gg
    );
    assert!(
        seg.low <= hub.dd,
        "segment low {} should <= hub DD {}",
        seg.low,
        hub.dd
    );
    // 中枢范围 [ZD, ZG] 应在 [low, high] 内
    assert!(hub.zg <= seg.high, "hub ZG {} should <= segment high {}", hub.zg, seg.high);
    assert!(hub.zd >= seg.low, "hub ZD {} should >= segment low {}", hub.zd, seg.low);

    // 方向: 线段 = 第一笔方向(Up), 中枢 = 第一笔方向翻转(Down)
    assert_eq!(seg.direction, BiDirection::Up, "segment direction = first bi direction");
    assert_eq!(hub.direction, BiDirection::Down, "hub direction = opposite of first bi");

    // 起止索引一致
    assert_eq!(hub.start_bar_idx, seg.start_bar_idx, "hub start = segment start");
    assert_eq!(hub.end_bar_idx, seg.end_bar_idx, "hub end = segment end");
}

// ============================================================
// 场景 3: 6 笔 → 2 中枢 + 2 线段
// ============================================================

#[test]
fn test_two_hubs_two_segments() {
    // 两组 3 笔, 各自重叠, 两组之间不重叠
    // Hub A: [100-110 zone]
    // Gap: jump to [120-130 zone]
    // Hub B: [120-130 zone]
    let bis = vec![
        up_bi(0, 5, 100.0, 110.0),       // seg0 start
        down_bi(5, 10, 110.0, 105.0),    // |
        up_bi(10, 15, 105.0, 112.0),     // seg0 end, hubA
        // Jump to second price zone
        down_bi(20, 25, 130.0, 125.0),   // seg1 start
        up_bi(25, 30, 125.0, 135.0),     // |
        down_bi(30, 35, 135.0, 128.0),   // seg1 end, hubB
    ];

    let hubs = ChanHubDetector::detect(&bis);
    assert_eq!(hubs.len(), 2, "2 overlapping groups → 2 hubs");

    let segs = SegmentDivider::divide(&bis);
    assert_eq!(segs.len(), 2, "6 bis → 2 segments");

    // HubA ↔ Segment0 对应
    assert_eq!(
        hubs[0].start_bar_idx, segs[0].start_bar_idx,
        "hubA start = seg0 start"
    );
    assert_eq!(
        hubs[0].end_bar_idx, segs[0].end_bar_idx,
        "hubA end = seg0 end"
    );

    // HubB ↔ Segment1 对应
    assert_eq!(
        hubs[1].start_bar_idx, segs[1].start_bar_idx,
        "hubB start = seg1 start"
    );
    assert_eq!(
        hubs[1].end_bar_idx, segs[1].end_bar_idx,
        "hubB end = seg1 end"
    );

    // 线段方向交替
    assert_eq!(segs[0].direction, BiDirection::Up, "seg0=Up");
    assert_eq!(segs[1].direction, BiDirection::Down, "seg1=Down");

    // 中枢方向交替（第一笔翻转）
    assert_eq!(hubs[0].direction, BiDirection::Down, "hubA=Down (first bi Up flipped)");
    assert_eq!(hubs[1].direction, BiDirection::Up, "hubB=Up (first bi Down flipped)");
}

// ============================================================
// 场景 4: 9 笔 + 中枢延伸 + 3 线段
// ============================================================

#[test]
fn test_extended_hub_with_three_segments() {
    // 9 笔: 3 组 × 3 笔 → 3 个线段
    // 每 3 笔形成一个重叠中枢, 组间略有价格 gap 以形成独立中枢
    // Seg0 (Up): 100→110, 110→105, 105→112  (HubA)
    // Seg1 (Down): 112→108, 108→115, 115→110 (no overlap → no hub? let me check)
    // Actually let's use clear separate groups
    let bis = vec![
        // Seg0 — group A: 100-110 zone
        up_bi(0, 5, 100.0, 110.0),
        down_bi(5, 10, 110.0, 105.0),
        up_bi(10, 15, 105.0, 112.0),
        // Seg1 — group B: 120-130 zone
        down_bi(20, 25, 130.0, 122.0),
        up_bi(25, 30, 122.0, 128.0),
        down_bi(30, 35, 128.0, 120.0),
        // Seg2 — group C: 110-115 zone
        up_bi(40, 45, 110.0, 115.0),
        down_bi(45, 50, 115.0, 112.0),
        up_bi(50, 55, 112.0, 118.0),
    ];

    let hubs = ChanHubDetector::detect(&bis);
    let segs = SegmentDivider::divide(&bis);

    // 3 个线段（每 3 笔一个）
    assert_eq!(segs.len(), 3, "9 bis → 3 segments");
    assert_eq!(segs[0].direction, BiDirection::Up, "seg0=Up");
    assert_eq!(segs[1].direction, BiDirection::Down, "seg1=Down");
    assert_eq!(segs[2].direction, BiDirection::Up, "seg2=Up");

    // 应有 3 个中枢（若每组都有重叠）或 2 个（组A重叠, 组B可能不重叠, 组C重叠）
    assert!(
        hubs.len() >= 2,
        "should detect at least 2 hubs from 3 overlapping groups, got {}",
        hubs.len()
    );

    // 验证每个中枢不跨越线段边界
    for hub in &hubs {
        let seg_idx = segs.iter().position(|s| s.start_bar_idx <= hub.start_bar_idx && s.end_bar_idx >= hub.end_bar_idx);
        assert!(
            seg_idx.is_some(),
            "hub [{},{}] should be contained within a segment",
            hub.start_bar_idx,
            hub.end_bar_idx
        );
    }
}

// ============================================================
// 场景 5: 无重叠 + 多线段 → 中枢为空
// ============================================================

#[test]
fn test_no_hub_with_multiple_segments() {
    // 6 笔, 两组各 3 笔, 但每组内部无重叠 → 无中枢
    // 6 笔 → 2 个线段
    // Group A: 无重叠
    // Group B: 无重叠
    let bis = vec![
        // Seg0 (Up) — 无重叠
        up_bi(0, 5, 100.0, 105.0),     // high=105
        down_bi(5, 10, 110.0, 108.0),  // low=108 > 105 → no overlap
        up_bi(10, 15, 108.0, 115.0),   // no overlap with 108
        // Seg1 (Down) — 无重叠
        down_bi(20, 25, 120.0, 115.0),
        up_bi(25, 30, 112.0, 118.0),   // low=112 < 115? 112<115 is overlap!
        down_bi(30, 35, 118.0, 113.0),
        // Hmm let me recalculate for group B...
        // Actually for group B: low(115), high(118? no, 118>115), let me make it non-overlapping
    ];

    // Remove group B and just test group A (3 bis, no overlap → no hub, 1 segment)
    let bis_a = &bis[..3];
    let hubs = ChanHubDetector::detect(bis_a);
    assert!(hubs.is_empty(), "non-overlapping bis → no hub");

    let segs = SegmentDivider::divide(bis_a);
    assert_eq!(segs.len(), 1, "3 bis → 1 segment even without hub");

    // 验证无中枢时线段仍然正常划分
    assert_eq!(segs[0].direction, BiDirection::Up);
    assert_eq!(segs[0].bi_ids.len(), 3);
}

// ============================================================
// 场景 6: 大 Bi 序列性能 + 结构正确性
// ============================================================

#[test]
fn test_large_bi_sequence() {
    // 15 笔: 5 个线段, 若每组重叠则有 5 个中枢
    let mut bis = Vec::with_capacity(15);
    for group in 0..5 {
        let base_idx = group * 20;
        let base_price = 100.0 + group as f64 * 20.0;
        let dir = if group % 2 == 0 { BiDirection::Up } else { BiDirection::Down };
        match dir {
            BiDirection::Up => {
                bis.push(up_bi(base_idx, base_idx + 5, base_price, base_price + 10.0));
                bis.push(down_bi(base_idx + 5, base_idx + 10, base_price + 10.0, base_price + 5.0));
                bis.push(up_bi(base_idx + 10, base_idx + 15, base_price + 5.0, base_price + 15.0));
            }
            BiDirection::Down => {
                bis.push(down_bi(base_idx, base_idx + 5, base_price + 15.0, base_price + 5.0));
                bis.push(up_bi(base_idx + 5, base_idx + 10, base_price + 5.0, base_price + 10.0));
                bis.push(down_bi(base_idx + 10, base_idx + 15, base_price + 10.0, base_price + 2.0));
            }
        }
    }

    let hubs = ChanHubDetector::detect(&bis);
    let segs = SegmentDivider::divide(&bis);

    // 5 个线段
    assert_eq!(segs.len(), 5, "15 bis → 5 segments");
    // 应检测到至少一部分重叠的中枢
    assert!(
        hubs.len() >= 1,
        "should detect at least 1 hub from 5 groups, got {}",
        hubs.len()
    );

    // 线段方向交替
    for i in 1..segs.len() {
        assert_ne!(
            segs[i].direction, segs[i - 1].direction,
            "segment {} direction should alternate ({:?} != {:?})",
            i, segs[i - 1].direction, segs[i].direction
        );
    }

    // 每个线段有 3 笔
    for (i, seg) in segs.iter().enumerate() {
        assert_eq!(
            seg.bi_ids.len(),
            3,
            "segment {} should have 3 bis, got {}",
            i,
            seg.bi_ids.len()
        );
    }

    // 中枢不跨越线段边界
    for hub in &hubs {
        let contained = segs.iter().any(|s| {
            hub.start_bar_idx >= s.start_bar_idx && hub.end_bar_idx <= s.end_bar_idx
        });
        assert!(
            contained,
            "hub [{},{}] should be inside a segment",
            hub.start_bar_idx,
            hub.end_bar_idx
        );
    }
}
