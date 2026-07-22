use std::collections::{HashMap, VecDeque};

pub type NodeId = String;

pub struct Dag {
    edges: HashMap<NodeId, Vec<NodeId>>, // from → [to]
    in_degree: HashMap<NodeId, usize>,
    nodes: Vec<NodeId>,
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

impl Dag {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            in_degree: HashMap::new(),
            nodes: Vec::new(),
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, id: NodeId) {
        if !self.in_degree.contains_key(&id) {
            self.in_degree.insert(id.clone(), 0);
            self.edges.entry(id.clone()).or_default();
            self.nodes.push(id);
        }
    }

    /// 添加有向边 from → to。自动注册节点如果不存在。重复边为幂等操作。
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.add_node(from.clone());
        self.add_node(to.clone());
        let neighbors = self.edges.entry(from.clone()).or_default();
        if neighbors.contains(&to) {
            return; // 重复边，跳过
        }
        neighbors.push(to.clone());
        *self.in_degree.entry(to).or_insert(0) += 1;
    }

    /// Kahn 拓扑排序。返回按层分组的执行顺序。
    /// 如果有循环依赖，返回 Err 并列出环中节点。
    pub fn sort(&self) -> Result<Vec<Vec<NodeId>>, Vec<NodeId>> {
        let mut in_deg = self.in_degree.clone();
        let mut queue: VecDeque<NodeId> = VecDeque::new();

        for (id, deg) in &in_deg {
            if *deg == 0 {
                queue.push_back(id.clone());
            }
        }

        let mut layers: Vec<Vec<NodeId>> = Vec::new();
        let mut processed = 0usize;

        while !queue.is_empty() {
            let layer: Vec<NodeId> = queue.drain(..).collect();
            for id in &layer {
                processed += 1;
                if let Some(neighbors) = self.edges.get(id) {
                    for n in neighbors {
                        if let Some(d) = in_deg.get_mut(n) {
                            *d -= 1;
                            if *d == 0 {
                                queue.push_back(n.clone());
                            }
                        }
                    }
                }
            }
            layers.push(layer);
        }

        if processed < self.nodes.len() {
            // 环中节点 = 仍有余入度的节点
            let cycle_nodes: Vec<NodeId> = in_deg
                .iter()
                .filter(|(_, &d)| d > 0)
                .map(|(id, _)| id.clone())
                .collect();
            Err(cycle_nodes)
        } else {
            // 防御性检查：排序结果中不应有重复节点
            let total: usize = layers.iter().map(|l| l.len()).sum();
            debug_assert_eq!(
                total,
                self.nodes.len(),
                "duplicate nodes in sort result: {} unique positions for {} nodes",
                total,
                self.nodes.len()
            );
            Ok(layers)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_chain() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("B".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 3); // A → B → C
    }

    #[test]
    fn test_fork() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("A".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 2); // A → [B, C]
    }

    #[test]
    fn test_merge() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "C".into());
        dag.add_edge("B".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 2); // [A, B] → C
    }

    #[test]
    fn test_cycle_detected() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("B".into(), "C".into());
        dag.add_edge("C".into(), "A".into());
        assert!(dag.sort().is_err());
    }

    #[test]
    fn test_add_duplicate_edge_is_idempotent() {
        // 无重复边的基准
        let mut dag1 = Dag::new();
        dag1.add_edge("A".into(), "B".into());
        dag1.add_edge("B".into(), "C".into());
        dag1.add_edge("A".into(), "C".into());
        let layers1 = dag1.sort().unwrap();

        // 相同 DAG，但 A→B 重复添加一次
        let mut dag2 = Dag::new();
        dag2.add_edge("A".into(), "B".into());
        dag2.add_edge("A".into(), "B".into()); // 重复边，应被去重
        dag2.add_edge("B".into(), "C".into());
        dag2.add_edge("A".into(), "C".into());
        let layers2 = dag2.sort().unwrap();

        // 重复边不应改变排序结果
        assert_eq!(layers1, layers2);
        assert_eq!(layers1.len(), 3); // A → B → C（C 同时依赖 A 或 B）
    }
}
