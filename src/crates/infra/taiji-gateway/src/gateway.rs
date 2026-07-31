//! 接引台 — Gateway trait + 认证类型。
//!
//! 设计参考：modules/gateway/接口设计.md §1-5（v2.4 职责澄清：只认证不授权）

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use taiji_types::agent::AgentId;

use crate::error::GatewayResult;

// ============================================================================
// 认证类型
// ============================================================================

/// 认证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthType {
    /// API 密钥验证。
    ApiKey,
    /// JWT 令牌验证。
    Jwt,
    /// Nostr 密钥对验证。
    Nostr,
}

/// 认证请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// 认证方式。
    pub auth_type: AuthType,
    /// 凭据（不同 auth_type 使用不同字段结构）。
    pub credentials: serde_json::Value,
}

/// 认证响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// 分配的 Agent ID。
    pub agent_id: AgentId,
    /// 会话 ID。
    pub session_id: String,
    /// 会话过期时间。
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// 认证上下文 — 包含认证方式、原始凭据和来源信息。
///
/// 用于租户解析（`TenantAwareGateway::resolve_tenant`）和
/// 资源级访问控制的完整认证信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// 认证方式。
    pub auth_type: AuthType,
    /// 原始凭据。
    pub credentials: serde_json::Value,
    /// 请求来源标识（IP/域名/客户端标识）。
    pub source: Option<String>,
}

impl From<AuthRequest> for AuthContext {
    fn from(req: AuthRequest) -> Self {
        Self {
            auth_type: req.auth_type,
            credentials: req.credentials,
            source: None,
        }
    }
}

// ============================================================================
// Gateway trait
// ============================================================================

/// 接引台核心 trait — 外部认证入口。
///
/// 负责：
/// - 外部身份认证（你是谁？）
/// - 会话创建、验证、销毁
///
/// 不负责：
/// - 内部权限判断（由 harness 负责）
/// - 权限数据存储（来自 permission-system）
#[async_trait]
pub trait Gateway: Send + Sync {
    /// 认证外部请求，返回 AgentId + session。
    async fn authenticate(&self, request: AuthRequest) -> GatewayResult<AuthResponse>;

    /// 验证 session 是否有效（供 harness 调用）。
    async fn validate_session(&self, agent_id: &AgentId) -> GatewayResult<bool>;

    /// 销毁 session（登出/超时）。
    async fn invalidate_session(&self, agent_id: &AgentId) -> GatewayResult<()>;
}
