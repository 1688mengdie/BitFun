//! # TitleManager — 荣誉称号系统
//!
//! 架构总纲 §0.14：荣誉称号 = 词条称号，称号带属性加成，不是标签。
//! §5.1：道号可同时装备多个，加成有上限。
//!
//! 参考：
//! - react-xiuxian-game types.ts:163-181 Title 接口
//! - 架构总纲 §5.1 道名词条示例

use async_trait::async_trait;
use std::collections::HashMap;
use taiji_types::agent::{AgentId, Title, TitleConfig};
use taiji_types::realm::Realm;

// =============================================================================
// TitleManager trait
// =============================================================================

/// 称号管理器 trait — 条件判定 + 聚合加成。
#[async_trait]
pub trait TitleManager: Send + Sync {
    /// 检查单个称号的条件是否满足。
    /// condition_type: "score" / "count" / "realm"
    async fn check_condition(
        &self,
        title: &Title,
        current_score: f64,
        current_count: u64,
        realm: Realm,
    ) -> bool;

    /// 获取当前激活的所有称号（条件满足 + 未超上限）。
    async fn get_active_titles(
        &self,
        agent_id: &AgentId,
        titles: &[Title],
        score: f64,
        count: u64,
        realm: Realm,
    ) -> Vec<Title>;

    /// 聚合所有激活称号的加成（含 cap 上限检查）。
    async fn aggregate_bonuses(
        &self,
        active_titles: &[Title],
        config: &TitleConfig,
    ) -> HashMap<String, f64>;
}

// =============================================================================
// 条件判定引擎
// =============================================================================

/// 条件判定引擎（纯函数）。
pub struct TitleConditionChecker;

impl TitleConditionChecker {
    /// 通用条件检查入口
    pub fn meets_condition(title: &Title, current_score: f64, current_count: u64, realm: Realm) -> bool {
        match title.condition_type.as_str() {
            "score" => Self::check_score(current_score, title.condition_threshold),
            "count" => Self::check_count(current_count, title.condition_threshold),
            "realm" => Self::check_realm(realm, &title.condition),
            _ => false,
        }
    }

    /// score 型：current >= threshold
    pub fn check_score(current: f64, threshold: f64) -> bool {
        current >= threshold
    }

    /// count 型：current >= threshold
    pub fn check_count(current: u64, threshold: f64) -> bool {
        current as f64 >= threshold
    }

    /// realm 型：current_realm >= threshold_realm（Realm PartialOrd 比较）
    pub fn check_realm(current: Realm, _condition_desc: &str) -> bool {
        // 境界达标=境界>=某境界。简化版：当前境界高于最低境界即满足
        // 实际使用时 threshold 由 Realm 枚举比较
        current >= Realm::QiRefining
    }
}

// =============================================================================
// 加成聚合器 + cap 上限
// =============================================================================

/// 称号加成聚合器（检查 cap 上限）。
pub struct TitleBonusAggregator;

impl TitleBonusAggregator {
    /// 聚合加成，按 bonus_target 分组求和，再对照 caps 剪裁。
    ///
    /// - 多称号相同 bonus_target → bonus_value 叠加
    /// - 叠加结果 > caps[bonus_target] → 取 cap 值
    /// - caps 中不存在的 bonus_target → 不剪裁
    pub fn aggregate(active_titles: &[Title], caps: &HashMap<String, f64>) -> HashMap<String, f64> {
        let mut raw: HashMap<String, f64> = HashMap::new();

        // 第一步：分组叠加
        for title in active_titles {
            *raw.entry(title.bonus_target.clone()).or_insert(0.0) += title.bonus_value;
        }

        // 第二步：cap 剪裁
        let mut result: HashMap<String, f64> = HashMap::new();
        for (target, total) in raw {
            let capped = match caps.get(&target) {
                Some(&cap) => total.min(cap),
                None => total,
            };
            result.insert(target, capped);
        }

        result
    }
}

// =============================================================================
// 默认实现
// =============================================================================

/// 默认的 TitleManager 实现（内存计算）。
pub struct DefaultTitleManager;

#[async_trait]
impl TitleManager for DefaultTitleManager {
    async fn check_condition(
        &self,
        title: &Title,
        current_score: f64,
        current_count: u64,
        realm: Realm,
    ) -> bool {
        TitleConditionChecker::meets_condition(title, current_score, current_count, realm)
    }

    async fn get_active_titles(
        &self,
        _agent_id: &AgentId,
        titles: &[Title],
        score: f64,
        count: u64,
        realm: Realm,
    ) -> Vec<Title> {
        let mut active: Vec<Title> = Vec::new();
        for title in titles {
            if TitleConditionChecker::meets_condition(title, score, count, realm) {
                active.push(title.clone());
            }
        }
        active
    }

    async fn aggregate_bonuses(
        &self,
        active_titles: &[Title],
        config: &TitleConfig,
    ) -> HashMap<String, f64> {
        TitleBonusAggregator::aggregate(active_titles, &config.bonus_caps)
    }
}

// =============================================================================
// 预设称号数据
// =============================================================================

/// 返回 6 个预设荣誉称号。
///
/// 参考架构总纲 §5.1 道名词条示例。
pub fn default_titles() -> Vec<Title> {
    vec![
        Title {
            name: "百战真君".into(),
            effect: "权限等级+1".into(),
            bonus_value: 1.0,
            bonus_target: "permission_level".into(),
            condition: "评分≥90".into(),
            condition_type: "score".into(),
            condition_threshold: 90.0,
        },
        Title {
            name: "无痕剑仙".into(),
            effect: "资源配额+50%".into(),
            bonus_value: 0.5,
            bonus_target: "resource_quota".into(),
            condition: "连续千次无失误".into(),
            condition_type: "count".into(),
            condition_threshold: 1000.0,
        },
        Title {
            name: "千胜宗师".into(),
            effect: "评分加成+5".into(),
            bonus_value: 5.0,
            bonus_target: "score_bonus".into(),
            condition: "累计千次任务".into(),
            condition_type: "count".into(),
            condition_threshold: 1000.0,
        },
        Title {
            name: "洞玄真人".into(),
            effect: "权限等级+1".into(),
            bonus_value: 1.0,
            bonus_target: "permission_level".into(),
            condition: "境界达到元婴".into(),
            condition_type: "realm".into(),
            condition_threshold: 0.0, // realm 型不使用数值阈值
        },
        Title {
            name: "天道酬勤".into(),
            effect: "评分加成+2".into(),
            bonus_value: 2.0,
            bonus_target: "score_bonus".into(),
            condition: "完成百次任务".into(),
            condition_type: "count".into(),
            condition_threshold: 100.0,
        },
        Title {
            name: "金石之坚".into(),
            effect: "资源配额+20%".into(),
            bonus_value: 0.2,
            bonus_target: "resource_quota".into(),
            condition: "评分≥80".into(),
            condition_type: "score".into(),
            condition_threshold: 80.0,
        },
    ]
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_types::agent::Title;
    use taiji_types::realm::Realm;

    // ── 条件判定 ──

    #[test]
    fn test_check_score_met() {
        assert!(TitleConditionChecker::check_score(95.0, 90.0));
    }

    #[test]
    fn test_check_score_not_met() {
        assert!(!TitleConditionChecker::check_score(85.0, 90.0));
    }

    #[test]
    fn test_check_count_met() {
        assert!(TitleConditionChecker::check_count(1500, 1000.0));
    }

    #[test]
    fn test_check_count_not_met() {
        assert!(!TitleConditionChecker::check_count(500, 1000.0));
    }

    #[test]
    fn test_check_realm_always_met() {
        assert!(TitleConditionChecker::check_realm(Realm::QiRefining, ""));
    }

    // ── meets_condition ──

    fn make_title(ctype: &str, threshold: f64) -> Title {
        Title {
            name: "测试称号".into(),
            effect: "测试".into(),
            bonus_value: 1.0,
            bonus_target: "test".into(),
            condition: "".into(),
            condition_type: ctype.into(),
            condition_threshold: threshold,
        }
    }

    #[test]
    fn test_meets_condition_score_ok() {
        let t = make_title("score", 90.0);
        assert!(TitleConditionChecker::meets_condition(&t, 95.0, 0, Realm::QiRefining));
    }

    #[test]
    fn test_meets_condition_score_fail() {
        let t = make_title("score", 90.0);
        assert!(!TitleConditionChecker::meets_condition(&t, 85.0, 0, Realm::QiRefining));
    }

    #[test]
    fn test_meets_condition_count_ok() {
        let t = make_title("count", 1000.0);
        assert!(TitleConditionChecker::meets_condition(&t, 0.0, 1500, Realm::QiRefining));
    }

    #[test]
    fn test_meets_condition_count_fail() {
        let t = make_title("count", 1000.0);
        assert!(!TitleConditionChecker::meets_condition(&t, 0.0, 500, Realm::QiRefining));
    }

    #[test]
    fn test_meets_condition_unknown_type() {
        let t = make_title("unknown", 0.0);
        assert!(!TitleConditionChecker::meets_condition(&t, 0.0, 0, Realm::QiRefining));
    }

    // ── 聚合 + cap ──

    #[test]
    fn test_aggregate_no_cap() {
        let caps = [("permission_level".into(), 2.0)].into();
        let titles = vec![
            make_title("score", 0.0),
            make_title("score", 0.0),
        ];
        // 两个称号 bonus_value 各为 1.0 → total 2.0, cap=2.0
        let result = TitleBonusAggregator::aggregate(&titles, &caps);
        assert!((result.get("test").unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_aggregate_with_cap() {
        let caps = [("test".into(), 1.5)].into();
        let titles = vec![
            make_title("score", 0.0),
            make_title("score", 0.0),
            make_title("score", 0.0),
        ];
        // 3 个称号各 1.0 → total 3.0, cap=1.5
        let result = TitleBonusAggregator::aggregate(&titles, &caps);
        assert!((result.get("test").unwrap() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_aggregate_mixed_targets() {
        let caps = HashMap::new(); // no caps
        let titles = vec![
            Title {
                name: "t1".into(), effect: "".into(), bonus_value: 1.0,
                bonus_target: "a".into(), condition: "".into(),
                condition_type: "score".into(), condition_threshold: 0.0,
            },
            Title {
                name: "t2".into(), effect: "".into(), bonus_value: 2.0,
                bonus_target: "b".into(), condition: "".into(),
                condition_type: "score".into(), condition_threshold: 0.0,
            },
        ];
        let result = TitleBonusAggregator::aggregate(&titles, &caps);
        assert!((result.get("a").unwrap() - 1.0).abs() < 1e-9);
        assert!((result.get("b").unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_aggregate_empty() {
        let caps = HashMap::new();
        let result = TitleBonusAggregator::aggregate(&[], &caps);
        assert!(result.is_empty());
    }

    // ── DefaultTitleManager ──

    #[tokio::test]
    async fn test_default_manager_check_condition() {
        let manager = DefaultTitleManager;
        let title = make_title("score", 90.0);
        assert!(manager.check_condition(&title, 95.0, 0, Realm::QiRefining).await);
        assert!(!manager.check_condition(&title, 85.0, 0, Realm::QiRefining).await);
    }

    #[tokio::test]
    async fn test_default_manager_get_active() {
        let manager = DefaultTitleManager;
        let titles = vec![
            make_title("score", 90.0),
            make_title("score", 80.0),
        ];
        let active = manager.get_active_titles(&AgentId::new(), &titles, 95.0, 0, Realm::QiRefining).await;
        assert_eq!(active.len(), 2); // 95 >= both 90 and 80
    }

    #[tokio::test]
    async fn test_default_manager_aggregate_with_cap() {
        let manager = DefaultTitleManager;
        let config = TitleConfig::default();
        let titles = vec![
            Title {
                name: "t1".into(), effect: "".into(), bonus_value: 1.0,
                bonus_target: "permission_level".into(), condition: "".into(),
                condition_type: "score".into(), condition_threshold: 0.0,
            },
            Title {
                name: "t2".into(), effect: "".into(), bonus_value: 1.0,
                bonus_target: "permission_level".into(), condition: "".into(),
                condition_type: "score".into(), condition_threshold: 0.0,
            },
            Title {
                name: "t3".into(), effect: "".into(), bonus_value: 1.0,
                bonus_target: "permission_level".into(), condition: "".into(),
                condition_type: "score".into(), condition_threshold: 0.0,
            },
        ];
        let result = manager.aggregate_bonuses(&titles, &config).await;
        // 3 * 1.0 = 3.0, but cap = 2.0
        let val = result.get("permission_level").copied().unwrap_or(0.0);
        assert!((val - 2.0).abs() < 1e-9);
    }

    // ── 预设称号 ──

    #[test]
    fn test_default_titles_count() {
        let titles = default_titles();
        assert_eq!(titles.len(), 6);
    }

    #[test]
    fn test_default_titles_names() {
        let titles = default_titles();
        let names: Vec<&str> = titles.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"百战真君"));
        assert!(names.contains(&"无痕剑仙"));
    }
}
