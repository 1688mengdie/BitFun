//! GroupChat manages group chat rooms (create/load/list) and room lifecycle
//! (join/leave/send).
//!
//! Contract: type-contract v1.3 §1.2/§1.3 + dispatch-prompts v1.3
//! R-GC-06 (create/load/list), R-GC-07 (join/leave), R-GC-08 (send).
//!
//! Owner exception (P0-2/P1-4): the owner actor is matched structurally via
//! `matches!(actor, GroupChatActor::Master)` — string comparison against
//! `GROUP_MASTER_ACTOR` is forbidden in this module.

use crate::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use crate::agentic::core::SessionConfig;
use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::service::config::{
    default_group_chat_member_limit, default_group_chat_reply_timeout_secs,
    get_global_config_service,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{
    GroupChatActor, GroupChatCreateRequest, GroupChatDeleteRequest, GroupChatError,
    GroupChatErrorCode, GroupChatIngestReplyRequest, GroupChatJoinRequest, GroupChatLeaveRequest,
    GroupChatMember, GroupChatMemberRole, GroupChatMessage, GroupChatMessageKind,
    GroupChatMessageStatus, GroupChatMessagesRequest, GroupChatMessagesResponse, GroupChatMode,
    GroupChatModeRequest, GroupChatPort, GroupChatRoom, GroupChatSendRequest, GroupChatSendResult,
    SessionStoragePathRequest, SessionStorePort,
};
use bitfun_services_core::session::{
    add_room_to_group_chats, remove_room_from_group_chats, GroupChatStore,
};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool name registered in the product tool pipeline.
pub const GROUP_CHAT_TOOL_NAME: &str = "group_chat";

/// Actions supported by the tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GroupChatAction {
    Create,
    Load,
    List,
    Join,
    Leave,
    Send,
    /// P1-2: timeout scan + reminder (R-GC-26, consumes reply_timeout_secs).
    ScanTimeouts,
    /// R-GC-25: cascade-delete a room (messages + member back-index cleanup).
    Delete,
}

impl GroupChatAction {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "load" => Some(Self::Load),
            "list" => Some(Self::List),
            "join" => Some(Self::Join),
            "leave" => Some(Self::Leave),
            "send" => Some(Self::Send),
            "scan_timeouts" => Some(Self::ScanTimeouts),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// Tool input.
#[derive(Debug, Clone, Deserialize)]
struct GroupChatInput {
    action: String,
    /// create: room name.
    #[serde(default)]
    name: Option<String>,
    /// create: owner actor (Master or Claw).
    #[serde(default)]
    owner: Option<GroupChatActor>,
    /// create: initial member session ids.
    #[serde(default)]
    initial_members: Vec<String>,
    /// create: mode (free | round_robin), default free.
    #[serde(default)]
    mode: Option<GroupChatMode>,
    /// load/join/leave/send: room id.
    #[serde(default)]
    room_id: Option<String>,
    /// join/leave: member session id.
    #[serde(default)]
    session_id: Option<String>,
    /// join/leave/send: acting actor.
    #[serde(default)]
    actor: Option<GroupChatActor>,
    /// send: message content.
    #[serde(default)]
    content: Option<String>,
    /// send: mention targets (@ 目标；空 = 全员)。
    #[serde(default)]
    mention_targets: Vec<GroupChatActor>,
    /// send: urgent flag.
    #[serde(default)]
    urgent: bool,
}

/// GroupChat tool.
#[derive(Debug, Default)]
pub struct GroupChatTool;

impl GroupChatTool {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the group-chats root for a workspace: sibling of the sessions
    /// root resolved through the core session store port.
    async fn group_chats_root(workspace_path: &str) -> BitFunResult<PathBuf> {
        let request = SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: None,
            remote_ssh_host: None,
        };
        let resolution = CoreSessionStorePort::default()
            .resolve_session_storage_path(request)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let sessions_root = resolution.effective_storage_path;
        let parent = sessions_root.parent().ok_or_else(|| {
            BitFunError::tool(format!(
                "sessions root has no parent directory: {}",
                sessions_root.display()
            ))
        })?;
        Ok(parent.join("group-chats"))
    }

    async fn store(workspace_path: &str) -> BitFunResult<GroupChatStore> {
        let root = Self::group_chats_root(workspace_path).await?;
        Ok(GroupChatStore::new(root))
    }

    /// Resolves the effective member limit from `group_chat.member_limit`
    /// (R-GC-26), falling back to the plan default on any read failure.
    async fn resolve_member_limit() -> usize {
        match get_global_config_service().await {
            Ok(service) => match service
                .get_config::<usize>(Some("group_chat.member_limit"))
                .await
            {
                Ok(value) if value > 0 => value,
                _ => default_group_chat_member_limit(),
            },
            Err(_) => default_group_chat_member_limit(),
        }
    }

    /// Resolves the effective reply timeout in seconds from
    /// `group_chat.reply_timeout_secs` (R-GC-26 / P1-2), falling back to the
    /// plan default on any read failure. `0` disables the timeout scan.
    async fn resolve_reply_timeout_secs() -> u64 {
        match get_global_config_service().await {
            Ok(service) => match service
                .get_config::<u64>(Some("group_chat.reply_timeout_secs"))
                .await
            {
                Ok(value) => value,
                Err(_) => default_group_chat_reply_timeout_secs(),
            },
            Err(_) => default_group_chat_reply_timeout_secs(),
        }
    }

    /// Timeout scan (P1-2): scans every room's Pending/Delivered messages
    /// older than `group_chat.reply_timeout_secs`, marks them Failed, and
    /// returns a timeout reminder list for the caller to surface.
    async fn scan_reply_timeouts(&self, workspace_path: &str) -> Result<Vec<Value>, BitFunError> {
        let timeout_secs = Self::resolve_reply_timeout_secs().await;
        if timeout_secs == 0 {
            return Ok(Vec::new());
        }
        let store = Self::store(workspace_path).await?;
        let (rooms, _) = store.list_rooms().await.map_err(store_tool_error)?;
        let now = current_unix_ms();
        let mut reminders = Vec::new();
        for room in &rooms {
            let timed_out = store
                .scan_timed_out_messages(&room.room_id, timeout_secs, now)
                .await
                .map_err(store_tool_error)?;
            for message in timed_out {
                reminders.push(json!({
                    "roomId": room.room_id,
                    "messageId": message.message_id,
                    "content": message.content,
                    "status": "failed",
                    "reason": format!(
                        "no reply within {timeout_secs}s",
                    ),
                }));
            }
        }
        Ok(reminders)
    }

    /// Loads a session's agent type for Claw validation (P1-7).
    async fn session_agent_type(
        coordinator: &ConversationCoordinator,
        session_id: &str,
    ) -> Option<String> {
        coordinator
            .get_session_manager()
            .get_session(session_id)
            .map(|session| session.agent_type)
    }

    /// Validates the owner actor (P2-4): when the owner is a Claw, its
    /// `agent_type` must be "Claw".
    fn validate_owner(owner: &GroupChatActor) -> Result<(), String> {
        match owner {
            GroupChatActor::Master => Ok(()),
            GroupChatActor::Claw { agent_type, .. } => {
                if agent_type == "Claw" {
                    Ok(())
                } else {
                    Err(format!(
                        "group chat owner must be a Claw assistant, got agent_type '{agent_type}'"
                    ))
                }
            }
            GroupChatActor::All => Err("group chat owner cannot be @all".to_string()),
        }
    }

    /// Ensures every member session exists as a Claw assistant session (P1-7).
    ///
    /// 主人定标 2026-08-13（P0 bug v1 返工）：创建群聊不是硬编码对话 ID，
    /// 应该根据 Claw 预设类型新建对话。群聊成员来自 assistant workspace
    /// 枚举，其 sessionId 可能是 `assistantId`（8 位 hex，如 `bd56fce3`）
    /// 或 workspace 稳定 id（`local_+UUID`，如 `local_5a1557...`，当该
    /// assistant workspace 没有 assistantId 时由前端 fallback）。
    ///
    /// 关键：workspace 目录路径必须来自 **workspace service 注册表**
    /// （`get_workspace(id).root_path`），不能拿 id 瞎拼
    /// `assistant_workspace_dir(id)` —— 实测：默认 Claw 助理
    /// （assistantId 为空）的 rootPath = `personal_assistant/workspace`，
    /// 拼出来是 `personal_assistant/workspace/local_5a1557...`（不存在）。
    ///
    /// 幂等：先查内存会话（get_session）→ 再查磁盘会话
    /// （load_session_metadata under the resolved workspace）→ 都不存在才新建。
    /// 新建指定 `session_id`（deterministic），重复调用不会重复建。
    async fn ensure_claw_member_sessions(
        coordinator: &ConversationCoordinator,
        member_ids: &[String],
    ) -> Result<(), String> {
        for session_id in member_ids {
            // 1) In-memory session: fast path.
            if let Some(agent_type) = Self::session_agent_type(coordinator, session_id).await {
                if agent_type == "Claw" {
                    continue;
                }
                return Err(format!(
                    "group chat member '{session_id}' is not a Claw assistant (agent_type '{agent_type}')"
                ));
            }

            // 2) Resolve the REAL assistant workspace path from the workspace
            //    registry (covers 8-hex assistantId and local_+UUID ids).
            let workspace_root = Self::resolve_assistant_workspace(coordinator, session_id).await?;
            let manager = coordinator.get_session_manager();

            // 3) Persisted session under the resolved workspace (restart-safe).
            let persisted = manager
                .load_session_metadata(&workspace_root, session_id)
                .await;
            match persisted {
                Ok(Some(metadata)) => {
                    if metadata.agent_type == "Claw" {
                        continue;
                    }
                    return Err(format!(
                        "group chat member '{session_id}' is not a Claw assistant (agent_type '{}')",
                        metadata.agent_type
                    ));
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(format!(
                        "failed to inspect group chat member session '{session_id}': {error}"
                    ));
                }
            }

            // 4) No session anywhere: create a Claw session bound to the real
            //    assistant workspace (主人定标：按 Claw 预设类型新建对话).
            let workspace_root_str = workspace_root.to_string_lossy().to_string();
            let config = SessionConfig {
                workspace_path: Some(workspace_root_str.clone()),
                project_workspace_path: Some(workspace_root_str.clone()),
                ..Default::default()
            };
            coordinator
                .create_session_with_workspace(
                    Some(session_id.to_string()),
                    format!("Assistant {session_id}"),
                    "Claw".to_string(),
                    config,
                    workspace_root_str,
                )
                .await
                .map_err(|error| {
                    format!(
                        "failed to create Claw session for group chat member '{session_id}': {error}"
                    )
                })?;
        }
        Ok(())
    }

    /// Resolves the real assistant workspace root path for a group chat member.
    ///
    /// The member id may be either an 8-hex `assistantId` (`bd56fce3`) or a
    /// workspace stable id (`local_+UUID`, used when the assistant workspace
    /// has no `assistantId`). Resolution order:
    ///
    /// 1. **Workspace registry by id** — the authoritative source. A registered
    ///    assistant workspace's `root_path` is the true directory (e.g.
    ///    `personal_assistant/workspace` for the default Claw, or
    ///    `personal_assistant/workspace-{assistantId}` for named ones).
    /// 2. **assistant_workspace_dir(id)** — legacy layout fallback for 8-hex
    ///    ids whose workspace is not registered (only used when the directory
    ///    actually exists).
    ///
    /// Returns a clear error when neither resolves to an existing directory
    /// (never silently skips the member).
    async fn resolve_assistant_workspace(
        coordinator: &ConversationCoordinator,
        session_id: &str,
    ) -> Result<std::path::PathBuf, String> {
        // 1) Workspace registry (authoritative): matches workspace.id exactly,
        //    which covers local_+UUID stable ids AND 8-hex ids when they are
        //    the workspace key.
        if let Some(service) = crate::service::workspace::get_global_workspace_service() {
            if let Some(workspace) = service.get_workspace(session_id).await {
                let root = workspace.root_path.clone();
                if root.exists() {
                    return Ok(root);
                }
                return Err(format!(
                    "assistant workspace '{}' (registered id '{}') does not exist on disk",
                    root.display(),
                    session_id
                ));
            }
        }

        // 2) Legacy assistant-workspace layout fallback: `workspace-{id}` dir
        //    under personal_assistant. Only accept it when the directory
        //    actually exists.
        let manager = coordinator.get_session_manager();
        let legacy_dir = manager
            .path_manager()
            .assistant_workspace_dir(session_id, None);
        if legacy_dir.exists() {
            return Ok(legacy_dir);
        }

        Err(format!(
            "assistant workspace not found for group chat member '{session_id}' (no registered workspace and no legacy assistant dir)"
        ))
    }

    /// create: validation chain + room persistence + initial member back-index.
    async fn execute_create(
        &self,
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let name = params.name.as_deref().unwrap_or("").trim();
        let owner = params.owner.clone().unwrap_or(GroupChatActor::Master);
        let mode = params.mode.unwrap_or(GroupChatMode::Free);
        let room = Self::create_room_impl(
            coordinator,
            workspace_path,
            name,
            owner,
            &params.initial_members,
            mode,
        )
        .await?;
        let store = Self::store(workspace_path).await?;
        let members = store
            .list_members(&room.room_id)
            .await
            .map_err(store_tool_error)?;
        Ok(json!({ "room": room, "members": members }))
    }

    /// load: read one room by id.
    async fn execute_load(
        &self,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let room_id = params
            .room_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("room_id is required for load".to_string()))?;
        let store = Self::store(workspace_path).await?;
        let room = store.load_room(room_id).await.map_err(store_tool_error)?;
        Ok(json!({ "room": room }))
    }

    /// list: list all rooms in the workspace.
    async fn execute_list(&self, workspace_path: &str) -> Result<Value, BitFunError> {
        let store = Self::store(workspace_path).await?;
        let (rooms, _) = store.list_rooms().await.map_err(store_tool_error)?;
        Ok(json!({ "rooms": rooms }))
    }

    /// join (R-GC-07): add a member with owner/master validation, Claw check,
    /// RoomFull check, then back-index the member.
    async fn execute_join(
        &self,
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let room_id = params
            .room_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("room_id is required for join".to_string()))?;
        let session_id = params
            .session_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("session_id is required for join".to_string()))?;
        let actor = params.actor.clone().unwrap_or(GroupChatActor::Master);
        let room =
            Self::join_room_impl(coordinator, workspace_path, room_id, session_id, actor).await?;
        Ok(json!({ "room": room }))
    }

    /// leave (R-GC-07): remove a member, clean up back-index, system message.
    async fn execute_leave(
        &self,
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let room_id = params
            .room_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("room_id is required for leave".to_string()))?;
        let session_id = params
            .session_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("session_id is required for leave".to_string()))?;
        let actor = params.actor.clone().unwrap_or(GroupChatActor::Master);
        let room =
            Self::leave_room_impl(coordinator, workspace_path, room_id, session_id, actor).await?;
        Ok(json!({ "room": room }))
    }

    /// delete (R-GC-25): cascade-delete a room — remove the room directory
    /// (messages included), clean every member's back-index (S-38 防幽灵),
    /// rebuild the index. Owner or master only (P1-4 enum match).
    async fn execute_delete(
        &self,
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let room_id = params
            .room_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("room_id is required for delete".to_string()))?;
        let actor = params.actor.clone().unwrap_or(GroupChatActor::Master);
        Self::delete_room_impl(coordinator, workspace_path, room_id, actor).await?;
        Ok(json!({ "deleted": true, "roomId": room_id }))
    }

    /// send (R-GC-08): persist the message, then dispatch to the targeted
    /// members with group correlation metadata (R-GC-11).
    async fn execute_send(
        &self,
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace_path: &str,
        params: &GroupChatInput,
    ) -> Result<Value, BitFunError> {
        let room_id = params
            .room_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("room_id is required for send".to_string()))?;
        let content = params
            .content
            .as_deref()
            .ok_or_else(|| BitFunError::tool("content is required for send".to_string()))?;
        if content.trim().is_empty() {
            return Err(BitFunError::tool("content cannot be empty".to_string()));
        }
        let author = params.actor.clone().unwrap_or(GroupChatActor::Master);
        let (message_id, delivered_to, failed_to) = Self::send_message_impl(
            coordinator,
            workspace_path,
            room_id,
            &author,
            content,
            &params.mention_targets,
            params.urgent,
        )
        .await?;

        Ok(json!({
            "messageId": message_id,
            "deliveredTo": delivered_to,
            "failedTo": failed_to,
        }))
    }

    /// Shared send pipeline (P0-2/P1-4): used by the tool AND the desktop
    /// command layer (thin wrapper) so the UI path dispatches through the same
    /// router chain (resolve_dispatch_plan → dispatch_to_targets) and the
    /// `urgent` flag takes effect. Persists the message first (P0-3).
    pub async fn send_message_impl(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace_path: &str,
        room_id: &str,
        author: &GroupChatActor,
        content: &str,
        mention_targets: &[GroupChatActor],
        urgent: bool,
    ) -> Result<(String, Vec<String>, Vec<serde_json::Value>), BitFunError> {
        if content.trim().is_empty() {
            return Err(BitFunError::tool("content cannot be empty".to_string()));
        }

        let store = Self::store(workspace_path).await?;
        let room = store.load_room(room_id).await.map_err(store_tool_error)?;

        // EmptyMembers: an empty group cannot dispatch to anyone.
        if room.members.is_empty() {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::EmptyMembers,
                format!("group '{room_id}' has no members; cannot send"),
            )));
        }

        // Author check (P0-2): master exception; a Claw author must be a member.
        let author_is_master = matches!(author, GroupChatActor::Master);
        if !author_is_master {
            let is_member = match author {
                GroupChatActor::Claw { session_id, .. } => room
                    .members
                    .iter()
                    .any(|member| &member.session_id == session_id),
                _ => false,
            };
            if !is_member {
                return Err(BitFunError::tool(format!(
                    "author is not a member of group '{room_id}'"
                )));
            }
        }

        // Resolve dispatch targets via the router (R-GC-10): Free broadcast /
        // RoundRobin single-pick (cursor persisted) / @all (P1-4) / targeted.
        let plan = super::group_chat_router::GroupChatRouter::resolve_dispatch_plan(
            &store,
            &room,
            mention_targets,
            urgent,
        )
        .await?;
        let targets = plan.targets;
        if targets.is_empty() {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::EmptyMembers,
                format!("no valid dispatch targets in group '{room_id}'"),
            )));
        }

        // Persist the message first (P0-3: message survives even if dispatch fails).
        let now = current_unix_ms();
        let message_id = format!(
            "msg-{}",
            uuid_v4_deterministic(&format!("{room_id}-send-{content}-{now}"))
        );
        let message = GroupChatMessage {
            message_id: message_id.clone(),
            room_id: room_id.to_string(),
            author: author.clone(),
            kind: match author {
                GroupChatActor::Master => GroupChatMessageKind::User,
                GroupChatActor::Claw { .. } => GroupChatMessageKind::Agent,
                GroupChatActor::All => GroupChatMessageKind::System,
            },
            content: content.to_string(),
            mention_targets: mention_targets.to_vec(),
            reply_to_message_id: None,
            timestamp: now,
            status: GroupChatMessageStatus::Pending,
        };
        store
            .append_message(room_id, &message)
            .await
            .map_err(store_tool_error)?;

        // Dispatch with group correlation metadata (R-GC-11) via the router.
        let group_author = match author {
            GroupChatActor::Master => bitfun_runtime_ports::GROUP_MASTER_ACTOR.to_string(),
            GroupChatActor::Claw { session_id, .. } => session_id.clone(),
            GroupChatActor::All => "__all__".to_string(),
        };
        let (delivered_to, failed_to) =
            super::group_chat_router::GroupChatRouter::dispatch_to_targets(
                coordinator,
                workspace_path,
                room_id,
                &message_id,
                content,
                &group_author,
                plan.urgent,
                &targets,
            )
            .await;

        // Mark delivered when at least one target received it.
        if !delivered_to.is_empty() {
            store
                .update_message_status(room_id, &message_id, GroupChatMessageStatus::Delivered)
                .await
                .map_err(store_tool_error)?;
        } else {
            store
                .update_message_status(room_id, &message_id, GroupChatMessageStatus::Failed)
                .await
                .map_err(store_tool_error)?;
        }

        Ok((message_id, delivered_to, failed_to))
    }
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Converts a store error into a tool error message.
pub(crate) fn store_tool_error(
    error: bitfun_services_core::session::GroupChatStoreError,
) -> BitFunError {
    BitFunError::tool(error.to_string())
}

/// Maps a `GroupChatErrorCode` to a stable machine-readable prefix that the
/// command layer (and the frontend) can branch on. The prefix is embedded in
/// tool/command error strings as `GroupChatErrorCode::<code>: <message>`
/// (P1-5: contract error codes stay observable across tool/command/UI).
pub fn group_chat_error_message(
    code: bitfun_runtime_ports::GroupChatErrorCode,
    message: impl Into<String>,
) -> String {
    format!(
        "GroupChatErrorCode::{}: {}",
        code_name(code),
        message.into()
    )
}

pub(crate) fn code_name(code: bitfun_runtime_ports::GroupChatErrorCode) -> &'static str {
    use bitfun_runtime_ports::GroupChatErrorCode as Code;
    match code {
        Code::NotFound => "NotFound",
        Code::AlreadyMember => "AlreadyMember",
        Code::NotOwner => "NotOwner",
        Code::EmptyMembers => "EmptyMembers",
        Code::RoomFull => "RoomFull",
        Code::DuplicateName => "DuplicateName",
        Code::InvalidTarget => "InvalidTarget",
        Code::NotClaw => "NotClaw",
    }
}

/// Parses the code prefix out of an error string produced by
/// [`group_chat_error_message`]. Returns `None` when the string carries no
/// code (legacy/plain errors fall back to a generic branch on the frontend).
pub fn parse_group_chat_error_code(
    message: &str,
) -> Option<bitfun_runtime_ports::GroupChatErrorCode> {
    use bitfun_runtime_ports::GroupChatErrorCode as Code;
    let prefix = message.strip_prefix("GroupChatErrorCode::")?;
    let code_name = prefix.split(':').next()?;
    Some(match code_name {
        "NotFound" => Code::NotFound,
        "AlreadyMember" => Code::AlreadyMember,
        "NotOwner" => Code::NotOwner,
        "EmptyMembers" => Code::EmptyMembers,
        "RoomFull" => Code::RoomFull,
        "DuplicateName" => Code::DuplicateName,
        "InvalidTarget" => Code::InvalidTarget,
        "NotClaw" => Code::NotClaw,
        _ => return None,
    })
}

/// Deterministic uuid-like id from a name (test-friendly; runtime ids are
/// unique per call because the inputs embed timestamps).
fn uuid_v4_deterministic(seed: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"bitfun-group-chat-v1\0");
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex.chars().take(32).collect()
}

/// Shared command-layer entry points (P0-2/P1-4 thin wrapper): the desktop
/// Tauri commands call these instead of re-implementing validation /
/// back-index / dispatch, so the UI path and the tool path share one pipeline.
impl GroupChatTool {
    /// create (P1-2 Claw check + P1-6 initial back-index).
    pub async fn create_room_impl(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        name: &str,
        owner: GroupChatActor,
        initial_members: &[String],
        mode: GroupChatMode,
    ) -> Result<GroupChatRoom, BitFunError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BitFunError::tool(
                "group chat name is required and cannot be empty".to_string(),
            ));
        }
        Self::validate_owner(&owner).map_err(BitFunError::tool)?;
        Self::ensure_claw_member_sessions(coordinator, initial_members)
            .await
            .map_err(BitFunError::tool)?;

        let member_limit = Self::resolve_member_limit().await;
        if initial_members.len() > member_limit {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::RoomFull,
                format!(
                    "group chat member count {} exceeds the limit {}",
                    initial_members.len(),
                    member_limit
                ),
            )));
        }

        let store = Self::store(workspace_path).await?;
        let (rooms, _) = store.list_rooms().await.map_err(store_tool_error)?;
        if rooms.iter().any(|room| room.name == name) {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::DuplicateName,
                format!("group chat name '{name}' already exists"),
            )));
        }

        let room_id = format!("group-{}", uuid_v4_deterministic(name));
        let now = current_unix_ms();
        let members: Vec<GroupChatMember> = initial_members
            .iter()
            .enumerate()
            .map(|(index, session_id)| GroupChatMember {
                session_id: session_id.clone(),
                role: if index == 0 {
                    GroupChatMemberRole::Owner
                } else {
                    GroupChatMemberRole::Member
                },
                joined_at: now,
                agent_type: "Claw".to_string(),
                display_name: None,
            })
            .collect();

        let room = GroupChatRoom {
            schema_version: 1,
            room_id: room_id.clone(),
            name: name.to_string(),
            owner,
            mode,
            round_robin_cursor: 0,
            created_at: now,
            last_active_at: now,
            status: bitfun_runtime_ports::GroupChatStatus::Active,
            member_limit,
            members: Vec::new(), // members live in members.json (P1-11)
        };

        store.save_room(&room).await.map_err(store_tool_error)?;
        store
            .save_members(&room_id, &members)
            .await
            .map_err(store_tool_error)?;

        // Initial member back-index (P1-6): tag each member with the room id.
        for member in &members {
            GroupChatTool::tag_member_group_chat_static(
                coordinator,
                workspace_path,
                &member.session_id,
                &room_id,
            )
            .await?;
        }

        Ok(room)
    }

    /// join (P1-2 Claw check + P1-3 back-index + P1-5 NotOwner code).
    pub async fn join_room_impl(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        room_id: &str,
        session_id: &str,
        actor: GroupChatActor,
    ) -> Result<GroupChatRoom, BitFunError> {
        let store = Self::store(workspace_path).await?;
        let mut room = store.load_room(room_id).await.map_err(store_tool_error)?;

        if room
            .members
            .iter()
            .any(|member| member.session_id == session_id)
        {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::AlreadyMember,
                format!("session '{session_id}' is already a member of group '{room_id}'"),
            )));
        }

        let is_owner_or_master = match (&room.owner, &actor) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => matches!(actor, GroupChatActor::Master),
        };
        if !is_owner_or_master {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::NotOwner,
                format!("only the owner or the master can add members to group '{room_id}'"),
            )));
        }

        // P1-7 Claw check: a member must be a Claw assistant session; a missing
        // session is auto-created as a Claw session (主人定标 2026-08-13).
        Self::ensure_claw_member_sessions(
            coordinator,
            std::slice::from_ref(&session_id.to_string()),
        )
        .await
        .map_err(|message| {
            BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::NotClaw,
                message,
            ))
        })?;

        let member_limit = Self::resolve_member_limit().await;
        if room.members.len() >= member_limit {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::RoomFull,
                format!("group '{room_id}' is full (limit {member_limit})"),
            )));
        }

        let now = current_unix_ms();
        let mut members = room.members.clone();
        members.push(GroupChatMember {
            session_id: session_id.to_string(),
            role: GroupChatMemberRole::Member,
            joined_at: now,
            agent_type: "Claw".to_string(),
            display_name: None,
        });
        store
            .save_members(room_id, &members)
            .await
            .map_err(store_tool_error)?;
        room.members = members;

        GroupChatTool::tag_member_group_chat_static(
            coordinator,
            workspace_path,
            session_id,
            room_id,
        )
        .await?;

        store
            .append_message(
                room_id,
                &GroupChatMessage {
                    message_id: format!(
                        "msg-{}",
                        uuid_v4_deterministic(&format!("{room_id}-join-{session_id}-{now}"))
                    ),
                    room_id: room_id.to_string(),
                    author: GroupChatActor::Master,
                    kind: GroupChatMessageKind::System,
                    content: format!("member '{session_id}' joined the group"),
                    mention_targets: Vec::new(),
                    reply_to_message_id: None,
                    timestamp: now,
                    status: GroupChatMessageStatus::Delivered,
                },
            )
            .await
            .map_err(store_tool_error)?;

        Ok(room)
    }

    /// leave (P1-3 back-index cleanup + P1-5 NotOwner code).
    pub async fn leave_room_impl(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        room_id: &str,
        session_id: &str,
        actor: GroupChatActor,
    ) -> Result<GroupChatRoom, BitFunError> {
        let store = Self::store(workspace_path).await?;
        let room = store.load_room(room_id).await.map_err(store_tool_error)?;

        let can_leave = matches!(actor, GroupChatActor::Master)
            || match (&room.owner, &actor) {
                (
                    GroupChatActor::Claw {
                        session_id: owner_id,
                        ..
                    },
                    GroupChatActor::Claw {
                        session_id: actor_id,
                        ..
                    },
                ) => owner_id == actor_id || actor_id == session_id,
                _ => match &actor {
                    GroupChatActor::Claw {
                        session_id: claw_session,
                        ..
                    } => claw_session == session_id,
                    _ => false,
                },
            };
        if !can_leave {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::NotOwner,
                format!(
                    "only the owner, the master, or the member itself can leave group '{room_id}'"
                ),
            )));
        }

        let members: Vec<GroupChatMember> = room
            .members
            .iter()
            .filter(|member| member.session_id != session_id)
            .cloned()
            .collect();
        store
            .save_members(room_id, &members)
            .await
            .map_err(store_tool_error)?;

        GroupChatTool::untag_member_group_chat_static(
            coordinator,
            workspace_path,
            session_id,
            room_id,
        )
        .await?;

        let now = current_unix_ms();
        store
            .append_message(
                room_id,
                &GroupChatMessage {
                    message_id: format!(
                        "msg-{}",
                        uuid_v4_deterministic(&format!("{room_id}-leave-{session_id}-{now}"))
                    ),
                    room_id: room_id.to_string(),
                    author: GroupChatActor::Master,
                    kind: GroupChatMessageKind::System,
                    content: format!("member '{session_id}' left the group"),
                    mention_targets: Vec::new(),
                    reply_to_message_id: None,
                    timestamp: now,
                    status: GroupChatMessageStatus::Delivered,
                },
            )
            .await
            .map_err(store_tool_error)?;

        let mut updated_room = room;
        updated_room.members = members;
        Ok(updated_room)
    }

    /// delete (R-GC-25): owner/master gate (P1-1) + full back-index cleanup
    /// with per-member untag tolerance (P1-6) + cascade delete.
    pub async fn delete_room_impl(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        room_id: &str,
        actor: GroupChatActor,
    ) -> Result<(), BitFunError> {
        let store = Self::store(workspace_path).await?;
        let room = store.load_room(room_id).await.map_err(store_tool_error)?;

        let is_owner_or_master = match (&room.owner, &actor) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => matches!(actor, GroupChatActor::Master),
        };
        if !is_owner_or_master {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::NotOwner,
                format!("only the owner or the master can delete group '{room_id}'"),
            )));
        }

        for member in &room.members {
            if let Err(error) = GroupChatTool::untag_member_group_chat_static(
                coordinator,
                workspace_path,
                &member.session_id,
                room_id,
            )
            .await
            {
                warn!(
                    "Group chat delete: failed to untag member '{}' from room '{}' (continuing): {}",
                    member.session_id, room_id, error
                );
            }
        }

        store.delete_room(room_id).await.map_err(store_tool_error)?;
        Ok(())
    }

    /// set_mode (P1-1): owner/master gate + cursor reset.
    pub async fn set_mode_impl(
        workspace_path: &str,
        room_id: &str,
        mode: GroupChatMode,
        actor: GroupChatActor,
    ) -> Result<GroupChatRoom, BitFunError> {
        let store = Self::store(workspace_path).await?;
        let mut room = store.load_room(room_id).await.map_err(store_tool_error)?;

        let is_owner_or_master = match (&room.owner, &actor) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => matches!(actor, GroupChatActor::Master),
        };
        if !is_owner_or_master {
            return Err(BitFunError::tool(group_chat_error_message(
                bitfun_runtime_ports::GroupChatErrorCode::NotOwner,
                format!("only the owner or the master can change the mode of group '{room_id}'"),
            )));
        }

        room.mode = mode;
        room.round_robin_cursor = 0; // 模式切换时 reset cursor (R-GC-10)
        store.save_room(&room).await.map_err(store_tool_error)?;
        Ok(room)
    }

    /// Static back-index helper (used by both tool and command paths).
    async fn tag_member_group_chat_static(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        session_id: &str,
        room_id: &str,
    ) -> BitFunResult<()> {
        let session_manager = coordinator.get_session_manager();
        let workspace_path = std::path::Path::new(workspace_path);
        session_manager
            .update_session_metadata(workspace_path, session_id, |metadata| {
                let custom = metadata.custom_metadata.as_ref();
                let patched = add_room_to_group_chats(custom, room_id);
                metadata.custom_metadata = Some(patched);
            })
            .await
            .map_err(BitFunError::tool)?;
        Ok(())
    }

    /// Static back-index cleanup (used by both tool and command paths).
    async fn untag_member_group_chat_static(
        coordinator: &ConversationCoordinator,
        workspace_path: &str,
        session_id: &str,
        room_id: &str,
    ) -> BitFunResult<()> {
        let session_manager = coordinator.get_session_manager();
        let workspace_path = std::path::Path::new(workspace_path);
        session_manager
            .update_session_metadata(workspace_path, session_id, |metadata| {
                let custom = metadata.custom_metadata.as_ref();
                let patched = remove_room_from_group_chats(custom, room_id);
                metadata.custom_metadata = Some(patched);
            })
            .await
            .map_err(BitFunError::tool)?;
        Ok(())
    }
}

#[async_trait]
impl Tool for GroupChatTool {
    fn name(&self) -> &str {
        GROUP_CHAT_TOOL_NAME
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage group chat rooms that coordinate multiple Claw assistant sessions.

Actions:
- "create": Create a room with a name, owner, initial members, and mode (free | round_robin). Members must be Claw assistant sessions; the owner is the master or a Claw assistant.
- "load": Load one room by room_id (with its member list and message metadata).
- "list": List all rooms in the current workspace.
- "join": Add a Claw assistant session to a room (owner or master only; dedup rejects existing members).
- "leave": Remove a member session from a room (owner, master, or the member itself).
- "send": Broadcast or targeted message dispatch to room members.
- "scan_timeouts": Scan all rooms for messages awaiting replies longer than `group_chat.reply_timeout_secs`; timed-out messages are marked failed and returned as timeout reminders (P1-2).
- "delete": Cascade-delete a room (messages + member back-index cleanup, owner or master only).

Arguments:
- "action": The action to perform.
- "name": Room name for "create" (required, non-empty, unique).
- "owner": Owner actor for "create": {"kind":"master"} or {"kind":"claw","sessionId":"...","agentType":"Claw"}.
- "initial_members": Member session ids for "create".
- "mode": "free" or "round_robin" (default "free").
- "room_id": Target room for load/join/leave/send.
- "session_id": Member session id for join/leave.
- "actor": Acting actor for join/leave/send (defaults to the master).
- "content": Message content for "send".
- "mention_targets": @ targets for "send"; empty = broadcast to all members.
- "urgent": When true, deliver as an urgent interruption to the target."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Manage group chat rooms coordinating multiple Claw assistant sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "load", "list", "join", "leave", "send", "scan_timeouts", "delete"],
                    "description": "The group chat action to perform."
                },
                "name": { "type": "string", "description": "Room name for create." },
                "owner": {
                    "type": "object",
                    "description": "Owner actor for create: {kind:'master'} or {kind:'claw',sessionId,agentType}.",
                    "properties": {
                        "kind": { "type": "string", "enum": ["master", "claw", "all"] }
                    },
                    "required": ["kind"]
                },
                "initial_members": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Initial member session ids for create."
                },
                "mode": {
                    "type": "string",
                    "enum": ["free", "round_robin"],
                    "description": "Communication mode (default free)."
                },
                "room_id": { "type": "string", "description": "Target room id." },
                "session_id": { "type": "string", "description": "Member session id for join/leave." },
                "actor": {
                    "type": "object",
                    "description": "Acting actor: {kind:'master'} or {kind:'claw',sessionId,agentType}.",
                    "properties": {
                        "kind": { "type": "string", "enum": ["master", "claw", "all"] }
                    },
                    "required": ["kind"]
                },
                "content": { "type": "string", "description": "Message content for send." },
                "mention_targets": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "@ targets for send; empty = broadcast."
                },
                "urgent": { "type": "boolean", "description": "Urgent delivery flag." }
            },
            "required": ["action"]
        })
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let parsed: GroupChatInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {error}")))?;
        let action = GroupChatAction::from_str(&parsed.action).ok_or_else(|| {
            BitFunError::tool(format!("unknown group_chat action '{}'", parsed.action))
        })?;
        let workspace_path = context
            .workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string())
            .ok_or_else(|| BitFunError::tool("workspace is required for group_chat".to_string()))?;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        let result = match action {
            GroupChatAction::Create => {
                self.execute_create(&coordinator, &workspace_path, &parsed)
                    .await?
            }
            GroupChatAction::Load => self.execute_load(&workspace_path, &parsed).await?,
            GroupChatAction::List => self.execute_list(&workspace_path).await?,
            GroupChatAction::Join => {
                self.execute_join(&coordinator, &workspace_path, &parsed)
                    .await?
            }
            GroupChatAction::Leave => {
                self.execute_leave(&coordinator, &workspace_path, &parsed)
                    .await?
            }
            GroupChatAction::Send => {
                self.execute_send(&coordinator, &workspace_path, &parsed)
                    .await?
            }
            GroupChatAction::ScanTimeouts => {
                let reminders = self.scan_reply_timeouts(&workspace_path).await?;
                json!({ "timeoutReminders": reminders })
            }
            GroupChatAction::Delete => {
                self.execute_delete(&coordinator, &workspace_path, &parsed)
                    .await?
            }
        };
        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some("group_chat operation completed".to_string()),
            image_attachments: None,
        }])
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> bitfun_agent_tools::ValidationResult {
        let parsed: Result<GroupChatInput, _> = serde_json::from_value(input.clone());
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                return bitfun_agent_tools::ValidationResult {
                    result: false,
                    message: Some(error.to_string()),
                    error_code: None,
                    meta: None,
                };
            }
        };
        if GroupChatAction::from_str(&parsed.action).is_none() {
            return bitfun_agent_tools::ValidationResult {
                result: false,
                message: Some(format!("unknown action '{}'", parsed.action)),
                error_code: None,
                meta: None,
            };
        }
        bitfun_agent_tools::ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }
}

/// Contract adapter (F-1): implements [`GroupChatPort`] on top of the shared
/// group-chat pipeline (`GroupChatTool` + `GroupChatStore`).
///
/// The trait requests (`GroupChatCreateRequest` etc.) do not carry a workspace
/// path, so the adapter owns the `workspace_path` it was constructed with.
/// Methods route through the exact same validation + persistence + dispatch
/// chain as the Tauri command layer (`session_api.rs`) and the agent tool.
#[derive(Debug, Clone)]
pub struct GroupChatPortImpl {
    workspace_path: String,
    #[cfg(test)]
    test_store: Option<GroupChatStore>,
}

impl GroupChatPortImpl {
    pub fn new(workspace_path: impl Into<String>) -> Self {
        Self {
            workspace_path: workspace_path.into(),
            #[cfg(test)]
            test_store: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_store(workspace_path: impl Into<String>, store: GroupChatStore) -> Self {
        Self {
            workspace_path: workspace_path.into(),
            test_store: Some(store),
        }
    }

    fn error(message: impl Into<String>) -> GroupChatError {
        let text = message.into();
        GroupChatError {
            code: parse_group_chat_error_code(&text).unwrap_or(GroupChatErrorCode::NotFound),
            message: text,
        }
    }

    /// Resolves the store for `self.workspace_path` through the shared
    /// group-chat storage chain (CoreSessionStorePort → sessions root → group-chats).
    /// Tests may inject a store backed by a temp dir via [`Self::with_store`].
    async fn store(&self) -> Result<GroupChatStore, GroupChatError> {
        #[cfg(test)]
        if let Some(store) = &self.test_store {
            return Ok(store.clone());
        }
        GroupChatTool::store(&self.workspace_path)
            .await
            .map_err(|error| Self::error(error.to_string()))
    }
}

#[async_trait::async_trait]
impl GroupChatPort for GroupChatPortImpl {
    async fn create_room(
        &self,
        req: GroupChatCreateRequest,
    ) -> Result<GroupChatRoom, GroupChatError> {
        let coordinator =
            get_global_coordinator().ok_or_else(|| Self::error("coordinator not initialized"))?;
        GroupChatTool::create_room_impl(
            &coordinator,
            &self.workspace_path,
            &req.name,
            req.owner,
            &req.initial_members,
            req.mode,
        )
        .await
        .map_err(|error| Self::error(error.to_string()))
    }

    async fn list_rooms(&self, workspace_path: &str) -> Result<Vec<GroupChatRoom>, GroupChatError> {
        // Prefer the injected test store; otherwise resolve through the given
        // workspace path (contract semantics: the caller may pass a different
        // workspace than the one this adapter was constructed with).
        #[cfg(test)]
        if let Some(store) = &self.test_store {
            return store
                .list_rooms()
                .await
                .map(|(rooms, _)| rooms)
                .map_err(|error| Self::error(error.to_string()));
        }
        let store = GroupChatTool::store(workspace_path)
            .await
            .map_err(|error| Self::error(error.to_string()))?;
        store
            .list_rooms()
            .await
            .map(|(rooms, _)| rooms)
            .map_err(|error| Self::error(error.to_string()))
    }

    async fn load_room(&self, room_id: &str) -> Result<GroupChatRoom, GroupChatError> {
        let store = self.store().await?;
        store
            .load_room(room_id)
            .await
            .map_err(|error| Self::error(error.to_string()))
    }

    async fn list_members(&self, room_id: &str) -> Result<Vec<GroupChatMember>, GroupChatError> {
        let store = self.store().await?;
        store
            .list_members(room_id)
            .await
            .map_err(|error| Self::error(error.to_string()))
    }

    async fn join_room(&self, req: GroupChatJoinRequest) -> Result<GroupChatRoom, GroupChatError> {
        let coordinator =
            get_global_coordinator().ok_or_else(|| Self::error("coordinator not initialized"))?;
        GroupChatTool::join_room_impl(
            &coordinator,
            &self.workspace_path,
            &req.room_id,
            &req.session_id,
            req.actor,
        )
        .await
        .map_err(|error| Self::error(error.to_string()))
    }

    async fn leave_room(
        &self,
        req: GroupChatLeaveRequest,
    ) -> Result<GroupChatRoom, GroupChatError> {
        let coordinator =
            get_global_coordinator().ok_or_else(|| Self::error("coordinator not initialized"))?;
        GroupChatTool::leave_room_impl(
            &coordinator,
            &self.workspace_path,
            &req.room_id,
            &req.session_id,
            req.actor,
        )
        .await
        .map_err(|error| Self::error(error.to_string()))
    }

    async fn delete_room(&self, req: GroupChatDeleteRequest) -> Result<(), GroupChatError> {
        let coordinator =
            get_global_coordinator().ok_or_else(|| Self::error("coordinator not initialized"))?;
        GroupChatTool::delete_room_impl(&coordinator, &self.workspace_path, &req.room_id, req.actor)
            .await
            .map_err(|error| Self::error(error.to_string()))
    }

    async fn set_mode(&self, req: GroupChatModeRequest) -> Result<GroupChatRoom, GroupChatError> {
        GroupChatTool::set_mode_impl(&self.workspace_path, &req.room_id, req.mode, req.actor)
            .await
            .map_err(|error| Self::error(error.to_string()))
    }

    async fn send_message(
        &self,
        req: GroupChatSendRequest,
    ) -> Result<GroupChatSendResult, GroupChatError> {
        let coordinator =
            get_global_coordinator().ok_or_else(|| Self::error("coordinator not initialized"))?;
        let (message_id, delivered_to, failed_to) = GroupChatTool::send_message_impl(
            &coordinator,
            &self.workspace_path,
            &req.room_id,
            &req.author,
            &req.content,
            &req.mention_targets,
            req.urgent,
        )
        .await
        .map_err(|error| Self::error(error.to_string()))?;
        let failed_to = failed_to
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect();
        Ok(GroupChatSendResult {
            message_id,
            delivered_to,
            failed_to,
        })
    }

    async fn list_messages(
        &self,
        req: GroupChatMessagesRequest,
    ) -> Result<GroupChatMessagesResponse, GroupChatError> {
        let store = self.store().await?;
        // P2-6: cursor shares the usize index domain with the store — no parse
        // round-trip through a string bridge.
        let window = store
            .list_messages(&req.room_id, req.limit, req.cursor)
            .await
            .map_err(|error| Self::error(error.to_string()))?;
        Ok(GroupChatMessagesResponse {
            messages: window.messages,
            next_cursor: window.next_cursor,
        })
    }

    async fn ingest_reply(&self, req: GroupChatIngestReplyRequest) -> Result<(), GroupChatError> {
        let store = self.store().await?;
        // F-2 convergence: delegate to the authority router core so the port
        // adapter, the Tauri command layer, and the reply router share one
        // behavior (P2-2 no-op on deleted message + deterministic reply id).
        super::group_chat_router::ingest_reply_core(
            &store,
            &req.room_id,
            &req.message_id,
            &req.reply_content,
            &req.author,
            req.timestamp,
        )
        .await
        .map_err(|error| Self::error(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::session::SessionManager;
    use std::sync::Arc;

    #[test]
    fn group_chat_action_parses_known_actions() {
        assert!(matches!(
            GroupChatAction::from_str("create"),
            Some(GroupChatAction::Create)
        ));
        assert!(matches!(
            GroupChatAction::from_str("load"),
            Some(GroupChatAction::Load)
        ));
        assert!(matches!(
            GroupChatAction::from_str("list"),
            Some(GroupChatAction::List)
        ));
        assert!(matches!(
            GroupChatAction::from_str("join"),
            Some(GroupChatAction::Join)
        ));
        assert!(matches!(
            GroupChatAction::from_str("leave"),
            Some(GroupChatAction::Leave)
        ));
        assert!(matches!(
            GroupChatAction::from_str("send"),
            Some(GroupChatAction::Send)
        ));
        assert!(matches!(
            GroupChatAction::from_str("scan_timeouts"),
            Some(GroupChatAction::ScanTimeouts)
        ));
        assert!(matches!(
            GroupChatAction::from_str("delete"),
            Some(GroupChatAction::Delete)
        ));
        assert!(GroupChatAction::from_str("bogus").is_none());
    }

    #[test]
    fn group_chat_owner_validation_rejects_non_claw_and_all() {
        assert!(GroupChatTool::validate_owner(&GroupChatActor::Master).is_ok());
        assert!(GroupChatTool::validate_owner(&GroupChatActor::Claw {
            session_id: "c-1".to_string(),
            agent_type: "Claw".to_string(),
        })
        .is_ok());
        assert!(GroupChatTool::validate_owner(&GroupChatActor::Claw {
            session_id: "c-2".to_string(),
            agent_type: "agentic".to_string(),
        })
        .is_err());
        assert!(GroupChatTool::validate_owner(&GroupChatActor::All).is_err());
    }

    #[test]
    fn group_chat_room_id_is_deterministic_and_unique() {
        let id_a = uuid_v4_deterministic("room-alpha");
        let id_b = uuid_v4_deterministic("room-alpha");
        let id_c = uuid_v4_deterministic("room-beta");
        assert_eq!(id_a, id_b);
        assert_ne!(id_a, id_c);
        assert_eq!(id_a.len(), 32);
    }

    #[test]
    fn group_chat_owner_exception_uses_enum_match_not_strings() {
        // P0-2/P1-4: master exception is expressed as matches!(actor, Master).
        let master = GroupChatActor::Master;
        let is_master = matches!(master, GroupChatActor::Master);
        assert!(is_master);

        let claw = GroupChatActor::Claw {
            session_id: "c-1".to_string(),
            agent_type: "Claw".to_string(),
        };
        assert!(!matches!(claw, GroupChatActor::Master));
    }

    #[test]
    fn group_chat_delete_permission_uses_enum_match_for_master_exception() {
        // R-GC-25: the delete permission gate mirrors the join gate —
        // owner (Claw owner session match) or master exception via enum match.
        // Non-owner Claw must NOT delete (NotOwner).
        let room = bitfun_runtime_ports::GroupChatRoom {
            schema_version: 1,
            room_id: "room-1".to_string(),
            name: "Room".to_string(),
            owner: GroupChatActor::Claw {
                session_id: "owner-1".to_string(),
                agent_type: "Claw".to_string(),
            },
            mode: bitfun_runtime_ports::GroupChatMode::Free,
            round_robin_cursor: 0,
            created_at: 1,
            last_active_at: 1,
            status: bitfun_runtime_ports::GroupChatStatus::Active,
            member_limit: 50,
            members: Vec::new(),
        };

        // Master exception (P0-2/P1-4): master may delete any room.
        let master_ok = match (&room.owner, &GroupChatActor::Master) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => matches!(GroupChatActor::Master, GroupChatActor::Master),
        };
        assert!(master_ok);

        // Owner Claw: same session id → allowed.
        let owner_actor = GroupChatActor::Claw {
            session_id: "owner-1".to_string(),
            agent_type: "Claw".to_string(),
        };
        let owner_ok = match (&room.owner, &owner_actor) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => false,
        };
        assert!(owner_ok);

        // Non-owner Claw → denied (NotOwner).
        let stranger = GroupChatActor::Claw {
            session_id: "stranger-1".to_string(),
            agent_type: "Claw".to_string(),
        };
        let stranger_ok = match (&room.owner, &stranger) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id,
            _ => false,
        };
        assert!(!stranger_ok);
    }

    #[test]
    fn group_chat_error_code_round_trips_through_prefix() {
        // P1-5: the command layer parses the code prefix back out of a tool
        // error so the frontend can branch on the contract error code.
        use bitfun_runtime_ports::GroupChatErrorCode as Code;
        for code in [
            Code::NotFound,
            Code::AlreadyMember,
            Code::NotOwner,
            Code::EmptyMembers,
            Code::RoomFull,
            Code::DuplicateName,
            Code::InvalidTarget,
            Code::NotClaw,
        ] {
            let message = group_chat_error_message(code, "boom");
            assert_eq!(parse_group_chat_error_code(&message), Some(code));
        }
        assert_eq!(parse_group_chat_error_code("plain error"), None);
    }

    #[test]
    fn group_chat_error_code_helpers_cover_all_variants() {
        // P1-5: code_name and parse must stay in lockstep with the contract
        // enum (a new variant without a name would fail here at compile time
        // via the exhaustive match, and at runtime via the round trip above).
        use bitfun_runtime_ports::GroupChatErrorCode as Code;
        assert_eq!(code_name(Code::NotOwner), "NotOwner");
        assert_eq!(code_name(Code::NotClaw), "NotClaw");
        assert_eq!(code_name(Code::EmptyMembers), "EmptyMembers");
        assert_eq!(code_name(Code::DuplicateName), "DuplicateName");
        assert_eq!(code_name(Code::RoomFull), "RoomFull");
        assert_eq!(code_name(Code::InvalidTarget), "InvalidTarget");
        assert_eq!(code_name(Code::AlreadyMember), "AlreadyMember");
        assert_eq!(code_name(Code::NotFound), "NotFound");
    }

    // ── F-1: GroupChatPort contract tests ──────────────────────────────
    // Storage-backed methods run the real store chain (temp dir); coordinator
    // methods assert the explicit error when the coordinator is not up.

    fn sample_room(room_id: &str, name: &str) -> GroupChatRoom {
        GroupChatRoom {
            schema_version: 1,
            room_id: room_id.to_string(),
            name: name.to_string(),
            owner: GroupChatActor::Master,
            mode: GroupChatMode::Free,
            round_robin_cursor: 0,
            created_at: 1,
            last_active_at: 1,
            status: bitfun_runtime_ports::GroupChatStatus::Active,
            member_limit: 50,
            members: Vec::new(),
        }
    }

    fn sample_member(session_id: &str, role: GroupChatMemberRole) -> GroupChatMember {
        GroupChatMember {
            session_id: session_id.to_string(),
            role,
            joined_at: 1,
            agent_type: "Claw".to_string(),
            display_name: Some(format!("Assistant {session_id}")),
        }
    }

    fn temp_store() -> (tempfile::TempDir, GroupChatStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = GroupChatStore::new(dir.path().join("group-chats"));
        (dir, store)
    }

    #[tokio::test]
    async fn group_chat_port_list_rooms_returns_seeded_rooms() {
        let (_dir, store) = temp_store();
        store
            .save_room(&sample_room("room-1", "Alpha"))
            .await
            .expect("save room");
        store
            .save_room(&sample_room("room-2", "Beta"))
            .await
            .expect("save room");
        let port = GroupChatPortImpl::with_store("/ws", store);

        let rooms = port.list_rooms("/ws").await.expect("list rooms");
        assert_eq!(rooms.len(), 2);
        assert!(rooms.iter().any(|r| r.name == "Alpha"));
    }

    #[tokio::test]
    async fn group_chat_port_load_room_reads_seeded_room_with_members() {
        let (_dir, store) = temp_store();
        store
            .save_room(&sample_room("room-1", "Alpha"))
            .await
            .expect("save room");
        store
            .save_members(
                "room-1",
                &[sample_member("m-1", GroupChatMemberRole::Member)],
            )
            .await
            .expect("save members");
        let port = GroupChatPortImpl::with_store("/ws", store);

        let room = port.load_room("room-1").await.expect("load room");
        assert_eq!(room.name, "Alpha");
        assert_eq!(room.members.len(), 1);
        assert_eq!(room.members[0].session_id, "m-1");
    }

    #[tokio::test]
    async fn group_chat_port_list_members_reads_seeded_members() {
        let (_dir, store) = temp_store();
        store
            .save_room(&sample_room("room-1", "Alpha"))
            .await
            .expect("save room");
        store
            .save_members(
                "room-1",
                &[
                    sample_member("m-1", GroupChatMemberRole::Owner),
                    sample_member("m-2", GroupChatMemberRole::Member),
                ],
            )
            .await
            .expect("save members");
        let port = GroupChatPortImpl::with_store("/ws", store);

        let members = port.list_members("room-1").await.expect("list members");
        assert_eq!(members.len(), 2);
        assert!(members.iter().any(|m| m.role == GroupChatMemberRole::Owner));
    }

    #[tokio::test]
    async fn group_chat_port_list_messages_returns_window_with_cursor() {
        let (_dir, store) = temp_store();
        store
            .save_room(&sample_room("room-1", "Alpha"))
            .await
            .expect("save room");
        let msg = GroupChatMessage {
            message_id: "msg-1".to_string(),
            room_id: "room-1".to_string(),
            author: GroupChatActor::Master,
            kind: GroupChatMessageKind::User,
            content: "hello".to_string(),
            mention_targets: Vec::new(),
            reply_to_message_id: None,
            timestamp: 1,
            status: GroupChatMessageStatus::Delivered,
        };
        store
            .append_message("room-1", &msg)
            .await
            .expect("append message");
        let port = GroupChatPortImpl::with_store("/ws", store);

        let res = port
            .list_messages(GroupChatMessagesRequest {
                room_id: "room-1".to_string(),
                limit: Some(50),
                cursor: None,
            })
            .await
            .expect("list messages");
        assert_eq!(res.messages.len(), 1);
        assert_eq!(res.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn group_chat_port_ingest_reply_marks_replied_and_appends_body() {
        let (_dir, store) = temp_store();
        store
            .save_room(&sample_room("room-1", "Alpha"))
            .await
            .expect("save room");
        let msg = GroupChatMessage {
            message_id: "msg-1".to_string(),
            room_id: "room-1".to_string(),
            author: GroupChatActor::Master,
            kind: GroupChatMessageKind::User,
            content: "question".to_string(),
            mention_targets: Vec::new(),
            reply_to_message_id: None,
            timestamp: 1,
            status: GroupChatMessageStatus::Delivered,
        };
        store
            .append_message("room-1", &msg)
            .await
            .expect("append message");
        let port = GroupChatPortImpl::with_store("/ws", store);

        port.ingest_reply(GroupChatIngestReplyRequest {
            room_id: "room-1".to_string(),
            message_id: "msg-1".to_string(),
            reply_content: "answer".to_string(),
            author: GroupChatActor::Claw {
                session_id: "m-1".to_string(),
                agent_type: "Claw".to_string(),
            },
            timestamp: 2,
        })
        .await
        .expect("ingest reply");

        let room = port.load_room("room-1").await.expect("load room");
        let res = port
            .list_messages(GroupChatMessagesRequest {
                room_id: "room-1".to_string(),
                limit: Some(50),
                cursor: None,
            })
            .await
            .expect("list messages");
        // Original message now Replied; reply body appended.
        assert_eq!(res.messages.len(), 2);
        assert_eq!(res.messages[0].status, GroupChatMessageStatus::Replied);
        assert!(res.messages.iter().any(|m| m.content == "answer"));
        assert!(room.members.is_empty());
    }

    #[tokio::test]
    async fn group_chat_port_create_room_without_coordinator_returns_clear_error() {
        // Coordinator-dependent methods must fail with a clear error when the
        // global coordinator is not initialized (contract boundary).
        let port = GroupChatPortImpl::new("/ws");
        let err = port
            .create_room(GroupChatCreateRequest {
                name: "Room".to_string(),
                owner: GroupChatActor::Master,
                initial_members: Vec::new(),
                mode: GroupChatMode::Free,
            })
            .await
            .expect_err("create must fail without coordinator");
        assert!(!err.message.is_empty());
    }

    #[tokio::test]
    async fn group_chat_port_join_leave_delete_set_mode_send_require_coordinator() {
        let port = GroupChatPortImpl::new("/ws");

        let join = port
            .join_room(GroupChatJoinRequest {
                room_id: "room-1".to_string(),
                session_id: "m-1".to_string(),
                actor: GroupChatActor::Master,
            })
            .await
            .expect_err("join must fail without coordinator");
        assert!(!join.message.is_empty());

        let leave = port
            .leave_room(GroupChatLeaveRequest {
                room_id: "room-1".to_string(),
                session_id: "m-1".to_string(),
                actor: GroupChatActor::Master,
            })
            .await
            .expect_err("leave must fail without coordinator");
        assert!(!leave.message.is_empty());

        let delete = port
            .delete_room(GroupChatDeleteRequest {
                room_id: "room-1".to_string(),
                actor: GroupChatActor::Master,
            })
            .await
            .expect_err("delete must fail without coordinator");
        assert!(!delete.message.is_empty());

        let send = port
            .send_message(GroupChatSendRequest {
                room_id: "room-1".to_string(),
                author: GroupChatActor::Master,
                content: "hi".to_string(),
                mention_targets: Vec::new(),
                urgent: false,
            })
            .await
            .expect_err("send must fail without coordinator");
        assert!(!send.message.is_empty());

        // set_mode does not need the coordinator (store-only path): with a
        // fresh store the room does not exist → NotFound, which is still a
        // clear contract error proving the call reached the store chain.
        let mode = port
            .set_mode(GroupChatModeRequest {
                room_id: "missing-room".to_string(),
                mode: GroupChatMode::Free,
                actor: GroupChatActor::Master,
            })
            .await
            .expect_err("set_mode on missing room must fail");
        assert!(!mode.message.is_empty());
    }

    // ── P0 fix (2026-08-13): group chat members are assistant workspaces;
    // a missing session is auto-created as a Claw session instead of failing
    // with "does not exist" (主人定标：按 Claw 预设类型新建对话). ──────────

    /// Minimal real-coordinator harness (mirrors session_message_tool tests).
    fn test_group_chat_coordinator_harness() -> (
        Arc<ConversationCoordinator>,
        Arc<SessionManager>,
        tempfile::TempDir,
    ) {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::{
            compression::{CompressionConfig, ContextCompressor},
            SessionContextStore, SessionManagerConfig,
        };
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::agentic::tools::{ToolPipeline, ToolStateManager};
        use crate::infrastructure::PathManager;
        use std::time::Duration;
        use tokio::sync::RwLock as TokioRwLock;
        use uuid::Uuid;

        let root = tempfile::tempdir().expect("test root");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    root.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: crate::agentic::session::PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue.clone(),
            Arc::new(EventRouter::new()),
            Arc::new(
                crate::runtime_ownership::CoreRuntimeOwnership::embedded_with_facts(
                    root.path().join(format!("ownership-{}", Uuid::new_v4())),
                    "bitfun".to_string(),
                    "test",
                ),
            ),
        ));
        (coordinator, session_manager, root)
    }

    #[tokio::test]
    async fn group_chat_missing_member_session_is_auto_created_as_claw() {
        // P0 fix: a member id that has no session yet (an assistant workspace
        // id) must be auto-created as a Claw session, not rejected with
        // "does not exist". Uses the 8-hex assistantId shape; the assistant
        // workspace dir must exist (the dir is the source of truth for
        // "this assistant is addable").
        let (coordinator, session_manager, root) = test_group_chat_coordinator_harness();
        let assistant_id = "bd56fce3";
        let assistant_dir = coordinator
            .get_session_manager()
            .path_manager()
            .assistant_workspace_dir(assistant_id, None);
        std::fs::create_dir_all(&assistant_dir).expect("create assistant workspace dir");

        // Before the fix this returned Err("group chat member session 'bd56fce3' does not exist").
        GroupChatTool::ensure_claw_member_sessions(&coordinator, &[assistant_id.to_string()])
            .await
            .expect("missing member session must be auto-created as Claw");

        let session = session_manager
            .get_session(assistant_id)
            .expect("auto-created session must exist");
        assert_eq!(session.agent_type, "Claw");
        assert_eq!(
            session.config.workspace_path.as_deref(),
            Some(assistant_dir.to_string_lossy().as_ref())
        );
        let _ = root;
    }

    #[tokio::test]
    async fn group_chat_local_uuid_member_resolves_via_legacy_assistant_dir() {
        // P0 v2 regression: the real-world default Claw assistant has NO
        // assistantId, so the frontend falls back to the workspace stable id
        // `local_<uuid>` (e.g. local_5a1557a8afd417b173d9ce873553e66a). The
        // v1 fix wrongly called `assistant_workspace_dir(local_...)` which
        // produced `personal_assistant/workspace/local_...` (missing) instead
        // of the real workspace root. Resolution must fail cleanly when the
        // id resolves to nothing, and auto-create the Claw session bound to
        // the real dir once it exists.
        let (coordinator, session_manager, root) = test_group_chat_coordinator_harness();
        let member_id = "local_5a1557a8afd417b173d9ce873553e66a";
        let resolved = GroupChatTool::resolve_assistant_workspace(&coordinator, member_id)
            .await
            .expect_err("unregistered local_ id without a legacy dir must fail cleanly");
        assert!(
            resolved.contains(member_id) && resolved.contains("not found"),
            "unexpected error: {resolved}"
        );

        // Now create the legacy assistant dir matching the id and retry.
        let legacy_dir = coordinator
            .get_session_manager()
            .path_manager()
            .assistant_workspace_dir(member_id, None);
        std::fs::create_dir_all(&legacy_dir).expect("create legacy assistant dir");

        GroupChatTool::ensure_claw_member_sessions(&coordinator, &[member_id.to_string()])
            .await
            .expect("member with existing legacy assistant dir must be auto-created as Claw");

        let session = session_manager
            .get_session(member_id)
            .expect("auto-created session must exist");
        assert_eq!(session.agent_type, "Claw");
        assert_eq!(
            session.config.workspace_path.as_deref(),
            Some(legacy_dir.to_string_lossy().as_ref())
        );
        let _ = root;
    }

    #[tokio::test]
    async fn group_chat_missing_member_without_assistant_workspace_returns_clear_error() {
        // The assistant workspace dir is the source of truth: a member id
        // without a session AND without a workspace dir must fail with a clear
        // error (never a silent skip).
        let (coordinator, _session_manager, root) = test_group_chat_coordinator_harness();
        let missing_id = "no-such-assistant";
        let err =
            GroupChatTool::ensure_claw_member_sessions(&coordinator, &[missing_id.to_string()])
                .await
                .expect_err("member without assistant workspace must fail");
        assert!(
            err.contains("not found") && err.contains(missing_id),
            "unexpected error: {err}"
        );
        let _ = root;
    }

    #[tokio::test]
    async fn group_chat_existing_non_claw_session_is_rejected() {
        // P1-7 contract is preserved: an existing session with a non-Claw
        // agent type must still be rejected (NotClaw), not overwritten.
        let (coordinator, session_manager, root) = test_group_chat_coordinator_harness();
        let session_id = "agentic-member";
        let assistant_dir = coordinator
            .get_session_manager()
            .path_manager()
            .assistant_workspace_dir(session_id, None);
        std::fs::create_dir_all(&assistant_dir).expect("create assistant workspace dir");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Agentic".to_string(),
                "agentic".to_string(),
                crate::agentic::core::SessionConfig {
                    workspace_path: Some(assistant_dir.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create agentic session");

        let err =
            GroupChatTool::ensure_claw_member_sessions(&coordinator, &[session_id.to_string()])
                .await
                .expect_err("non-Claw member must be rejected");
        assert!(
            err.contains("is not a Claw assistant"),
            "unexpected error: {err}"
        );
        let _ = root;
    }
}
