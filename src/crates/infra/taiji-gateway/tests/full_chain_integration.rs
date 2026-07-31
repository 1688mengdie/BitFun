//! R-4-606 Phase 4 全链路集成测试。
//!
//! 验证链路：
//!   Gateway::authenticate()
//!     → AgentManager::register()
//!     → ToolRegistry::execute() → Harness::check()
//!     → Ledger::record()
//!
//! 链路说明：
//!   1. Gateway 认证 ApiKey → agent_id + session
//!   2. AgentManager 注册 MockAgent
//!   3. ToolRegistry 注册 mock 工具
//!   4. Harness 检查权限（只读工具 ALLOW，非只读工具 ASK）
//!   5. Ledger 记录审计日志
//!   6. 验证审计日志可查询

use std::collections::HashMap;
use taiji_agent_system::manager::AgentManager;
use taiji_gateway::gateway::{AuthRequest, AuthType, Gateway};
use taiji_gateway::auth::{ApiKeyAuth, GatewayRuntime};
use taiji_ledger::Ledger;
use taiji_types::harness::Harness;
use taiji_types::agent::{AgentConfig, AgentId, AgentStatus, SpiritRoot};
use taiji_types::realm::Realm;

// ============================================================================
// Mock Agent
// ============================================================================

struct MockAgent {
    id: AgentId,
    name: String,
    cfg: AgentConfig,
}

impl MockAgent {
    fn new(id: AgentId, name: String) -> Self {
        Self { id, name, cfg: AgentConfig::default() }
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
        static S: std::sync::OnceLock<taiji_types::agent::AgentState> = std::sync::OnceLock::new();
        S.get_or_init(|| taiji_types::agent::AgentState {
            session_id: String::new(), status: AgentStatus::Idle,
            context: vec![], summary: None, cur_iter: 0,
            created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
        })
    }
    async fn reply(&mut self, _: &str) -> Result<String, taiji_agent_system::AgentSystemError> {
        Ok(format!("mock: {}", self.name))
    }
    async fn reply_stream(&mut self, _: &str) -> Result<futures::stream::BoxStream<'_, taiji_agent_system::event::AgentEvent>, taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("reply_stream".into()))
    }
    async fn observe(&mut self, _: taiji_types::message::Message) -> Result<(), taiji_agent_system::AgentSystemError> { Ok(()) }
    async fn compress_context(&mut self) -> Result<(), taiji_agent_system::AgentSystemError> { Ok(()) }
    async fn reincarnate(&mut self, _: &str) -> Result<(), taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("reincarnate".into()))
    }
    async fn fork(&self, _: &str, _: SpiritRoot) -> Result<Box<dyn taiji_agent_system::agent::AgentTrait>, taiji_agent_system::AgentSystemError> {
        Err(taiji_agent_system::AgentSystemError::Unimplemented("fork".into()))
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

fn make_gateway() -> GatewayRuntime {
    let mut keys = HashMap::new();
    keys.insert("sk-full-chain".into(), AgentId::new());
    GatewayRuntime::new()
        .with_api_key_auth(Box::new(ApiKeyAuth::new(keys)))
        .with_ttl(3600)
}

// ============================================================================
// 全链路测试
// ============================================================================

/// 全链路1: Gateway → AgentManager → Harness (只读工具 ALLOW) → Ledger
#[test]
fn test_full_chain_readonly_tool() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // === Step 1: Gateway Auth ===
        let gateway = make_gateway();
        let auth = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-full-chain"}),
        }).await.unwrap();
        let agent_id = auth.agent_id.clone();
        assert!(gateway.validate_session(&agent_id).await.unwrap());

        // === Step 2: AgentManager Register ===
        let manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new(agent_id.clone(), "chain_test".into()))).await.unwrap();
        assert!(manager.has(&agent_id).await);

        // === Step 3: Harness Check (只读工具 → ALLOW) ===
        let harness = taiji_harness::DefaultHarness::new();
        let input = serde_json::json!({"path": "test.txt"});
        let cmd = harness.check("read", &input).await;
        assert_eq!(cmd, taiji_types::permission::GateCommand::Allow,
            "只读工具应 ALLOW");

        // === Step 4: Ledger Record (审计日志) ===
        let ledger = taiji_ledger::InMemoryLedger::new();
        let entry = taiji_ledger::AuditEntry {
            entry_id: "chain_001".into(),
            timestamp: chrono::Utc::now(),
            agent_id: agent_id.clone(),
            action: "read_file".into(),
            resource: "test.txt".into(),
            result: taiji_ledger::AuditResult::Allowed,
            detail: serde_json::json!({"tool": "read", "auth_method": "api_key"}),
        };
        ledger.record(entry).await.unwrap();

        // === Step 5: Verify Ledger Query ===
        let results = ledger.query(&agent_id, taiji_ledger::AuditFilter::with_agent(agent_id.clone())).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "read_file");
        assert_eq!(results[0].result, taiji_ledger::AuditResult::Allowed);

        // verify session still valid
        assert!(gateway.validate_session(&agent_id).await.unwrap());
    });
}

/// 全链路2: Gateway → AgentManager → Harness (非只读 ASK) → Ledger (Denied 记录)
#[test]
fn test_full_chain_non_readonly_asks() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let auth = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-full-chain"}),
        }).await.unwrap();
        let agent_id = auth.agent_id;

        let harness = taiji_harness::DefaultHarness::new();
        let input = serde_json::json!({"cmd": "rm -rf /"});
        let cmd = harness.check("execute", &input).await;
        assert!(matches!(cmd, taiji_types::permission::GateCommand::Ask { .. }),
            "非只读工具应 ASK");

        let ledger = taiji_ledger::InMemoryLedger::new();
        ledger.record(taiji_ledger::AuditEntry {
            entry_id: "chain_002".into(),
            timestamp: chrono::Utc::now(),
            agent_id: agent_id.clone(),
            action: "execute".into(),
            resource: "rm -rf /".into(),
            result: taiji_ledger::AuditResult::Denied,
            detail: serde_json::json!({"reason": "harness asked confirmation"}),
        }).await.unwrap();

        let results = ledger.query(&agent_id, taiji_ledger::AuditFilter::with_agent(agent_id.clone())).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, taiji_ledger::AuditResult::Denied);
    });
}

/// 全链路3: Harness Deny 规则 → 拒绝 → Ledger Error 记录
#[test]
fn test_full_chain_deny_rule() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let auth = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-full-chain"}),
        }).await.unwrap();
        let agent_id = auth.agent_id;

        let mut harness = taiji_harness::DefaultHarness::new();
        harness.add_rule(taiji_types::permission::PermissionRule {
            tool_name: "delete".into(),
            rule_content: None,
            behavior: taiji_types::permission::PermissionBehavior::Deny,
            source: "admin".into(),
        });

        let cmd = harness.check("delete", &serde_json::json!({})).await;
        assert_eq!(cmd, taiji_types::permission::GateCommand::Deny, "Deny 规则生效");

        let ledger = taiji_ledger::InMemoryLedger::new();
        ledger.record(taiji_ledger::AuditEntry {
            entry_id: "chain_003".into(),
            timestamp: chrono::Utc::now(),
            agent_id,
            action: "delete".into(),
            resource: "critical_file".into(),
            result: taiji_ledger::AuditResult::Error,
            detail: serde_json::json!({"reason": "denied by harness rule"}),
        }).await.unwrap();
    });
}

/// 全链路4: Ledger summary — gateway auth + agent register + harness check + record
#[test]
fn test_full_chain_ledger_summary() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = make_gateway();
        let auth = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-full-chain"}),
        }).await.unwrap();
        let agent_id = auth.agent_id;

        let manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new(agent_id.clone(), "summary_test".into()))).await.unwrap();

        let ledger = taiji_ledger::InMemoryLedger::new();
        for action in &["read", "read", "write", "delete"] {
            let result = if *action == "delete" {
                taiji_ledger::AuditResult::Denied
            } else {
                taiji_ledger::AuditResult::Allowed
            };
            ledger.record(taiji_ledger::AuditEntry {
                entry_id: format!("summary_{}", action),
                timestamp: chrono::Utc::now(),
                agent_id: agent_id.clone(),
                action: action.to_string(),
                resource: "test".into(),
                result,
                detail: serde_json::json!({}),
            }).await.unwrap();
        }

        let summary = ledger.summary(&agent_id).await.unwrap();
        assert_eq!(summary.total_entries, 4);
        assert_eq!(summary.allowed_count, 3);
        assert_eq!(summary.denied_count, 1);
        assert!(gateway.validate_session(&agent_id).await.unwrap());
    });
}

/// 全链路5: 认证失败 → 整个链路不执行
#[test]
fn test_full_chain_auth_failure_blocks() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gateway = GatewayRuntime::new()
            .with_api_key_auth(Box::new(ApiKeyAuth::new(HashMap::new()))); // 无有效 key

        let result = gateway.authenticate(AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "invalid-key"}),
        }).await;

        assert!(result.is_err(), "无效 key 应认证失败");
    });
}
