//! PermissionDataSource — 权限数据来源
//!
//! harness 从 permission-system 加载权限数据的接口。
//! permission-system 管理面提供数据，harness 运行时消费。
//!
//! 来源: modules/harness/接口设计.md §5 — PermissionDataSource [v2.4]

use async_trait::async_trait;
use taiji_types::agent::AgentId;
use taiji_types::permission::ResourceQuota;
use tokio::sync::broadcast;

use crate::error::HarnessError;

/// Agent 权限上下文 — 灵根白名单 + 资源配额
///
/// 由 PermissionDataSource 加载，供 Harness::check 在流水线中使用。
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// Agent ID
    pub agent_id: AgentId,
    /// 灵根白名单（允许的工具列表）
    pub allowed_tools: Vec<String>,
    /// 资源配额
    pub quota: ResourceQuota,
}

/// 权限变更事件
#[derive(Debug, Clone)]
pub struct PermissionChangeEvent {
    pub agent_id: AgentId,
    pub change_type: String,
}

/// 权限数据源 — harness 从 permission-system 加载配置的接口
///
/// 来源: modules/harness/接口设计.md:103-114 — PermissionDataSource
#[async_trait]
pub trait PermissionDataSource: Send + Sync {
    /// 加载 Agent 的权限上下文（灵根白名单 + 配额）
    async fn load_permission_context(&self, agent_id: &AgentId) -> Result<PermissionContext, HarnessError>;

    /// 监听权限变更事件
    async fn watch_updates(&self) -> broadcast::Receiver<PermissionChangeEvent>;
}
