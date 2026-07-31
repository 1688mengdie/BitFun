//! R-4-604 Gateway → AgentSystem 认证+session 集成测试。
//!
//! 验证全链路：
//!   Gateway::authenticate() → AuthResponse { agent_id, session_id }
//!     → AgentManager 可接收该 agent_id 的 Agent → session 可被验证

use std::collections::HashMap;
use taiji_agent_system::manager::AgentManager;
use taiji_gateway::gateway::{AuthRequest, AuthType, Gateway};
use taiji_gateway::auth::{ApiKeyAuth, GatewayRuntime};
use taiji_types::agent::{AgentConfig, AgentId, AgentStatus, SpiritRoot};
use taiji_types::realm::Realm;

// ============================================================================
// Mock Agent — 实现 AgentTrait 的最小化 mock
// ============================================================================

struct MockAgent {
    id: AgentId,
    name: String,
    cfg: AgentConfig,
}

impl MockAgent {
    fn new(id: AgentId, name: String) -> Self {
        Self {
            id, name,
            cfg: AgentConfig::default(),
        }
    }
}

#[async_trait::async_trait]
impl taiji_agent_system::agent::AgentTrait for MockAgent {
    fn agent_id(&self) -> &AgentId { &self.id }
    fn spirit_root(&self) -> SpiritRoot { SpiritRoot::Metal }
    fn realm(&self) -> Realm { Realm::QiRefining }
    fn status(&self) -> AgentStatus { AgentStatus::Idle }
    fn config(&self) -> &AgentConfig { &self.cfg }
    fn state(&self) -> &taiji_types::agent::AgentState {
        // 返回静态默认状态（测试用）
        use std::sync::OnceLock;
        static DEFAULT_STATE: OnceLock<taiji_types::agent::AgentState> = OnceLock::new();
        DEFAULT_STATE.get_or_init(|| taiji_types::agent::AgentState {
            session_id: String::new(),
            status: AgentStatus::Idle,
            context: vec![],
            summary: None,
            cur_iter: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }
    async fn reply(&mut self, _input: &str) -> Result<String, taiji_agent_system::AgentSystemError> {
        Ok(format!("mock reply from {}", self.name))
    }
    async fn reply_stream(
        &mut self, _input: &str,
    ) -> Result<futures::stream::BoxStream<'_, taiji_agent_system::event::AgentEvent>, taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("reply_stream not implemented".into()))
    }
    async fn observe(&mut self, _msg: taiji_types::message::Message) -> Result<(), taiji_agent_system::AgentSystemError> { Ok(()) }
    async fn compress_context(&mut self) -> Result<(), taiji_agent_system::AgentSystemError> { Ok(()) }
    async fn reincarnate(&mut self, _target_commit: &str) -> Result<(), taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("reincarnate not implemented".into()))
    }
    async fn fork(&self, _child_name: &str, _spirit_root: SpiritRoot) -> Result<Box<dyn taiji_agent_system::agent::AgentTrait>, taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("fork not implemented".into()))
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

fn make_gateway() -> GatewayRuntime {
    let mut keys = HashMap::new();
    keys.insert("sk-test-key".into(), AgentId::new());
    keys.insert("sk-agent-alpha".into(), AgentId::new());
    GatewayRuntime::new()
        .with_api_key_auth(Box::new(ApiKeyAuth::new(keys)))
        .with_ttl(3600)
}

// ============================================================================
// 测试
// ============================================================================

#[test]
fn test_gateway_auth_creates_session() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let request = AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-test-key"}),
        };
        let response = gateway.authenticate(request).await.unwrap();

        assert!(!response.session_id.is_empty());
        assert!(response.expires_at > chrono::Utc::now());
        assert!(gateway.validate_session(&response.agent_id).await.unwrap());
    });
}

#[test]
fn test_gateway_auth_to_agentmanager_registration() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let auth_resp = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-agent-alpha"}),
        }).await.unwrap();
        let agent_id = auth_resp.agent_id;

        // AgentManager 基于 gateway 返回的 agent_id 注册 Agent
        let manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new(agent_id.clone(), "alpha".into()))).await.unwrap();
        assert!(manager.has(&agent_id).await);

        // Gateway session 仍然有效
        assert!(gateway.validate_session(&agent_id).await.unwrap());
    });
}

#[test]
fn test_gateway_auth_invalidate_after_agent_unregister() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let auth_resp = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-test-key"}),
        }).await.unwrap();
        let agent_id = auth_resp.agent_id.clone();

        let manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new(agent_id.clone(), "test".into()))).await.unwrap();
        assert!(manager.has(&agent_id).await);
        assert!(gateway.validate_session(&agent_id).await.unwrap());

        // 注销 Agent
        manager.unregister(&agent_id).await.unwrap();
        assert!(!manager.has(&agent_id).await);

        // Gateway session 独立于 Agent 注册
        assert!(gateway.validate_session(&agent_id).await.unwrap());

        // 销毁 session
        gateway.invalidate_session(&agent_id).await.unwrap();
        assert!(!gateway.validate_session(&agent_id).await.unwrap());
    });
}

#[test]
fn test_gateway_session_isolated_per_agent() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut keys = HashMap::new();
        keys.insert("key-a".into(), AgentId::new());
        keys.insert("key-b".into(), AgentId::new());
        let gateway = GatewayRuntime::new()
            .with_api_key_auth(Box::new(ApiKeyAuth::new(keys)))
            .with_ttl(3600);

        let resp_a = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "key-a"}),
        }).await.unwrap();

        let resp_b = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "key-b"}),
        }).await.unwrap();

        assert_ne!(resp_a.agent_id, resp_b.agent_id);
        assert_ne!(resp_a.session_id, resp_b.session_id);

        // 销毁 A 不影响 B
        gateway.invalidate_session(&resp_a.agent_id).await.unwrap();
        assert!(!gateway.validate_session(&resp_a.agent_id).await.unwrap());
        assert!(gateway.validate_session(&resp_b.agent_id).await.unwrap());
    });
}
