use crate::error::Result;
use crate::node::{ComputeNode, NodeConfig};
use std::collections::HashMap;

pub type NodeConstructor = Box<dyn Fn(&NodeConfig) -> Result<Box<dyn ComputeNode>> + Send + Sync>;

pub struct NodeFactory {
    registry: HashMap<String, NodeConstructor>,
}

impl Default for NodeFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeFactory {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    pub fn register(&mut self, type_name: &str, ctor: NodeConstructor) {
        self.registry.insert(type_name.to_string(), ctor);
    }

    pub fn create(&self, type_name: &str, config: &NodeConfig) -> Result<Box<dyn ComputeNode>> {
        match self.registry.get(type_name) {
            Some(ctor) => ctor(config),
            None => Err(crate::error::TaijiError::Config(format!(
                "unknown node type: '{}'",
                type_name
            ))),
        }
    }

    pub fn list_types(&self) -> Vec<&str> {
        self.registry.keys().map(|s| s.as_str()).collect()
    }

    pub fn contains(&self, type_name: &str) -> bool {
        self.registry.contains_key(type_name)
    }
}

/// 一行注册 ComputeNode 到 NodeFactory。
///
/// 用法：
/// ```ignore
/// register_node!(factory, "ma_cross", taiji_example::MaCross, "ma_cross");
/// register_node!(factory, "bar_node", taiji_bar::BarNode, "bar_node");
/// ```
#[macro_export]
macro_rules! register_node {
    ($factory:expr, $type_name:expr, $node_ty:ty, $id:expr) => {
        $factory.register(
            $type_name,
            Box::new(|config: &$crate::node::NodeConfig| -> $crate::error::Result<Box<dyn $crate::node::ComputeNode>> {
                let mut node = <$node_ty>::new($id.into());
                let store = $crate::store::StateStore::new();
                node.on_init(config, &store)?;
                Ok(Box::new(node))
            }),
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::node::{ComputeNode, NodeConfig};
    use crate::store::StateStore;
    use crate::types::bar::{Freq, RawBar};
    use crate::types::state::StateKey;

    struct MockNode {
        id: String,
    }
    impl ComputeNode for MockNode {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn name(&self) -> &'static str {
            "mock"
        }
        fn input_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn output_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
            Ok(())
        }
        fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_register_and_create() {
        let mut factory = NodeFactory::new();
        factory.register(
            "mock",
            Box::new(|config: &NodeConfig| {
                let _ = config;
                Ok(Box::new(MockNode { id: "mock1".into() }))
            }),
        );
        let node = factory.create("mock", &NodeConfig::new()).unwrap();
        assert_eq!(node.id(), "mock1");
    }

    #[test]
    fn test_unknown_type() {
        let factory = NodeFactory::new();
        assert!(factory.create("nonexistent", &NodeConfig::new()).is_err());
    }

    #[test]
    fn test_list_types() {
        let mut factory = NodeFactory::new();
        factory.register(
            "a",
            Box::new(|_: &NodeConfig| Ok(Box::new(MockNode { id: "a".into() }))),
        );
        factory.register(
            "b",
            Box::new(|_: &NodeConfig| Ok(Box::new(MockNode { id: "b".into() }))),
        );
        assert_eq!(factory.list_types().len(), 2);
    }
}
