//! 图遍历检索 — 基于页面间引用关系的图查询。
//!
//! 广度优先搜索（BFS）深度 N=2，寻找与初始结果关联的页面。
//!
//! 参考: gbrain (MIT) core/search — R-5-301 — Rust 翻译实现

use std::collections::{HashSet, VecDeque};

use taiji_types::knowledge::Page;

/// 从初始页面列表出发，BFS 查找关联页面。
///
/// 通过页面 metadata 中的 `links` 字段（String 列表）建立引用关系。
/// 深度限制为 `max_depth`（默认 2）。
pub fn bfs_related_pages(
    seeds: &[Page],
    all_pages: &[Page],
    max_depth: usize,
) -> Vec<Page> {
    if seeds.is_empty() || all_pages.is_empty() {
        return vec![];
    }

    // 建立 slug → Page 索引
    let page_map: std::collections::HashMap<String, &Page> = all_pages
        .iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    // 建立 slug → links 映射
    let link_map: std::collections::HashMap<String, Vec<String>> = all_pages
        .iter()
        .map(|p| {
            let links = p
                .metadata
                .get("links")
                .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
                .unwrap_or_default();
            (p.id.clone(), links)
        })
        .collect();

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut related: Vec<Page> = Vec::new();

    // 初始种子入队
    for seed in seeds {
        if visited.insert(seed.id.clone()) {
            queue.push_back((seed.id.clone(), 0));
        }
    }

    while let Some((slug, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        if let Some(links) = link_map.get(&slug) {
            for linked_slug in links {
                if visited.insert(linked_slug.clone()) {
                    if let Some(page) = page_map.get(linked_slug) {
                        related.push((*page).clone());
                    }
                    queue.push_back((linked_slug.clone(), depth + 1));
                }
            }
        }
    }

    related
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_page(id: &str, links: Vec<&str>) -> Page {
        Page {
            id: id.to_string(),
            title: id.to_string(),
            content: String::new(),
            source_id: "test".into(),
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: serde_json::json!({"links": links}),
        }
    }

    fn make_page_no_links(id: &str) -> Page {
        Page {
            id: id.to_string(),
            title: id.to_string(),
            content: String::new(),
            source_id: "test".into(),
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_empty_seeds() {
        let all = vec![make_page("a", vec![])];
        let result = bfs_related_pages(&[], &all, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_links() {
        let seeds = vec![make_page_no_links("a")];
        let all = vec![
            make_page_no_links("a"),
            make_page_no_links("b"),
        ];
        let result = bfs_related_pages(&seeds, &all, 2);
        assert!(result.is_empty(), "no links should yield no relations");
    }

    #[test]
    fn test_direct_link() {
        let seeds = vec![make_page("a", vec!["b"])];
        let all = vec![
            make_page("a", vec!["b"]),
            make_page("b", vec![]),
        ];
        let result = bfs_related_pages(&seeds, &all, 2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn test_depth_2() {
        // a → b → c
        let seeds = vec![make_page("a", vec!["b"])];
        let all = vec![
            make_page("a", vec!["b"]),
            make_page("b", vec!["c"]),
            make_page("c", vec![]),
        ];
        let result = bfs_related_pages(&seeds, &all, 2);
        assert_eq!(result.len(), 2);
        let ids: HashSet<String> = result.into_iter().map(|p| p.id).collect();
        assert!(ids.contains("b"));
        assert!(ids.contains("c"));
    }

    #[test]
    fn test_depth_limit() {
        // a → b → c → d, max_depth=1 → only b
        let seeds = vec![make_page("a", vec!["b"])];
        let all = vec![
            make_page("a", vec!["b"]),
            make_page("b", vec!["c"]),
            make_page("c", vec!["d"]),
            make_page("d", vec![]),
        ];
        let result = bfs_related_pages(&seeds, &all, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn test_self_link_ignored() {
        let seeds = vec![make_page("a", vec!["a", "b"])];
        let all = vec![
            make_page("a", vec!["a", "b"]),
            make_page("b", vec![]),
        ];
        let result = bfs_related_pages(&seeds, &all, 2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn test_no_link_field() {
        let seeds = vec![make_page("a", vec![])];
        let all = vec![make_page("a", vec![])];
        let result = bfs_related_pages(&seeds, &all, 2);
        assert!(result.is_empty());
    }
}
