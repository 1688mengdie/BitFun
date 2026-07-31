use std::sync::Arc;
use std::time::Duration;
use taiji_ledger::{AuditFilter, AuditResult, InMemoryLedger, Ledger};
use taiji_types::agent::AgentId;

/// 审计事件（模拟 event-bus 消息体）
#[derive(Debug, Clone)]
struct AuditEvent {
    topic: String,
    agent_id: AgentId,
    action: String,
    resource: String,
    result: AuditResult,
    detail: serde_json::Value,
}

const TOPIC_AUDIT_IDENTITY: &str = "audit.identity";
const TOPIC_AUDIT_STATE: &str = "audit.state";
const TOPIC_AUDIT_TASK: &str = "audit.task";

fn start_audit_subscriber(
    ledger: Arc<dyn Ledger + Send + Sync>,
    mut rx: tokio::sync::broadcast::Receiver<AuditEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let entry = taiji_ledger::AuditEntry {
                entry_id: format!("{}_{}", event.topic.replace('.', "_"), chrono::Utc::now().timestamp_millis()),
                timestamp: chrono::Utc::now(),
                agent_id: event.agent_id,
                action: event.action,
                resource: event.resource,
                result: event.result,
                detail: event.detail,
            };
            if let Err(e) = ledger.record(entry).await {
                eprintln!("Ledger record 失败: {}", e);
            }
        }
    })
}

fn make_event(topic: &str, agent_id: &AgentId, action: &str, result: AuditResult) -> AuditEvent {
    AuditEvent {
        topic: topic.into(),
        agent_id: agent_id.clone(),
        action: action.into(),
        resource: format!("resource/{}", action),
        result,
        detail: serde_json::json!({"key": "value"}),
    }
}

#[tokio::test]
async fn test_audit_identity_event_recorded() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    tx.send(make_event(TOPIC_AUDIT_IDENTITY, &agent, "read_file", AuditResult::Allowed)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].action, "read_file");
    assert_eq!(results[0].result, AuditResult::Allowed);
}

#[tokio::test]
async fn test_audit_state_event_recorded() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    tx.send(make_event(TOPIC_AUDIT_STATE, &agent, "realm_upgrade", AuditResult::Allowed)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_audit_task_event_recorded() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    tx.send(make_event(TOPIC_AUDIT_TASK, &agent, "execute_strategy", AuditResult::Allowed)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_audit_denied_event_recorded() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    tx.send(make_event(TOPIC_AUDIT_IDENTITY, &agent, "delete_file", AuditResult::Denied)).unwrap();
    tx.send(make_event(TOPIC_AUDIT_IDENTITY, &agent, "write_file", AuditResult::Error)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let filter = AuditFilter { result: Some(AuditResult::Denied), ..AuditFilter::with_agent(agent.clone()) };
    let results = ledger.query(&agent, filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].action, "delete_file");
}

#[tokio::test]
async fn test_audit_multi_agent_isolation() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent_a = AgentId::new();
    let agent_b = AgentId::new();
    tx.send(make_event(TOPIC_AUDIT_IDENTITY, &agent_a, "read", AuditResult::Allowed)).unwrap();
    tx.send(make_event(TOPIC_AUDIT_IDENTITY, &agent_b, "write", AuditResult::Denied)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let a_results = ledger.query(&agent_a, AuditFilter::with_agent(agent_a.clone())).await.unwrap();
    assert_eq!(a_results.len(), 1);
    assert_eq!(a_results[0].action, "read");
    let b_results = ledger.query(&agent_b, AuditFilter::with_agent(agent_b.clone())).await.unwrap();
    assert_eq!(b_results.len(), 1);
    assert_eq!(b_results[0].action, "write");
}

#[tokio::test]
async fn test_audit_bulk_events() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(128);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    let topics = [TOPIC_AUDIT_IDENTITY, TOPIC_AUDIT_STATE, TOPIC_AUDIT_TASK];
    for (i, topic) in topics.iter().enumerate() {
        for _ in 0..3 {
            tx.send(make_event(topic, &agent, &format!("action_{}", i), AuditResult::Allowed)).unwrap();
        }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
    assert_eq!(results.len(), 9);
}

#[tokio::test]
async fn test_audit_event_detail_preserved() {
    let (tx, rx) = tokio::sync::broadcast::channel::<AuditEvent>(64);
    let ledger = Arc::new(InMemoryLedger::new());
    let handle = start_audit_subscriber(ledger.clone(), rx);
    let agent = AgentId::new();
    let detail = serde_json::json!({"file_path": "/etc/config.yaml", "bytes_written": 1024});
    tx.send(AuditEvent {
        topic: TOPIC_AUDIT_TASK.into(),
        agent_id: agent.clone(),
        action: "write_config".into(),
        resource: "config.yaml".into(),
        result: AuditResult::Allowed,
        detail: detail.clone(),
    }).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    let results = ledger.query(&agent, AuditFilter::with_agent(agent.clone())).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].detail, detail);
    assert_eq!(results[0].resource, "config.yaml");
}
