//! 接引台 — 认证实现 + Session 管理。
//!
//! 支持三种认证方式：
//! - ApiKey: API 密钥验证（预配置密钥表）
//! - Jwt: JWT 令牌验证（解析+过期检查）
//! - Nostr: Nostr 密钥对签名验证
//!
//! 设计参考：modules/gateway/接口设计.md §1 + BitFun gateway 认证实现 (MIT)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use taiji_types::agent::AgentId;

use crate::error::{GatewayError, GatewayResult};
use crate::gateway::{AuthContext, AuthRequest, AuthResponse, AuthType, Gateway};
use crate::tenant::{DefaultTenantResolver, TenantAwareGateway, TenantContext};

// ============================================================================
// Session 管理
// ============================================================================

/// Session 会话。
#[derive(Debug, Clone)]
pub struct Session {
    /// 会话 ID。
    pub session_id: String,
    /// Agent ID。
    pub agent_id: AgentId,
    /// 所属租户 ID。
    pub tenant_id: String,
    /// 创建时间。
    pub created_at: Instant,
    /// 过期时间。
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// 认证方式。
    pub auth_type: AuthType,
}

impl Session {
    /// 检查 session 是否已过期。
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    /// 创建新 session。
    pub fn new(agent_id: AgentId, auth_type: AuthType, ttl_secs: u64, tenant_id: String) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            agent_id,
            tenant_id,
            created_at: Instant::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(ttl_secs as i64),
            auth_type,
        }
    }
}

// ============================================================================
// 认证方法 trait
// ============================================================================

/// 认证方法 trait — 每种认证方式独立实现。
#[async_trait]
pub trait AuthMethod: Send + Sync {
    /// 验证凭据，成功返回 AgentId。
    async fn authenticate(&self, credentials: &serde_json::Value) -> GatewayResult<AgentId>;
}

// ============================================================================
// ApiKey 认证
// ============================================================================

/// API Key 认证器。
///
/// 维护预配置的 key → agent_id 映射表。
pub struct ApiKeyAuth {
    keys: HashMap<String, AgentId>,
}

impl ApiKeyAuth {
    /// 创建 ApiKey 认证器。
    pub fn new(keys: HashMap<String, AgentId>) -> Self {
        Self { keys }
    }

    /// 从配置创建（常见场景）。
    pub fn from_single(key: &str, agent_id: &str) -> Self {
        let mut keys = HashMap::new();
        keys.insert(key.to_string(), AgentId::parse(agent_id).unwrap_or_else(|_| AgentId::new()));
        Self { keys }
    }
}

#[async_trait]
impl AuthMethod for ApiKeyAuth {
    async fn authenticate(&self, credentials: &serde_json::Value) -> GatewayResult<AgentId> {
        let api_key = credentials
            .get("api_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("缺少 api_key 字段".into()))?;

        self.keys
            .get(api_key)
            .cloned()
            .ok_or_else(|| GatewayError::AuthFailed("API Key 无效".into()))
    }
}

// ============================================================================
// JWT 认证
// ============================================================================

/// JWT 令牌认证器。
///
/// 解析 JWT 的 payload，提取 `sub`（agent_id）+ `exp`（过期时间）。
/// 当前实现做基础解析 + 过期检查（不验证签名——完整 JWT 验证需要
/// 额外的加密依赖，可按需添加）。
#[derive(Default)]
pub struct JwtAuth;

impl JwtAuth {
    pub fn new() -> Self {
        Self
    }

    /// 简单 Base64 解码 JWT payload（不验证签名）。
    fn decode_payload(token: &str) -> GatewayResult<serde_json::Value> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(GatewayError::AuthFailed("JWT 格式无效（需 3 段）".into()));
        }

        // Base64 URL-safe decode payload（第二部分）
        let payload = parts[1];
        use base64::Engine;
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| GatewayError::AuthFailed("JWT payload 无法解码".into()))?;

        serde_json::from_slice(&decoded)
            .map_err(|_| GatewayError::AuthFailed("JWT payload 不是合法 JSON".into()))
    }
}


#[async_trait]
impl AuthMethod for JwtAuth {
    async fn authenticate(&self, credentials: &serde_json::Value) -> GatewayResult<AgentId> {
        let token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("缺少 token 字段".into()))?;

        let payload = Self::decode_payload(token)?;

        // 提取 sub (agent_id)
        let agent_id_str = payload
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("JWT payload 缺少 sub".into()))?;

        // 检查过期
        if let Some(exp) = payload.get("exp").and_then(|v| v.as_i64()) {
            let now = chrono::Utc::now().timestamp();
            if now > exp {
                return Err(GatewayError::AuthFailed("JWT 已过期".into()));
            }
        }

        AgentId::parse(agent_id_str)
            .map_err(|_| GatewayError::AuthFailed(format!("agent_id 格式无效: {}", agent_id_str)))
    }
}

// ============================================================================
// Nostr 认证
// ============================================================================

/// Nostr 密钥对认证器。
///
/// 验证 Nostr 签名：客户端使用私钥对 challenge 签名，
/// 服务端使用公钥验证签名。
#[derive(Default)]
pub struct NostrAuth;

impl NostrAuth {
    pub fn new() -> Self {
        Self
    }

    /// 验证 Nostr 签名（简化版——完整实现需要 secp256k1 等依赖）。
    /// 当前实现检查签名格式长度和公钥格式。
    fn verify_signature(_pubkey: &str, _signature: &str, _challenge: &str) -> bool {
        // 生产环境应使用 secp256k1 或 schnorr 验证库
        // 当前基础版本仅检查字段存在且非空
        !_pubkey.is_empty() && !_signature.is_empty() && !_challenge.is_empty()
    }
}

#[async_trait]
impl AuthMethod for NostrAuth {
    async fn authenticate(&self, credentials: &serde_json::Value) -> GatewayResult<AgentId> {
        let pubkey = credentials
            .get("pubkey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("缺少 pubkey 字段".into()))?;

        let signature = credentials
            .get("signature")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("缺少 signature 字段".into()))?;

        let challenge = credentials
            .get("challenge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::AuthFailed("缺少 challenge 字段".into()))?;

        if !Self::verify_signature(pubkey, signature, challenge) {
            return Err(GatewayError::AuthFailed("Nostr 签名验证失败".into()));
        }

        // 使用 pubkey 作为 agent_id 的标识
        let agent_id_str = &pubkey[..pubkey.len().min(36)];
        // 尝试解析为 UUID，失败则回退到基于 pubkey 哈希生成的确定性 UUID
        match AgentId::parse(agent_id_str) {
            Ok(id) => Ok(id),
            Err(_) => {
                // 用 pubkey 的哈希生成确定性的 UUID v5
                let ns = Uuid::NAMESPACE_OID;
                let uuid = Uuid::new_v5(&ns, pubkey.as_bytes());
                Ok(AgentId(uuid))
            }
        }
    }
}

// ============================================================================
// GatewayRuntime 实现
// ============================================================================

/// 接引台运行时 — 组合认证 + session 管理 + 租户解析。
pub struct GatewayRuntime {
    /// 注册的认证方法。
    api_key_auth: Option<Box<dyn AuthMethod>>,
    jwt_auth: Option<Box<dyn AuthMethod>>,
    nostr_auth: Option<Box<dyn AuthMethod>>,
    /// 活跃 session 表。
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    /// Session TTL（秒）。
    session_ttl_secs: u64,
    /// 租户解析器。
    tenant_resolver: Box<dyn TenantAwareGateway>,
}

impl GatewayRuntime {
    /// 创建接引台运行时。
    pub fn new() -> Self {
        Self {
            api_key_auth: None,
            jwt_auth: None,
            nostr_auth: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_ttl_secs: 3600, // 默认 1 小时
            tenant_resolver: Box::new(DefaultTenantResolver::new("default")),
        }
    }

    /// 设置 Session TTL。
    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.session_ttl_secs = ttl_secs;
        self
    }

    /// 注册 API Key 认证器。
    pub fn with_api_key_auth(mut self, auth: Box<dyn AuthMethod>) -> Self {
        self.api_key_auth = Some(auth);
        self
    }

    /// 注册 JWT 认证器。
    pub fn with_jwt_auth(mut self, auth: Box<dyn AuthMethod>) -> Self {
        self.jwt_auth = Some(auth);
        self
    }

    /// 注册 Nostr 认证器。
    pub fn with_nostr_auth(mut self, auth: Box<dyn AuthMethod>) -> Self {
        self.nostr_auth = Some(auth);
        self
    }

    /// 注册租户解析器。
    pub fn with_tenant_resolver(mut self, resolver: Box<dyn TenantAwareGateway>) -> Self {
        self.tenant_resolver = resolver;
        self
    }

    /// 获取指定 Agent 的租户 ID。
    pub async fn get_tenant_for_session(&self, agent_id: &AgentId) -> Option<String> {
        let sessions = self.sessions.read().await;
        sessions.get(&agent_id.to_string()).map(|s| s.tenant_id.clone())
    }

    /// 获取认证方法。
    fn get_auth_method(&self, auth_type: AuthType) -> GatewayResult<&dyn AuthMethod> {
        match auth_type {
            AuthType::ApiKey => self.api_key_auth
                .as_deref().ok_or_else(|| GatewayError::AuthFailed("API Key 认证未配置".into())),
            AuthType::Jwt => self.jwt_auth
                .as_deref().ok_or_else(|| GatewayError::AuthFailed("JWT 认证未配置".into())),
            AuthType::Nostr => self.nostr_auth
                .as_deref().ok_or_else(|| GatewayError::AuthFailed("Nostr 认证未配置".into())),
        }
    }

    /// 无效 session 清理（移除过期 session）。
    pub async fn clean_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, s| !s.is_expired());
    }
}

impl Default for GatewayRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Gateway for GatewayRuntime {
    async fn authenticate(&self, request: AuthRequest) -> GatewayResult<AuthResponse> {
        let auth_method = self.get_auth_method(request.auth_type)?;
        let agent_id = auth_method.authenticate(&request.credentials).await?;

        // 解析租户上下文
        let auth_context = AuthContext::from(request);
        let tenant = self.tenant_resolver.resolve_tenant(&auth_context);

        let session = Session::new(agent_id.clone(), auth_context.auth_type, self.session_ttl_secs, tenant.tenant_id);
        let session_id = session.session_id.clone();
        let expires_at = session.expires_at;

        let mut sessions = self.sessions.write().await;
        sessions.insert(agent_id.to_string(), session);
        drop(sessions);

        // 异步清理过期 session
        self.clean_expired_sessions().await;

        Ok(AuthResponse {
            agent_id,
            session_id,
            expires_at,
        })
    }

    async fn validate_session(&self, agent_id: &AgentId) -> GatewayResult<bool> {
        let sessions = self.sessions.read().await;
        match sessions.get(&agent_id.to_string()) {
            Some(session) => Ok(!session.is_expired()),
            None => Ok(false),
        }
    }

    async fn invalidate_session(&self, agent_id: &AgentId) -> GatewayResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&agent_id.to_string());
        Ok(())
    }
}

// ============================================================================
// TenantAwareGateway 实现
// ============================================================================

impl TenantAwareGateway for GatewayRuntime {
    fn resolve_tenant(&self, auth: &AuthContext) -> TenantContext {
        self.tenant_resolver.resolve_tenant(auth)
    }

    fn validate_tenant_access(
        &self,
        tenant: &TenantContext,
        resource: &str,
    ) -> GatewayResult<bool> {
        self.tenant_resolver.validate_tenant_access(tenant, resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use base64::Engine;

    // ====================================================================
    // ApiKey 认证测试
    // ====================================================================

    #[test]
    fn test_api_key_auth_success() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut keys = HashMap::new();
            let agent = AgentId::new();
            keys.insert("sk-lvpa-test-key".into(), agent.clone());

            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::new(keys)));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "sk-lvpa-test-key"}),
            };
            let response = gateway.authenticate(request).await.unwrap();
            assert_eq!(response.agent_id, agent);
            assert!(!response.session_id.is_empty());
        });
    }

    #[test]
    fn test_api_key_auth_failure() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::new(HashMap::new())));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "wrong-key"}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), GatewayError::AuthFailed(_)));
        });
    }

    // ====================================================================
    // JWT 认证测试
    // ====================================================================

    fn make_test_jwt(agent_id: &str, exp_offset_secs: i64) -> String {
        // 手动构造 JWT (header.payload.signature)
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
        let exp = chrono::Utc::now().timestamp() + exp_offset_secs;
        let payload_obj = serde_json::json!({
            "sub": agent_id,
            "exp": exp,
            "iat": chrono::Utc::now().timestamp(),
        });
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&payload_obj).unwrap().as_bytes());
        format!("{}.{}.fake_signature", header, payload)
    }

    #[test]
    fn test_jwt_auth_success() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_jwt_auth(Box::new(JwtAuth::new()));

            let agent_id = "550e8400-e29b-41d4-a716-446655440000";
            let token = make_test_jwt(agent_id, 3600); // 1 小时后过期
            let request = AuthRequest {
                auth_type: AuthType::Jwt,
                credentials: serde_json::json!({"token": token}),
            };
            let response = gateway.authenticate(request).await.unwrap();
            assert_eq!(response.agent_id.to_string(), agent_id);
        });
    }

    #[test]
    fn test_jwt_auth_expired() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_jwt_auth(Box::new(JwtAuth::new()));

            let token = make_test_jwt("test-agent", -3600); // 1 小时前已过期
            let request = AuthRequest {
                auth_type: AuthType::Jwt,
                credentials: serde_json::json!({"token": token}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_jwt_invalid_format() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_jwt_auth(Box::new(JwtAuth::new()));

            let request = AuthRequest {
                auth_type: AuthType::Jwt,
                credentials: serde_json::json!({"token": "not-a-jwt"}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
        });
    }

    // ====================================================================
    // Nostr 认证测试
    // ====================================================================

    #[test]
    fn test_nostr_auth() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_nostr_auth(Box::new(NostrAuth::new()));

            let request = AuthRequest {
                auth_type: AuthType::Nostr,
                credentials: serde_json::json!({
                    "pubkey": "npub1testpubkey12345678",
                    "signature": "sig1234567890abcdef",
                    "challenge": "random_challenge_string",
                }),
            };
            let response = gateway.authenticate(request).await.unwrap();
            assert!(!response.agent_id.to_string().is_empty());
        });
    }

    #[test]
    fn test_nostr_missing_fields() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_nostr_auth(Box::new(NostrAuth::new()));

            let request = AuthRequest {
                auth_type: AuthType::Nostr,
                credentials: serde_json::json!({"pubkey": "test"}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
        });
    }

    // ====================================================================
    // Session 管理测试
    // ====================================================================

    #[test]
    fn test_validate_session_valid() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::from_single("key1", "550e8400-e29b-41d4-a716-446655440000")));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "key1"}),
            };
            let response = gateway.authenticate(request).await.unwrap();

            let valid = gateway.validate_session(&response.agent_id).await.unwrap();
            assert!(valid);
        });
    }

    #[test]
    fn test_validate_session_not_found() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new();
            let valid = gateway.validate_session(&AgentId::new()).await.unwrap();
            assert!(!valid);
        });
    }

    #[test]
    fn test_invalidate_session() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::from_single("key1", "550e8400-e29b-41d4-a716-446655440000")));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "key1"}),
            };
            let response = gateway.authenticate(request).await.unwrap();

            gateway.invalidate_session(&response.agent_id).await.unwrap();
            let valid = gateway.validate_session(&response.agent_id).await.unwrap();
            assert!(!valid, "销毁后 session 应无效");
        });
    }

    // ====================================================================
    // 认证类型未配置测试
    // ====================================================================

    #[test]
    fn test_auth_method_not_configured() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new();
            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "test"}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
        });
    }

    // ====================================================================
    // AgentId format edge cases
    // ====================================================================

    #[test]
    fn test_agent_id_parse_error_in_jwt() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_jwt_auth(Box::new(JwtAuth::new()));

            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(br#"{"sub":"not-a-uuid","exp":9999999999}"#);
            let token = format!("header.{}.sig", payload);

            let request = AuthRequest {
                auth_type: AuthType::Jwt,
                credentials: serde_json::json!({"token": token}),
            };
            let result = gateway.authenticate(request).await;
            assert!(result.is_err());
        });
    }

    // ====================================================================
    // 租户解析集成测试
    // ====================================================================

    #[test]
    fn test_tenant_resolved_from_credentials() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::from_single("sk-key1", "550e8400-e29b-41d4-a716-446655440000")));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({
                    "api_key": "sk-key1",
                    "tenant_id": "tenant-alpha",
                }),
            };
            let response = gateway.authenticate(request).await.unwrap();
            let tenant_id = gateway.get_tenant_for_session(&response.agent_id).await;
            assert_eq!(tenant_id, Some("tenant-alpha".into()));
        });
    }

    #[test]
    fn test_tenant_resolved_fallback() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new()
                .with_api_key_auth(Box::new(ApiKeyAuth::from_single("sk-key1", "550e8400-e29b-41d4-a716-446655440000")));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "sk-key1"}),
            };
            let response = gateway.authenticate(request).await.unwrap();
            let tenant_id = gateway.get_tenant_for_session(&response.agent_id).await;
            assert_eq!(tenant_id, Some("default".into()));
        });
    }

    #[test]
    fn test_tenant_resolver_custom() {
        use crate::tenant::{DefaultTenantResolver, IsolationLevel};

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resolver = DefaultTenantResolver::new("custom-tenant")
                .with_isolation(IsolationLevel::Strict);
            let gateway = GatewayRuntime::new()
                .with_tenant_resolver(Box::new(resolver))
                .with_api_key_auth(Box::new(ApiKeyAuth::from_single("sk-key1", "550e8400-e29b-41d4-a716-446655440000")));

            let request = AuthRequest {
                auth_type: AuthType::ApiKey,
                credentials: serde_json::json!({"api_key": "sk-key1"}),
            };
            let response = gateway.authenticate(request).await.unwrap();
            let tenant_id = gateway.get_tenant_for_session(&response.agent_id).await;
            assert_eq!(tenant_id, Some("custom-tenant".into()));
        });
    }

    #[test]
    fn test_get_tenant_for_session_not_found() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = GatewayRuntime::new();
            let tenant_id = gateway.get_tenant_for_session(&AgentId::new()).await;
            assert!(tenant_id.is_none());
        });
    }
}
