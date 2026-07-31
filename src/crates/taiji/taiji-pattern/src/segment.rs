//! 缠论线段生成 + 特征序列
//!
//! 线段（Segment）由连续交替的笔构成，是缠论分析框架的第二层结构。
//! 特征序列提供线段终点的判定依据。
//!
//! 设计原则：纯值类型，无 Arc/RwLock/Atomic，通过 StateStore 共享。
//!
//! 参考: 量化总纲 §7.5-7.6（K 线处理 → 折线处理 + 段合并规则）— R-3-302 — Rust 翻译实现
//! 格式参考: chanlun-rs (MIT) segment.rs — 仅算法格式参考

use serde::{Deserialize, Serialize};

use crate::bi::{Bi, BiDirection};

// ============================================================
// 线段状态
// ============================================================

/// 线段状态（R-3-302-01）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SegmentStatus {
    Forming,   // 进行中（未确认终点）
    Confirmed, // 已确认
}

// ============================================================
// 线段结构体
// ============================================================

/// 线段 — 由连续交替笔构成的缠论第二层结构（R-3-302-01）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub direction: BiDirection,
    pub start_bar_idx: usize,
    pub end_bar_idx: usize,
    pub high: f64,
    pub low: f64,
    pub bi_ids: Vec<String>,
    pub status: SegmentStatus,
    pub feature_high: f64,   // 特征序列高点极值
    pub feature_low: f64,    // 特征序列低点极值
}

// ============================================================
// 特征分型 + 四象 + 特征序列
// ============================================================

/// 特征分型（特征序列的元素）（R-3-302-02）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFractal {
    pub bi_id: String,
    pub fractal_type: String, // "top" / "bottom"
    pub feature_value: f64,   // 特征值（顶=high, 底=low）
    pub merged: bool,         // 是否被合并
}

/// 四象类型（R-3-302-02）
///
/// 基于特征序列中连续分型的方向和包含关系判定合并范围：
/// - OldYang / OldYin: 顺+逆+同 全部合并（连续两根与原线段方向相同且互相包含）
/// - YoungYang / YoungYin: 仅顺+同合并（仅一根同向）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FourPhases {
    OldYang,   // 老阳：顺+逆+同 全部合并
    OldYin,    // 老阴：顺+逆+同 全部合并
    YoungYang, // 少阳：仅顺+同合并
    YoungYin,  // 少阴：仅顺+同合并
}

/// 特征序列（R-3-302-02）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSequence {
    pub fractals: Vec<FeatureFractal>,
    pub direction: BiDirection, // 线段方向
}

impl FeatureSequence {
    /// 从 Bi 序列生成特征序列
    ///
    /// 向上线段取顶分型特征值（high），向下线段取底分型特征值（low）。
    ///
    /// 参考: 理论总纲 §四（波段结构）+ 量化总纲 §7.5-7.6（K线→折线+段合并）— R-3-302 — Rust 翻译实现
    pub fn from_bis(bis: &[Bi], direction: BiDirection) -> Self {
        let is_up = direction == BiDirection::Up;
        let fractals = bis
            .iter()
            .map(|bi| {
                let (ftype, fval) = if is_up {
                    let high = bi.start_price.max(bi.end_price);
                    ("top".to_string(), high)
                } else {
                    let low = bi.start_price.min(bi.end_price);
                    ("bottom".to_string(), low)
                };
                FeatureFractal {
                    bi_id: format!("bi:{}", bi.start_index),
                    fractal_type: ftype,
                    feature_value: fval,
                    merged: false,
                }
            })
            .collect();

        FeatureSequence { fractals, direction }
    }

    /// 四象判定 — 基于特征序列中连续分型的方向和包含关系
    ///
    /// 当特征序列中连续两根的方向与原线段方向相同且互相包含（顺+同）时为老阳/老阴，
    /// 仅一根同向为少阳/少阴。
    ///
    /// 参考: 量化总纲 §7.5-7.6（段合并规则）— R-3-302 — Rust 翻译实现
    pub fn judge_four_phases(&self) -> FourPhases {
        let is_up = self.direction == BiDirection::Up;
        let mut consecutive_same = 0usize;

        for pair in self.fractals.windows(2) {
            let a = &pair[0];
            let b = &pair[1];

            // 检查是否与原线段方向相同
            let a_same = (is_up && a.fractal_type == "top")
                || (!is_up && a.fractal_type == "bottom");
            let b_same = (is_up && b.fractal_type == "top")
                || (!is_up && b.fractal_type == "bottom");

            if a_same && b_same {
                // 检查互相包含：两根特征值的区间是否重叠
                let a_val = a.feature_value;
                let b_val = b.feature_value;
                let contained = if is_up {
                    // 顶分型：两根的 high 互相接近（重叠）
                    (a_val - b_val).abs() < (a_val + b_val) * 0.01
                } else {
                    (a_val - b_val).abs() < (a_val + b_val) * 0.01
                };

                if contained {
                    consecutive_same += 1;
                    if consecutive_same >= 2 {
                        return if is_up {
                            FourPhases::OldYang
                        } else {
                            FourPhases::OldYin
                        };
                    }
                } else {
                    consecutive_same = 0;
                }
            } else {
                consecutive_same = 0;
            }
        }

        // Only one same-direction consecutive pair (young)
        if consecutive_same >= 1 {
            return if is_up {
                FourPhases::YoungYang
            } else {
                FourPhases::YoungYin
            };
        }

        // Default: young (no strong signal)
        if is_up {
            FourPhases::YoungYang
        } else {
            FourPhases::YoungYin
        }
    }

    /// 合并规则 — 老阳/老阴时合并顺+逆+同，否则仅合并顺+同
    ///
    /// 合并后原始数据保留（merged 标记而非删除）。
    ///
    /// 参考: 量化总纲 §7.6（段合并规则）— R-3-302 — Rust 翻译实现
    pub fn merge(&mut self, phases: FourPhases) {
        match phases {
            FourPhases::OldYang | FourPhases::OldYin => {
                // 老阳/老阴：顺+逆+同 全部合并
                // 遍历所有分型，相邻的标记为合并
                let mut i = 0;
                while i + 1 < self.fractals.len() {
                    let is_contained = {
                        let a = &self.fractals[i];
                        let b = &self.fractals[i + 1];
                        if a.merged {
                            i += 1;
                            continue;
                        }
                        let diff = (a.feature_value - b.feature_value).abs();
                        let avg = (a.feature_value + b.feature_value) / 2.0;
                        diff < avg * 0.02
                    };
                    if is_contained {
                        self.fractals[i + 1].merged = true;
                    }
                    i += 1;
                }
            }
            FourPhases::YoungYang | FourPhases::YoungYin => {
                // 少阳/少阴：仅顺+同合并
                let is_up = self.direction == BiDirection::Up;
                let mut i = 0;
                while i + 1 < self.fractals.len() {
                    let a = &self.fractals[i];
                    let b = &self.fractals[i + 1];
                    if a.merged {
                        i += 1;
                        continue;
                    }
                    let a_same = (is_up && a.fractal_type == "top")
                        || (!is_up && a.fractal_type == "bottom");
                    let b_same = (is_up && b.fractal_type == "top")
                        || (!is_up && b.fractal_type == "bottom");

                    if a_same && b_same {
                        let diff = (a.feature_value - b.feature_value).abs();
                        let avg = (a.feature_value + b.feature_value) / 2.0;
                        if diff < avg * 0.02 {
                            self.fractals[i + 1].merged = true;
                        }
                    }
                    i += 1;
                }
            }
        }
    }
}

// ============================================================
// 线段划分器
// ============================================================

/// 线段划分器（R-3-302-03）
///
/// 从 Bi 序列划分线段。核心逻辑：
/// - 第一段从第一个 Bi 开始，方向为 Bi 的方向
/// - 每 3 笔构成一个线段（交替方向）
/// - 不足 3 笔时返回空
///
/// 参考: 量化总纲 §7.1（由大到小定结构）+ §7.5（K线→折线→方向段）— R-3-302 — Rust 翻译实现
pub struct SegmentDivider;

impl SegmentDivider {
    /// 从 Bi 序列划分线段
    ///
    /// 每 3 笔构成一个线段，方向由第一笔确定，后续线段交替方向。
    /// 不足 3 笔时返回空 Vec。
    /// 最后一段若不足 3 笔标记为 Forming。
    pub fn divide(bis: &[Bi]) -> Vec<Segment> {
        if bis.len() < 3 {
            return vec![];
        }

        let mut segments: Vec<Segment> = Vec::new();
        let mut start = 0usize;

        while start + 3 <= bis.len() {
            let end = start + 3; // 每段取 3 笔
            let direction = bis[start].direction;

            // 计算线段极值
            let mut high = f64::MIN;
            let mut low = f64::MAX;
            let mut feature_high = f64::MIN;
            let mut feature_low = f64::MAX;
            let mut bi_ids = Vec::with_capacity(3);

            for bi in bis[start..end].iter() {
                let bh = bi.start_price.max(bi.end_price);
                let bl = bi.start_price.min(bi.end_price);
                if bh > high {
                    high = bh;
                }
                if bl < low {
                    low = bl;
                }
                // 特征极值：向上线段取顶分型（high），向下取底分型（low）
                if direction == BiDirection::Up {
                    if bh > feature_high {
                        feature_high = bh;
                    }
                    if bl < feature_low {
                        feature_low = bl;
                    }
                } else {
                    if bl < feature_low {
                        feature_low = bl;
                    }
                    if bh > feature_high {
                        feature_high = bh;
                    }
                }
                bi_ids.push(format!("bi:{}", bi.start_index));
            }

            let is_confirmed = end - start >= 3;
            // 检查是否为最后一段且笔数不足再确认
            let is_last_incomplete = is_confirmed && start + 3 < bis.len() && start + 6 > bis.len();

            let status = if is_confirmed && !is_last_incomplete {
                SegmentStatus::Confirmed
            } else {
                SegmentStatus::Forming
            };

            segments.push(Segment {
                id: format!("seg:{}", segments.len()),
                direction,
                start_bar_idx: bis[start].start_index,
                end_bar_idx: bis[end - 1].end_index,
                high,
                low,
                bi_ids,
                status,
                feature_high,
                feature_low,
            });

            start = end;
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bi(start_idx: usize, end_idx: usize, dir: BiDirection, sp: f64, ep: f64) -> Bi {
        Bi {
            start_index: start_idx,
            end_index: end_idx,
            direction: dir,
            start_price: sp,
            end_price: ep,
        }
    }

    // ── Segment serde ──

    #[test]
    fn test_segment_serde_roundtrip() {
        let seg = Segment {
            id: "seg:0".into(),
            direction: BiDirection::Up,
            start_bar_idx: 0,
            end_bar_idx: 10,
            high: 110.0,
            low: 90.0,
            bi_ids: vec!["bi:0".into(), "bi:1".into(), "bi:2".into()],
            status: SegmentStatus::Confirmed,
            feature_high: 108.0,
            feature_low: 92.0,
        };
        let json = serde_json::to_string(&seg).unwrap();
        let deserialized: Segment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "seg:0");
        assert_eq!(deserialized.direction, BiDirection::Up);
        assert_eq!(deserialized.status, SegmentStatus::Confirmed);
    }

    // ── 3 bis → 1 segment ──

    #[test]
    fn test_three_bis_form_one_segment() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Up, 95.0, 105.0),
            make_bi(3, 6, BiDirection::Down, 105.0, 98.0),
            make_bi(6, 9, BiDirection::Up, 98.0, 110.0),
        ];
        let segments = SegmentDivider::divide(&bis);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].direction, BiDirection::Up);
        assert_eq!(segments[0].bi_ids.len(), 3);
        assert_eq!(segments[0].status, SegmentStatus::Confirmed);
    }

    // ── 2 bis → empty ──

    #[test]
    fn test_less_than_three_bis_returns_empty() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Up, 95.0, 105.0),
            make_bi(3, 6, BiDirection::Down, 105.0, 98.0),
        ];
        let segments = SegmentDivider::divide(&bis);
        assert!(segments.is_empty());
    }

    // ── empty → empty ──

    #[test]
    fn test_empty_bis_returns_empty() {
        let segments = SegmentDivider::divide(&[]);
        assert!(segments.is_empty());
    }

    // ── 6 bis → 2 segments ──

    #[test]
    fn test_six_bis_form_two_segments() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Up, 95.0, 105.0),
            make_bi(3, 6, BiDirection::Down, 105.0, 98.0),
            make_bi(6, 9, BiDirection::Up, 98.0, 110.0),
            make_bi(9, 12, BiDirection::Down, 110.0, 100.0),
            make_bi(12, 15, BiDirection::Up, 100.0, 108.0),
            make_bi(15, 18, BiDirection::Down, 108.0, 96.0),
        ];
        let segments = SegmentDivider::divide(&bis);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].direction, BiDirection::Up);
        assert_eq!(segments[1].direction, BiDirection::Down);
    }

    // ── Segment extremes correct ──

    #[test]
    fn test_segment_extremes_correct() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Up, 95.0, 105.0),
            make_bi(3, 6, BiDirection::Down, 105.0, 98.0),
            make_bi(6, 9, BiDirection::Up, 98.0, 112.0),
        ];
        let segments = SegmentDivider::divide(&bis);
        assert_eq!(segments.len(), 1);
        assert!((segments[0].high - 112.0).abs() < 1e-9);
        assert!((segments[0].low - 95.0).abs() < 1e-9);
    }

    // ── FeatureSequence tests ──

    #[test]
    fn test_feature_sequence_from_up_bis() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Up, 95.0, 105.0),
            make_bi(3, 6, BiDirection::Down, 105.0, 98.0),
            make_bi(6, 9, BiDirection::Up, 98.0, 110.0),
        ];
        let fs = FeatureSequence::from_bis(&bis, BiDirection::Up);
        assert_eq!(fs.fractals.len(), 3);
        // Up segment → feature type = "top" (high)
        for f in &fs.fractals {
            assert_eq!(f.fractal_type, "top");
        }
        assert_eq!(fs.direction, BiDirection::Up);
    }

    #[test]
    fn test_feature_sequence_from_down_bis() {
        let bis = vec![
            make_bi(0, 3, BiDirection::Down, 105.0, 95.0),
            make_bi(3, 6, BiDirection::Up, 95.0, 102.0),
            make_bi(6, 9, BiDirection::Down, 102.0, 90.0),
        ];
        let fs = FeatureSequence::from_bis(&bis, BiDirection::Down);
        assert_eq!(fs.fractals.len(), 3);
        // Down segment → feature type = "bottom" (low)
        for f in &fs.fractals {
            assert_eq!(f.fractal_type, "bottom");
        }
    }

    // ── FourPhases serde ──

    #[test]
    fn test_four_phases_serde_roundtrip() {
        for p in &[
            FourPhases::OldYang,
            FourPhases::OldYin,
            FourPhases::YoungYang,
            FourPhases::YoungYin,
        ] {
            let json = serde_json::to_string(p).unwrap();
            let deserialized: FourPhases = serde_json::from_str(&json).unwrap();
            assert_eq!(*p, deserialized);
        }
    }

    // ── FeatureSequence merge tests ──

    #[test]
    fn test_feature_sequence_merge_same_direction() {
        // YoungYang: merge only same-direction + contained
        let mut fs = FeatureSequence {
            fractals: vec![
                FeatureFractal {
                    bi_id: "bi:0".into(),
                    fractal_type: "top".into(),
                    feature_value: 105.0,
                    merged: false,
                },
                FeatureFractal {
                    bi_id: "bi:1".into(),
                    fractal_type: "top".into(),
                    feature_value: 106.0, // close to 105 → contained
                    merged: false,
                },
            ],
            direction: BiDirection::Up,
        };
        fs.merge(FourPhases::YoungYang);
        assert!(fs.fractals[1].merged);
    }

    // ── SegmentStatus serde ──

    #[test]
    fn test_segment_status_serde_roundtrip() {
        for s in &[SegmentStatus::Forming, SegmentStatus::Confirmed] {
            let json = serde_json::to_string(s).unwrap();
            let deserialized: SegmentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, deserialized);
        }
    }
}
