//! WardenRuntime — mechanism-level enforcement of Warden discipline rules.
//!
//! The Warden SKILL defines what a Warden *would* do (poke, remind, record)
//! as an agent; this runtime turns those rules into hooks on the agent loop:
//!
//! - **Turn-driven** ([`WardenRuntime::on_turn_outcome`]): every turn outcome
//!   advances the poke scheduler and evaluates consecutive failures against a
//!   configurable [`ViolationPolicy`] (default L1=1, L2=2, L3=3).
//! - **Tool-driven** ([`WardenRuntime::on_tool_outcome`]): every finished tool
//!   call updates a per-session consecutive tool-failure counter; errors
//!   escalate through the same [`ViolationPolicy`] ladder (rule
//!   `warden.tool-failure`) while successes clear the counter. This is a
//!   finer-grained audit layered on top of the turn-driven one.
//! - **Violation recording (R-25)**: when the policy fires, a
//!   [`PenaltyRequest`] with source [`WARDEN_RUNTIME_SESSION`] is executed
//!   through [`PunishmentExecutor::execute_penalty`]; the violation is
//!   recorded on the shame wall and resulting reminders are queued as
//!   `PokePenalty` internal messages and delivered by the scheduler at the
//!   next turn start (see `scheduler.rs` wiring). Per user ruling R-25 the
//!   escalation ladder only changes the reminder, never RBAC state: no
//!   demotion, no read-only patch, no freeze.
//! - **Challenge-Poke**: a Poisson-driven `ChallengePoke` internal message is
//!   queued on a randomized basis (default average 6.5 turns, per SKILL 5-8).
//! - **Persistence**: when constructed with
//!   [`WardenRuntime::with_shame_wall_path`], the shame wall registry is loaded
//!   at startup and saved after every penalty.
//!
//! All thresholds are configurable; the runtime never hard-codes rules beyond
//! the defaults below.

use crate::agentic::core::{InternalReminderKind, Message};
use crate::agentic::coordination::turn_outcome::TurnOutcomeStatus;
use crate::agentic::session::SessionManager;
use crate::agentic::warden::punishment_executor::PunishmentExecutor;
use crate::agentic::warden::{
    ChallengePokeConfig, PenaltyLevel, PenaltyRequest, PokeMessage, PokePriorityManager,
    ShameWallRegistry, ViolationRecord, WARDEN_RUNTIME_SESSION,
};
use chrono::Utc;
use log::warn;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Default rule set referenced by Challenge-Poke messages.
///
/// Mirrors the Warden SKILL's "iron-rules compliance proof" requirement.
pub const DEFAULT_CHALLENGE_RULES: [&str; 1] = ["iron-rules-compliance"];

/// Classification of one finished tool call for Warden audit.
///
/// F3: admission-level rejections (stale tool catalog, deferred-tool gateway,
/// runtime restrictions) are protocol-layer outcomes, not execution
/// violations. They never contribute to the tool-failure counter or the
/// penalty ladder; only real execution failures (`ExecutionFailed`) do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenToolOutcome {
    /// The tool call succeeded; clears the consecutive tool-failure counter.
    Success,
    /// The tool's admission was rejected before execution (stale/deferred
    /// gate, runtime restrictions). A deliberate no-op for the failure
    /// counter: neither counted as a violation nor resetting existing counts.
    AdmissionRejected,
    /// The tool really failed during execution; counts toward the penalty
    /// ladder (rule `warden.tool-failure`).
    ExecutionFailed,
}

/// Consecutive-failure thresholds mapped to penalty levels.
///
/// Configurable so downstream callers can tighten or loosen the ladder without
/// changing the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolationPolicy {
    /// Consecutive failures at or above which an L1 penalty fires (default 1).
    pub l1_at: u32,
    /// Consecutive failures at or above which an L2 penalty fires (default 2).
    pub l2_at: u32,
    /// Consecutive failures at or above which an L3 penalty fires (default 3).
    pub l3_at: u32,
}

impl Default for ViolationPolicy {
    fn default() -> Self {
        Self {
            l1_at: 1,
            l2_at: 2,
            l3_at: 3,
        }
    }
}

impl ViolationPolicy {
    /// Map a consecutive-failure count to the penalty level it triggers.
    ///
    /// Returns `None` when the count has not reached `l1_at` yet.
    pub fn level_for(&self, consecutive_failures: u32) -> Option<PenaltyLevel> {
        if consecutive_failures >= self.l3_at {
            Some(PenaltyLevel::L3)
        } else if consecutive_failures >= self.l2_at {
            Some(PenaltyLevel::L2)
        } else if consecutive_failures >= self.l1_at {
            Some(PenaltyLevel::L1)
        } else {
            None
        }
    }
}

/// Severity label for a violation record, matching the Warden SKILL ladder.
fn severity_for_level(level: &PenaltyLevel) -> &'static str {
    match level {
        PenaltyLevel::L1 => "minor",
        PenaltyLevel::L2 => "major",
        PenaltyLevel::L3 | PenaltyLevel::L4 => "critical",
    }
}

/// Scheduler-embedded Warden runtime.
///
/// Owns the punishment executor, shame wall registry, poke priority manager
/// and challenge scheduler, and exposes turn hooks the agent loop calls.
pub struct WardenRuntime {
    punisher: PunishmentExecutor,
    shame_wall: ShameWallRegistry,
    poke_priority: PokePriorityManager,
    challenge: ChallengePokeConfig,
    violation_policy: ViolationPolicy,
    /// Per-session consecutive failure count (reset on Completed).
    consecutive_failures: HashMap<String, u32>,
    /// Per-session consecutive tool-failure count (reset on tool success),
    /// independent of the turn-level counter.
    tool_failures: HashMap<String, u32>,
    /// Internal messages queued for the next turn start of a session.
    pending_reminders: HashMap<String, Vec<Message>>,
    /// Optional shame-wall persistence path (aligned to the Warden SKILL's
    /// `.master-framework/shame-wall-registry.json` by default, configurable
    /// to a skill-convention path such as `L0/SHAME_WALL.md`).
    shame_wall_path: Option<PathBuf>,
}

impl WardenRuntime {
    /// Create a runtime with default policy (in-memory shame wall).
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            punisher: PunishmentExecutor::new(session_manager),
            shame_wall: ShameWallRegistry::default(),
            poke_priority: PokePriorityManager::new(),
            challenge: ChallengePokeConfig::new(
                6.5,
                42,
                DEFAULT_CHALLENGE_RULES.iter().map(|s| s.to_string()).collect(),
            ),
            violation_policy: ViolationPolicy::default(),
            consecutive_failures: HashMap::new(),
            tool_failures: HashMap::new(),
            pending_reminders: HashMap::new(),
            shame_wall_path: None,
        }
    }

    /// Create a runtime that persists the shame wall registry to `path`.
    ///
    /// An existing registry is loaded at startup; a missing or unparseable
    /// file falls back to an empty registry (the failure is logged, not fatal).
    pub fn with_shame_wall_path(session_manager: Arc<SessionManager>, path: PathBuf) -> Self {
        let mut runtime = Self::new(session_manager);
        match ShameWallRegistry::load_from_path(&path) {
            Ok(registry) => runtime.shame_wall = registry,
            Err(err) => warn!(
                "warden runtime: falling back to empty shame wall registry at {}: {}",
                path.display(),
                err
            ),
        }
        runtime.shame_wall_path = Some(path);
        runtime
    }

    /// Replace the violation policy (thresholds for L1/L2/L3 penalties).
    pub fn set_violation_policy(&mut self, policy: ViolationPolicy) {
        self.violation_policy = policy;
    }

    /// Replace the challenge-poke configuration (rate, seed, rule set).
    pub fn set_challenge_config(&mut self, config: ChallengePokeConfig) {
        self.challenge = config;
    }

    /// Advance the global turn counter and evaluate the outcome.
    ///
    /// Called once per completed agent turn by the scheduler:
    /// - `Failed` increments the session's consecutive-failure count and, when
    ///   the policy threshold is reached, executes a penalty (L1 → L2 → L3)
    ///   and queues the penalty reminders for the next turn.
    /// - `Completed` clears the failure count and defer state.
    /// - `Cancelled` is a no-op.
    ///
    /// A Challenge-Poke may fire on any turn, independently of the outcome.
    pub async fn on_turn_outcome(
        &mut self,
        session_id: &str,
        status: TurnOutcomeStatus,
        turn_id: &str,
    ) {
        // R-26: the user-controllable RBAC/Warden master switch fully disables
        // the Warden runtime (no failure tracking, no violation records, no
        // reminders) when off.
        if !crate::service::config::rbac_enabled() {
            return;
        }

        self.poke_priority.advance_turn();

        match status {
            TurnOutcomeStatus::Failed => {
                self.handle_failed_turn(session_id, turn_id).await;
            }
            TurnOutcomeStatus::Completed => {
                self.consecutive_failures.remove(session_id);
                self.poke_priority.reset_defer_count(session_id);
            }
            TurnOutcomeStatus::Cancelled => {}
        }

        // Challenge-Poke fires on a Poisson schedule, outcome-independent.
        if self.challenge.should_challenge() {
            let poke = self
                .challenge
                .build_challenge_message(Uuid::new_v4().to_string());
            let text = serde_json::to_string(&poke)
                .unwrap_or_else(|_| format_challenge_fallback(&poke));
            self.push_reminder(
                session_id,
                Message::internal_reminder(InternalReminderKind::ChallengePoke, text),
            );
        }
    }

    /// Evaluate one finished tool call of a session.
    ///
    /// Called by the tool pipeline on its custom point (outside the hook
    /// dispatch channel, so `app.hooks.enabled` cannot gate it):
    /// - `ExecutionFailed` increments the session's consecutive tool-failure
    ///   count and, when the policy threshold is reached, executes a penalty
    ///   (L1 → L2 → L3) with rule id `warden.tool-failure`.
    /// - `Success` clears the tool-failure count.
    /// - `AdmissionRejected` (F3: stale/deferred gate or runtime-restriction
    ///   rejections) is a protocol-layer outcome, not an execution violation:
    ///   it is a deliberate no-op — neither counted nor clearing existing
    ///   counts — so a stale-tool wave cannot fire a penalty and cannot reset
    ///   a genuine escalation ladder in progress.
    ///
    /// Tool-level violations are independent of the turn-level counter; a
    /// successful turn never resets the tool counter and a successful tool
    /// never resets the turn counter. Challenge-Poke is not triggered here.
    pub async fn on_tool_outcome(
        &mut self,
        session_id: &str,
        tool_name: &str,
        failure_kind: WardenToolOutcome,
    ) {
        // R-26: master switch off disables tool-level Warden tracking.
        if !crate::service::config::rbac_enabled() {
            return;
        }

        match failure_kind {
            WardenToolOutcome::Success => {
                self.tool_failures.remove(session_id);
            }
            WardenToolOutcome::AdmissionRejected => {
                // Protocol-layer rejection: not an execution violation, and
                // deliberately neutral to any in-progress escalation ladder.
            }
            WardenToolOutcome::ExecutionFailed => {
                self.handle_failed_tool(session_id, tool_name).await;
            }
        }
    }

    /// Take (and clear) the queued reminders for `session_id`.
    pub fn take_pending_reminders(&mut self, session_id: &str) -> Vec<Message> {
        self.pending_reminders.remove(session_id).unwrap_or_default()
    }

    /// Drop all per-session Warden state for `session_id` (session-end cleanup).
    ///
    /// Clears failure counters, queued reminders and poke defer state so a
    /// recycled session id cannot inherit stale enforcement state. The shame
    /// wall registry is a historical record keyed by session name and is
    /// intentionally preserved.
    pub fn cleanup_session(&mut self, session_id: &str) {
        self.consecutive_failures.remove(session_id);
        self.tool_failures.remove(session_id);
        self.pending_reminders.remove(session_id);
        self.poke_priority.clear_session(session_id);
    }

    /// Current consecutive-failure count for a session (observation/test hook).
    pub fn consecutive_failures(&self, session_id: &str) -> u32 {
        self.consecutive_failures.get(session_id).copied().unwrap_or(0)
    }

    /// Current consecutive tool-failure count for a session (observation/test hook).
    pub fn tool_failures(&self, session_id: &str) -> u32 {
        self.tool_failures.get(session_id).copied().unwrap_or(0)
    }

    /// Current shame wall registry (observation/test hook).
    pub fn shame_wall(&self) -> &ShameWallRegistry {
        &self.shame_wall
    }

    /// Current global turn counter (observation/test hook).
    pub fn current_turn(&self) -> u64 {
        self.poke_priority.current_turn()
    }

    async fn handle_failed_turn(&mut self, session_id: &str, turn_id: &str) {
        let count = {
            let entry = self.consecutive_failures.entry(session_id.to_string()).or_insert(0);
            *entry += 1;
            *entry
        };

        let Some(level) = self.violation_policy.level_for(count) else {
            return;
        };

        self.apply_violation_penalty(
            session_id,
            "warden.consecutive-failure",
            format!(
                "turn failed (turn_id={}, consecutive_failures={})",
                turn_id, count
            ),
            serde_json::json!({
                "turn_id": turn_id,
                "status": TurnOutcomeStatus::Failed.as_str(),
                "consecutive_failures": count,
            }),
            &level,
        )
        .await;
    }

    async fn handle_failed_tool(&mut self, session_id: &str, tool_name: &str) {
        let count = {
            let entry = self.tool_failures.entry(session_id.to_string()).or_insert(0);
            *entry += 1;
            *entry
        };

        let Some(level) = self.violation_policy.level_for(count) else {
            return;
        };

        self.apply_violation_penalty(
            session_id,
            "warden.tool-failure",
            format!(
                "tool failed (tool={}, consecutive_tool_failures={})",
                tool_name, count
            ),
            serde_json::json!({
                "tool_name": tool_name,
                "consecutive_tool_failures": count,
            }),
            &level,
        )
        .await;
    }

    async fn apply_violation_penalty(
        &mut self,
        session_id: &str,
        rule_id: &str,
        description: String,
        evidence: serde_json::Value,
        level: &PenaltyLevel,
    ) {
        let now = Utc::now().to_rfc3339();
        let request = PenaltyRequest {
            target_session_id: session_id.to_string(),
            level: level.clone(),
            violations: vec![ViolationRecord {
                rule_id: rule_id.to_string(),
                description,
                severity: severity_for_level(level).to_string(),
                timestamp: now.clone(),
                evidence,
            }],
            requested_by: WARDEN_RUNTIME_SESSION.to_string(),
        };

        match self
            .punisher
            .execute_penalty(request, &mut self.shame_wall, &now)
            .await
        {
            Ok(outcome) => {
                for reminder in outcome.prepended_reminders {
                    self.push_reminder(
                        session_id,
                        Message::internal_reminder(InternalReminderKind::PokePenalty, reminder.text),
                    );
                }
                if let Some(path) = &self.shame_wall_path {
                    if let Err(err) = self.shame_wall.save_to_path(path) {
                        warn!(
                            "warden runtime: failed to persist shame wall at {}: {}",
                            path.display(),
                            err
                        );
                    }
                }
            }
            Err(err) => {
                warn!(
                    "warden runtime: penalty failed for session '{}' (level={:?}): {}",
                    session_id, level, err
                );
            }
        }
    }

    fn push_reminder(&mut self, session_id: &str, message: Message) {
        self.pending_reminders
            .entry(session_id.to_string())
            .or_default()
            .push(message);
    }
}

/// Human-readable fallback for a Challenge-Poke message (used only if JSON
/// serialization unexpectedly fails).
fn format_challenge_fallback(poke: &PokeMessage) -> String {
    format!(
        "[Challenge-Poke {}] rules={} deadline={} turns",
        poke.poke_id,
        poke.rule_ids.join(","),
        poke.deadline_turns
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::MessageContent;
    use std::collections::BTreeSet;

    fn runtime() -> WardenRuntime {
        // verify_warden_session short-circuits the warden-runtime source, so
        // no real SessionManager-backed session is required for penalties.
        WardenRuntime::new(test_session_manager())
    }

    fn test_session_manager() -> Arc<SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManagerConfig,
        };
        use crate::infrastructure::app_paths::PathManager;
        use std::time::Duration;

        let root = std::env::temp_dir().join(format!("bitfun-warden-test-{}", Uuid::new_v4()));
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(root.join("user-root")));
        let persistence_manager =
            Arc::new(PersistenceManager::new(path_manager).expect("persistence manager"));
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

    #[test]
    fn violation_policy_default_ladder() {
        let policy = ViolationPolicy::default();
        assert_eq!(policy.level_for(0), None);
        assert_eq!(policy.level_for(1), Some(PenaltyLevel::L1));
        assert_eq!(policy.level_for(2), Some(PenaltyLevel::L2));
        assert_eq!(policy.level_for(3), Some(PenaltyLevel::L3));
        assert_eq!(policy.level_for(9), Some(PenaltyLevel::L3));
    }

    #[test]
    fn violation_policy_custom_thresholds() {
        let policy = ViolationPolicy {
            l1_at: 3,
            l2_at: 5,
            l3_at: 7,
        };
        assert_eq!(policy.level_for(2), None);
        assert_eq!(policy.level_for(3), Some(PenaltyLevel::L1));
        assert_eq!(policy.level_for(5), Some(PenaltyLevel::L2));
        assert_eq!(policy.level_for(7), Some(PenaltyLevel::L3));
    }

    #[tokio::test]
    async fn consecutive_failures_escalate_l1_l2_l3() {
        let mut rt = runtime();
        // Challenge disabled for deterministic penalty assertions.
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 1);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L1 fires on first failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 2);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L2 fires on second failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t3").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 3);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L3 fires on third failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L3
        );
    }

    #[tokio::test]
    async fn completed_turn_resets_failure_state() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 1);
        rt.take_pending_reminders("sess-b");

        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Completed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 0, "completed resets failures");

        // Next failure starts at L1 again.
        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t3").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 1);
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-b").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );
    }

    #[tokio::test]
    async fn challenge_poke_fires_with_rate_one() {
        let mut rt = runtime();
        // rate=1.0 -> every turn pokes deterministically.
        rt.set_challenge_config(ChallengePokeConfig::new(
            1.0,
            7,
            BTreeSet::from(["iron-rules-compliance".to_string()]),
        ));

        rt.on_turn_outcome("sess-c", TurnOutcomeStatus::Completed, "t1").await;
        let reminders = rt.take_pending_reminders("sess-c");
        assert_eq!(reminders.len(), 1, "rate=1.0 must poke every turn");
        let MessageContent::Text(text) = &reminders[0].content else {
            panic!("challenge reminder must be a text message");
        };
        assert!(
            text.to_lowercase().contains("challenge"),
            "challenge poke must be serialized, got: {text}"
        );
    }

    #[tokio::test]
    async fn pending_reminders_take_is_destructive() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            1.0,
            7,
            BTreeSet::from(["iron-rules-compliance".to_string()]),
        ));

        rt.on_turn_outcome("sess-d", TurnOutcomeStatus::Completed, "t1").await;
        let first = rt.take_pending_reminders("sess-d");
        assert_eq!(first.len(), 1);
        let second = rt.take_pending_reminders("sess-d");
        assert!(second.is_empty(), "take clears the queue");
    }

    #[tokio::test]
    async fn cleanup_session_drops_all_per_session_state() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // Build per-session state: failure counters, tool failures, reminders.
        rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-e"), 1);
        rt.on_tool_outcome("sess-e", "Write", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-e"), 1);
        // Failure paths above queue escalation reminders; drain them so the
        // count below covers only the explicit push.
        rt.take_pending_reminders("sess-e");
        rt.push_reminder(
            "sess-e",
            Message::internal_reminder(InternalReminderKind::PokePenalty, "penalty"),
        );
        assert_eq!(rt.take_pending_reminders("sess-e").len(), 1);
        // A sibling session must be untouched.
        rt.on_turn_outcome("sess-f", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-f"), 1);

        rt.cleanup_session("sess-e");
        assert_eq!(rt.consecutive_failures("sess-e"), 0, "failures cleared");
        assert_eq!(rt.tool_failures("sess-e"), 0, "tool failures cleared");
        assert!(
            rt.take_pending_reminders("sess-e").is_empty(),
            "reminders cleared"
        );
        assert_eq!(rt.consecutive_failures("sess-f"), 1, "sibling untouched");

        // Idempotent: clearing a session with no state is a no-op.
        rt.cleanup_session("sess-e");
    }

    #[tokio::test]
    async fn shame_wall_persistence_round_trip() {
        let dir = std::env::temp_dir().join(format!("warden-test-{}", Uuid::new_v4()));
        let path = dir.join("shame-wall-registry.json");

        {
            let mut rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
            rt.set_challenge_config(ChallengePokeConfig::new(
                f64::INFINITY,
                1,
                BTreeSet::new(),
            ));
            rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t1").await;
            rt.take_pending_reminders("sess-e");
            assert!(path.exists(), "penalty must persist the registry");
        }

        // A second runtime loads the persisted registry.
        let rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-e").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1,
            "loaded registry keeps the recorded penalty"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_shame_wall_file_starts_empty() {
        let dir = std::env::temp_dir().join(format!("warden-test-missing-{}", Uuid::new_v4()));
        let path = dir.join("shame-wall-registry.json");
        let rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
        assert!(rt.shame_wall().entries.is_empty(), "missing file -> empty registry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tool_failures_escalate_l1_l2_l3() {
        let mut rt = runtime();
        // Challenge disabled for deterministic penalty assertions.
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_tool_outcome("sess-f", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-f"), 1);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L1 fires on first tool failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );

        rt.on_tool_outcome("sess-f", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-f"), 2);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L2 fires on second tool failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );

        rt.on_tool_outcome("sess-f", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-f"), 3);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L3 fires on third tool failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L3
        );
    }

    #[tokio::test]
    async fn successful_tool_resets_failure_count() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_tool_outcome("sess-g", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-g"), 1);
        rt.take_pending_reminders("sess-g");

        rt.on_tool_outcome("sess-g", "ExecCommand", WardenToolOutcome::Success).await;
        assert_eq!(rt.tool_failures("sess-g"), 0, "success clears tool failures");

        // Next failure starts at L1 again.
        rt.on_tool_outcome("sess-g", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-g"), 1);
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-g").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );
    }

    #[tokio::test]
    async fn tool_failures_independent_from_turn_failures() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // One failed turn: turn counter = 1, tool counter untouched.
        rt.on_turn_outcome("sess-h", TurnOutcomeStatus::Failed, "t1").await;
        rt.take_pending_reminders("sess-h");
        assert_eq!(rt.consecutive_failures("sess-h"), 1);
        assert_eq!(rt.tool_failures("sess-h"), 0, "tool counter untouched by turn failure");

        // A successful tool must not reset the turn counter.
        rt.on_tool_outcome("sess-h", "ExecCommand", WardenToolOutcome::Success).await;
        assert_eq!(rt.consecutive_failures("sess-h"), 1, "turn counter unaffected by tool success");

        // A failed tool increments only the tool counter.
        rt.on_tool_outcome("sess-h", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        rt.take_pending_reminders("sess-h");
        assert_eq!(rt.tool_failures("sess-h"), 1);
        assert_eq!(rt.consecutive_failures("sess-h"), 1, "tool failure does not touch turn counter");
    }

    #[tokio::test]
    async fn admission_rejected_never_counts_as_tool_failure() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_tool_outcome("sess-i", "ExecCommand", WardenToolOutcome::AdmissionRejected)
            .await;
        assert_eq!(
            rt.tool_failures("sess-i"),
            0,
            "F3: admission rejection is not an execution violation"
        );
        assert!(
            rt.take_pending_reminders("sess-i").is_empty(),
            "no penalty reminder for admission rejection"
        );
        assert!(
            rt.shame_wall().entry_for_session("sess-i").is_none(),
            "no shame-wall record for admission rejection"
        );
    }

    #[tokio::test]
    async fn admission_rejected_is_neutral_to_in_progress_escalation_ladder() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // One real failure: L1 fired, ladder in progress.
        rt.on_tool_outcome("sess-j", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-j"), 1);
        rt.take_pending_reminders("sess-j");

        // F3: a stale/admission-rejected wave must not reset the ladder...
        rt.on_tool_outcome("sess-j", "ExecCommand", WardenToolOutcome::AdmissionRejected)
            .await;
        assert_eq!(
            rt.tool_failures("sess-j"),
            1,
            "admission rejection is a no-op, not a success"
        );

        // ...and the next real failure still escalates to L2.
        rt.on_tool_outcome("sess-j", "ExecCommand", WardenToolOutcome::ExecutionFailed).await;
        assert_eq!(rt.tool_failures("sess-j"), 2);
        let reminders = rt.take_pending_reminders("sess-j");
        assert_eq!(reminders.len(), 1, "L2 fires on the second real failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-j").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );
    }
}
