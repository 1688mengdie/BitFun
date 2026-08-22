//! Review propagation along the conversation tree - basic version
//!
//! When a leaf agent completes, review results propagate upward along the parent_session_id chain.

use log::{debug, info};

pub struct ReviewPropagationManager;

/// Review propagation action
pub enum ReviewPropagationAction {
    /// No action needed
    None,
    /// Suggest triggering a review of the parent session
    ReviewNeeded {
        parent_session_id: String,
        child_session_id: String,
    },
}

impl ReviewPropagationManager {
    /// Triggered when a leaf agent completes - checks the parent session and decides whether to propagate a review
    ///
    /// `suppress`（A-3，2026-08-22）：后台子代理路径（start_background_subagent）
    /// 完成由全文 follow-up turn 单通道投递（通知句 + 最终回复全文），
    /// ReviewPropagation 的极简 reminder 是第二条独立消息通道 → 双通道冗余
    /// （元数据通知 + 最终消息双通知，2026-08-18 复发）。该路径传 `true`
    /// 抑制 reminder，避免父会话收到两条完成通知；前台/调度路径传 `false`
    /// 保持既有 review 提醒语义。
    pub fn on_leaf_completed(
        session_id: &str,
        agent_type: &str,
        response_text: &str,
        parent_session_id: Option<&str>,
        suppress: bool,
    ) -> ReviewPropagationAction {
        info!(
            "ReviewPropagation: leaf agent completed session={} agent_type={} text_len={} parent={:?} suppress={}",
            session_id,
            agent_type,
            response_text.len(),
            parent_session_id,
            suppress,
        );

        if suppress {
            debug!(
                "ReviewPropagation: suppressed by caller (background subagent path delivers its own full-text follow-up): session={}, parent={:?}",
                session_id, parent_session_id
            );
            return ReviewPropagationAction::None;
        }

        match parent_session_id {
            Some(parent_id) if !parent_id.is_empty() => {
                debug!(
                    "ReviewPropagation: review may be needed for parent session={} (child={} completed)",
                    parent_id, session_id
                );
                ReviewPropagationAction::ReviewNeeded {
                    parent_session_id: parent_id.to_string(),
                    child_session_id: session_id.to_string(),
                }
            }
            _ => ReviewPropagationAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_leaf_completed_with_parent_suggests_review() {
        let action = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            Some("parent-1"),
            false,
        );
        match action {
            ReviewPropagationAction::ReviewNeeded {
                parent_session_id,
                child_session_id,
            } => {
                assert_eq!(parent_session_id, "parent-1");
                assert_eq!(child_session_id, "child-1");
            }
            ReviewPropagationAction::None => panic!("expected ReviewNeeded"),
        }
    }

    #[test]
    fn on_leaf_completed_without_parent_returns_none() {
        let action = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            None,
            false,
        );
        assert!(matches!(action, ReviewPropagationAction::None));

        let empty_parent = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            Some(""),
            false,
        );
        assert!(matches!(empty_parent, ReviewPropagationAction::None));
    }

    // A-3 防回退（2026-08-22）：后台子代理路径（start_background_subagent）
    // 完成由全文 follow-up turn 单通道投递；ReviewPropagation 极简 reminder
    // 是第二条独立通道 → 双通道冗余（元数据通知 + 最终消息双通知，
    // 2026-08-18 复发）。suppress=true 时即使有 parent 也必须返回 None——
    // 防未来有人改回无条件投递导致双通道复发。
    #[test]
    fn on_leaf_completed_suppressed_by_background_path_returns_none_even_with_parent() {
        let action = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            Some("parent-1"),
            true,
        );
        assert!(
            matches!(action, ReviewPropagationAction::None),
            "background subagent path must not deliver the duplicate review reminder"
        );
    }
}
