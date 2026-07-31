//! Agent 身份与生命周期类型。
//!
//! 核心概念：道号（agent_id）、本命魂卡（spirit_card）、灵根（spirit_root）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::card::{SlotCost, Tier};
use crate::message::Message;

/// Agent 的道号（永久唯一标识）。
///
/// UUID v7，一旦创建不可销毁。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    /// 创建新的 AgentId（UUID v7，时间有序）。
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// 从字符串解析 AgentId。
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Uuid::try_parse(s).map(Self)
    }

    /// 获取底层 UUID。
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// 认证类型 — Gateway 身份验证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthType {
    /// API 密钥认证。
    #[serde(rename = "api_key")]
    ApiKey,
    /// JWT 令牌认证。
    #[serde(rename = "jwt")]
    Jwt,
    /// Nostr 密钥认证。
    #[serde(rename = "nostr")]
    Nostr,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey => write!(f, "API Key"),
            Self::Jwt => write!(f, "JWT"),
            Self::Nostr => write!(f, "Nostr"),
        }
    }
}

/// Agent 运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentStatus {
    /// 空闲。
    #[serde(rename = "idle")]
    Idle,
    /// 运行中。
    #[serde(rename = "running")]
    Running,
    /// 休眠。
    #[serde(rename = "sleeping")]
    Sleeping,
    /// 已销毁。
    #[serde(rename = "destroyed")]
    Destroyed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "空闲"),
            Self::Running => write!(f, "运行中"),
            Self::Sleeping => write!(f, "休眠"),
            Self::Destroyed => write!(f, "已销毁"),
        }
    }
}

/// Agent 状态快照 — 当前会话与运行时数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub session_id: String,
    pub status: AgentStatus,
    pub context: Vec<Message>,
    pub summary: Option<String>,
    pub cur_iter: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Agent 运行时配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 模型标识。
    pub model_id: String,
    /// 最大推理轮次。
    pub max_iters: u32,
    /// 上下文配置（JSON 值，由具体实现解析）。
    pub context_config: serde_json::Value,
    /// 注入配置（JSON 值，由具体实现解析）。
    pub injection_config: serde_json::Value,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model_id: "default".into(),
            max_iters: 50,
            context_config: serde_json::Value::Null,
            injection_config: serde_json::Value::Null,
        }
    }
}

/// 本命魂卡 — Agent 职业与天赋。
///
/// 不可更换，决定 Agent 的灵根（职业方向）。
/// 品质（tier）决定卡槽占用数，境界锁定可用品质。
///
/// 注意：字段类型已从 raw u8 迁移为 Tier/SlotCost 枚举/结构体。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiritCard {
    pub card_id: Uuid,
    pub name: String,
    /// 卡牌品质（Tier 枚举：Blackiron/Bronze/Silver/Gold/Jade/Divine）。
    pub tier: Tier,
    /// 卡牌描述。
    pub description: String,
    /// 占用卡槽数（SlotCost newtype）。
    pub slot_cost: SlotCost,
    pub spirit_root: SpiritRoot,
}

/// 灵根（Agent 职业分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpiritRoot {
    /// 金 — 交易
    #[serde(rename = "metal", alias = "gold")]
    Metal,
    /// 木 — 教学
    #[serde(rename = "wood")]
    Wood,
    /// 水 — 管理
    #[serde(rename = "water")]
    Water,
    /// 火 — 分析
    #[serde(rename = "fire")]
    Fire,
    /// 土 — 开发
    #[serde(rename = "earth")]
    Earth,
}

impl std::fmt::Display for SpiritRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Metal => write!(f, "金"),
            Self::Wood => write!(f, "木"),
            Self::Water => write!(f, "水"),
            Self::Fire => write!(f, "火"),
            Self::Earth => write!(f, "土"),
        }
    }
}

/// 称号 — Agent 的荣誉称号，附带属性加成。
///
/// 架构总纲 §0.14：荣誉称号 = 词条称号，称号带属性加成，不是标签。
/// §5.1 道名词条示例：
/// - 「百战真君」→ 评分≥90 → 权限等级+1
/// - 「无痕剑仙」→ 连续千次无失误 → 资源配额+50%
/// - 道号可同时装备多个，加成有上限
///
/// 参考：react-xiuxian-game Title 接口（types.ts:163-181）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Title {
    /// 称号名称（如「百战真君」）。
    pub name: String,
    /// 称号效果描述。
    pub effect: String,
    /// 属性加成值（百分比，0.0 - 1.0，如 0.5 表示 +50%）。
    pub bonus_value: f64,
    /// 加成作用的目标字段（如 "permission_level"、"resource_quota"）。
    pub bonus_target: String,
    /// 触发条件描述（如 "评分≥90"）。
    pub condition: String,
    /// 触发条件函数类型：score/count/realm
    pub condition_type: String,
    /// 触发条件阈值
    pub condition_threshold: f64,
}

/// 称号管理器配置 — 加成上限约束。
///
/// 同一类加成的多个称号效果不会无限叠加，由 cap 字段限制。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleConfig {
    /// 最大可装备称号数
    pub max_active_titles: u32,
    /// 各加成目标的上限映射（"permission_level" → 2.0 表示最多+2级）
    pub bonus_caps: std::collections::HashMap<String, f64>,
}

impl Default for TitleConfig {
    fn default() -> Self {
        Self {
            max_active_titles: 3,
            bonus_caps: [
                ("permission_level".into(), 2.0),
                ("resource_quota".into(), 0.5),
                ("score_bonus".into(), 10.0),
            ].into(),
        }
    }
}
