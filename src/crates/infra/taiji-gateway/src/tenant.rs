//! 接引台 — 租户隔离层（多租户/多用户数据隔离）。
//!
//! 提供：
//! - `IsolationLevel` — 三种隔离级别（Shared / Scoped / Strict）
//! - `TenantContext` — 租户上下文，包含资源归属检查
//! - `TenantAwareGateway` trait — 租户感知网关扩展
//! - `DefaultTenantResolver` — 从凭据解析租户的默认实现
//! - `TenantGate` — 护山大阵的租户隔离检查步骤
//!
//! 设计参考：modules/gateway/接口设计.md §4（v2.4 租户隔离扩展）

use serde::{Deserialize, Serialize};

use crate::error::GatewayResult;
use crate::gateway::AuthContext;

// ============================================================================
// 隔离级别
// ============================================================================

/// 租户隔离级别。
///
/// 定义租户间数据访问的隔离强度：
/// - `Shared`: 共享模式，所有租户无隔离（单租户场景默认）
/// - `Scoped`: 限定模式，同一隔离域的租户可互访
/// - `Strict`: 严格模式，租户间完全隔离
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// 共享 — 所有租户共享数据，无隔离限制。
    #[serde(rename = "shared")]
    Shared,
    /// 限定 — 同一隔离域的租户可互访（需要显式 scope 声明）。
    #[serde(rename = "scoped")]
    Scoped,
    /// 严格 — 租户间完全隔离，跨租户资源访问被拒绝。
    #[serde(rename = "strict")]
    Strict,
}

// ============================================================================
// 租户上下文
// ============================================================================

/// 租户上下文 — 标识当前请求所属的租户及隔离级别。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantContext {
    /// 租户 ID。
    pub tenant_id: String,
    /// 隔离级别。
    pub isolation_level: IsolationLevel,
}

impl TenantContext {
    /// 创建共享模式租户上下文。
    pub fn shared(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            isolation_level: IsolationLevel::Shared,
        }
    }

    /// 创建限定模式租户上下文。
    pub fn scoped(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            isolation_level: IsolationLevel::Scoped,
        }
    }

    /// 创建严格模式租户上下文。
    pub fn strict(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            isolation_level: IsolationLevel::Strict,
        }
    }

    /// 检查资源是否属于当前租户。
    ///
    /// 识别两种资源路径格式：
    /// - `/tenants/{tenant_id}/...`（层级路径格式）
    /// - `{tenant_id}:resource_name`（前缀冒号格式）
    pub fn owns_resource(&self, resource: &str) -> bool {
        resource.starts_with(&format!("/tenants/{}", self.tenant_id))
            || resource.starts_with(&format!("{}:", self.tenant_id))
    }
}

// ============================================================================
// TenantAwareGateway trait
// ============================================================================

/// 租户感知网关 trait — 扩展 Gateway 的租户解析与隔离验证。
///
/// 实现者通常同时实现 `Gateway` trait，但本 trait 保持独立以允许
/// 单独替换租户解析策略。
pub trait TenantAwareGateway: Send + Sync {
    /// 从认证上下文中解析租户信息。
    fn resolve_tenant(&self, auth: &AuthContext) -> TenantContext;

    /// 验证租户是否有权访问指定资源。
    ///
    /// 返回 `Ok(true)` 表示允许访问，`Ok(false)` 表示拒绝，
    /// `Err` 表示验证过程出错。
    fn validate_tenant_access(
        &self,
        tenant: &TenantContext,
        resource: &str,
    ) -> GatewayResult<bool>;
}

// ============================================================================
// 默认租户解析器
// ============================================================================

/// 默认租户解析器 — 从凭据中提取 `tenant_id` 字段做租户解析。
///
/// 如果凭据中未提供 `tenant_id`，使用预设的默认值。
/// 隔离级别通过构建器方法设置，默认 `Shared`。
#[derive(Debug, Clone)]
pub struct DefaultTenantResolver {
    /// 默认租户 ID（凭据未指定时使用）。
    pub default_tenant: String,
    /// 默认隔离级别。
    pub default_isolation: IsolationLevel,
}

impl DefaultTenantResolver {
    /// 创建默认租户解析器，使用指定租户 ID 和 `Shared` 隔离级别。
    pub fn new(tenant_id: &str) -> Self {
        Self {
            default_tenant: tenant_id.to_string(),
            default_isolation: IsolationLevel::Shared,
        }
    }

    /// 设置默认隔离级别。
    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.default_isolation = level;
        self
    }
}

impl TenantAwareGateway for DefaultTenantResolver {
    fn resolve_tenant(&self, auth: &AuthContext) -> TenantContext {
        let tenant_id = auth
            .credentials
            .get("tenant_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.default_tenant)
            .to_string();

        TenantContext {
            tenant_id,
            isolation_level: self.default_isolation,
        }
    }

    fn validate_tenant_access(
        &self,
        tenant: &TenantContext,
        resource: &str,
    ) -> GatewayResult<bool> {
        match tenant.isolation_level {
            IsolationLevel::Shared | IsolationLevel::Scoped => Ok(true),
            IsolationLevel::Strict => Ok(tenant.owns_resource(resource)),
        }
    }
}

// ============================================================================
// TenantGate — 护山大阵租户隔离检查步骤
// ============================================================================

/// 租户门 — 护山大阵的租户隔离检查步骤。
///
/// 输出 `taiji_types::permission::GateCommand`，供 `DefaultHarness::check()`
/// 管线的租户隔离阶段使用。
///
/// # 策略
///
/// | 隔离级别 | 本租户资源 | 跨租户资源 |
/// |----------|-----------|-----------|
/// | Shared   | Allow     | Allow     |
/// | Scoped   | Allow     | Allow     |
/// | Strict   | Allow     | **Deny**  |
///
/// # 在 harness 中使用
///
/// ```ignore
/// use taiji_gateway::tenant::TenantGate;
/// use taiji_types::permission::GateCommand;
///
/// if let Some(tenant) = current_tenant {
///     let cmd = TenantGate::check(&tenant, tool_name);
///     if cmd == GateCommand::Deny {
///         return GateCommand::Deny;
///     }
/// }
/// ```
pub struct TenantGate;

impl TenantGate {
    /// 执行租户隔离检查。
    ///
    /// - `Strict` 模式下，非本租户资源返回 `GateCommand::Deny`
    /// - `Scoped` 和 `Shared` 模式下始终返回 `GateCommand::Allow`
    pub fn check(tenant: &TenantContext, resource: &str) -> taiji_types::permission::GateCommand {
        use taiji_types::permission::GateCommand;

        match tenant.isolation_level {
            IsolationLevel::Strict => {
                if tenant.owns_resource(resource) {
                    GateCommand::Allow
                } else {
                    GateCommand::Deny
                }
            }
            IsolationLevel::Scoped | IsolationLevel::Shared => GateCommand::Allow,
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{AuthRequest, AuthType};

    // ====================================================================
    // TenantContext 构造
    // ====================================================================

    #[test]
    fn test_tenant_context_shared() {
        let ctx = TenantContext::shared("tenant-alpha");
        assert_eq!(ctx.tenant_id, "tenant-alpha");
        assert_eq!(ctx.isolation_level, IsolationLevel::Shared);
    }

    #[test]
    fn test_tenant_context_scoped() {
        let ctx = TenantContext::scoped("tenant-beta");
        assert_eq!(ctx.isolation_level, IsolationLevel::Scoped);
    }

    #[test]
    fn test_tenant_context_strict() {
        let ctx = TenantContext::strict("tenant-gamma");
        assert_eq!(ctx.isolation_level, IsolationLevel::Strict);
    }

    // ====================================================================
    // TenantContext::owns_resource
    // ====================================================================

    #[test]
    fn test_owns_resource_path_format() {
        let ctx = TenantContext::strict("tenant-alpha");
        assert!(ctx.owns_resource("/tenants/tenant-alpha/data/kline"));
        assert!(!ctx.owns_resource("/tenants/tenant-beta/data/kline"));
    }

    #[test]
    fn test_owns_resource_prefix_format() {
        let ctx = TenantContext::strict("tenant-alpha");
        assert!(ctx.owns_resource("tenant-alpha:strategy-01"));
        assert!(!ctx.owns_resource("tenant-beta:strategy-01"));
    }

    #[test]
    fn test_owns_resource_empty() {
        let ctx = TenantContext::strict("tenant-alpha");
        assert!(!ctx.owns_resource(""));
        assert!(!ctx.owns_resource("/tenants/"));
    }

    // ====================================================================
    // DefaultTenantResolver
    // ====================================================================

    #[test]
    fn test_default_resolver_uses_credential_tenant() {
        let resolver = DefaultTenantResolver::new("default-tenant");
        let auth = AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"tenant_id": "tenant-alpha", "api_key": "sk-1"}),
            source: None,
        };
        let tenant = resolver.resolve_tenant(&auth);
        assert_eq!(tenant.tenant_id, "tenant-alpha");
        assert_eq!(tenant.isolation_level, IsolationLevel::Shared);
    }

    #[test]
    fn test_default_resolver_fallback() {
        let resolver = DefaultTenantResolver::new("default-tenant");
        let auth = AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-1"}),
            source: None,
        };
        let tenant = resolver.resolve_tenant(&auth);
        assert_eq!(tenant.tenant_id, "default-tenant");
    }

    #[test]
    fn test_default_resolver_empty_tenant_fallback() {
        let resolver = DefaultTenantResolver::new("fallback-tenant");
        let auth = AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"tenant_id": "", "api_key": "sk-1"}),
            source: None,
        };
        let tenant = resolver.resolve_tenant(&auth);
        assert_eq!(tenant.tenant_id, "fallback-tenant");
    }

    #[test]
    fn test_default_resolver_with_custom_isolation() {
        let resolver = DefaultTenantResolver::new("tenant-alpha")
            .with_isolation(IsolationLevel::Strict);
        let auth = AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "sk-1"}),
            source: None,
        };
        let tenant = resolver.resolve_tenant(&auth);
        assert_eq!(tenant.isolation_level, IsolationLevel::Strict);
    }

    // ====================================================================
    // validate_tenant_access
    // ====================================================================

    #[test]
    fn test_validate_access_shared() {
        let resolver = DefaultTenantResolver::new("tenant-alpha");
        let tenant = TenantContext::shared("tenant-alpha");
        let allowed = resolver
            .validate_tenant_access(&tenant, "any/resource")
            .unwrap();
        assert!(allowed);
    }

    #[test]
    fn test_validate_access_scoped() {
        let resolver = DefaultTenantResolver::new("tenant-alpha");
        let tenant = TenantContext::scoped("tenant-alpha");
        let allowed = resolver
            .validate_tenant_access(&tenant, "any/resource")
            .unwrap();
        assert!(allowed);
    }

    #[test]
    fn test_validate_access_strict_own() {
        let resolver = DefaultTenantResolver::new("tenant-alpha")
            .with_isolation(IsolationLevel::Strict);
        let tenant = resolver.resolve_tenant(&AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"tenant_id": "tenant-alpha"}),
            source: None,
        });
        let allowed = resolver
            .validate_tenant_access(&tenant, "/tenants/tenant-alpha/data")
            .unwrap();
        assert!(allowed);
    }

    #[test]
    fn test_validate_access_strict_other() {
        let resolver = DefaultTenantResolver::new("tenant-alpha")
            .with_isolation(IsolationLevel::Strict);
        let tenant = resolver.resolve_tenant(&AuthContext {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"tenant_id": "tenant-alpha"}),
            source: None,
        });
        let allowed = resolver
            .validate_tenant_access(&tenant, "/tenants/tenant-beta/data")
            .unwrap();
        assert!(!allowed, "严格模式下跨租户访问应被拒绝");
    }

    // ====================================================================
    // TenantGate
    // ====================================================================

    #[test]
    fn test_tenant_gate_shared_allows_all() {
        let tenant = TenantContext::shared("tenant-alpha");
        let cmd = TenantGate::check(&tenant, "any/resource");
        assert_eq!(cmd, taiji_types::permission::GateCommand::Allow);
    }

    #[test]
    fn test_tenant_gate_scoped_allows_all() {
        let tenant = TenantContext::scoped("tenant-alpha");
        let cmd = TenantGate::check(&tenant, "any/resource");
        assert_eq!(cmd, taiji_types::permission::GateCommand::Allow);
    }

    #[test]
    fn test_tenant_gate_strict_allows_own() {
        let tenant = TenantContext::strict("tenant-alpha");
        let cmd = TenantGate::check(&tenant, "/tenants/tenant-alpha/config");
        assert_eq!(cmd, taiji_types::permission::GateCommand::Allow);
    }

    #[test]
    fn test_tenant_gate_strict_denies_other() {
        let tenant = TenantContext::strict("tenant-alpha");
        let cmd = TenantGate::check(&tenant, "/tenants/tenant-beta/config");
        assert_eq!(cmd, taiji_types::permission::GateCommand::Deny);
    }

    #[test]
    fn test_tenant_gate_strict_denies_unprefixed() {
        let tenant = TenantContext::strict("tenant-alpha");
        let cmd = TenantGate::check(&tenant, "no-prefix/resource");
        assert_eq!(cmd, taiji_types::permission::GateCommand::Deny);
    }

    // ====================================================================
    // Serde 序列化/反序列化
    // ====================================================================

    #[test]
    fn test_isolation_level_serde() {
        for level in &[IsolationLevel::Shared, IsolationLevel::Scoped, IsolationLevel::Strict] {
            let json = serde_json::to_string(level).unwrap();
            let back: IsolationLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, back);
        }
    }

    #[test]
    fn test_tenant_context_serde() {
        let ctx = TenantContext::strict("tenant-alpha");
        let json = serde_json::to_string(&ctx).unwrap();
        let back: TenantContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tenant_id, "tenant-alpha");
        assert_eq!(back.isolation_level, IsolationLevel::Strict);
    }

    #[test]
    fn test_default_resolver_serde() {
        let resolver = DefaultTenantResolver::new("tenant-alpha")
            .with_isolation(IsolationLevel::Scoped);
        let auth = AuthContext {
            auth_type: AuthType::Jwt,
            credentials: serde_json::json!({"tenant_id": "tenant-beta"}),
            source: Some("10.0.0.1".into()),
        };
        let tenant = resolver.resolve_tenant(&auth);
        assert_eq!(tenant.tenant_id, "tenant-beta");
        assert_eq!(tenant.isolation_level, IsolationLevel::Scoped);
    }

    // ====================================================================
    // AuthContext → AuthRequest 转换
    // ====================================================================

    #[test]
    fn test_auth_context_from_request() {
        let req = AuthRequest {
            auth_type: AuthType::ApiKey,
            credentials: serde_json::json!({"api_key": "test-key"}),
        };
        let ctx = AuthContext::from(req);
        assert_eq!(ctx.auth_type, AuthType::ApiKey);
        assert_eq!(ctx.credentials["api_key"], "test-key");
        assert!(ctx.source.is_none());
    }
}
