//! WorkshopDag — 工坊 DAG 拓扑排序 + 可执行节点查询。
//!
//! 参考: taiji-engine dag.rs (Phase 2) — Kahn 拓扑排序算法
//!       Phase-工坊系统-类型契约.md §三 — DAG 执行

use std::collections::{HashMap, HashSet, VecDeque};

use taiji_types::workshop_dungeon::WorkshopDagNode;

/// 工坊 DAG 执行引擎。
#[derive(Debug, Clone)]
pub struct WorkshopDag {
    nodes: Vec<WorkshopDagNode>,
    /// 邻接表: node_idx → 依赖此节点的后续节点索引列表
    edges: HashMap<usize, Vec<usize>>,
    /// 入度: node_idx → 前置节点数
    in_degree: HashMap<usize, usize>,
}

impl WorkshopDag {
    /// 从 DAG 节点列表构建 DAG。
    ///
    /// 边由节点间的 input_keys → output_keys 匹配自动推导：
    /// 如果节点 B 的 input_keys 包含节点 A 的某个 output_key，
    /// 则添加 A → B 边。
    pub fn new(nodes: &[WorkshopDagNode]) -> Self {
        // 建立 key → node_index 索引（每个 output key 对应产出节点）
        let mut output_map: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, node) in nodes.iter().enumerate() {
            for key in &node.output_keys {
                output_map.entry(key).or_default().push(i);
            }
        }

        let mut edges: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut in_degree: HashMap<usize, usize> = (0..nodes.len()).map(|i| (i, 0)).collect();

        for (i, node) in nodes.iter().enumerate() {
            for input_key in &node.input_keys {
                if let Some(providers) = output_map.get(input_key.as_str()) {
                    for &provider in providers {
                        if provider != i {
                            edges.entry(provider).or_default().push(i);
                            *in_degree.entry(i).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        Self {
            nodes: nodes.to_vec(),
            edges,
            in_degree,
        }
    }

    /// 拓扑排序（Kahn 算法）。返回节点索引的执行顺序。
    /// 有环时返回 Err(cycle_nodes_name)。
    pub fn sort(&self) -> Result<Vec<usize>, Vec<String>> {
        let mut in_degree = self.in_degree.clone();
        let mut queue: VecDeque<usize> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&i, _)| i)
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(next_nodes) = self.edges.get(&node) {
                for &next in next_nodes {
                    if let Some(deg) = in_degree.get_mut(&next) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(next);
                        }
                    }
                }
            }
        }

        if result.len() == self.nodes.len() {
            Ok(result)
        } else {
            let remaining: HashSet<usize> = (0..self.nodes.len()).collect();
            let processed: HashSet<usize> = result.iter().cloned().collect();
            let cycle_indices: Vec<usize> = remaining.difference(&processed).cloned().collect();
            let cycle_names: Vec<String> = cycle_indices.iter()
                .map(|&i| self.nodes[i].name.clone())
                .collect();
            Err(cycle_names)
        }
    }

    /// 获取可执行的节点索引（所有前置 input 已由 `completed_node_names` 提供）。
    pub fn get_executable_nodes(&self, completed_node_names: &HashSet<String>) -> Vec<usize> {
        let completed_keys: HashSet<&str> = self.nodes.iter()
            .filter(|n| completed_node_names.contains(&n.name))
            .flat_map(|n| n.output_keys.iter().map(|k| k.as_str()))
            .collect();

        let mut executable = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if completed_node_names.contains(&node.name) {
                continue; // 已完成的跳过
            }
            let all_inputs_satisfied = node.input_keys.iter()
                .all(|k| completed_keys.contains(k.as_str()) || k.is_empty());
            if all_inputs_satisfied {
                executable.push(i);
            }
        }
        executable
    }

    /// 返回节点数量。
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取节点的前置依赖名称列表。
    pub fn prerequisites(&self, node_name: &str) -> Vec<String> {
        self.nodes.iter()
            .find(|n| n.name == node_name)
            .map(|n| n.input_keys.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_nodes() -> Vec<WorkshopDagNode> {
        vec![
            WorkshopDagNode { name: "需求分析".into(), description: None, input_keys: vec![], output_keys: vec!["spec".into()] },
            WorkshopDagNode { name: "编码实现".into(), description: None, input_keys: vec!["spec".into()], output_keys: vec!["code".into()] },
            WorkshopDagNode { name: "代码审查".into(), description: None, input_keys: vec!["code".into()], output_keys: vec!["reviewed_code".into()] },
            WorkshopDagNode { name: "构建部署".into(), description: None, input_keys: vec!["reviewed_code".into()], output_keys: vec!["deployment".into()] },
        ]
    }

    #[test]
    fn test_dag_topological_sort() {
        let nodes = make_sample_nodes();
        let dag = WorkshopDag::new(&nodes);
        let order = dag.sort().unwrap();
        // 确保每个节点在依赖它的节点之前
        assert!(order.iter().position(|&i| i == 0) < order.iter().position(|&i| i == 1));
        assert!(order.iter().position(|&i| i == 1) < order.iter().position(|&i| i == 2));
        assert!(order.iter().position(|&i| i == 2) < order.iter().position(|&i| i == 3));
    }

    #[test]
    fn test_dag_executable_nodes() {
        let nodes = make_sample_nodes();
        let dag = WorkshopDag::new(&nodes);

        // 所有节点未完成 → 只有需求分析（无 input）
        let completed = HashSet::new();
        let executable = dag.get_executable_nodes(&completed);
        assert_eq!(executable, vec![0]);

        // 需求分析完成 → 编码实现可执行
        let completed: HashSet<String> = ["需求分析"].into_iter().map(|s| s.to_string()).collect();
        let executable = dag.get_executable_nodes(&completed);
        assert_eq!(executable, vec![1]);

        // 全部完成 → 无
        let completed: HashSet<String> = ["需求分析", "编码实现", "代码审查", "构建部署"].into_iter().map(|s| s.to_string()).collect();
        let executable = dag.get_executable_nodes(&completed);
        assert!(executable.is_empty());
    }

    #[test]
    fn test_dag_cycle_detection() {
        let nodes = vec![
            WorkshopDagNode { name: "A".into(), description: None, input_keys: vec!["b_out".into()], output_keys: vec!["a_out".into()] },
            WorkshopDagNode { name: "B".into(), description: None, input_keys: vec!["a_out".into()], output_keys: vec!["b_out".into()] },
        ];
        let dag = WorkshopDag::new(&nodes);
        let result = dag.sort();
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_prerequisites() {
        let nodes = make_sample_nodes();
        let dag = WorkshopDag::new(&nodes);
        let prereqs = dag.prerequisites("编码实现");
        assert_eq!(prereqs, vec!["spec"]);
    }

    #[test]
    fn test_dag_no_dependency_nodes() {
        let nodes = vec![
            WorkshopDagNode { name: "独立任务A".into(), description: None, input_keys: vec![], output_keys: vec!["a".into()] },
            WorkshopDagNode { name: "独立任务B".into(), description: None, input_keys: vec![], output_keys: vec!["b".into()] },
        ];
        let dag = WorkshopDag::new(&nodes);
        let order = dag.sort().unwrap();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn test_dag_node_count() {
        let nodes = make_sample_nodes();
        let dag = WorkshopDag::new(&nodes);
        assert_eq!(dag.node_count(), 4);
    }
}
