//! PermissionConfigManager trait + InMemoryPermissionManager — 灵根管理面 CRUD。
//!
//! 架构总纲 §5.2 管理面职责：
//! - 灵根白名单定义（SpiritRoot → ToolDomain 映射）
//! - 灵石配额设置（Agent → ResourceQuota）
//! - 称号特权管理（Title → PermissionLevel）
//! - 境界→工具映射（Realm → ToolDomain 列表）
//! - 收费 tier 映射（Realm → ProductTier）— 技术总纲 §八
//!
//! 参考: modules/permission-system/接口设计.md

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use taiji_tool_bus::TaijiToolDomain;
use taiji_types::agent::{AgentId, SpiritRoot};
use taiji_types::permission::{PermissionLevel, ResourceQuota};
use taiji_types::realm::Realm;

use crate::error::PermissionSystemError;

// ── 收费 Tier（技术总纲 §八） ──

/// 产品收费层级 — 对应 product.json 的 variant。
///
/// 参考：技术总纲 §8.2 多版本 product.json 加载 + §8.4 定价分层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProductTier {
    /// 免费版 — A股/指数 辅助决策 + 教学辅导
    Free,
    /// 标准版 — 期货 L1-L3 量化交易 + 模拟交易
    Standard,
    /// 终极版 — L4 全维度 + 全品种共振 + 智能交易
    Ultimate,
    /// 机构版 — 多账户 + WT 部署 + 定制风控
    Enterprise,
}

/// 根据境界判断收费 tier。
///
/// 映射规则（技术总纲 §8.4 + 量化总纲 §0）：
/// - Free: 炼气~筑基（QiRefining ~ Foundation）
/// - Standard: 金丹~元婴（GoldenCore ~ NascentSoul）
/// - Ultimate: 化神~炼虚（SpiritSevering ~ VoidRefining）
/// - Enterprise: 飞升（ImmortalAscension）
pub fn resolve_tier(realm: Realm) -> ProductTier {
    match realm {
        Realm::QiRefining | Realm::Foundation => ProductTier::Free,
        Realm::GoldenCore | Realm::NascentSoul => ProductTier::Standard,
        Realm::SpiritSevering | Realm::VoidRefining => ProductTier::Ultimate,
        Realm::ImmortalAscension => ProductTier::Enterprise,
    }
}

/// 根据 tier 和工坊类型判断 Agent 是否有权访问。
///
/// 工坊访问门控（技术总纲 §八 + 架构总纲 §7.1）：
/// - Free: 不可访问金算坊（交易工坊）
/// - Standard: 可访问天机坊 + 金算坊
/// - Ultimate: 可访问全部 4 个工坊
/// - Enterprise: 可访问全部 4 个工坊
pub fn can_access_workshop(tier: ProductTier, workshop: &str) -> bool {
    match tier {
        ProductTier::Free => {
            // Free 不可访问金算坊（Trading）
            !matches!(workshop, "jinsuan" | "Trading")
        }
        ProductTier::Standard => {
            // Standard 可访问天机坊+金算坊
            matches!(workshop, "tianji" | "jinsuan" | "Trading" | "Development" | "Basic")
        }
        ProductTier::Ultimate | ProductTier::Enterprise => {
            // Ultimate/Enterprise 可访问全部
            true
        }
    }
}

/// 权限配置管理器 trait — 灵根系统管理面核心接口。
///
/// 所有方法为 CRUD 操作，不做运行时权限判定（那是 harness 的职责）。
/// 提供配置数据，供 harness 预加载到缓存。
#[async_trait]
pub trait PermissionConfigManager: Send + Sync {
    // ── 灵根白名单 ──

    /// 设置灵根可访问的工具域白名单。
    async fn set_spirit_root_whitelist(
        &self,
        spirit_root: SpiritRoot,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError>;

    /// 获取所有灵根的白名单映射。
    async fn get_spirit_root_whitelist(
        &self,
    ) -> Result<HashMap<SpiritRoot, Vec<TaijiToolDomain>>, PermissionSystemError>;

    /// 获取单个灵根的白名单。
    async fn get_whitelist_for_root(
        &self,
        spirit_root: &SpiritRoot,
    ) -> Result<Vec<TaijiToolDomain>, PermissionSystemError>;

    // ── 灵石配额 ──

    /// 设置 Agent 的资源配额。
    async fn set_stone_quota(
        &self,
        agent_id: &AgentId,
        quota: ResourceQuota,
    ) -> Result<(), PermissionSystemError>;

    /// 获取 Agent 的资源配额。
    async fn get_stone_quota(
        &self,
        agent_id: &AgentId,
    ) -> Result<ResourceQuota, PermissionSystemError>;

    // ── 称号特权 ──

    /// 设置称号对应的权限等级。
    async fn set_title_privilege(
        &self,
        title: &str,
        level: PermissionLevel,
    ) -> Result<(), PermissionSystemError>;

    /// 获取称号对应的权限等级。
    async fn get_title_privilege(
        &self,
        title: &str,
    ) -> Result<PermissionLevel, PermissionSystemError>;

    // ── 境界→工具映射 ──

    /// 设置境界可访问的工具域列表。
    async fn set_realm_tool_mapping(
        &self,
        realm: Realm,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError>;

    /// 获取境界可访问的工具域列表。
    async fn get_realm_tool_mapping(
        &self,
        realm: &Realm,
    ) -> Result<Vec<TaijiToolDomain>, PermissionSystemError>;

    // ── 批量加载（供 harness 预加载缓存） ──

    /// 导出完整配置快照（供 harness 启动时预加载）。
    async fn export_snapshot(&self) -> Result<PermissionSnapshot, PermissionSystemError>;
}

/// 权限配置快照 — 供 harness 预加载到缓存。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PermissionSnapshot {
    pub spirit_root_whitelist: HashMap<SpiritRoot, Vec<TaijiToolDomain>>,
    pub agent_quotas: HashMap<AgentId, ResourceQuota>,
    pub title_privileges: HashMap<String, PermissionLevel>,
    pub realm_tool_mappings: HashMap<Realm, Vec<TaijiToolDomain>>,
}

/// 内存权限管理器 — PermissionConfigManager 的默认内存实现。
pub struct InMemoryPermissionManager {
    spirit_root_whitelist: RwLock<HashMap<SpiritRoot, Vec<TaijiToolDomain>>>,
    agent_quotas: RwLock<HashMap<AgentId, ResourceQuota>>,
    title_privileges: RwLock<HashMap<String, PermissionLevel>>,
    realm_tool_mappings: RwLock<HashMap<Realm, Vec<TaijiToolDomain>>>,
}

impl InMemoryPermissionManager {
    pub fn new() -> Self {
        let mgr = Self {
            spirit_root_whitelist: RwLock::new(HashMap::new()),
            agent_quotas: RwLock::new(HashMap::new()),
            title_privileges: RwLock::new(HashMap::new()),
            realm_tool_mappings: RwLock::new(HashMap::new()),
        };

        // 初始化默认灵根白名单
        let _ = mgr.set_spirit_root_whitelist_sync(SpiritRoot::Metal, vec![
            TaijiToolDomain::Basic,
            TaijiToolDomain::Trading,
        ]);
        let _ = mgr.set_spirit_root_whitelist_sync(SpiritRoot::Wood, vec![
            TaijiToolDomain::Basic,
            TaijiToolDomain::Teaching,
        ]);
        let _ = mgr.set_spirit_root_whitelist_sync(SpiritRoot::Water, vec![
            TaijiToolDomain::Basic,
            TaijiToolDomain::Agent,
        ]);
        let _ = mgr.set_spirit_root_whitelist_sync(SpiritRoot::Fire, vec![
            TaijiToolDomain::Basic,
            TaijiToolDomain::Analysis,
        ]);
        let _ = mgr.set_spirit_root_whitelist_sync(SpiritRoot::Earth, vec![
            TaijiToolDomain::Basic,
            TaijiToolDomain::Development,
        ]);

        // 初始化默认境界→工具映射
        let _ = mgr.set_realm_tool_mapping_sync(
            taiji_types::realm::Realm::QiRefining,
            vec![TaijiToolDomain::Basic],
        );

        mgr
    }

    fn set_spirit_root_whitelist_sync(
        &self,
        spirit_root: SpiritRoot,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError> {
        self.spirit_root_whitelist
            .write()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .insert(spirit_root, tool_domains);
        Ok(())
    }

    fn set_realm_tool_mapping_sync(
        &self,
        realm: Realm,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError> {
        self.realm_tool_mappings
            .write()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .insert(realm, tool_domains);
        Ok(())
    }
}

impl Default for InMemoryPermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PermissionConfigManager for InMemoryPermissionManager {
    async fn set_spirit_root_whitelist(
        &self,
        spirit_root: SpiritRoot,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError> {
        self.set_spirit_root_whitelist_sync(spirit_root, tool_domains)
    }

    async fn get_spirit_root_whitelist(
        &self,
    ) -> Result<HashMap<SpiritRoot, Vec<TaijiToolDomain>>, PermissionSystemError> {
        let map = self.spirit_root_whitelist
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?;
        Ok(map.clone())
    }

    async fn get_whitelist_for_root(
        &self,
        spirit_root: &SpiritRoot,
    ) -> Result<Vec<TaijiToolDomain>, PermissionSystemError> {
        let map = self.spirit_root_whitelist
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?;
        Ok(map.get(spirit_root).cloned().unwrap_or_default())
    }

    async fn set_stone_quota(
        &self,
        agent_id: &AgentId,
        quota: ResourceQuota,
    ) -> Result<(), PermissionSystemError> {
        self.agent_quotas
            .write()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .insert(agent_id.clone(), quota);
        Ok(())
    }

    async fn get_stone_quota(
        &self,
        agent_id: &AgentId,
    ) -> Result<ResourceQuota, PermissionSystemError> {
        let quotas = self.agent_quotas
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?;
        quotas.get(agent_id).cloned().ok_or_else(|| {
            PermissionSystemError::AgentNotFound(agent_id.clone())
        })
    }

    async fn set_title_privilege(
        &self,
        title: &str,
        level: PermissionLevel,
    ) -> Result<(), PermissionSystemError> {
        self.title_privileges
            .write()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .insert(title.to_string(), level);
        Ok(())
    }

    async fn get_title_privilege(
        &self,
        title: &str,
    ) -> Result<PermissionLevel, PermissionSystemError> {
        let privileges = self.title_privileges
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?;
        privileges.get(title).cloned().ok_or_else(|| {
            PermissionSystemError::TitleNotFound(title.to_string())
        })
    }

    async fn set_realm_tool_mapping(
        &self,
        realm: Realm,
        tool_domains: Vec<TaijiToolDomain>,
    ) -> Result<(), PermissionSystemError> {
        self.set_realm_tool_mapping_sync(realm, tool_domains)
    }

    async fn get_realm_tool_mapping(
        &self,
        realm: &Realm,
    ) -> Result<Vec<TaijiToolDomain>, PermissionSystemError> {
        let map = self.realm_tool_mappings
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?;
        Ok(map.get(realm).cloned().unwrap_or_default())
    }

    async fn export_snapshot(&self) -> Result<PermissionSnapshot, PermissionSystemError> {
        let whitelist = self.spirit_root_whitelist
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .clone();
        let quotas = self.agent_quotas
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .clone();
        let titles = self.title_privileges
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .clone();
        let realms = self.realm_tool_mappings
            .read()
            .map_err(|e| PermissionSystemError::Storage(e.to_string()))?
            .clone();

        Ok(PermissionSnapshot {
            spirit_root_whitelist: whitelist,
            agent_quotas: quotas,
            title_privileges: titles,
            realm_tool_mappings: realms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_tool_bus::TaijiToolDomain;
    use taiji_types::agent::{AgentId, SpiritRoot};
    use taiji_types::permission::{PermissionLevel, ResourceQuota};
    use taiji_types::realm::Realm;

    #[tokio::test]
    async fn test_default_whitelist() {
        let mgr = InMemoryPermissionManager::new();
        let whitelist = mgr.get_spirit_root_whitelist().await.unwrap();
        assert_eq!(whitelist.len(), 5); // 5 灵根都有默认白名单
    }

    #[tokio::test]
    async fn test_get_whitelist_for_root() {
        let mgr = InMemoryPermissionManager::new();
        let domains = mgr.get_whitelist_for_root(&SpiritRoot::Metal).await.unwrap();
        assert!(domains.contains(&TaijiToolDomain::Basic));
        assert!(domains.contains(&TaijiToolDomain::Trading));
    }

    #[tokio::test]
    async fn test_set_and_get_whitelist() {
        let mgr = InMemoryPermissionManager::new();
        mgr.set_spirit_root_whitelist(SpiritRoot::Metal, vec![TaijiToolDomain::Basic]).await.unwrap();
        let domains = mgr.get_whitelist_for_root(&SpiritRoot::Metal).await.unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0], TaijiToolDomain::Basic);
    }

    #[tokio::test]
    async fn test_stone_quota() {
        let mgr = InMemoryPermissionManager::new();
        let id = AgentId::new();
        let quota = ResourceQuota { max_rps: 50, max_concurrency: 5, token_budget: 500_000 };
        mgr.set_stone_quota(&id, quota).await.unwrap();

        let got = mgr.get_stone_quota(&id).await.unwrap();
        assert_eq!(got.max_rps, 50);
        assert_eq!(got.token_budget, 500_000);
    }

    #[tokio::test]
    async fn test_stone_quota_not_found() {
        let mgr = InMemoryPermissionManager::new();
        let result = mgr.get_stone_quota(&AgentId::new()).await;
        assert!(matches!(result, Err(PermissionSystemError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_title_privilege() {
        let mgr = InMemoryPermissionManager::new();
        mgr.set_title_privilege("百战真君", PermissionLevel::Expert).await.unwrap();
        let level = mgr.get_title_privilege("百战真君").await.unwrap();
        assert_eq!(level, PermissionLevel::Expert);
    }

    #[tokio::test]
    async fn test_title_privilege_not_found() {
        let mgr = InMemoryPermissionManager::new();
        let result = mgr.get_title_privilege("无此称号").await;
        assert!(matches!(result, Err(PermissionSystemError::TitleNotFound(_))));
    }

    #[tokio::test]
    async fn test_realm_tool_mapping() {
        let mgr = InMemoryPermissionManager::new();
        mgr.set_realm_tool_mapping(Realm::Foundation, vec![TaijiToolDomain::Basic, TaijiToolDomain::Agent]).await.unwrap();
        let domains = mgr.get_realm_tool_mapping(&Realm::Foundation).await.unwrap();
        assert_eq!(domains.len(), 2);
    }

    #[tokio::test]
    async fn test_realm_tool_mapping_default() {
        let mgr = InMemoryPermissionManager::new();
        // QiRefining 有默认映射（Basic）
        let domains = mgr.get_realm_tool_mapping(&Realm::QiRefining).await.unwrap();
        assert_eq!(domains, vec![TaijiToolDomain::Basic]);
    }

    #[tokio::test]
    async fn test_realm_tool_mapping_empty() {
        let mgr = InMemoryPermissionManager::new();
        // GoldenCore 无默认映射 → 空
        let domains = mgr.get_realm_tool_mapping(&Realm::GoldenCore).await.unwrap();
        assert!(domains.is_empty());
    }

    #[tokio::test]
    async fn test_export_snapshot() {
        let mgr = InMemoryPermissionManager::new();
        let snapshot = mgr.export_snapshot().await.unwrap();
        assert_eq!(snapshot.spirit_root_whitelist.len(), 5);
        assert!(snapshot.agent_quotas.is_empty());
        assert!(snapshot.title_privileges.is_empty());
        assert_eq!(snapshot.realm_tool_mappings.len(), 1); // QiRefining
    }

    #[tokio::test]
    async fn test_snapshot_serde() {
        let mgr = InMemoryPermissionManager::new();
        let snapshot = mgr.export_snapshot().await.unwrap();
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: PermissionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.spirit_root_whitelist.len(), 5);
    }

    // ── ProductTier / resolve_tier ──

    #[test]
    fn test_resolve_tier_free() {
        assert_eq!(resolve_tier(Realm::QiRefining), ProductTier::Free);
        assert_eq!(resolve_tier(Realm::Foundation), ProductTier::Free);
    }

    #[test]
    fn test_resolve_tier_standard() {
        assert_eq!(resolve_tier(Realm::GoldenCore), ProductTier::Standard);
        assert_eq!(resolve_tier(Realm::NascentSoul), ProductTier::Standard);
    }

    #[test]
    fn test_resolve_tier_ultimate() {
        assert_eq!(resolve_tier(Realm::SpiritSevering), ProductTier::Ultimate);
        assert_eq!(resolve_tier(Realm::VoidRefining), ProductTier::Ultimate);
    }

    #[test]
    fn test_resolve_tier_enterprise() {
        assert_eq!(resolve_tier(Realm::ImmortalAscension), ProductTier::Enterprise);
    }

    #[test]
    fn test_product_tier_serde() {
        for tier in &[ProductTier::Free, ProductTier::Standard, ProductTier::Ultimate, ProductTier::Enterprise] {
            let json = serde_json::to_string(tier).unwrap();
            let back: ProductTier = serde_json::from_str(&json).unwrap();
            assert_eq!(*tier, back);
        }
    }

    // ── can_access_workshop ──

    #[test]
    fn test_free_cannot_access_jinsuan() {
        assert!(!can_access_workshop(ProductTier::Free, "jinsuan"));
        assert!(!can_access_workshop(ProductTier::Free, "Trading"));
        assert!(can_access_workshop(ProductTier::Free, "tianji"));
        assert!(can_access_workshop(ProductTier::Free, "danqing"));
    }

    #[test]
    fn test_standard_can_access_tianji_and_jinsuan() {
        assert!(can_access_workshop(ProductTier::Standard, "tianji"));
        assert!(can_access_workshop(ProductTier::Standard, "jinsuan"));
        // Standard 默认可访问天机坊+金算坊，其他工坊可能受限
        assert!(can_access_workshop(ProductTier::Standard, "Trading"));
        assert!(can_access_workshop(ProductTier::Standard, "Basic"));
    }

    #[test]
    fn test_ultimate_can_access_all() {
        assert!(can_access_workshop(ProductTier::Ultimate, "tianji"));
        assert!(can_access_workshop(ProductTier::Ultimate, "jinsuan"));
        assert!(can_access_workshop(ProductTier::Ultimate, "danqing"));
        assert!(can_access_workshop(ProductTier::Ultimate, "liuying"));
    }

    #[test]
    fn test_enterprise_can_access_all() {
        assert!(can_access_workshop(ProductTier::Enterprise, "tianji"));
        assert!(can_access_workshop(ProductTier::Enterprise, "jinsuan"));
        assert!(can_access_workshop(ProductTier::Enterprise, "danqing"));
    }
}
