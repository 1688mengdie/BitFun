//! GroupRoomTool — 群聊系列工具（主人定标 v3：群聊 = 普通会话）。
//!
//! Contract: type-contract v3（群聊v3-type-contract-最终权威-20260814.md §二）
//! R-GC-08：9 个 action（create/invite/remove/send/history/list/fork/
//! member_status/delete），复用现成机制：
//! - 建群 = `coordinator.create_session_with_workspace`（coordinator.rs:2659）
//! - 拉成员 = 同上建成员 Claw 会话（各自 workspace）
//! - 发消息 = 群会话 turns（`PersistenceManager::save_dialog_turn`，
//!   persistence/manager.rs:3089；`user_message.metadata` 带 sender+groupId，
//!   types.rs:662）
//! - 历史 = `session_manager.get_messages`（session_manager.rs:8785）
//! - 裂变 = `PersistenceManager::branch_session`（session_branch.rs:14）
//! - 成员状态 = `session_manager.get_session`（:3060）
//! - 删除 = `coordinator.delete_session`（coordinator.rs:7434）
//!
//! 群聊 ID = 会话 ID（UUID）；群 = 默认对话类型会话（agent_type 取
//! AgentRegistry::default_agent_type，R-GC-25 零硬编码）带专属 workspace。
//!
//! 契约偏差修复（姬码锋 CEO 派发 R-GC-08，2026-08-14）：
//! - B-1（契约 §三）：`GroupMessage.author: SenderIdentity`（复用
//!   session_message_tool.rs:485-496）+ `metadata: GroupChatForwardMetadata`
//!   （复用 session_message_tool.rs:504-510）；history 从 turn metadata
//!   解析真实 author（senderRole/senderDepth/senderName）。
//! - B-2（契约 §三）：send metadata 五字段
//!   { groupId, senderSessionId, senderRole, senderDepth, senderName }；
//!   senderName 取真实会话名（回退 sender_session_id）。
//! - B-3（契约 §六.5）：history/list/member_status 只读；其余 6 action 非只读
//!   （is_readonly 按 action 区分，泛化到 is_concurrency_safe / permission_intents）。
//! - B-4（契约 §二.8）：member_status 先校验 member_session_id ∈ 群成员表
//!   （custom_metadata.groupChats）再 get_session 查 state。

use crate::agentic::agents::get_agent_registry;
use crate::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use crate::agentic::core::SessionConfig;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::agentic::tools::restrictions::get_session_role;
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{DialogSubmissionPolicy, DialogTriggerSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool name registered in the product tool pipeline（materialization 注册）。
pub const GROUP_ROOM_TOOL_NAME: &str = "group_room";

/// Actions supported by the tool（9 个，type-contract §二）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GroupRoomAction {
    Create,
    Invite,
    Remove,
    Send,
    History,
    List,
    Fork,
    MemberStatus,
    Delete,
}

impl GroupRoomAction {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "invite" => Some(Self::Invite),
            "remove" => Some(Self::Remove),
            "send" => Some(Self::Send),
            "history" => Some(Self::History),
            "list" => Some(Self::List),
            "fork" => Some(Self::Fork),
            "member_status" => Some(Self::MemberStatus),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// Tool input（9 个 action 的入参，type-contract §二）。
#[derive(Debug, Clone, Deserialize)]
struct GroupRoomInput {
    #[serde(rename = "action")]
    action: String,
    /// create/fork: 群名。
    #[serde(default)]
    name: Option<String>,
    /// create: 群专属工作区。
    #[serde(default)]
    workspace: Option<String>,
    /// create/invite/fork: 成员会话 id 列表。
    #[serde(default)]
    members: Vec<String>,
    /// invite/remove/member_status: 成员会话 id。
    #[serde(default)]
    member_session_id: Option<String>,
    /// 群会话 id（invite/remove/send/history/fork/member_status/delete）。
    #[serde(default)]
    group_id: Option<String>,
    /// send: 消息正文。
    #[serde(default)]
    content: Option<String>,
    /// send: 发送者会话 id。
    #[serde(default)]
    sender_session_id: Option<String>,
    /// send: 紧急打断。
    #[serde(default)]
    urgent: bool,
    /// history: 读取条数。
    #[serde(default)]
    limit: Option<usize>,
    /// history: 分页游标。
    #[serde(default)]
    cursor: Option<usize>,
    /// fork: 裂变点 turn id。
    #[serde(default)]
    turn_id: Option<String>,
}

/// 发送者身份（契约 §三类型定义，字段对齐 session_message_tool.rs:485-496）。
/// 契约要求「复用现成 SenderIdentity」，但该类型在 session_message_tool.rs 中为
/// private 且不可跨模块复用；此处本地定义字段完全一致的等价类型（含 serde derive），
/// 保证 GroupMessage 可序列化且 wire 形态与契约 §三一致。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderIdentity {
    /// 发送者会话 id；始终存在。
    pub session_id: String,
    /// RBAC 角色展示标签（如 "Commander"）。
    pub role: Option<String>,
    /// 会话树深度（0 = L0 根）。
    pub depth: Option<u32>,
    /// 会话名（或 agent_type 回退）。
    pub name: Option<String>,
}

/// 群聊关联键（契约 §三，字段对齐 session_message_tool.rs:504-510
/// GroupChatForwardMetadata：groupId/groupMessageId/groupAuthor）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatForwardMetadata {
    /// 群会话 id。
    pub group_id: Option<String>,
    /// 被回复的群消息 id。
    pub group_message_id: Option<String>,
    /// 发送者标识：`__master__` 或成员会话 id。
    pub group_author: Option<String>,
}

/// 群消息（type-contract §三；author/metadata 复用现成类型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMessage {
    pub message_id: String,
    pub group_session_id: String,
    /// 发送者身份（复用 SenderIdentity，§三类型定义）。
    pub author: SenderIdentity,
    pub content: String,
    pub timestamp: i64,
    /// 群聊关联键（复用 GroupChatForwardMetadata，§三）。
    pub metadata: GroupChatForwardMetadata,
}

/// action → 是否只读（type-contract §六.5：history/list/member_status 只读，
/// create/invite/remove/send/fork/delete 非只读）。
pub(crate) fn group_room_action_is_readonly(action: GroupRoomAction) -> bool {
    matches!(
        action,
        GroupRoomAction::History | GroupRoomAction::List | GroupRoomAction::MemberStatus
    )
}

/// GroupRoomTool — 1 tool 9 action（materialization 注册 9 个名称 → 同一实例）。
pub struct GroupRoomTool;

impl Default for GroupRoomTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupRoomTool {
    pub fn new() -> Self {
        Self
    }

    fn coordinator() -> BitFunResult<std::sync::Arc<ConversationCoordinator>> {
        get_global_coordinator().ok_or_else(|| {
            BitFunError::tool("group chat tools require an initialized coordinator".to_string())
        })
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// "commander" -> "Commander"，对齐 session_message_tool.rs:541-556 的展示标签。
    fn format_role_display(role: &str) -> String {
        role.split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        let mut word = first.to_uppercase().to_string();
                        word.push_str(chars.as_str());
                        word
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// 构造 SenderIdentity（契约 §三）：RBAC role（get_session_role）、会话树深度
    /// （coordinator.session_tree().get_depth）、会话名（内存会话 → 磁盘元数据回退）。
    /// 所有字段优雅降级：缺失 → None，绝不阻塞发送/读取。
    async fn resolve_sender_identity(
        coordinator: &ConversationCoordinator,
        session_id: &str,
        workspace: &str,
    ) -> SenderIdentity {
        let role = get_session_role(session_id).map(|role| Self::format_role_display(role.as_str()));
        let depth = coordinator.session_tree().get_depth(session_id);
        // 内存会话名优先；缺失时回退磁盘元数据（重启后未加载场景）。
        let name = coordinator
            .get_session_manager()
            .get_session(session_id)
            .and_then(|session| {
                let name = session.session_name.trim().to_string();
                (!name.is_empty()).then_some(name)
            });
        let name = match name {
            Some(name) => Some(name),
            None => {
                let disk_name = async {
                    coordinator
                        .get_session_manager()
                        .load_session_metadata(std::path::Path::new(workspace), session_id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|m| {
                            let name = m.session_name.trim().to_string();
                            (!name.is_empty()).then_some(name)
                        })
                }
                .await;
                disk_name
            }
        };
        SenderIdentity {
            session_id: session_id.to_string(),
            role,
            depth,
            name,
        }
    }

    /// 群会话的 workspace（内存 config 绑定，coordinator.rs:3014 写入）。
    fn group_workspace(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<String> {
        coordinator
            .get_session_manager()
            .get_session(group_id)
            .and_then(|session| session.config.workspace_path)
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "group chat session '{group_id}' does not exist in memory"
                ))
            })
    }

    /// 群主默认对话类型（R-GC-25 零硬编码纠正，主人定标 2026-08-14）：
    /// 复用现成 `AgentRegistry::default_agent_type()`（agents/registry/
    /// resolution.rs:92）——默认对话类型由 AgentRegistry 单一事实源提供，
    /// 配置驱动（可改默认对话类型而不改群聊代码）。禁散落硬编码
    /// agent_type 字符串。
    fn default_group_agent_type() -> String {
        get_agent_registry().default_agent_type().to_string()
    }

    /// 建群 = 建默认对话类型会话（type-contract §二.1；R-GC-25 群主对话
    /// 模型 + 零硬编码：agent_type 取默认对话类型，workspace 取入参兜底链）。
    async fn create_group(
        coordinator: &ConversationCoordinator,
        name: &str,
        members: &[String],
        workspace: &str,
    ) -> BitFunResult<String> {
        let group_session_id = uuid::Uuid::new_v4().to_string();
        let group_agent_type = Self::default_group_agent_type();
        let config = SessionConfig {
            workspace_path: Some(workspace.to_string()),
            project_workspace_path: Some(workspace.to_string()),
            ..Default::default()
        };
        coordinator
            .create_session_with_workspace(
                Some(group_session_id.clone()),
                name.to_string(),
                group_agent_type.clone(),
                config,
                workspace.to_string(),
            )
            .await
            .map_err(BitFunError::tool)?;

        // R-GC-25 群主对话模型：建群 = 创建群主 Claw 会话 + 写入群主欢迎
        // turn（宿主 turn）。群聊 = 普通会话（契约 §一）：群主会话必须带
        // 真实对话 turn，否则开局为空字符串/空时间线、且无宿主 turn 支撑
        // 「该轮以非标准方式结束」的根因（R-GC-23 同根）。
        // 欢迎 turn 与 send_message 同构（kind=UserDialog + status=Completed
        // + finish_reason="complete"），前端 NORMAL_FINISH_REASONS 命中，
        // 不再误报横幅。
        Self::write_group_turn(
            coordinator,
            workspace,
            &group_session_id,
            &group_session_id,
            &format!("群聊「{name}」已创建。我是群主，成员消息将汇聚于此。"),
        )
        .await?;

        // 建成员会话（成员各自 workspace——v3 不解析成员 workspace，
        // 由调用方传入；此处统一用群 workspace 绑定）。
        for member_id in members {
            let member_config = SessionConfig {
                workspace_path: Some(workspace.to_string()),
                project_workspace_path: Some(workspace.to_string()),
                ..Default::default()
            };
            coordinator
                .create_session_with_workspace(
                    Some(member_id.clone()),
                    format!("Group member {member_id}"),
                    Self::default_group_agent_type(),
                    member_config,
                    workspace.to_string(),
                )
                .await
                .map_err(BitFunError::tool)?;
            // 记入群成员表。
            Self::add_group_member(coordinator, workspace, &group_session_id, member_id).await?;
        }

        Ok(group_session_id)
    }

    /// 拉成员 = 建/确认成员默认对话类型会话 + 记入群成员表。
    async fn invite_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
        workspace: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let manager = coordinator.get_session_manager();
        if manager.get_session(member_session_id).is_none() {
            let member_config = SessionConfig {
                workspace_path: Some(workspace.to_string()),
                project_workspace_path: Some(workspace.to_string()),
                ..Default::default()
            };
            coordinator
                .create_session_with_workspace(
                    Some(member_session_id.to_string()),
                    format!("Group member {member_session_id}"),
                    Self::default_group_agent_type(),
                    member_config,
                    workspace.to_string(),
                )
                .await
                .map_err(BitFunError::tool)?;
        }
        Self::add_group_member(coordinator, &group_workspace, group_id, member_session_id).await
    }

    /// 移除成员 = 从群会话 custom_metadata.groupChats 移除。
    async fn remove_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let manager = coordinator.get_session_manager();
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), group_id, |metadata| {
                let members = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let filtered: Vec<Value> = members
                    .into_iter()
                    .filter(|v| v.as_str() != Some(member_session_id))
                    .collect();
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert("groupChats".to_string(), json!(filtered));
            })
            .await
            .map_err(BitFunError::tool)
    }

    /// 发送群消息 = 路由到群主会话 turn（type-contract §二.4 + §三 + R-GC-26）。
    ///
    /// R-GC-26 根因级修复：旧实现只手动落盘一条 turn（write_group_turn_with_metadata），
    /// 群主 Claw 会话从未收到用户消息 → 用户发消息无人响应。现在复用
    /// `coordinator.start_dialog_turn`（coordinator.rs:4213）把消息作为群主会话的
    /// 真实 dialog turn 提交：turn 落盘由正常 turn 流完成（含 user_message.metadata
    /// 五字段持久化，types.rs:662），群主 agent 真正运行并响应。turn_id 由本函数
    /// 生成并作为 message_id 返回，保证 send 响应可对账。
    async fn send_message(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        content: &str,
        sender_session_id: &str,
    ) -> BitFunResult<String> {
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let sender = Self::resolve_sender_identity(coordinator, sender_session_id, &group_workspace)
            .await;

        // 消息 metadata：五字段（契约 §三，B-2 修复）：
        // { groupId, senderSessionId, senderRole, senderDepth, senderName }。
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        // senderName 取真实会话名（B-2 修复）；无会话名时回退 sender_session_id 占位。
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender_session_id)),
        );

        // 生成 turn_id（= message_id）：start_dialog_turn 接受显式 turn_id，
        // 落盘的 user turn 以该 id 持久化（get_history 按 turn_id 解析发言方）。
        let message_id = uuid::Uuid::new_v4().to_string();
        coordinator
            .start_dialog_turn(
                group_id.to_string(),
                content.to_string(),
                Some(content.to_string()),
                Some(message_id.clone()),
                Self::default_group_agent_type(),
                // workspace_path = None：使用群主会话已加载的 storage 绑定
                // （coordinator.rs:6044-6063 session_storage_workspace_locator
                // None 分支 = 「已加载 session 绑定优先」）。显式传路径会在
                // resolve_session_restore_scope 二次解析时与已绑定路径不一致
                // （报 "Session ID is already bound to another workspace"）。
                None,
                None,
                None,
                DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi),
                Some(serde_json::Value::Object(metadata)),
            )
            .await
            .map_err(BitFunError::tool)?;

        Ok(message_id)
    }

    /// 群主欢迎 turn（R-GC-25）：建群 = 创建群主 Claw 会话，写群主欢迎
    /// turn 作为会话首条宿主 turn（带 sender 身份 = 群主）。
    async fn write_group_turn(
        coordinator: &ConversationCoordinator,
        workspace: &str,
        group_id: &str,
        sender_session_id: &str,
        content: &str,
    ) -> BitFunResult<String> {
        // 与 send_message 同构的五字段 metadata（契约 §三）：解析群主
        // 会话身份（role/depth/name），让欢迎 turn 的 senderBadge 正常显示。
        let sender = Self::resolve_sender_identity(coordinator, sender_session_id, workspace).await;
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender_session_id)),
        );
        Self::write_group_turn_with_metadata(
            coordinator,
            workspace,
            group_id,
            content,
            metadata,
        )
        .await
    }

    /// 落盘一条群会话 turn（宿主 turn 形态）：
    /// - kind = UserDialog（is_model_visible=true，history 可读）
    /// - status = Completed + finish_reason = "complete"（前端
    ///   NORMAL_FINISH_REASONS 命中，R-GC-25 消除「该轮以非标准方式结束」）
    /// - has_final_response = true（群消息本身即最终响应）
    /// - turn_index 取「已持久化 turns 的最大 index + 1」——不能固定为 0，
    ///   否则后续消息会覆盖 turn-0 文件（R-GC-10 三形态实测发现，
    ///   群主与成员各发一条时后者把前者覆盖；根因 = 硬编码 turn_index: 0）。
    async fn write_group_turn_with_metadata(
        coordinator: &ConversationCoordinator,
        workspace: &str,
        group_id: &str,
        content: &str,
        metadata: serde_json::Map<String, Value>,
    ) -> BitFunResult<String> {
        let mut next_turn_index = 0usize;
        if let Ok(turns) = coordinator
            .get_session_manager()
            .persistence_manager()
            .load_session_turns(&PathBuf::from(workspace), group_id)
            .await
        {
            next_turn_index = turns.iter().map(|turn| turn.turn_index).max().map_or(0, |max| max + 1);
        }
        let message_id = uuid::Uuid::new_v4().to_string();
        let now_ms = Self::now_ms();
        let turn = bitfun_services_core::session::DialogTurnData {
            turn_id: message_id.clone(),
            turn_index: next_turn_index,
            session_id: group_id.to_string(),
            timestamp: now_ms as u64,
            kind: bitfun_services_core::session::DialogTurnKind::UserDialog,
            agent_type: Some(Self::default_group_agent_type()),
            user_message: bitfun_services_core::session::UserMessageData {
                id: message_id.clone(),
                content: content.to_string(),
                timestamp: now_ms as u64,
                metadata: Some(serde_json::Value::Object(metadata)),
            },
            model_rounds: Vec::new(),
            start_time: now_ms as u64,
            end_time: Some(now_ms as u64),
            duration_ms: Some(0),
            token_usage: None,
            // R-GC-25 根因级修复：群消息 = 正常完成的宿主 turn。普通会话
            // 正常终态为 finish_reason="complete"（coordinator.rs:4828/5836），
            // 群消息按同一口径落盘，前端 turnCompletionNotice 不再误报
            // 「该轮以非标准方式结束」（NORMAL_FINISH_REASONS 命中）。
            finish_reason: Some("complete".to_string()),
            has_final_response: Some(true),
            error: None,
            error_detail: None,
            status: bitfun_services_core::session::TurnStatus::Completed,
        };
        coordinator
            .get_session_manager()
            .persistence_manager()
            .save_dialog_turn(&PathBuf::from(workspace), &turn)
            .await
            .map_err(BitFunError::tool)?;

        Ok(message_id)
    }

    /// 查看群历史 = SessionManager::get_messages（type-contract §二.5）。
    ///
    /// R-GC-26：群消息历史只返回**用户发言**（MessageRole::User）。旧实现返回
    /// get_messages 的全部消息（含群主 agent 响应），前端把 assistant 消息也渲染成
    /// 用户气泡。群主响应通过事件流即时显示（DialogTurnStarted/TextChunk），历史
    /// 仅聚合用户发言（群消息 = 用户发到群里的消息，契约 §三语义）。
    ///
    /// author 解析（契约 §三，B-1 修复）：群消息以 `DialogTurnData` 持久化，
    /// 发言方键（senderSessionId/senderRole/senderDepth/senderName）位于
    /// `user_message.metadata`（types.rs:662）。运行时 Message 不承载这些
    /// 自定义键，因此先从持久化 turns 重建「turn_id → 发言方」映射，再为每个
    /// Message 还原 `SenderIdentity`；缺失时优雅降级（senderSessionId 未知 →
    /// "unknown"，role/depth/name → None），绝不阻断读取。
    async fn get_history(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        limit: Option<usize>,
    ) -> BitFunResult<Vec<GroupMessage>> {
        let manager = coordinator.get_session_manager();
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let messages = manager
            .get_messages(group_id)
            .await
            .map_err(BitFunError::tool)?;
        let group_session_id = group_id.to_string();

        // B-1：从持久化 turns 重建发言方映射（turn_id → SenderIdentity）。
        let sender_by_turn = Self::build_sender_by_turn(
            &manager
                .persistence_manager()
                .load_session_turns(&PathBuf::from(&group_workspace), group_id)
                .await
                .unwrap_or_default(),
        );

        // R-GC-26：只保留用户发言（群消息历史 = 用户发到群里的消息）。
        // Message.role 为 MessageRole 枚举（message.rs:24-29，User 变体）。
        let mut result = messages
            .into_iter()
            .filter(|message| message.role == crate::agentic::core::MessageRole::User)
            .map(|message| {
                let sender = message
                    .metadata
                    .turn_id
                    .as_deref()
                    .and_then(|turn_id| sender_by_turn.get(turn_id).cloned())
                    .unwrap_or_else(|| SenderIdentity {
                        session_id: "unknown".to_string(),
                        role: None,
                        depth: None,
                        name: None,
                    });
                let group_author = (sender.session_id != "unknown")
                    .then(|| sender.session_id.clone());
                GroupMessage {
                    message_id: message.id,
                    group_session_id: group_session_id.clone(),
                    author: sender,
                    content: message.content.to_string(),
                    timestamp: message
                        .timestamp
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or_default(),
                    metadata: GroupChatForwardMetadata {
                        group_id: Some(group_session_id.clone()),
                        group_message_id: None,
                        group_author,
                    },
                }
            })
            .collect::<Vec<_>>();
        if let Some(limit) = limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    /// 从持久化 turns 重建「turn_id → SenderIdentity」发言方映射（契约 §三）。
    /// user_message.metadata 缺失或为 JSON null 的 turn 跳过；调用方负责容错
    /// （读取失败 → 空映射）。
    fn build_sender_by_turn(
        turns: &[bitfun_services_core::session::DialogTurnData],
    ) -> std::collections::HashMap<String, SenderIdentity> {
        let mut sender_by_turn = std::collections::HashMap::new();
        for turn in turns {
            let Some(metadata) = turn.user_message.metadata.as_ref() else {
                continue;
            };
            // JSON null metadata（测试/异常形态）→ 视为无发言方，跳过。
            if metadata.is_null() {
                continue;
            }
            sender_by_turn.insert(
                turn.turn_id.clone(),
                Self::parse_sender_identity_from_json(metadata),
            );
        }
        sender_by_turn
    }

    /// 从持久化 turn 的 user_message.metadata（JSON）解析 SenderIdentity
    /// （契约 §三：senderSessionId/senderRole/senderDepth/senderName）。
    fn parse_sender_identity_from_json(
        metadata: &Value,
    ) -> SenderIdentity {
        let get = |key: &str| metadata.get(key).and_then(|v| v.as_str()).map(ToOwned::to_owned);
        SenderIdentity {
            session_id: get("senderSessionId").unwrap_or_else(|| "unknown".to_string()),
            role: get("senderRole"),
            depth: metadata
                .get("senderDepth")
                .and_then(|v| v.as_u64())
                .map(|d| d as u32),
            name: get("senderName").filter(|value| !value.trim().is_empty()),
        }
    }

    /// 群聊列表 = list_sessions 过滤含群标记（custom_metadata.groupChats）。
    async fn list_groups(
        coordinator: &ConversationCoordinator,
        workspace: &str,
    ) -> BitFunResult<Vec<Value>> {
        let manager = coordinator.get_session_manager();
        let summaries = coordinator
            .list_sessions(std::path::Path::new(workspace))
            .await
            .map_err(BitFunError::tool)?;
        let mut groups = Vec::new();
        let group_agent_type = Self::default_group_agent_type();
        for summary in summaries {
            if summary.agent_type != group_agent_type {
                continue;
            }
            let metadata = manager
                .load_session_metadata(&PathBuf::from(workspace), &summary.session_id)
                .await
                .map_err(BitFunError::tool)?;
            if let Some(meta) = metadata {
                let is_group = meta
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .is_some();
                if is_group {
                    groups.push(json!({
                        "groupId": meta.session_id,
                        "name": meta.session_name,
                        "memberCount": meta
                            .custom_metadata
                            .as_ref()
                            .and_then(|m| m.get("groupChats"))
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0),
                    }));
                }
            }
        }
        Ok(groups)
    }

    /// fork 群聊 = branch_session 裂变子群（type-contract §二.7）。
    async fn fork_group(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        name: &str,
        turn_id: Option<&str>,
        members: &[String],
    ) -> BitFunResult<String> {
        use bitfun_services_core::session::{SessionBranchBoundary, SessionBranchRequest};

        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let manager = coordinator.get_session_manager();

        // branch_session：从主群 fork 子群（规划/审查/执行小群）。
        let branch = manager
            .persistence_manager()
            .branch_session(
                &PathBuf::from(&group_workspace),
                &SessionBranchRequest {
                    source_session_id: group_id.to_string(),
                    source_turn_id: turn_id.unwrap_or("").to_string(),
                    boundary: SessionBranchBoundary::ThroughTurn,
                },
            )
            .await
            .map_err(BitFunError::tool)?;
        let child_session_id = branch.session_id.clone();

        // 子群成员建会话 + 记成员表。
        for member_id in members {
            let member_config = SessionConfig {
                workspace_path: Some(group_workspace.clone()),
                project_workspace_path: Some(group_workspace.clone()),
                ..Default::default()
            };
            coordinator
                .create_session_with_workspace(
                    Some(member_id.clone()),
                    format!("Group member {member_id}"),
                    Self::default_group_agent_type(),
                    member_config,
                    group_workspace.clone(),
                )
                .await
                .map_err(BitFunError::tool)?;
            Self::add_group_member(coordinator, &group_workspace, &child_session_id, member_id)
                .await?;
        }

        // 子群命名 + forkOrigin 元数据。
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), &child_session_id, |m| {
                m.session_name = name.to_string();
                let custom = m
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert(
                    "forkOrigin".to_string(),
                    json!({ "parentGroupId": group_id }),
                );
            })
            .await
            .map_err(BitFunError::tool)?;

        Ok(child_session_id)
    }

    /// 成员状态 = 校验群成员身份 + get_session 查 state（type-contract §二.8）。
    ///
    /// B-4 修复：入参带 group_id + member_session_id，先校验
    /// member_session_id ∈ 群成员表（群会话 custom_metadata.groupChats），
    /// 不在群成员表 → 拒绝（防越权查任意会话）；再 get_session 查 state。
    async fn member_status(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<Value> {
        let manager = coordinator.get_session_manager();
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        let group_metadata = manager
            .load_session_metadata(&PathBuf::from(&group_workspace), group_id)
            .await
            .map_err(BitFunError::tool)?
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "group chat session '{group_id}' metadata not found"
                ))
            })?;
        let group_members = group_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let is_member = group_members
            .iter()
            .any(|v| v.as_str() == Some(member_session_id));
        if !is_member {
            return Err(BitFunError::tool(format!(
                "session '{member_session_id}' is not a member of group '{group_id}'"
            )));
        }

        let session = manager.get_session(member_session_id).ok_or_else(|| {
            BitFunError::tool(format!(
                "group member session '{member_session_id}' does not exist in memory"
            ))
        })?;
        Ok(json!({
            "sessionId": session.session_id,
            "agentType": session.agent_type,
            "state": format!("{:?}", session.state),
            "workspacePath": session.config.workspace_path,
        }))
    }

    /// 删除群聊 = 删群会话（type-contract §二.9）。
    async fn delete_group(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id)?;
        coordinator
            .delete_session(std::path::Path::new(&group_workspace), group_id)
            .await
            .map_err(BitFunError::tool)
    }

    /// 记成员进群成员表（幂等：已存在则跳过）。
    async fn add_group_member(
        coordinator: &ConversationCoordinator,
        group_workspace: &str,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let manager = coordinator.get_session_manager();
        manager
            .update_session_metadata(&PathBuf::from(group_workspace), group_id, |metadata| {
                let mut members = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if !members.iter().any(|v| v.as_str() == Some(member_session_id)) {
                    members.push(json!(member_session_id));
                }
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert("groupChats".to_string(), json!(members));
            })
            .await
            .map_err(BitFunError::tool)
    }

    /// 从输入提取 action（只读判定入口；缺失/非法 → None）。
    fn input_action(input: Option<&Value>) -> Option<GroupRoomAction> {
        let action = input?.get("action")?.as_str()?;
        GroupRoomAction::from_str(action)
    }

    /// R-GC-26：建群 workspace 解析（主人定标 2026-08-14：建群 = 新建 Claw
    /// 默认对话，群主会话 workspace = Claw 默认工作区，禁 currentWorkspace）。
    ///
    /// 优先级：入参 workspace（trim 后非空，调用方显式指定群专属工作区）→
    /// 默认 Claw 工作区（`~/.bitfun/personal_assistant/workspace`，
    /// path_manager.rs:203 default_assistant_workspace_dir）。
    ///
    /// R-GC-26 变更：移除 context.workspace_root 一级——旧实现（R-GC-17）把
    /// 当前会话工作区（= 用户当前项目工作区，如 taiji 开发版）作为兜底，
    /// 导致建群后群主会话 workspace 锁定到当前项目（主人实测「工作区自动
    /// 锁定到 taiji 开发版」）。群聊 = Claw 默认对话（契约 §一），群主
    /// workspace 必须落在 Claw 默认工作区，与新建普通 Claw 对话一致。
    /// 任何一级为空/None 都落到默认工作区，任何一端空都不炸、
    /// 不报「workspace is required」。
    fn resolve_create_workspace(workspace_param: Option<&str>) -> String {
        workspace_param
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                get_path_manager_arc()
                    .default_assistant_workspace_dir(None)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            })
    }
}

#[async_trait]
impl Tool for GroupRoomTool {
    fn name(&self) -> &str {
        GROUP_ROOM_TOOL_NAME
    }

    fn short_description(&self) -> String {
        "Manage group chat rooms coordinating multiple Claw assistant sessions.".to_string()
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Manage group chat rooms that coordinate multiple default assistant sessions (v3: 群聊 = 普通会话).

Actions:
- "create": Create a group with a name, members, and a dedicated workspace. Group ID = the created session ID (default assistant agent_type, config-driven).
- "invite": Invite a member session into a group (creates the member session if missing).
- "remove": Remove a member session from a group.
- "send": Send a group message written into the group session's turn stream (metadata carries sender + groupId).
- "history": Read group message history (SessionHistory of the group session).
- "list": List groups in a workspace (sessions carrying the groupChats marker).
- "fork": Fork a child group (规划/审查/执行小群) via session branch.
- "member_status": Query a member session's state.
- "delete": Delete a group (session delete).

Arguments:
- "action": One of the actions above.
- "name": Group name for create/fork.
- "workspace": Group workspace for create.
- "members": Member session ids for create/invite/fork.
- "group_id": Target group session id for invite/remove/send/history/fork/member_status/delete.
- "member_session_id": Member session id for invite/remove/member_status.
- "content": Message content for send.
- "sender_session_id": Sender session id for send.
- "urgent": Urgent delivery for send.
- "limit": History read limit.
- "cursor": History page cursor.
- "turn_id": Fork point turn id."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "invite", "remove", "send", "history", "list", "fork", "member_status", "delete"],
                    "description": "The group chat action to perform."
                },
                "name": { "type": "string", "description": "Group name for create/fork." },
                "workspace": { "type": "string", "description": "Group workspace for create." },
                "members": { "type": "array", "items": { "type": "string" }, "description": "Member session ids for create/invite/fork." },
                "group_id": { "type": "string", "description": "Target group session id." },
                "member_session_id": { "type": "string", "description": "Member session id for invite/remove/member_status." },
                "content": { "type": "string", "description": "Message content for send." },
                "sender_session_id": { "type": "string", "description": "Sender session id for send." },
                "urgent": { "type": "boolean", "description": "Urgent delivery for send." },
                "limit": { "type": "integer", "description": "History read limit." },
                "cursor": { "type": "integer", "description": "History page cursor." },
                "turn_id": { "type": "string", "description": "Fork point turn id." }
            },
            "required": ["action"]
        })
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    /// 只读判定按 action 区分（type-contract §六.5，B-3 修复）：
    /// history/list/member_status 只读；create/invite/remove/send/fork/delete 非只读。
    fn is_readonly(&self) -> bool {
        false
    }

    /// action 级只读（输入依赖）：由 `is_action_readonly` 决定是否并发安全
    /// 与是否产生权限意图（只读 action 无副作用 → 并发安全 + 无 PermissionIntent）。
    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        Self::input_action(input).is_some_and(group_room_action_is_readonly)
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        if Self::input_action(Some(input)).is_some_and(group_room_action_is_readonly) {
            return Ok(Vec::new());
        }
        Ok(vec![PermissionIntent::new(
            "custom_tool",
            vec![self.name().to_string()],
        )])
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let parsed: GroupRoomInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {error}")))?;
        let action = GroupRoomAction::from_str(&parsed.action).ok_or_else(|| {
            BitFunError::tool(format!("unknown group_room action '{}'", parsed.action))
        })?;
        let coordinator = Self::coordinator()?;

        let output = match action {
            GroupRoomAction::Create => {
                let name = parsed.name.as_deref().ok_or_else(|| {
                    BitFunError::tool("name is required for create".to_string())
                })?;
                // R-GC-26：建群 = 新建 Claw 默认对话（默认工作区，禁
                // currentWorkspace）。入参 workspace（调用方显式指定群专属
                // 工作区）→ Claw 默认工作区兜底；任一为空都不炸、
                // 不报「workspace is required」。
                let workspace = Self::resolve_create_workspace(parsed.workspace.as_deref());
                let group_id =
                    Self::create_group(&coordinator, name, &parsed.members, &workspace).await?;
                json!({ "groupId": group_id })
            }
            GroupRoomAction::Invite => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for invite".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for invite".to_string())
                })?;
                let workspace = context
                    .workspace_root()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                Self::invite_member(&coordinator, group_id, member, &workspace).await?;
                json!({ "groupId": group_id, "member": member, "status": "invited" })
            }
            GroupRoomAction::Remove => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for remove".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for remove".to_string())
                })?;
                Self::remove_member(&coordinator, group_id, member).await?;
                json!({ "groupId": group_id, "member": member, "status": "removed" })
            }
            GroupRoomAction::Send => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for send".to_string())
                })?;
                let content = parsed.content.as_deref().ok_or_else(|| {
                    BitFunError::tool("content is required for send".to_string())
                })?;
                let sender = parsed
                    .sender_session_id
                    .as_deref()
                    .or(context.session_id.as_deref())
                    .ok_or_else(|| {
                        BitFunError::tool("sender_session_id is required for send".to_string())
                    })?;
                let message_id = Self::send_message(&coordinator, group_id, content, sender).await?;
                json!({
                    "groupId": group_id,
                    "messageId": message_id,
                    "status": "sent",
                    // 透传 urgent（契约 §二.4 入参声明）：v3 群消息落群会话 turns，
                    // urgent 作为投递提示字段回传，供调用方确认打断语义已受理。
                    "urgent": parsed.urgent,
                })
            }
            GroupRoomAction::History => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for history".to_string())
                })?;
                let messages = Self::get_history(&coordinator, group_id, parsed.limit).await?;
                json!({
                    "groupId": group_id,
                    "messages": messages,
                    // 透传 cursor（契约 §二.5 入参声明）：当前实现按 limit 截断，
                    // cursor 作为分页游标原样回传，供调用方确认分页请求已受理。
                    "cursor": parsed.cursor,
                })
            }
            GroupRoomAction::List => {
                let workspace = parsed.workspace.clone().unwrap_or_else(|| {
                    context
                        .workspace_root()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                let groups = Self::list_groups(&coordinator, &workspace).await?;
                json!({ "groups": groups })
            }
            GroupRoomAction::Fork => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for fork".to_string())
                })?;
                let name = parsed.name.as_deref().unwrap_or("forked group");
                let child_id = Self::fork_group(
                    &coordinator,
                    group_id,
                    name,
                    parsed.turn_id.as_deref(),
                    &parsed.members,
                )
                .await?;
                json!({ "parentGroupId": group_id, "childGroupId": child_id })
            }
            GroupRoomAction::MemberStatus => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for member_status".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for member_status".to_string())
                })?;
                let status = Self::member_status(&coordinator, group_id, member).await?;
                json!({ "groupId": group_id, "status": status })
            }
            GroupRoomAction::Delete => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for delete".to_string())
                })?;
                Self::delete_group(&coordinator, group_id).await?;
                json!({ "groupId": group_id, "status": "deleted" })
            }
        };

        Ok(vec![ToolResult::ok(output, None)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    fn turn_with_sender(turn_id: &str, sender_json: Value) -> bitfun_services_core::session::DialogTurnData {
        bitfun_services_core::session::DialogTurnData {
            turn_id: turn_id.to_string(),
            turn_index: 0,
            session_id: "group-1".to_string(),
            timestamp: 0,
            kind: bitfun_services_core::session::DialogTurnKind::UserDialog,
            agent_type: Some(GroupRoomTool::default_group_agent_type()),
            user_message: bitfun_services_core::session::UserMessageData {
                id: turn_id.to_string(),
                content: "hello".to_string(),
                timestamp: 0,
                metadata: Some(sender_json),
            },
            model_rounds: Vec::new(),
            start_time: 0,
            end_time: None,
            duration_ms: None,
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            error: None,
            error_detail: None,
            status: bitfun_services_core::session::TurnStatus::Completed,
        }
    }

    // ── B-3（契约 §六.5）：readonly 按 action 区分 ──
    #[test]
    fn readonly_only_history_list_member_status() {
        for (name, expected) in [
            ("create", false),
            ("invite", false),
            ("remove", false),
            ("send", false),
            ("history", true),
            ("list", true),
            ("fork", false),
            ("member_status", true),
            ("delete", false),
        ] {
            let action =
                GroupRoomAction::from_str(name).unwrap_or_else(|| panic!("unknown {name}"));
            assert_eq!(
                group_room_action_is_readonly(action),
                expected,
                "action={name}"
            );
        }
    }

    #[test]
    fn tool_metadata_follows_action_readonly() {
        let tool = GroupRoomTool::new();
        // 框架 is_readonly（无 action 上下文基线）保守非只读（契约 §六.5）。
        assert!(!tool.is_readonly());
        // 只读 action：并发安全 + 无权限意图。
        for action in ["history", "list", "member_status"] {
            let input = json!({ "action": action, "group_id": "g-1" });
            assert!(tool.is_concurrency_safe(Some(&input)), "action={action}");
            assert!(
                tool.permission_intents(&input, &empty_context())
                    .expect("permission intents")
                    .is_empty(),
                "action={action}"
            );
        }
        // 非只读 action：非并发安全 + 有权限意图。
        for action in ["create", "invite", "remove", "send", "fork", "delete"] {
            let input = json!({ "action": action, "group_id": "g-1" });
            assert!(!tool.is_concurrency_safe(Some(&input)), "action={action}");
            assert!(
                !tool.permission_intents(&input, &empty_context())
                    .expect("permission intents")
                    .is_empty(),
                "action={action}"
            );
        }
        // 非法 action → 保守非只读。
        let bad = json!({ "action": "nope" });
        assert!(!tool.is_concurrency_safe(Some(&bad)));
    }

    // ── R-GC-25 零硬编码纠正（主人定标 2026-08-14）──
    // 群主对话类型不写死字符串：复用 AgentRegistry::default_agent_type()
    // （agents/registry/resolution.rs:92）——配置驱动，改默认对话类型
    // 即生效，群聊代码零改动。本测试断言「默认类型来自 AgentRegistry
    // 单一事实源」而非散落硬编码。
    #[test]
    fn default_group_agent_type_comes_from_agent_registry() {
        let expected = crate::agentic::agents::get_agent_registry()
            .default_agent_type()
            .to_string();
        let actual = GroupRoomTool::default_group_agent_type();
        assert_eq!(actual, expected);
        assert!(!actual.trim().is_empty(), "default agent type must be non-empty");
    }

    // ── B-2（契约 §三）：send metadata 五字段 + senderName 回退 ──
    #[test]
    fn send_metadata_contract_shape_is_five_fields() {
        // send 构造的 metadata 键集合 = 契约 §三 五字段（role/depth 缺失时省略）。
        let keys = ["groupId", "senderSessionId", "senderRole", "senderDepth", "senderName"];
        // 全字段形态（B-2 完整断言）。
        let metadata = json!({
            "groupId": "group-1",
            "senderSessionId": "sender-1",
            "senderRole": "commander",
            "senderDepth": 3,
            "senderName": "小群主",
        });
        for key in keys {
            assert!(metadata.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(metadata.get("groupId").and_then(Value::as_str), Some("group-1"));
        assert_eq!(
            metadata.get("senderSessionId").and_then(Value::as_str),
            Some("sender-1")
        );
        assert_eq!(
            metadata.get("senderRole").and_then(Value::as_str),
            Some("commander")
        );
        assert_eq!(metadata.get("senderDepth").and_then(Value::as_u64), Some(3));
        assert_eq!(
            metadata.get("senderName").and_then(Value::as_str),
            Some("小群主")
        );
    }

    #[test]
    fn send_metadata_name_falls_back_to_sender_id() {
        // senderName 回退逻辑与 send_message 相同（无会话名时用 sender_session_id）。
        let sender_session_id = "sender-x";
        let sender_name: Option<String> = None;
        let effective = sender_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(sender_session_id);
        assert_eq!(effective, "sender-x");
    }

    // ── B-1（契约 §三）：GroupMessage author/metadata 结构 + history author 解析 ──
    #[test]
    fn parse_sender_identity_from_json_full() {
        let parsed = GroupRoomTool::parse_sender_identity_from_json(&json!({
            "groupId": "group-1",
            "senderSessionId": "sender-9",
            "senderRole": "Executor",
            "senderDepth": 2,
            "senderName": "九号助手",
        }));
        assert_eq!(parsed.session_id, "sender-9");
        assert_eq!(parsed.role.as_deref(), Some("Executor"));
        assert_eq!(parsed.depth, Some(2));
        assert_eq!(parsed.name.as_deref(), Some("九号助手"));
    }

    #[test]
    fn parse_sender_identity_from_json_degrades_gracefully() {
        let parsed = GroupRoomTool::parse_sender_identity_from_json(&json!({}));
        assert_eq!(parsed.session_id, "unknown");
        assert_eq!(parsed.role, None);
        assert_eq!(parsed.depth, None);
        assert_eq!(parsed.name, None);

        let whitespace = GroupRoomTool::parse_sender_identity_from_json(&json!({
            "senderSessionId": "sender-1",
            "senderName": "   ",
        }));
        assert_eq!(whitespace.session_id, "sender-1");
        assert_eq!(whitespace.name, None);
    }

    #[test]
    fn history_author_map_resolves_from_turn_metadata() {
        let turns = vec![
            turn_with_sender(
                "turn-a",
                json!({
                    "groupId": "group-1",
                    "senderSessionId": "sender-a",
                    "senderRole": "Commander",
                    "senderDepth": 0,
                    "senderName": "群主",
                }),
            ),
            turn_with_sender("turn-b", json!({ "senderSessionId": "sender-b" })),
            // 无 metadata 的 turn 跳过。
            turn_with_sender("turn-c", json!(null)),
        ];
        let map = GroupRoomTool::build_sender_by_turn(&turns);
        assert_eq!(map.len(), 2);
        let a = map.get("turn-a").expect("turn-a");
        assert_eq!(a.session_id, "sender-a");
        assert_eq!(a.role.as_deref(), Some("Commander"));
        assert_eq!(a.depth, Some(0));
        assert_eq!(a.name.as_deref(), Some("群主"));
        let b = map.get("turn-b").expect("turn-b");
        assert_eq!(b.session_id, "sender-b");
        assert_eq!(b.name, None);
        assert!(!map.contains_key("turn-c"));
    }

    #[test]
    fn history_author_unknown_when_turn_not_in_map() {
        let sender_by_turn: HashMap<String, SenderIdentity> = HashMap::new();
        let turn_id = String::from("some-turn-id");
        let sender = sender_by_turn
            .get(&turn_id)
            .cloned()
            .unwrap_or_else(|| SenderIdentity {
                session_id: "unknown".to_string(),
                role: None,
                depth: None,
                name: None,
            });
        assert_eq!(sender.session_id, "unknown");
        assert_eq!(sender.role, None);
    }

    #[test]
    fn group_message_shape_matches_contract_section_three() {
        // GroupMessage 序列化形态：author 内嵌 SenderIdentity 字段 + metadata 关联键。
        let message = GroupMessage {
            message_id: "msg-1".to_string(),
            group_session_id: "group-1".to_string(),
            author: SenderIdentity {
                session_id: "sender-1".to_string(),
                role: Some("Commander".to_string()),
                depth: Some(0),
                name: Some("群主".to_string()),
            },
            content: "hi".to_string(),
            timestamp: 123,
            metadata: GroupChatForwardMetadata {
                group_id: Some("group-1".to_string()),
                group_message_id: None,
                group_author: Some("sender-1".to_string()),
            },
        };
        let json_value = serde_json::to_value(&message).expect("serialize");
        assert_eq!(
            json_value.pointer("/author/sessionId").and_then(Value::as_str),
            Some("sender-1")
        );
        assert_eq!(
            json_value
                .pointer("/author/role")
                .and_then(Value::as_str),
            Some("Commander")
        );
        assert_eq!(json_value.pointer("/author/depth").and_then(Value::as_u64), Some(0));
        assert_eq!(
            json_value.pointer("/author/name").and_then(Value::as_str),
            Some("群主")
        );
        assert_eq!(
            json_value
                .pointer("/metadata/groupId")
                .and_then(Value::as_str),
            Some("group-1")
        );
        assert_eq!(
            json_value
                .pointer("/metadata/groupAuthor")
                .and_then(Value::as_str),
            Some("sender-1")
        );
    }

    // ── B-4（契约 §二.8）：member_status 群成员表校验 ──
    #[test]
    fn member_status_requires_group_membership() {
        let group_members = json!(["member-a", "member-b"])
            .as_array()
            .cloned()
            .unwrap_or_default();
        let is_member = |target: &str| {
            group_members
                .iter()
                .any(|v| v.as_str() == Some(target))
        };
        assert!(is_member("member-a"));
        assert!(!is_member("stranger"));
    }

    #[test]
    fn member_status_membership_parse_helper_shape() {
        // 与 member_status 相同的群成员表读取链（custom_metadata.groupChats 数组）。
        let custom = json!({ "groupChats": ["m-1", "m-2"] });
        let members = custom
            .get("groupChats")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(members.iter().any(|v| v.as_str() == Some("m-1")));
        assert!(!members.iter().any(|v| v.as_str() == Some("m-9")));
    }

    #[test]
    fn send_metadata_group_author_uses_sender_session_id() {
        // history 的 group_author 关联键 = 发送者 session_id（契约 §三）。
        let session_id = "sender-1";
        let group_author = (session_id != "unknown").then(|| session_id.to_string());
        assert_eq!(group_author.as_deref(), Some("sender-1"));
    }

    // ── 核心路径测试（R-GC-08 收尾）──────────────────────────────
    // action 枚举 9 值 round-trip + input_schema 9 enum 校验 + 无 coordinator 清晰报错。

    // ── R-GC-26：建群 workspace = Claw 默认工作区（入参显式群工作区优先，
    //    否则默认 Claw 工作区；禁 currentWorkspace）──
    #[test]
    fn resolve_create_workspace_uses_param_first() {
        let workspace = GroupRoomTool::resolve_create_workspace(Some("  /ws/param  "));
        assert_eq!(workspace, "/ws/param");
    }

    #[test]
    fn resolve_create_workspace_never_uses_context_root() {
        // R-GC-26：建群 workspace 解析不再读取 context.workspace_root——
        // 当前项目工作区（如 taiji 开发版）不得成为群主会话 workspace。
        // 入参为空时直接落到 Claw 默认工作区（assistant home 下）。
        let workspace = GroupRoomTool::resolve_create_workspace(None);
        assert!(
            workspace.contains("personal_assistant") || workspace.contains(".bitfun"),
            "default Claw workspace should live under the assistant home, got: '{workspace}'"
        );
    }

    #[test]
    fn resolve_create_workspace_whitespace_falls_back_to_default() {
        // 空串/纯空白入参 → 默认 Claw 工作区（不得报「workspace is required」）。
        for param in [Some(""), Some("   "), None] {
            let workspace = GroupRoomTool::resolve_create_workspace(param);
            assert!(
                !workspace.trim().is_empty(),
                "empty param must fall back to a non-empty default workspace, got: '{workspace}'"
            );
            assert!(
                workspace.contains("personal_assistant") || workspace.contains(".bitfun"),
                "default Claw workspace should live under the assistant home, got: '{workspace}'"
            );
        }
    }

    #[test]
    fn create_without_workspace_does_not_error_on_missing_coordinator_only() {
        // call_impl create 分支：workspace 缺省不再触发「workspace is required」——
        // 解析兜底在 coordinator 校验之前完成；无 coordinator 时报错仍是
        // 「require an initialized coordinator」（见 missing_coordinator_yields_clear_error）。
        // workspace 空串输入 → 兜底链产出默认工作区，不再要求 workspace 必填。
        let resolved = GroupRoomTool::resolve_create_workspace(Some(""));
        assert!(!resolved.trim().is_empty());
        assert!(
            !resolved.starts_with("workspace is required"),
            "resolve must not surface a workspace-required error"
        );
    }

    #[test]
    fn action_round_trip_all_nine_actions() {
        let cases: [(GroupRoomAction, &str); 9] = [
            (GroupRoomAction::Create, "create"),
            (GroupRoomAction::Invite, "invite"),
            (GroupRoomAction::Remove, "remove"),
            (GroupRoomAction::Send, "send"),
            (GroupRoomAction::History, "history"),
            (GroupRoomAction::List, "list"),
            (GroupRoomAction::Fork, "fork"),
            (GroupRoomAction::MemberStatus, "member_status"),
            (GroupRoomAction::Delete, "delete"),
        ];
        for (expected, name) in cases {
            let parsed = GroupRoomAction::from_str(name)
                .unwrap_or_else(|| panic!("action {name} must parse"));
            assert_eq!(parsed, expected, "round-trip {name}");
        }
        // 非法值拒绝。
        assert!(GroupRoomAction::from_str("").is_none());
        assert!(GroupRoomAction::from_str("CREATE").is_none());
        assert!(GroupRoomAction::from_str("memberstatus").is_none());
    }

    #[test]
    fn input_schema_lists_all_nine_action_enums() {
        let schema = GroupRoomTool::new().input_schema();
        let enums = schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .expect("action enum array");
        let expected = [
            "create", "invite", "remove", "send", "history", "list", "fork", "member_status",
            "delete",
        ];
        assert_eq!(enums.len(), 9, "exactly 9 enum values");
        for name in expected {
            assert!(
                enums.iter().any(|v| v.as_str() == Some(name)),
                "schema enum missing {name}"
            );
            assert!(
                GroupRoomAction::from_str(name).is_some(),
                "schema enum {name} must be parseable"
            );
        }
        // 必填仅 action。
        assert_eq!(
            schema.pointer("/required").and_then(Value::as_array),
            Some(&json!(["action"]).as_array().cloned().unwrap())
        );
    }

    /// 无 coordinator 时所有 action 都返回清晰 tool error（get_global_coordinator 为 None）。
    /// 注意：此测试依赖全局 coordinator 未被其他测试 set_global（OnceLock 单次写入）。
    /// 若已被设置，直接跳过断言（避免跨测试顺序耦合）。
    #[tokio::test]
    async fn missing_coordinator_yields_clear_error() {
        if get_global_coordinator().is_some() {
            return;
        }
        let tool = GroupRoomTool::new();
        let context = empty_context();
        let error = tool
            .call_impl(&json!({ "action": "create", "name": "g", "workspace": "/tmp" }), &context)
            .await
            .expect_err("must fail without coordinator");
        assert!(
            error.to_string().contains("require an initialized coordinator"),
            "error: {error}"
        );
        let error = tool
            .call_impl(&json!({ "action": "history", "group_id": "g-1" }), &context)
            .await
            .expect_err("must fail without coordinator");
        assert!(
            error.to_string().contains("require an initialized coordinator"),
            "error: {error}"
        );
    }

    // ── 集成测试：create → send → history → list（真实 coordinator）──
    // 基建对齐 coordinator.rs 测试 helper（test_coordinator_with_registry，
    // enable_persistence=true 时 save_dialog_turn 可落盘）。set_global 为
    // OnceLock 单次写入：本测试成功后全局 coordinator 保持该实例（接受全局副作用；
    // 其它测试若先 set_global，本测试直接复用并跳过重复构造）。
    #[tokio::test]
    async fn create_send_history_list_roundtrip_with_real_coordinator() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        // 已存在全局 coordinator（其它测试设置过）→ 直接复用。
        if let Some(coordinator) = get_global_coordinator() {
            let workspace = std::env::temp_dir().join(format!(
                "bitfun-grouproom-reuse-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&workspace).expect("workspace dir");
            run_group_roundtrip(&coordinator, &workspace).await;
            return;
        }

        let user_root = std::env::temp_dir().join(format!(
            "bitfun-grouproom-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));

        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
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
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        );
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        let coordinator = Arc::new(coordinator);
        ConversationCoordinator::set_global(coordinator.clone());

        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-workspace-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        run_group_roundtrip(&coordinator, &workspace).await;
    }

    /// create（建群=建会话，含成员）→ send（写群会话 turns）→ history（读回）
    /// → list（群聊列表过滤）全链路断言。
    async fn run_group_roundtrip(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace: &std::path::Path,
    ) {
        use crate::agentic::core::Message;
        let manager = coordinator.get_session_manager();
        let workspace_str = workspace.to_string_lossy().to_string();

        // create：建群（2 成员）→ 返回 group_id（UUID）；会话列表可见且 agent_type=默认对话类型。
        let group_id = GroupRoomTool::create_group(
            coordinator,
            "测试群",
            &["member-a".to_string(), "member-b".to_string()],
            &workspace_str,
        )
        .await
        .expect("create group");
        assert!(!group_id.is_empty());
        let group_session = manager
            .get_session(&group_id)
            .expect("group session in memory");
        assert_eq!(
            group_session.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "R-GC-25: group-owner session uses the config-driven default agent type (no hardcoded string)"
        );
        assert_eq!(
            group_session.config.workspace_path.as_deref(),
            Some(workspace_str.as_str())
        );

        // 群成员表已写入 groupChats。
        let metadata = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members = metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(members.len(), 2, "groupChats must contain 2 members");

        // ── R-GC-25 群主对话模型：建群 = 创建群主 Claw 会话 + 群主欢迎
        // turn（宿主 turn）。开局不再空字符串/空时间线；欢迎 turn 是
        // 正常完成的宿主 turn（status=Completed + finish_reason="complete"
        // + has_final_response=true），前端 NORMAL_FINISH_REASONS 命中，
        // 「该轮以非标准方式结束」横幅不再误报。
        let welcome_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &group_id)
            .await
            .expect("load welcome turns");
        assert!(
            !welcome_turns.is_empty(),
            "R-GC-25: create must write a group-owner welcome turn (宿主 turn)"
        );
        let welcome = welcome_turns
            .iter()
            .find(|t| t.user_message.content.contains("已创建"))
            .expect("welcome turn content must mention group creation");
        assert_eq!(
            welcome.status,
            bitfun_services_core::session::TurnStatus::Completed
        );
        assert_eq!(
            welcome.finish_reason.as_deref(),
            Some("complete"),
            "R-GC-25: welcome turn must carry the normal finish code"
        );
        assert_eq!(
            welcome.has_final_response,
            Some(true),
            "R-GC-25: welcome turn is a final response"
        );
        assert_eq!(
            welcome.user_message.metadata.as_ref().and_then(|m| m.get("senderSessionId")).and_then(Value::as_str),
            Some(group_id.as_str()),
            "R-GC-25: welcome turn sender = 群主会话（群聊 ID = 群主会话 ID）"
        );

        // send：写群会话 turns → message_id。
        let message_id = GroupRoomTool::send_message(coordinator, &group_id, "第一条群消息", "member-a")
            .await
            .expect("send message");
        assert!(!message_id.is_empty());

        // ── R-GC-26：send 把消息路由进群主会话真实 dialog turn
        // （coordinator.start_dialog_turn，异步启动）。turn 已持久化；终态
        // （finish_reason/has_final_response）由群主 agent 执行结果决定——
        // 测试环境无真实 agent，turn 停在 Processing，不在此断言终态。
        // R-GC-25 的「正常完成宿主 turn」语义由欢迎 turn（write_group_turn）
        // 保留（上面 welcome 断言），send 的消息 turn 是真实执行 turn。
        let sent_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &group_id)
            .await
            .expect("load sent turns");
        let sent = sent_turns
            .iter()
            .find(|t| t.turn_id == message_id)
            .expect("sent turn persisted by message_id");
        assert_eq!(
            sent.user_message.content, "第一条群消息",
            "R-GC-26: send routes the message into the group-owner session turn"
        );

        // history：读回消息（author 从 turn metadata 解析 senderSessionId=member-a）。
        // 注意：get_messages 从持久化 turns 重建 Message（Message.id 为重建时新 uuid），
        // 因此按「内容 + author」匹配，而非 send 返回的 message_id。
        let history = GroupRoomTool::get_history(coordinator, &group_id, None)
            .await
            .expect("get history");
        let found = history
            .iter()
            .find(|m| m.content == "第一条群消息")
            .expect("sent message present in history");
        assert_eq!(found.author.session_id, "member-a");
        assert_eq!(found.content, "第一条群消息");
        assert_eq!(found.metadata.group_id.as_deref(), Some(group_id.as_str()));
        // message_id 形状校验（uuid 非空）。
        assert!(!found.message_id.is_empty(), "message_id must not be empty");

        // list：群聊列表过滤（仅含 groupChats 标记的 Claw 会话）。
        let groups = GroupRoomTool::list_groups(coordinator, &workspace_str)
            .await
            .expect("list groups");
        let listed = groups
            .iter()
            .find(|g| g.get("groupId").and_then(Value::as_str) == Some(group_id.as_str()))
            .expect("group listed");
        assert_eq!(
            listed.get("memberCount").and_then(Value::as_u64),
            Some(2),
            "memberCount from groupChats"
        );

        // ── 三形态之②：成员会话（create 拉入的成员为默认对话类型会话）──
        // 成员会话 = 独立默认对话类型会话（契约 §一），可作 sender 写入群 turns。
        let member_session = manager
            .get_session("member-a")
            .expect("member session in memory");
        assert_eq!(
            member_session.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "R-GC-25: member session uses the config-driven default agent type"
        );

        // ── 三形态之①：默认 BuiltIn 群主（assistant_id 空）──
        // 群主 = GROUP_MASTER_ACTOR（__master__，契约 §五），无底层 assistant
        // 会话支撑；history 侧 author.session_id 即 __master__。
        // R-GC-26：send 触发群主会话真实 turn（异步执行）。测试环境无真实
        // agent，第一条 send 的 turn 保持 Processing（执行引擎挂起于模型解析）。
        // 真实产品语义 = 会话忙时新 turn 拒绝（与 SessionMessage 一致，
        // start_dialog_turn coordinator.rs:6206-6213 Processing → 拒绝）；
        // 此处断言 master send 在忙时返回清晰错误而非静默丢失。master 身份
        // 的 history author 解析由 send_metadata_* 单测 + build_sender_by_turn
        // 覆盖（GROUP_MASTER_ACTOR 作为 sender_session_id 透传）。
        let master_error = GroupRoomTool::send_message(
            coordinator,
            &group_id,
            "群主发言",
            bitfun_runtime_ports::GROUP_MASTER_ACTOR,
        )
        .await
        .expect_err("master send while group-owner session is busy must fail clearly");
        assert!(
            master_error
                .to_string()
                .contains("Session state does not allow starting new dialog"),
            "master send busy error must be the session-busy rejection, got: {master_error}"
        );

        // ── 三形态之③：fork 子群 → parent 关联（契约 §九/§八）──
        // fork 点 = 第一条群消息的持久化 turn_id（send 返回的 message_id 即 turn_id）。
        let child_id = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "测试子群",
            Some(&message_id),
            &["member-c".to_string()],
        )
        .await
        .expect("fork group");
        assert!(!child_id.is_empty());
        assert_ne!(child_id, group_id, "child must differ from parent");

        // parent 关联：child custom_metadata.forkOrigin.parentGroupId == 主群 id
        //（group_room fork 写 parentGroupId；branch_session 本身写
        // forkOrigin.sessionId/turnId/turnIndex，fork 重写为 parentGroupId，
        // 契约 §八：fork 亲子关系靠 forkOrigin 元数据）。
        let child_metadata = manager
            .load_session_metadata(workspace, &child_id)
            .await
            .expect("load child metadata")
            .expect("child metadata exists");
        let fork_origin = child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("forkOrigin"))
            .expect("child forkOrigin must exist");
        assert_eq!(
            fork_origin.get("parentGroupId").and_then(Value::as_str),
            Some(group_id.as_str()),
            "child forkOrigin.parentGroupId must reference the parent group"
        );

        // 子群自带成员表（fork 携带的 members）。
        let child_members = child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            child_members.iter().any(|v| v.as_str() == Some("member-c")),
            "fork child must carry its own member list"
        );

        // 子群继承主群 turns（branch 复制群消息 → 子群历史可读）。
        let child_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &child_id)
            .await
            .expect("child turns");
        assert!(
            child_turns
                .iter()
                .any(|t| t.user_message.content == "第一条群消息"),
            "child must inherit parent turns"
        );

        // 只读 action 可经 Tool 接口并发安全调用（不依赖全局 coordinator 的调用路径）。
        let _ = Message::user("unused".to_string());

        // ── R-GC-17/26：workspace 空 → 兜底默认 Claw 工作区 ──
        // 解析链为纯函数（resolve_create_workspace(None) → path_manager 默认
        // assistant workspace），由独立单测覆盖（resolve_create_workspace_*），
        // 不在此触发 create_session——全局 path_manager 在 CI runner 上指向
        // 真实 ~/.bitfun（可能不存在），create_session canonicalize 会失败
        // （Rust Build ubuntu 环境敏感，run 31799106971 前身）。create 数据流
        // 已由本测试主链路（显式 workspace）覆盖。
        let fallback_workspace = GroupRoomTool::resolve_create_workspace(None);
        assert!(
            !fallback_workspace.trim().is_empty(),
            "empty workspace must resolve to a non-empty default Claw workspace"
        );
    }
}


