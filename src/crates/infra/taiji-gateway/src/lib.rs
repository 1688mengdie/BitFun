//! taiji-gateway — 接引台（认证网关 + 租户隔离 + ACP/MCP 协议）。
//!
//! 负责：
//! - 外部身份认证（ApiKey/Jwt/Nostr）
//! - 会话管理（创建/验证/销毁）
//! - 多租户/多用户隔离（TenantContext + TenantGate）
//! - ACP 协议（Agent Communication Protocol，JSON-RPC 2.0 over stdio）
//! - MCP 协议骨架（Model Context Protocol，待完整集成）

pub mod acp;
pub mod auth;
pub mod error;
pub mod gateway;
pub mod mcp;
pub mod tenant;

pub use acp::{AcpClient, AcpServer, AcpToolBridge, StopReason};
pub use auth::{ApiKeyAuth, GatewayRuntime, JwtAuth, NostrAuth, Session};
pub use error::{GatewayError, GatewayResult};
pub use gateway::{AuthContext, AuthRequest, AuthResponse, AuthType, Gateway};
pub use mcp::{McpClient, McpToolDefinition, McpResourceDefinition};
pub use tenant::{
    DefaultTenantResolver, IsolationLevel, TenantAwareGateway, TenantContext, TenantGate,
};
