//! Local customizations ported onto the upstream feature-sliced runtime-ports
//! module structure (20260812 sync of `perf(build)!: slice portable contract
//! capabilities`).
//!
//! These types are local-only (BitFun fork customizations). Upstream split the
//! monolithic `lib.rs` into owner-scoped feature modules; the GroupChat /
//! AgentType / Warden / steering additions below are not part of upstream and
//! must be preserved for the fork's 群聊契约 / RBAC / steering.

use serde::{Deserialize, Serialize};

use super::{PortError, PortErrorKind, PortResult};

/// Shared agent type used by SessionControl and SessionMessage tools.
///
/// Known built-in variants have canonical serde representations:
/// - `Agentic` → `"agentic"` (canonical)
/// - `Plan` → `"Plan"` (canonical)
/// - `Cowork` → `"Cowork"` (canonical)
///
/// Any unrecognised string deserializes into `Other(String)`, so the enum
/// automatically tolerates agent types added by custom or external registries
/// without requiring a crate-level code change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentType {
    /// Known built-in variant: `agentic`.
    #[serde(rename = "agentic", alias = "Agentic", alias = "AGENTIC")]
    Agentic,
    /// Known built-in variant: `Plan`.
    #[serde(rename = "Plan", alias = "plan", alias = "PLAN")]
    Plan,
    /// Known built-in variant: `Cowork`.
    #[serde(rename = "Cowork", alias = "cowork", alias = "COWORK")]
    Cowork,
    /// Known built-in variant: `DeepResearch` (official research agent).
    #[serde(
        rename = "DeepResearch",
        alias = "deepresearch",
        alias = "DEEPRESEARCH"
    )]
    DeepResearch,
    /// Catch-all for any agent type string not in the known set (custom / external).
    #[serde(untagged)]
    Other(String),
}

impl AgentType {
    /// Returns the canonical wire representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
            Self::DeepResearch => "DeepResearch",
            Self::Other(value) => value.as_str(),
        }
    }

    /// Default agent type used when none is specified.
    pub const fn default_value() -> Self {
        Self::Agentic
    }

    /// Returns `true` if this is one of the three known built-in variants.
    pub fn is_known_builtin(&self) -> bool {
        matches!(
            self,
            Self::Agentic | Self::Plan | Self::Cowork | Self::DeepResearch
        )
    }
}

impl From<&str> for AgentType {
    fn from(value: &str) -> Self {
        match value {
            "agentic" | "Agentic" | "AGENTIC" => Self::Agentic,
            "Plan" | "plan" | "PLAN" => Self::Plan,
            "Cowork" | "cowork" | "COWORK" => Self::Cowork,
            "DeepResearch" | "deepresearch" | "DEEPRESEARCH" => Self::DeepResearch,
            other => Self::Other(other.to_string()),
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// prepended_reminders kind constants (Warden bootstrap/penalty injection kinds)
// ---------------------------------------------------------------------------

/// `prepended_reminders` kind value for penalty/violation record injection.
///
/// Injected into a violating session's context at every turn until cleared.
pub const POKE_PENALTY_KIND: &str = "PokePenalty";

/// `prepended_reminders` kind value for self-boot check (iron-rule summary +
/// Warden protocol declaration).
pub const SELF_BOOT_CHECK_KIND: &str = "SelfBootCheck";

/// `prepended_reminders` kind value for RBAC role-reminder injection.
pub const RBAC_ROLE_REMINDER_KIND: &str = "RbacRoleReminder";

// ---------------------------------------------------------------------------
// GroupChat contract (local fork customization, 常开)
// ---------------------------------------------------------------------------

/// 主人保留字（P0-2 修复）：主人无 Claw session_id，用保留字标识。
/// 权限校验对主人开例外通道（建群/拉人/发言全通）。
pub const GROUP_MASTER_ACTOR: &str = "__master__";

/// 群聊房间（持久化于 group-chats/<room_id>/meta.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatRoom {
    pub schema_version: u32,       // 存储格式版本（P1-11 修复）
    pub room_id: String,           // 群 ID（uuid，校验复用 validate_session_id 语义）
    pub name: String,              // 群名
    pub owner: GroupChatActor,     // 群主（创建者，主人或 Claw）
    pub mode: GroupChatMode,       // 通信模式
    pub round_robin_cursor: usize, // 轮转游标（P1-10 修复，后端落盘）
    pub created_at: i64,           // Unix ms
    pub last_active_at: i64,
    pub status: GroupChatStatus,
    pub member_limit: usize, // 成员上限（R-GC-26 配置化落地）
    #[serde(skip)] // members 唯一权威源 = members.json（P1-11 修复）
    pub members: Vec<GroupChatMember>,
}

/// 群聊参与者（P0-2 修复 + 复审 P0-1 修复：tag 化序列化，对齐 runtime-ports lib.rs 惯例）
/// 序列化形态（internally tagged，与 TS 一致）：
///   Master → {"kind":"master"}
///   Claw   → {"kind":"claw","sessionId":"...","agentType":"Claw"}
///   All    → {"kind":"all"}（@全体，复审 P1-4 修复：显式语义，非空数组哨兵）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupChatActor {
    Master, // 主人（__master__ 保留字）
    #[serde(rename_all = "camelCase")]
    Claw {
        session_id: String,
        agent_type: String,
    }, // Claw 助理会话（字段 camelCase 对齐 TS）
    All,    // @全体（P1-4 修复）
}

/// 通信模式（A+B 混合）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatMode {
    Free,       // A：自由聊天（全员可见，任何成员随时发言）
    RoundRobin, // B：轮转调度（cursor 点名发言）
}

/// 群聊成员（Claw 助理）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatMember {
    pub session_id: String, // 成员会话 ID（Claw 助理会话）
    pub role: GroupChatMemberRole,
    pub joined_at: i64,
    pub agent_type: String,           // 必须 "Claw"（P1-7 后端强制校验）
    pub display_name: Option<String>, // 来自 identity.name
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatMemberRole {
    Owner,  // 群主（创建者为 Owner）
    Member, // 普通成员
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatStatus {
    Active,
    Archived,
}

/// 群聊消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatMessage {
    pub message_id: String,         // 消息 ID（uuid）
    pub room_id: String,            // 所属群
    pub author: GroupChatActor,     // 发送者（主人或 Claw，P0-2 修复）
    pub kind: GroupChatMessageKind, // user | agent | system
    pub content: String,
    pub mention_targets: Vec<GroupChatActor>, // @ 目标（成员或全体；P1-6 语义明确）
    pub reply_to_message_id: Option<String>,  // 回复关联
    pub timestamp: i64,
    pub status: GroupChatMessageStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatMessageKind {
    User,   // 主人发言
    Agent,  // Claw 助理发言
    System, // 系统事件（成员加入/退出/模式切换/群删除）
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatMessageStatus {
    Pending,   // 派发中
    Delivered, // 已送达成员
    Replied,   // 已有成员回复（P1-6 修复）
    Failed,    // 派发失败
}

#[async_trait::async_trait]
pub trait GroupChatPort: Send + Sync {
    /// 创建群
    async fn create_room(
        &self,
        req: GroupChatCreateRequest,
    ) -> Result<GroupChatRoom, GroupChatError>;
    /// 加载群列表
    async fn list_rooms(&self, workspace_path: &str) -> Result<Vec<GroupChatRoom>, GroupChatError>;
    /// 加载单群
    async fn load_room(&self, room_id: &str) -> Result<GroupChatRoom, GroupChatError>;
    /// 读成员列表（P1-1 修复：serde(skip) 后 members 需独立读取通道）
    async fn list_members(&self, room_id: &str) -> Result<Vec<GroupChatMember>, GroupChatError>;
    /// 拉人进群
    async fn join_room(&self, req: GroupChatJoinRequest) -> Result<GroupChatRoom, GroupChatError>;
    /// 踢人出群
    async fn leave_room(&self, req: GroupChatLeaveRequest)
        -> Result<GroupChatRoom, GroupChatError>;
    /// 删除群（P0-3 修复：级联清消息 + 成员反标清理）
    async fn delete_room(&self, req: GroupChatDeleteRequest) -> Result<(), GroupChatError>;
    /// 切换模式（自由/轮转）
    async fn set_mode(&self, req: GroupChatModeRequest) -> Result<GroupChatRoom, GroupChatError>;
    /// 发消息（广播/定向/轮转）
    async fn send_message(
        &self,
        req: GroupChatSendRequest,
    ) -> Result<GroupChatSendResult, GroupChatError>;
    /// 读消息历史
    async fn list_messages(
        &self,
        req: GroupChatMessagesRequest,
    ) -> Result<GroupChatMessagesResponse, GroupChatError>;
    /// 回执写入（P1-5 修复：成员回复聚合回房间，驱动 Replied 状态）
    async fn ingest_reply(&self, req: GroupChatIngestReplyRequest) -> Result<(), GroupChatError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatCreateRequest {
    pub name: String,
    pub owner: GroupChatActor,        // 主人或 Claw（P0-2 修复）
    pub initial_members: Vec<String>, // 初始成员 session_id（Claw）
    pub mode: GroupChatMode,          // 默认 Free（P2-9 修复：前端不传时用 Free）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatJoinRequest {
    pub room_id: String,
    pub session_id: String,    // 新成员（Claw）
    pub actor: GroupChatActor, // 操作者（Owner 或主人）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatLeaveRequest {
    pub room_id: String,
    pub session_id: String,    // 被移出者
    pub actor: GroupChatActor, // 操作者（Owner/主人/自己）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatDeleteRequest {
    pub room_id: String,
    pub actor: GroupChatActor, // Owner 或主人
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatModeRequest {
    pub room_id: String,
    pub mode: GroupChatMode,
    pub actor: GroupChatActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatSendRequest {
    pub room_id: String,
    pub author: GroupChatActor, // 主人或成员（P0-2 修复）
    pub content: String,
    pub mention_targets: Vec<GroupChatActor>, // 空 = 全员
    pub urgent: bool,                         // @ 某人打断
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatSendResult {
    pub message_id: String,
    pub delivered_to: Vec<String>, // 已派发成员 session_id
    pub failed_to: Vec<GroupChatDeliveryFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatDeliveryFailure {
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatMessagesRequest {
    pub room_id: String,
    pub limit: Option<usize>,
    /// P2-6: cursor semantics unified as the next message index (opaque to the
    /// frontend, owned by the store). The contract exposes `usize` directly —
    /// no string bridge — so the Tauri command, the store, and the frontend
    /// all agree on the same cursor domain.
    pub cursor: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatMessagesResponse {
    pub messages: Vec<GroupChatMessage>,
    /// P2-6: next page start index (same domain as the request cursor).
    pub next_cursor: Option<usize>,
}

/// 回执写入请求（P1-5 修复：成员回复聚合回房间）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatIngestReplyRequest {
    pub room_id: String,
    pub message_id: String,     // 被回复的群聊消息 ID
    pub reply_content: String,  // 成员回复内容
    pub author: GroupChatActor, // 回复者（Claw）
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatError {
    pub code: GroupChatErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatErrorCode {
    NotFound,      // 群不存在
    AlreadyMember, // 已入群（去重）
    NotOwner,      // 非群主（无权限拉人/踢人/删群/切模式）
    EmptyMembers,  // 空群发消息
    RoomFull,      // 成员上限（R-GC-26 配置化）
    DuplicateName, // 群名重复
    InvalidTarget, // @ 目标无效
    NotClaw,       // 非 Claw 助理（P1-7 强制校验）
}

// ---------------------------------------------------------------------------
// Warden contract (local fork customization, 常开 — 阶段 1 保留，阶段 2 移除)
// ---------------------------------------------------------------------------

/// Request for a model-backed Warden audit judgement.
///
/// The judgement provider decides whether a finished tool call or failed turn
/// deserves a poke, which candidate rules apply, and what evidence should be
/// attached. When the port is unavailable or the judgement times out, the
/// caller falls back to the mechanical rule ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WardenAuditJudgementRequest {
    /// Session whose tool call / turn is being judged.
    pub session_id: String,
    /// Effective tool name of the finished tool call.
    pub tool_name: String,
    /// Effective arguments of the finished tool call (scene fingerprint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_args: Option<serde_json::Value>,
    /// Candidate rule ids the mechanical ladder would apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_ids: Vec<String>,
    /// Evidence summary available to the judgement (failure counts, error
    /// text, phase/target facts the caller can provide).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
}

/// Judgement result produced by a model-backed Warden provider.
///
/// A provider must not fail the audit loop: `should_poke = false` with empty
/// rule ids is a valid "no poke" verdict.
///
/// WARDEN-07: `shouldPoke` is intentionally *not* `#[serde(default)]`. A
/// verdict missing the field (or an empty object) fails to deserialize, so a
/// malformed model response falls back to the mechanical rule ladder instead
/// of silently defaulting to `false` and suppressing a poke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WardenAuditJudgementResponse {
    pub should_poke: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_ids: Vec<String>,
    /// Evidence items the model wants to see before poking (follow-ups).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_requested: Vec<String>,
}

/// Model-backed judgement for Warden audit decisions.
///
/// Providers construct a judgement prompt from the request, parse the model
/// response as [`WardenAuditJudgementResponse`], and return an error when the
/// response cannot be parsed or the judgement times out; the caller then
/// falls back to the mechanical rule ladder. Providers that do not support
/// model judgement keep the default typed unsupported response.
#[async_trait::async_trait]
pub trait WardenModelJudgementPort: Send + Sync {
    async fn judge_audit(
        &self,
        request: WardenAuditJudgementRequest,
    ) -> PortResult<WardenAuditJudgementResponse> {
        let _ = request;
        Err(PortError::new(
            PortErrorKind::NotAvailable,
            "model-backed warden judgement is not supported by this provider",
        ))
    }
}

// ---------------------------------------------------------------------------
// Steering / fission helpers (local fork customization)
// ---------------------------------------------------------------------------

/// RoundInjection steering-dedup marker (TOKEN-01).
///
/// The caller-supplied steering id uniquely identifies this user-steering
/// event end to end (the scheduler generates it in `buffer_steering` as
/// `Uuid::new_v4()`). `UserSteering` injections always carry it; the other
/// kinds return `None`.
#[cfg(feature = "agent-api")]
pub fn round_injection_dedup_key(injection: &super::RoundInjection) -> Option<&str> {
    use super::RoundInjectionKind;
    match injection.kind {
        RoundInjectionKind::UserSteering => Some(injection.id.as_str()),
        RoundInjectionKind::BackgroundResult | RoundInjectionKind::ThreadGoalObjectiveUpdated => {
            None
        }
    }
}

/// Appends a prepended reminder to a round injection in place (local helper).
#[cfg(feature = "agent-api")]
pub fn round_injection_push_reminder(
    injection: &mut super::RoundInjection,
    kind: impl Into<String>,
    text: impl Into<String>,
) {
    injection.prepended_reminders.push(super::AgentDialogPrependedReminder {
        kind: kind.into(),
        text: text.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_type_round_trips_all_variants() {
        assert_eq!(AgentType::from("agentic"), AgentType::Agentic);
        assert_eq!(AgentType::from("Plan"), AgentType::Plan);
        assert_eq!(AgentType::from("cowork"), AgentType::Cowork);
        assert_eq!(AgentType::from("DEEPRESEARCH"), AgentType::DeepResearch);
        assert_eq!(AgentType::from("custom-x"), AgentType::Other("custom-x".to_string()));
        assert_eq!(AgentType::default_value(), AgentType::Agentic);
        assert!(AgentType::Agentic.is_known_builtin());
        assert!(!AgentType::Other("x".to_string()).is_known_builtin());
        assert_eq!(AgentType::Other("x".to_string()).to_string(), "x");
    }

    #[test]
    fn group_chat_actor_master_round_trips_as_tagged_kind_master() {
        let actor = GroupChatActor::Master;

        let value = serde_json::to_value(&actor).expect("master actor should serialize");
        assert_eq!(value, json!({ "kind": "master" }));

        let back: GroupChatActor =
            serde_json::from_value(value).expect("master actor should deserialize");
        assert_eq!(back, GroupChatActor::Master);
    }

    #[test]
    fn group_chat_actor_claw_round_trips_with_camel_case_session_and_agent_type() {
        let actor = GroupChatActor::Claw {
            session_id: "x".to_string(),
            agent_type: "Claw".to_string(),
        };

        let value = serde_json::to_value(&actor).expect("claw actor should serialize");
        assert_eq!(
            value,
            json!({
                "kind": "claw",
                "sessionId": "x",
                "agentType": "Claw"
            })
        );

        let back: GroupChatActor =
            serde_json::from_value(value).expect("claw actor should deserialize");
        assert_eq!(back, actor);
    }

    #[test]
    fn group_chat_actor_all_round_trips_as_tagged_kind_all() {
        let actor = GroupChatActor::All;

        let value = serde_json::to_value(&actor).expect("all actor should serialize");
        assert_eq!(value, json!({ "kind": "all" }));

        let back: GroupChatActor =
            serde_json::from_value(value).expect("all actor should deserialize");
        assert_eq!(back, GroupChatActor::All);
    }

    #[test]
    fn group_chat_messages_cursor_is_usize_domain() {
        let request = GroupChatMessagesRequest {
            room_id: "room_1".to_string(),
            limit: Some(20),
            cursor: Some(5),
        };
        let value = serde_json::to_value(&request).expect("serialize messages request");
        assert_eq!(value["cursor"], json!(5));
        assert_eq!(value["limit"], json!(20));

        let back: GroupChatMessagesRequest =
            serde_json::from_value(value).expect("deserialize messages request");
        assert_eq!(back.cursor, Some(5));

        let response = GroupChatMessagesResponse {
            messages: Vec::new(),
            next_cursor: Some(25),
        };
        let response_value = serde_json::to_value(&response).expect("serialize response");
        assert_eq!(response_value["nextCursor"], json!(25));

        // Full round-trip on the response side too — deserializing the serialized
        // value must land back on the same usize cursor (P2-6 contract).
        let response_back: GroupChatMessagesResponse =
            serde_json::from_value(response_value).expect("deserialize response");
        assert_eq!(response_back.next_cursor, Some(25));
    }
}
