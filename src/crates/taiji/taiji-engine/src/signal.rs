use crate::node::NodeId;
use std::collections::HashMap;
use std::sync::RwLock;

/// 信号分类
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignalCategory {
    Pivot,
    Trend,
    Magnet,
    Risk,
    Custom(String),
}

/// 信号描述符——每个策略 crate 注册其产出的信号
#[derive(Debug, Clone)]
pub struct SignalDescriptor {
    pub name: &'static str,
    pub node: NodeId,
    pub category: SignalCategory,
    pub description: &'static str,
}

/// 全局信号注册表（懒初始化，线程安全）
pub struct SignalRegistry {
    descriptors: HashMap<String, SignalDescriptor>,
}

impl Default for SignalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalRegistry {
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    /// 获取全局单例
    pub fn global() -> &'static RwLock<Self> {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<RwLock<SignalRegistry>> = OnceLock::new();
        INSTANCE.get_or_init(|| RwLock::new(SignalRegistry::new()))
    }

    pub fn register(&mut self, desc: SignalDescriptor) {
        self.descriptors.insert(desc.name.to_string(), desc);
    }

    pub fn get(&self, name: &str) -> Option<&SignalDescriptor> {
        self.descriptors.get(name)
    }

    pub fn list_by_category(&self, cat: &SignalCategory) -> Vec<&SignalDescriptor> {
        self.descriptors
            .values()
            .filter(|d| &d.category == cat)
            .collect()
    }

    pub fn list_by_node(&self, node: &NodeId) -> Vec<&SignalDescriptor> {
        self.descriptors
            .values()
            .filter(|d| &d.node == node)
            .collect()
    }

    pub fn all(&self) -> Vec<&SignalDescriptor> {
        self.descriptors.values().collect()
    }
}
