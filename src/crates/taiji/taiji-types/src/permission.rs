//! 权限类型 — 护山大阵门规与门令体系。
//!
//! 参考源：
//! - PermissionMode → agentscope permission/_types.py:18-35
//! - PermissionBehavior → agentscope permission/_types.py:33-38 + _decision.py:10-68
//! - PermissionRule → agentscope permission/_rule.py:8-36 + modules/harness/实现参考.py:46-57
//! - PermissionDecision → agentscope permission/_decision.py:10-68 (GateCommand)
//! - ResourceQuota → gbrain/src/mcp/rate-limit.ts:17-24 RateLimitOpts

use serde::{Deserialize, Serialize};

// ── 权限模式（护山大阵戒备等级） ──

/// 权限模式 — 护山大阵当前戒备等级。
///
/// 参考 agentscope PermissionMode（_types.py:18-35）。
/// 五种模式对应不同运行场景的安全策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionMode {
    /// 默认模式 — 每步操作需权限裁决。
    #[serde(rename = "default")]
    Default,
    /// 接受编辑 — 工作目录内自动允许写操作。
    #[serde(rename = "accept_edits")]
    AcceptEdits,
    /// 探索模式 — 只读，禁止修改。
    #[serde(rename = "explore")]
    Explore,
    /// 绕过模式 — 跳过安全检查（沙箱/容器）。
    #[serde(rename = "bypass")]
    Bypass,
    /// 不问模式 — 所有 ASK 转为 DENY（无人值守）。
    #[serde(rename = "dont_ask")]
    DontAsk,
}

// ── 权限行为（门令三态 + PASSTHROUGH） ──

/// 权限行为 — 护山大阵对操作请求的裁决结果。
///
/// 参考 agentscope PermissionBehavior（_types.py:33-38）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionBehavior {
    /// 允许。
    #[serde(rename = "allow")]
    Allow,
    /// 拒绝。
    #[serde(rename = "deny")]
    Deny,
    /// 询问用户。
    #[serde(rename = "ask")]
    Ask,
    /// 透传 — 交由引擎继续匹配后续规则。
    #[serde(rename = "passthrough")]
    Passthrough,
}

// ── 权限规则（门规） ──

/// 权限规则条目 — 对某工具的操作约束。
///
/// 参考 agentscope PermissionRule（_rule.py:8-36）。
///
/// rule_content 的匹配语义取决于 tool_name：
/// - Bash: 子串匹配命令
/// - Write/Read: glob 匹配文件路径
/// - 其他工具: 通用匹配逻辑
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub tool_name: String,
    pub rule_content: Option<String>,
    pub behavior: PermissionBehavior,
    pub source: String,
}

// ── 权限决策（门令） ──

/// 权限决策 — 护山大阵对一次工具调用的完整裁决。
///
/// 参考 agentscope PermissionDecision（_decision.py:10-68）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub behavior: PermissionBehavior,
    pub message: String,
    pub decision_reason: Option<String>,
    pub bypass_immune: bool,
}

// ── 权限动作 ──

/// 权限动作类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "read")]
    Read(String),
    #[serde(rename = "write")]
    Write(String),
    #[serde(rename = "execute")]
    Execute(String),
    #[serde(rename = "admin")]
    Admin(String),
}

// ── 资源配额 ──

/// 资源配额 — Agent 可消耗的计算资源上限。
///
/// 参考 gbrain RateLimitOpts（rate-limit.ts:17-24）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// 每秒请求数上限。
    pub max_rps: u32,
    /// 最大并发数。
    pub max_concurrency: u32,
    /// Token 预算。
    pub token_budget: u64,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_rps: 100,
            max_concurrency: 10,
            token_budget: 1_000_000,
        }
    }
}

// ── 门令（护山大阵裁决结果） ──

/// 门令 — 护山大阵对工具调用的裁决结果。
///
/// 对应 agentscope PermissionBehavior 的 Allow/Deny/Ask 三态，
/// 移除 Passthrough（由调用方自行处理透传）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GateCommand {
    /// 允许。
    #[serde(rename = "allow")]
    Allow,
    /// 拒绝。
    #[serde(rename = "deny")]
    Deny,
    /// 询问用户 — 附带建议规则。
    #[serde(rename = "ask")]
    Ask {
        suggested_rules: Vec<PermissionRule>,
    },
}

/// 戒备等级 — 护山大阵当前运行模式。
///
/// 复用 PermissionMode 语义，类型别名保持命名空间清晰。
pub type GuardLevel = PermissionMode;

/// 权限等级 — 荣誉称号/境界赋予的操作权限层级。
///
/// 参考：架构总纲 §5.2 — 称号特权管理
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// 基础级 — 默认权限，读写搜三件套。
    #[serde(rename = "basic")]
    Basic,
    /// 标准级 — 数据库查询等。
    #[serde(rename = "standard")]
    Standard,
    /// 进阶级 — 实时行情、缠论中枢等。
    #[serde(rename = "advanced")]
    Advanced,
    /// 专家级 — 天机推演、多 Agent 协作等。
    #[serde(rename = "expert")]
    Expert,
    /// 管理员级 — 全栈操作。
    #[serde(rename = "admin")]
    Admin,
}
