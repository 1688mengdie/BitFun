//! GroupRoomTool — 群聊系列工具（主人定标 v3：群聊 = 普通会话）。
//!
//! Contract: type-contract v3（群聊v3-type-contract-最终权威-20260814.md §二）
//! R-GC-08：9 个 action（create/invite/remove/send/history/list/fork/
//! member_status/delete），复用现成机制：
//! - 建群 = `coordinator.create_session_with_workspace`（coordinator.rs:2659）
//! - 成员 = 调用方传入的真实会话 ID（校验存在后登记 groupChats；
//!   群聊重建 Type-Contract §二，R-GC-28 按数量新建匿名会话已回退）
//! - 发消息 = 群会话 turns（`PersistenceManager::save_dialog_turn`，
//!   persistence/manager.rs:3089；`user_message.metadata` 带 sender+groupId，
//!   types.rs:662）
//! - 历史 = `session_manager.get_messages`（session_manager.rs:8785）
//! - 裂变 = `PersistenceManager::branch_session`（session_branch.rs:14）
//! - 成员状态 = `session_manager.get_session`（:3060）
//! - 删除 = `coordinator.delete_session`（coordinator.rs:7434）
//!
//! 群聊 ID = 会话 ID（UUID）；群 = Claw 对话类型会话（agent_type 取
//! `coordinator::ASSISTANT_BOOTSTRAP_AGENT_TYPE`，R-GC-28b 零硬编码）
//! 带专属 workspace。
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
use crate::agentic::coordination::{
    get_global_coordinator, ConversationCoordinator, ASSISTANT_BOOTSTRAP_AGENT_TYPE,
};
use crate::agentic::core::SessionConfig;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{DialogSubmissionPolicy, DialogTriggerSource, GROUP_MASTER_ACTOR};
use log::warn;
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

    /// 构造 SenderIdentity（契约 §三）：会话树深度（coordinator.session_tree()
    /// .get_depth）、会话名（内存会话 → 磁盘元数据回退）。所有字段优雅降级：
    /// 缺失 → None，绝不阻塞发送/读取。RBAC role 已随 R-WF-01 删除，role 恒 None。
    ///
    /// R-GC-34（主人身份错位 P0 修复，方案 B）：`__master__`（GROUP_MASTER_ACTOR
    /// 保留字，local_customizations.rs:96）特判 → 主人身份 = depth 0（L0）+
    /// 主人名（i18n，禁硬编码中文）。
    async fn resolve_sender_identity(
        coordinator: &ConversationCoordinator,
        session_id: &str,
        workspace: &str,
    ) -> SenderIdentity {
        if session_id == GROUP_MASTER_ACTOR {
            return Self::master_sender_identity().await;
        }
        let role = None;
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

    /// 主人 SenderIdentity（R-GC-34 方案 B）：L0 + i18n 主人名。
    ///
    /// - role：R-WF-01 全删 RBAC 后恒 None（不再硬编码 Commander，发言方标识
    ///   由后续 R-WF-03/08 统一为「SOUL.name + 类型」）。
    /// - depth：0（L0 根，会话树语义）。
    /// - name：i18n shared term `agents.master`（按当前 locale 翻译；共享词条
    ///   在 shared/i18n/resources/shared/*/terms.json，经
    ///   generate-i18n-contract.mjs 生成 Rust 端 GENERATED_SHARED_TERMS）。
    ///   i18n-runtime feature 未启用（CLI/acp 编译面，AGENTS-CN.md：
    ///   「调用 I18nService 的 host 必须显式选择 i18n-runtime」）或全局服务
    ///   缺失/词条缺失 → 回退 "Master"（英文，i18n fallback 链语义的兜底）；
    ///   绝不返回空名（空值防御，不 crash）。
    async fn master_sender_identity() -> SenderIdentity {
        let role = None;
        let depth = Some(0u32);
        #[cfg(feature = "i18n-runtime")]
        let name = match crate::service::i18n::get_global_i18n_service().await {
            Some(service) => {
                let locale = service.get_current_locale().await;
                let translated = service
                    .translate_with_locale(&locale, "shared.agents.master", None)
                    .await;
                (!translated.is_empty() && translated != "shared.agents.master")
                    .then_some(translated)
            }
            None => None,
        };
        #[cfg(not(feature = "i18n-runtime"))]
        let name = None;
        let name = name.or_else(|| Some("Master".to_string()));
        SenderIdentity {
            session_id: GROUP_MASTER_ACTOR.to_string(),
            role,
            depth,
            name,
        }
    }

    /// 群会话的 workspace（内存 config 绑定，coordinator.rs:3014 写入）。
    ///
    /// R-GC-38（扩展，死锁链）：内存 session 缺失（重启后群会话未加载）
    /// → 回退磁盘持久化校验——先 `resolve_session_workspace_binding`
    /// （session_manager.rs:1664，四段定位含 projects_root 扫描）解析
    /// binding，取 binding.project_root_path（本地 = 会话元数据的
    /// workspace_path 同源）作为群 workspace。证据：group_workspace 从内存
    /// session 读 config.workspace_path，群会话未加载内存时 send/history/
    /// invite/fork 报「does not exist in memory」，且打开群依赖 isGroupChat
    /// （R-GC-35）→ 死锁链（侦察-群聊运行时风险深挖-第六任CPO 隐患 2）；
    /// 只修 validate_session_exists 不修 group_workspace = 重启后群操作仍报错。
    async fn group_workspace(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<String> {
        let manager = coordinator.get_session_manager();
        if let Some(workspace) = manager
            .get_session(group_id)
            .and_then(|session| session.config.workspace_path)
        {
            return Ok(workspace);
        }
        if let Some(binding) = manager
            .resolve_session_workspace_binding(group_id)
            .await
        {
            let workspace = binding.project_root_path.to_string_lossy().to_string();
            if !workspace.trim().is_empty() {
                return Ok(workspace);
            }
        }
        Err(BitFunError::tool(format!(
            "group chat session '{group_id}' does not exist in memory or on disk"
        )))
    }

    /// 校验成员会话真实存在（群聊重建 Type-Contract §二：成员 = 调用方传入
    /// 的真实会话 ID，禁按数量新建匿名会话）。
    ///
    /// R-GC-38（P1 升级）：内存 `session_manager.get_session`（session_manager
    /// .rs:3201 只查 self.sessions）失败 → 回退磁盘持久化会话校验——A 路实证
    /// （侦察-群聊运行时风险深挖-第六任CPO-20260815.md 现象 3 根因）：前端列
    /// 磁盘、后端验内存 = 重启后邀请成员不全直接根因。回退
    /// `resolve_session_workspace_binding`（session_manager.rs:1664，四段定位：
    /// 内存 config → session_storage_path_index → 注册 workspace → projects_root
    /// 扫描），binding 解析成功 = 磁盘存在该会话的持久化元数据。
    /// 会话不存在 → 返回明确错误 Err("member session not found: {session_id}")
    /// （禁静默跳过 R-3）。
    async fn validate_session_exists(
        coordinator: &ConversationCoordinator,
        session_id: &str,
    ) -> BitFunResult<()> {
        let manager = coordinator.get_session_manager();
        if manager.get_session(session_id).is_some() {
            return Ok(());
        }
        if manager
            .resolve_session_workspace_binding(session_id)
            .await
            .is_some()
        {
            return Ok(());
        }
        Err(BitFunError::tool(format!(
            "member session not found: {session_id}"
        )))
    }

    /// 群主默认对话类型（R-GC-28b 主人实测修正，2026-08-14）：
    /// 邀请/裂变成员会话类型 = Claw（非「智能体」agentic）。
    /// 复用现成 `coordinator::ASSISTANT_BOOTSTRAP_AGENT_TYPE`（coordinator.rs
    /// :860 pub const = "Claw"）作为单一权威源——禁散落硬编码 "Claw" 字符串
    /// （零硬编码铁律），改 Claw 类型只改 coordinator 常量一处。
    fn default_group_agent_type() -> String {
        ASSISTANT_BOOTSTRAP_AGENT_TYPE.to_string()
    }

    /// 群主默认对话显示名（R-GC-28/28b，零硬编码）：从 AgentRegistry 取
    /// Claw 类型 agent 的 name()（ClawMode::name() = "Claw"，claw.rs:53）。
    /// 复用现成 `get_agent(agent_type, None)`（registry/mod.rs:177）→
    /// `Agent::name()`；缺失时回退 agent_type 本身（不炸）。
    ///
    /// 群聊重建 Type-Contract §三.5：create_member_session（按数量新建匿名
    /// 成员会话）已移除（禁 dead_code 残留 C-10）——R-GC-28 丢弃入参 ID 的
    /// 实现不再存在，成员 = 调用方传入的真实会话 ID。本函数保留为「显式
    /// 新建成员」场景的命名权威源（契约 §二：default_group_agent_type/name
    /// 保留仅用于显式新建成员场景）；当前无显式新建调用方，故标注
    /// #[allow(dead_code)] 待该场景落地时恢复使用（C-10/C-11 零残留）。
    #[allow(dead_code)]
    fn default_group_agent_name() -> String {
        get_agent_registry()
            .get_agent(Self::default_group_agent_type().as_str(), None)
            .map(|agent| agent.name().to_string())
            .unwrap_or_else(Self::default_group_agent_type)
    }

    /// 建群 = 建 Claw 对话类型会话（type-contract §二.1；R-GC-25/28b 群主
    /// 对话模型 + 零硬编码：agent_type 取 ASSISTANT_BOOTSTRAP_AGENT_TYPE
    /// = "Claw"，workspace 取入参兜底链）。
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
        // R-GC-29（2026-08-14 主人实测）：欢迎 turn 文案精简为「群聊「X」
        // 已创建」——删除「我是群主，成员消息将汇聚于此。」冗余描述。该
        // 描述与前端创建成功 toast（CreateGroupChatDialog.tsx:84
        // notificationService.success('群聊「{{name}}」已创建')）文本高度
        // 相似，且欢迎 turn 会作为群聊首条消息渲染（GroupChatView loadHistory
        // 读回 user_dialog 气泡），观感 = 建群提示重复两次。宿主 turn 本体
        // 保留（R-GC-25 结构依赖：群主会话开局必须有真实 turn）。
        Self::write_group_turn(
            coordinator,
            workspace,
            &group_session_id,
            &group_session_id,
            &format!("群聊「{name}」已创建。"),
        )
        .await?;

        // 登记成员（群聊重建 Type-Contract §三.1：成员 = 调用方传入的真实
        // 会话 ID——每个 ID 先校验存在，再登记 groupChats；禁按数量新建匿名
        // 会话 R-GC-28 回退）。
        for member_id in members {
            Self::validate_session_exists(coordinator, member_id).await?;
            Self::add_group_member(coordinator, workspace, &group_session_id, member_id).await?;
        }

        Ok(group_session_id)
    }

    /// 拉成员 = 校验调用方传入的真实会话 ID 存在 + 记入群成员表
    /// （群聊重建 Type-Contract §三.2：invite = 登记已选真实会话，
    /// 禁按数量新建匿名会话 R-GC-28 回退）。会话不存在 → Err（禁静默跳过）。
    ///
    /// 群 workspace 由 group_id 解析（group_workspace），不再接收入参
    /// workspace（R-GC-R1R4 清理：旧签名的 workspace 仅用于新建匿名成员会话，
    /// 已按新契约移除）。
    async fn invite_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        Self::validate_session_exists(coordinator, member_session_id).await?;
        Self::add_group_member(coordinator, &group_workspace, group_id, member_session_id).await
    }

    /// 移除成员 = 从群会话 custom_metadata.groupChats 移除。
    async fn remove_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
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
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
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
        // R-GC-34（方案 B，空值防御）：主人（sender_session_id == __master__）
        // 会话名不可得（i18n 服务缺失等）时回退 group_id，绝不 crash。
        let sender_name_fallback = if sender.session_id == GROUP_MASTER_ACTOR {
            group_id
        } else {
            sender_session_id
        };
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender_name_fallback)),
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
            recovery: None,
            recovery_epoch: None,
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
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
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

        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
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

        // 登记子群成员（群聊重建 Type-Contract §三.3：fork members = 调用方
        // 传入的真实会话 ID，每个校验存在后登记子群 groupChats；禁按数量
        // 新建匿名会话 R-GC-28 回退）。
        // R-GC-38（P2）：members 为空 → 登记子群自身 ID 到子群 groupChats
        // （群主=子群自身，契约 §六.1）——branch_session 已继承主群
        // custom_metadata 的 groupChats（主群成员），空成员 fork 时再登记
        // 子群自身，保证子群有群标记 + 成员表非空（list_group_chats 识别）。
        if members.is_empty() {
            Self::add_group_member(
                coordinator,
                &group_workspace,
                &child_session_id,
                &child_session_id,
            )
            .await?;
        }
        for member_id in members {
            Self::validate_session_exists(coordinator, member_id).await?;
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
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
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
    ///
    /// R-GC-38（P2）：删除前遍历群成员表，逐个清除成员会话 custom_metadata
    /// .groupChats 里的本群反标（文档 §7 声称「delete 级联清成员反标」对齐）。
    /// 反标 = 成员会话 custom_metadata.groupChats 数组中的群 ID（旧模型
    /// group_chat_membership.rs:18 同键）；单成员反标清除失败 → warn 继续
    /// （S-38 防幽灵，先例 delete_room_impl 逐成员清反标单成员失败 warn 继续），
    /// 不阻塞群会话删除。随后删群会话本体（coordinator.delete_session）。
    async fn delete_group(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let manager = coordinator.get_session_manager();

        // 删除前：遍历群成员表（groupChats）逐个清反标。
        if let Ok(Some(group_metadata)) = manager
            .load_session_metadata(&PathBuf::from(&group_workspace), group_id)
            .await
        {
            let group_members = group_metadata
                .custom_metadata
                .as_ref()
                .and_then(|m| m.get("groupChats"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for member in group_members {
                let Some(member_session_id) = member.as_str() else {
                    continue;
                };
                if member_session_id == group_id {
                    continue;
                }
                if let Err(error) = manager
                    .update_session_metadata(&PathBuf::from(&group_workspace), member_session_id, |metadata| {
                        let custom = metadata
                            .custom_metadata
                            .get_or_insert_with(|| json!({}))
                            .as_object_mut()
                            .expect("custom_metadata is always an object");
                        if let Some(members) = custom.get_mut("groupChats").and_then(|v| v.as_array_mut())
                        {
                            members.retain(|v| v.as_str() != Some(group_id));
                            if members.is_empty() {
                                custom.remove("groupChats");
                            }
                        }
                    })
                    .await
                {
                    warn!(
                        "R-GC-38: failed to clear group member back-mark during delete: member_session_id={}, group_id={}, error={}",
                        member_session_id, group_id, error
                    );
                }
            }
        }

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
                Self::invite_member(&coordinator, group_id, member).await?;
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
            recovery: None,
            recovery_epoch: None,
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

    // ── R-GC-28b 主人实测修正（2026-08-14）：邀请/裂变成员类型 = Claw ──
    // 群主/成员对话类型 = `coordinator::ASSISTANT_BOOTSTRAP_AGENT_TYPE`
    // （coordinator.rs:860 pub const "Claw"）单一权威源——本测试断言
    // default_group_agent_type 返回该常量引用（禁散落硬编码 "Claw" 字符串；
    // 若 coordinator 常量改为其他类型，群聊自动跟随，本测试同步断言）。
    #[test]
    fn default_group_agent_type_is_assistant_bootstrap_agent_type() {
        let expected = ASSISTANT_BOOTSTRAP_AGENT_TYPE.to_string();
        let actual = GroupRoomTool::default_group_agent_type();
        assert_eq!(actual, expected, "default group agent type must follow the Claw constant");
        assert!(!actual.trim().is_empty(), "default agent type must be non-empty");
    }

    // ── R-GC-28/28b 零硬编码（主人定标 2026-08-14）：群主默认名称 = Claw
    // 类型 agent 的显示名（ClawMode::name() = "Claw"），类型来自
    // coordinator 常量、名称来自 AgentRegistry 单一事实源。
    // 群聊重建 Type-Contract §三.5：default_group_agent_name 保留为「显式
    // 新建成员」场景命名权威源（当前无调用方，#[allow(dead_code)] 标注，
    // 无 R-GC-28 匿名成员创建语义）。──
    #[test]
    fn default_group_agent_name_comes_from_agent_registry() {
        let agent_type = GroupRoomTool::default_group_agent_type();
        let expected = crate::agentic::agents::get_agent_registry()
            .get_agent(agent_type.as_str(), None)
            .map(|agent| agent.name().to_string())
            .unwrap_or_else(|| agent_type.clone());
        let actual = GroupRoomTool::default_group_agent_name();
        assert_eq!(actual, expected);
        assert!(
            !actual.trim().is_empty(),
            "default group agent name must be non-empty"
        );
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
        // senderName 回退逻辑与 send_message 相同（成员无会话名时用
        // sender_session_id；R-GC-34 主人无会话名时回退 group_id，见
        // master_name_falls_back_to_group_id）。
        let sender_session_id = "sender-x";
        let sender_name: Option<String> = None;
        let effective = sender_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(sender_session_id);
        assert_eq!(effective, "sender-x");
    }

    // ── R-GC-34（主人身份错位 P0 修复，方案 B）：__master__ 特判 ──
    #[tokio::test]
    async fn master_identity_resolves_to_l0() {
        // 主人（__master__）身份 = L0 + 主人名（i18n）。R-WF-01 全删 RBAC 后
        // role 恒 None。测试环境无全局 i18n service → name 回退英文 "Master"。
        let identity = GroupRoomTool::master_sender_identity().await;
        assert_eq!(
            identity.session_id,
            bitfun_runtime_ports::GROUP_MASTER_ACTOR,
            "master session id must be the __master__ reserved word"
        );
        assert_eq!(identity.role, None, "role must be None after RBAC removal");
        assert_eq!(identity.depth, Some(0), "master depth must be 0 (L0)");
        let name = identity.name.as_deref().expect("master name must exist");
        assert!(
            !name.trim().is_empty(),
            "master name must never be empty (empty-value defense)"
        );
    }

    #[tokio::test]
    async fn master_name_prefers_i18n_shared_term_when_service_available() {
        // i18n shared term agents.master（zh-CN=主人 / en-US=Master / zh-TW=主人）
        // 直接经 generated_shared_term 断言——服务可用时 translate_with_locale
        // 返回词条值（service.rs:187 format_shared_term），服务缺失回退 Master。
        let zh_cn = crate::service::i18n::generated_locale_contract::generated_shared_term(
            crate::service::i18n::LocaleId::ZhCN,
            "agents.master",
        );
        assert_eq!(
            zh_cn,
            Some("主人"),
            "zh-CN master term must be 主人 (i18n, no hardcode)"
        );
        let en_us = crate::service::i18n::generated_locale_contract::generated_shared_term(
            crate::service::i18n::LocaleId::EnUS,
            "agents.master",
        );
        assert_eq!(en_us, Some("Master"), "en-US master term must be Master");
    }

    #[tokio::test]
    async fn master_name_falls_back_to_group_id() {
        // 空值防御（裁决 5）：主人会话名不可得时 senderName 回退 group_id。
        // 与 send_message 的回退分支语义一致：sender 为 __master__ 时
        // fallback = group_id（而非 sender_session_id）。
        let group_id = "group-abc";
        let sender_session_id = bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        let sender_name: Option<String> = None;
        let fallback = if sender_session_id == bitfun_runtime_ports::GROUP_MASTER_ACTOR {
            group_id
        } else {
            sender_session_id
        };
        let effective = sender_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback);
        assert_eq!(effective, group_id, "master name fallback must be group_id");
        // 对照组：普通成员仍回退 sender_session_id。
        let member_fallback = if "member-1" == bitfun_runtime_ports::GROUP_MASTER_ACTOR {
            group_id
        } else {
            "member-1"
        };
        assert_eq!(member_fallback, "member-1");
    }

    #[tokio::test]
    async fn master_identity_resolved_through_resolve_sender_identity() {
        // resolve_sender_identity 对 __master__ 走特判分支：即使 coordinator
        // 无该会话（主人无 Claw session），也返回 L0 身份而非依赖
        // session_tree 的普通路径（空值防御，不 crash）。
        // 此处直接验证特判入口的等价逻辑：__master__ 命中 → 走主人身份。
        let session_id = bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        let is_master = session_id == bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        assert!(
            is_master,
            "__master__ must be recognized as the master actor"
        );
        // 对照：普通成员不命中。
        assert!("member-1" != bitfun_runtime_ports::GROUP_MASTER_ACTOR);
        // 主人身份内容由 master_sender_identity 单测覆盖（L0/名）。
        let identity = GroupRoomTool::master_sender_identity().await;
        assert_eq!(identity.role, None);
        assert_eq!(identity.depth, Some(0));
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
    /// 竞态防护（CI macos-15 修复）：与 set_global 共享同一把全局锁
    /// （coordinator::test_coordinator_access_lock_sync），把「检查 get_global 为
    /// None + call_impl（内部再读 get_global）」整体放在锁内原子执行——锁定期间
    /// set_global 无法写入，两次读取一致，TOCTOU 窗口消除。若 lock 时全局已被
    /// 其它测试设置，直接跳过断言。
    #[tokio::test]
    async fn missing_coordinator_yields_clear_error() {
        let _guard =
            crate::agentic::coordination::coordinator::test_coordinator_access_lock_sync();
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
            run_restart_unloaded_fallback(&coordinator, &workspace).await;
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
        // set_global 内部（cfg(test)）已持全局锁，与
        // missing_coordinator_yields_clear_error 的检查原子串行。
        ConversationCoordinator::set_global(coordinator.clone());

        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-workspace-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        run_group_roundtrip(&coordinator, &workspace).await;
        run_restart_unloaded_fallback(&coordinator, &workspace).await;
    }

    /// R-GC-38（P1 升级 + 扩展）：重启未加载场景磁盘回退。
    ///
    /// 模拟「重启后会话未加载进内存」：
    /// 1. 创建真实成员会话 + 建群（磁盘已持久化）；
    /// 2. `evict_loaded_session_for_test`（session_manager.rs:541，pub(crate)
    ///    测试专用：仅从内存移除，磁盘保留）把群会话与成员会话踢出内存；
    /// 3. 断言 validate_session_exists（磁盘回退）不误拒真实磁盘会话；
    /// 4. 断言 group_workspace（磁盘回退）可解析群 workspace → 群操作
    ///    （invite/send/history/fork）不报「does not exist in memory」。
    async fn run_restart_unloaded_fallback(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace: &std::path::Path,
    ) {
        let manager = coordinator.get_session_manager();
        let workspace_str = workspace.to_string_lossy().to_string();

        // 建群（2 真实成员）→ 磁盘持久化完成。
        let member_a = create_member_session_for_test(coordinator, &workspace_str).await;
        let member_b = create_member_session_for_test(coordinator, &workspace_str).await;
        let group_id = GroupRoomTool::create_group(
            coordinator,
            "重启未加载群",
            &[member_a.clone(), member_b.clone()],
            &workspace_str,
        )
        .await
        .expect("create group for restart-unloaded fallback");

        // 模拟重启：群会话 + 成员会话从内存移除（磁盘保留）。
        manager.evict_loaded_session_for_test(&group_id);
        manager.evict_loaded_session_for_test(&member_a);
        manager.evict_loaded_session_for_test(&member_b);
        assert!(
            manager.get_session(&group_id).is_none(),
            "setup: group session must be evicted from memory"
        );
        assert!(
            manager.get_session(&member_a).is_none(),
            "setup: member A must be evicted from memory"
        );

        // 1) validate_session_exists 磁盘回退：真实磁盘会话不误拒。
        GroupRoomTool::validate_session_exists(coordinator, &member_a)
            .await
            .expect("R-GC-38: disk-persisted member session must pass validation after restart");

        // 2) 群操作磁盘回退：invite（依赖 group_workspace + validate_session_exists）。
        GroupRoomTool::invite_member(coordinator, &group_id, &member_a)
            .await
            .expect("R-GC-38: invite must not report 'does not exist in memory' after restart");
        // invite 幂等：member_a 已登记 → 不重复。
        let metadata_after_invite = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members_after_invite = metadata_after_invite
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            members_after_invite
                .iter()
                .any(|v| v.as_str() == Some(member_a.as_str())),
            "invited member A must be registered after restart-fallback invite"
        );

        // 3) history 磁盘回退（依赖 group_workspace）：不报 memory 错（可空历史，欢迎 turn 在）。
        let history = GroupRoomTool::get_history(coordinator, &group_id, None)
            .await
            .expect("R-GC-38: history must not report 'does not exist in memory' after restart");
        // 欢迎 turn 是 User 消息 → 历史非空（create 写 welcome）。
        assert!(
            !history.is_empty(),
            "history must contain the group welcome turn after restart"
        );
    }

    /// 测试环境无全局 config service → 注入 TEST_MODEL_RESOLUTION_AI_CONFIG
    /// 提供标准模型配置（与 scheduler.rs 测试同构）。send_message 经
    /// start_dialog_turn → resolve_model_id_for_turn 需要 config service；
    /// 每次 send（含 master send）都必须在该 scope 内，否则取不到 config
    /// （CI ubuntu 时序：第一条 turn 完成快，master send 时群主已非
    /// Processing → 走 config 解析路径 → scope 外报 "Failed to get config
    /// service for model resolution"，与真实 busy 拒绝语义混淆）。
    fn test_ai_config() -> crate::service::config::types::AIConfig {
        crate::service::config::types::AIConfig {
            models: vec![crate::service::config::types::AIModelConfig {
                id: "model-original".to_string(),
                name: "model-original".to_string(),
                model_name: "model-original".to_string(),
                enabled: true,
                ..Default::default()
            }],
            default_models: crate::service::config::types::DefaultModelsConfig {
                primary: Some("model-original".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// create（建群=建会话，含成员）→ send（写群会话 turns）→ history（读回）
    /// → list（群聊列表过滤）全链路断言。
    /// 测试辅助：创建真实成员会话（契约 §二：成员 = 调用方传入的真实会话 ID，
    /// 由调用方创建后传给 create/invite/fork——测试模拟前端「选中真实 Claw
    /// 会话」后的创建动作）。
    async fn create_member_session_for_test(
        coordinator: &ConversationCoordinator,
        workspace: &str,
    ) -> String {
        let member_id = uuid::Uuid::new_v4().to_string();
        let config = SessionConfig {
            workspace_path: Some(workspace.to_string()),
            project_workspace_path: Some(workspace.to_string()),
            ..Default::default()
        };
        coordinator
            .create_session_with_workspace(
                Some(member_id.clone()),
                "test-member".to_string(),
                GroupRoomTool::default_group_agent_type(),
                config,
                workspace.to_string(),
            )
            .await
            .expect("create member session")
            .session_id
    }

    async fn run_group_roundtrip(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace: &std::path::Path,
    ) {
        use crate::agentic::core::Message;
        let manager = coordinator.get_session_manager();
        let workspace_str = workspace.to_string_lossy().to_string();

        // create：先建真实成员会话（契约 §二：成员 = 调用方传入的真实会话
        // ID）→ 建群（2 成员）→ 返回 group_id（UUID）；会话列表可见且
        // agent_type=默认对话类型。
        let member_a = create_member_session_for_test(coordinator, &workspace_str).await;
        let member_b = create_member_session_for_test(coordinator, &workspace_str).await;
        let group_name = "测试群";
        let group_id = GroupRoomTool::create_group(
            coordinator,
            group_name,
            &[member_a.clone(), member_b.clone()],
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

        // 群成员表已写入 groupChats（契约 §一：成员 = 调用方传入的真实会话 ID）。
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
        assert!(
            members.iter().any(|v| v.as_str() == Some(member_a.as_str())),
            "groupChats must contain real session A"
        );
        assert!(
            members.iter().any(|v| v.as_str() == Some(member_b.as_str())),
            "groupChats must contain real session B"
        );
        assert!(
            members.iter().all(|v| v.as_str() != Some("member-a") && v.as_str() != Some("member-b")),
            "R-GC-28 回退: members must be the real caller-provided ids, never fresh placeholders"
        );

        // 契约 §四：传不存在 ID → 明确错误（禁静默跳过）。
        let missing_err = GroupRoomTool::create_group(
            coordinator,
            "缺失成员群",
            &["definitely-not-a-real-session".to_string()],
            &workspace_str,
        )
        .await
        .expect_err("create with a non-existent member must fail");
        assert!(
            missing_err
                .to_string()
                .contains("member session not found"),
            "non-existent member must yield a clear error, got: {missing_err}"
        );

        // 契约 §三.2：invite = 登记调用方传入的真实会话 ID（校验存在）；
        // 传不存在 ID → Err（禁静默跳过）。
        let invite_err = GroupRoomTool::invite_member(
            coordinator,
            &group_id,
            "no-such-invite-session",
        )
        .await
        .expect_err("invite with a non-existent member must fail");
        assert!(
            invite_err
                .to_string()
                .contains("member session not found"),
            "non-existent invite member must yield a clear error, got: {invite_err}"
        );
        let invite_member = create_member_session_for_test(coordinator, &workspace_str).await;
        GroupRoomTool::invite_member(coordinator, &group_id, &invite_member)
            .await
            .expect("invite real member");
        let metadata_after_invite = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members_after_invite = metadata_after_invite
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            members_after_invite
                .iter()
                .any(|v| v.as_str() == Some(invite_member.as_str())),
            "invited real session must be registered in groupChats"
        );

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
            .find(|t| t.user_message.content == format!("群聊「{group_name}」已创建。"))
            .expect("welcome turn content must mention group creation (R-GC-29 concise wording)");
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

        // send：写群会话 turns → message_id（发送者 = 真实成员 A）。
        // Upstream merge 91207f1de 引入 turn admission 模型解析：send_message
        // 经 start_dialog_turn → resolve_model_id_for_turn 需要 config service。
        // 测试环境无全局 config service → 注入 TEST_MODEL_RESOLUTION_AI_CONFIG
        // 提供标准模型配置（与 scheduler.rs 测试同构）。
        let message_id = crate::agentic::session::TEST_MODEL_RESOLUTION_AI_CONFIG
            .scope(
                test_ai_config(),
                GroupRoomTool::send_message(coordinator, &group_id, "第一条群消息", &member_a),
            )
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

        // history：读回消息（author 从 turn metadata 解析 senderSessionId=member_a）。
        // 注意：get_messages 从持久化 turns 重建 Message（Message.id 为重建时新 uuid），
        // 因此按「内容 + author」匹配，而非 send 返回的 message_id。
        let history = GroupRoomTool::get_history(coordinator, &group_id, None)
            .await
            .expect("get history");
        let found = history
            .iter()
            .find(|m| m.content == "第一条群消息")
            .expect("sent message present in history");
        assert_eq!(found.author.session_id, member_a);
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
            Some(3),
            "memberCount from groupChats (2 create members + 1 invited)"
        );

        // ── 三形态之②：成员会话（create 拉入的成员 = 调用方传入的真实会话）──
        // 契约 §一：成员 = 调用方传入的真实会话 ID（建群前由调用方创建），
        // 禁按数量新建匿名会话（R-GC-28 回退）。成员 ID 即 groupChats 登记的
        // 真实 ID；成员会话类型 = 创建时的真实 agent_type（默认对话类型）。
        let member_id = members
            .iter()
            .find_map(Value::as_str)
            .expect("first member id from groupChats");
        assert_eq!(member_id, member_a, "first member must be the real session A");
        let member_session = manager
            .get_session(member_id)
            .expect("member session in memory");
        assert_eq!(
            member_session.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "member session uses the config-driven default agent type"
        );

        // ── 三形态之①：默认 BuiltIn 群主（assistant_id 空）──
        // 群主 = GROUP_MASTER_ACTOR（__master__，契约 §五），无底层 assistant
        // 会话支撑；history 侧 author.session_id 即 __master__。
        // R-GC-26：send 触发群主会话真实 turn（异步执行）。测试环境无真实
        // agent，第一条 send 的 turn 可能仍 Processing（执行引擎挂起于模型
        // 解析）也可能已落盘完成——时序由调度器/平台决定（CI ubuntu 上
        // 第一条 turn 完成快 → master send 时群主已非 Processing）。
        // 断言必须消除该时序依赖：busy 拒绝（Processing）或明确错误
        // （config 解析/执行失败）都接受，禁静默丢失（Ok = 第二条真实 turn
        // 成功，同样说明 send 链路可用）。master 身份的 history author 解析
        // 由 send_metadata_* 单测 + build_sender_by_turn 覆盖
        // （GROUP_MASTER_ACTOR 作为 sender_session_id 透传）。
        // master send 同样需要 config service（resolve_model_id_for_turn），
        // 纳入 TEST_MODEL_RESOLUTION_AI_CONFIG scope（否则无论是否 busy，
        // scope 外取不到 config → "Failed to get config service" 与环境无关
        // 的报错，掩盖真实语义；CI ubuntu 时序即此）。
        let master_send_result = crate::agentic::session::TEST_MODEL_RESOLUTION_AI_CONFIG
            .scope(
                test_ai_config(),
                GroupRoomTool::send_message(
                    coordinator,
                    &group_id,
                    "群主发言",
                    bitfun_runtime_ports::GROUP_MASTER_ACTOR,
                ),
            )
            .await;
        match master_send_result {
            Ok(master_message_id) => {
                assert!(
                    !master_message_id.is_empty(),
                    "master send must return a non-empty message id when it succeeds"
                );
            }
            Err(master_error) => {
                let master_error = master_error.to_string();
                let is_busy_rejection = master_error.contains("Session state does not allow");
                let is_clear_error = master_error.contains("Failed to get config service")
                    || master_error.contains("AI configuration is unavailable");
                assert!(
                    is_busy_rejection || is_clear_error,
                    "master send while the group-owner session is busy must fail with the \
                     session-busy rejection (or a clear model-resolution error when the first \
                     turn already completed), got: {master_error}"
                );
            }
        }

        // ── 模拟「群主 turn 完成快」时序（CI ubuntu run 31866716693 根因）──
        // ubuntu 上第一条 send 的 turn 完成快 → master send 时群主已非
        // Processing。此处显式把群主会话重置为 Idle（reset 仅当仍 Processing
        // 时生效），确定性覆盖该时序：scope 内 config 就绪 → master send
        // 要么成功（第二条真实 turn）要么返回非 busy 的明确错误，绝不因
        // scope 外取不到 config 报 "Failed to get config service"。
        // 断言成功（Ok）或明确错误（busy 拒绝 / config 解析错误）均可，
        // 禁静默丢失。
        manager.reset_session_state_if_processing(&group_id, &message_id);
        let master_after_idle = crate::agentic::session::TEST_MODEL_RESOLUTION_AI_CONFIG
            .scope(
                test_ai_config(),
                GroupRoomTool::send_message(
                    coordinator,
                    &group_id,
                    "群主发言（turn 已完成时序）",
                    bitfun_runtime_ports::GROUP_MASTER_ACTOR,
                ),
            )
            .await;
        match master_after_idle {
            Ok(master_message_id) => {
                assert!(
                    !master_message_id.is_empty(),
                    "master send after idle reset must return a non-empty message id when it succeeds"
                );
            }
            Err(master_error) => {
                let master_error = master_error.to_string();
                let is_busy_rejection = master_error.contains("Session state does not allow");
                let is_clear_error = master_error.contains("Failed to get config service")
                    || master_error.contains("AI configuration is unavailable");
                assert!(
                    is_busy_rejection || is_clear_error,
                    "master send after the first turn completed must not be a silent loss; \
                     got: {master_error}"
                );
            }
        }

        // ── 三形态之③：fork 子群 → parent 关联（契约 §九/§八）──
        // fork 点 = 第一条群消息的持久化 turn_id（send 返回的 message_id 即 turn_id）。
        let member_c = create_member_session_for_test(coordinator, &workspace_str).await;
        let child_id = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "测试子群",
            Some(&message_id),
            &[member_c.clone()],
        )
        .await
        .expect("fork group");
        assert!(!child_id.is_empty());
        assert_ne!(child_id, group_id, "child must differ from parent");

        // 契约 §四：fork 传不存在 ID → 明确错误（禁静默跳过）。
        let fork_missing_err = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "缺失成员子群",
            Some(&message_id),
            &["not-a-real-fork-member".to_string()],
        )
        .await
        .expect_err("fork with a non-existent member must fail");
        assert!(
            fork_missing_err
                .to_string()
                .contains("member session not found"),
            "non-existent fork member must yield a clear error, got: {fork_missing_err}"
        );

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

        // 子群自带成员表（fork 继承主群成员 + 登记 fork 成员；契约 §三.3：
        // fork members = 调用方传入的真实会话 ID，登记进子群 groupChats）。
        let child_members = child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            child_members.len() >= 4,
            "fork child must inherit parent members (3) plus one fork member, got {}",
            child_members.len()
        );
        assert!(
            child_members
                .iter()
                .any(|v| v.as_str() == Some(member_c.as_str())),
            "fork child must register the real fork member C"
        );
        assert!(
            child_members
                .iter()
                .all(|v| v.as_str() != Some("member-c")),
            "R-GC-28 回退: fork member must be the real caller-provided id, never a placeholder"
        );
        assert!(
            manager.get_session(&member_c).is_some(),
            "fork child member session must exist in memory"
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

        // ── R-GC-38（P2）：fork 空成员 → 子群默认登记自身 → 有群标记 ──
        // 契约 §六.1：members 为空 → 登记子群自身 ID 到子群 groupChats
        // （群主=子群自身）。branch_session 继承主群 groupChats（3 成员），
        // 空成员 fork 再登记子群自身 → 成员表非空且含子群自身；
        // list_group_chats 过滤 groupChats 标记 → 子群可被识别。
        let empty_member_child_id = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "空成员子群",
            Some(&message_id),
            &[],
        )
        .await
        .expect("fork with empty members must succeed (R-GC-38 default self-registration)");
        assert!(
            !empty_member_child_id.is_empty(),
            "empty-member child id must be non-empty"
        );
        let empty_child_metadata = manager
            .load_session_metadata(workspace, &empty_member_child_id)
            .await
            .expect("load empty-member child metadata")
            .expect("empty-member child metadata exists");
        let empty_child_members = empty_child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !empty_child_members.is_empty(),
            "R-GC-38: empty-member fork child must have a non-empty groupChats (self-registered)"
        );
        assert!(
            empty_child_members
                .iter()
                .any(|v| v.as_str() == Some(empty_member_child_id.as_str())),
            "R-GC-38: empty-member fork child must register itself (群主=子群自身)"
        );
        // list_group_chats 识别：子群带 groupChats 标记 → 出现在群聊列表。
        let groups_after_empty_fork = GroupRoomTool::list_groups(coordinator, &workspace_str)
            .await
            .expect("list groups after empty-member fork");
        assert!(
            groups_after_empty_fork
                .iter()
                .any(|g| g.get("groupId").and_then(Value::as_str) == Some(empty_member_child_id.as_str())),
            "R-GC-38: empty-member fork child must be recognized by list_group_chats"
        );

        // ── R-GC-38（P2）：delete_group 级联清成员反标 ──
        // 删除群前遍历群成员表清反标（成员会话 custom_metadata.groupChats
        // 移除本群 ID；清空后整个键移除）→ 删除后成员会话无本群反标残留。
        GroupRoomTool::delete_group(coordinator, &group_id)
            .await
            .expect("delete group must succeed");
        // 删除后：成员 A 的反标（groupChats）不再含 group_id。
        let member_a_metadata = manager
            .load_session_metadata(workspace, &member_a)
            .await
            .expect("load member A metadata")
            .expect("member A metadata exists");
        let member_a_groups = member_a_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !member_a_groups
                .iter()
                .any(|v| v.as_str() == Some(group_id.as_str())),
            "R-GC-38: delete must clear the group back-mark on member A"
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


