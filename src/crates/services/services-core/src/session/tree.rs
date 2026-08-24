//! Conversation tree manager — an in-memory parent/child index that mirrors the
//! persisted `SessionRelationship` lineage.
//!
//! This is a pure data structure, not persisted. All relationship facts are
//! driven by `SessionMetadata.relationship` (and re-built from creator markers
//! for the SESSION-11 crash window). Traversal is bounded by
//! `MAX_TREE_RECURSION_DEPTH` to prevent stack overflow in deep trees.

use bitfun_core_types::session_tree::MAX_TREE_RECURSION_DEPTH;
use std::collections::HashMap;
use std::sync::RwLock;

/// Session tree error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTreeError {
    CycleDetected { child_id: String, ancestor: String },
    SelfReference(String),
}

/// Conversation tree manager - pure in-memory data structure, not persisted.
pub struct SessionTreeManager {
    /// parent_id -> child_ids mapping
    edges: RwLock<HashMap<String, Vec<String>>>,
    /// child_id -> parent_id reverse index (O(1) parent lookup)
    child_to_parent: RwLock<HashMap<String, String>>,
    /// session_id -> depth mapping
    depths: RwLock<HashMap<String, u32>>,
    /// Maximum nesting depth
    pub max_depth: u32,
}

impl SessionTreeManager {
    pub fn new(max_depth: u32) -> Self {
        Self {
            edges: RwLock::new(HashMap::new()),
            child_to_parent: RwLock::new(HashMap::new()),
            depths: RwLock::new(HashMap::new()),
            max_depth,
        }
    }

    /// Register a parent-child relationship.
    ///
    /// Depth values exceeding `max_depth` are clamped with a warning instead of
    /// rejecting the registration, preventing cascading failures in deep trees.
    /// Cycle and self-reference are rejected.
    pub fn register_child(
        &self,
        parent_id: &str,
        child_id: &str,
        depth: u32,
    ) -> Result<(), SessionTreeError> {
        if child_id == parent_id {
            return Err(SessionTreeError::SelfReference(child_id.to_string()));
        }
        let clamped_depth = if depth > self.max_depth {
            log::warn!(
                "register_child: depth {} exceeds max_depth {} for child_id={}, clamping",
                depth,
                self.max_depth,
                child_id
            );
            self.max_depth
        } else {
            depth
        };
        // Cycle check: walking up the reverse index must not reach the child.
        let mut current = parent_id.to_string();
        loop {
            match self.get_parent(&current) {
                Some(p) if p == child_id => {
                    return Err(SessionTreeError::CycleDetected {
                        child_id: child_id.to_string(),
                        ancestor: current,
                    });
                }
                Some(p) => current = p,
                None => break,
            }
        }
        let mut edges = self.edges.write().unwrap();
        edges
            .entry(parent_id.to_string())
            .or_default()
            .push(child_id.to_string());
        self.child_to_parent
            .write()
            .unwrap()
            .insert(child_id.to_string(), parent_id.to_string());
        self.depths
            .write()
            .unwrap()
            .insert(child_id.to_string(), clamped_depth);
        Ok(())
    }

    /// Calculate subtree max depth (iterative DFS to prevent stack overflow).
    pub fn subtree_depth(&self, session_id: &str) -> u32 {
        let mut max_depth: u32 = 0;
        let mut stack: Vec<(String, u32)> = vec![(session_id.to_string(), 0)];
        let mut visited = std::collections::HashSet::new();

        while let Some((id, recursion_depth)) = stack.pop() {
            if recursion_depth > MAX_TREE_RECURSION_DEPTH {
                continue;
            }
            if !visited.insert(id.clone()) {
                continue;
            }
            let own = self.depths.read().unwrap().get(&id).copied().unwrap_or(0);
            max_depth = max_depth.max(own);
            if let Some(children) = self.edges.read().unwrap().get(&id) {
                for child_id in children.iter() {
                    stack.push((child_id.clone(), recursion_depth + 1));
                }
            }
        }

        max_depth
    }

    /// Get direct child node IDs
    pub fn get_children(&self, session_id: &str) -> Vec<String> {
        self.edges
            .read()
            .unwrap()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all descendant node IDs (direct and indirect children), BFS traversal
    pub fn get_descendants(&self, session_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut stack = vec![session_id.to_string()];
        let mut seen = std::collections::HashSet::new();
        seen.insert(session_id.to_string()); // exclude self
        while let Some(id) = stack.pop() {
            for child in self.get_children(&id) {
                if seen.insert(child.clone()) {
                    result.push(child.clone());
                    stack.push(child);
                }
            }
        }
        result
    }

    /// Get the parent node (O(1) reverse-index lookup)
    pub fn get_parent(&self, session_id: &str) -> Option<String> {
        self.child_to_parent
            .read()
            .unwrap()
            .get(session_id)
            .cloned()
    }

    /// Get the depth of a node (O(1) lookup)
    pub fn get_depth(&self, session_id: &str) -> Option<u32> {
        self.depths.read().unwrap().get(session_id).copied()
    }

    /// Collect all ancestor session_ids along the parent chain (nearest first)
    pub fn walk_ancestors(&self, session_id: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = session_id.to_string();
        while let Some(parent) = self.get_parent(&current) {
            ancestors.push(parent.clone());
            current = parent;
        }
        ancestors
    }

    /// Remove a subtree (iterative, not recursive - prevents stack overflow).
    pub fn remove_subtree(&self, session_id: &str) {
        let mut stack = vec![session_id.to_string()];
        let mut to_remove = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            to_remove.push(id.clone());
            for child in self.get_children(&id) {
                stack.push(child);
            }
        }
        for id in &to_remove {
            if let Some(parent_id) = self.get_parent(id) {
                if let Some(mut parent_children) = self.edges.write().unwrap().get_mut(&parent_id) {
                    parent_children.retain(|x| x != id);
                }
            }
            self.edges.write().unwrap().remove(id);
            self.child_to_parent.write().unwrap().remove(id);
            self.depths.write().unwrap().remove(id);
        }
    }

    /// Batch-load tree relationships from persisted session metadata, rebuilding
    /// lost lineage from creator markers (`session-<parent_session_id>`) or the
    /// `parentSessionId` custom-metadata key.
    ///
    /// Depth for each child is derived from the registered parent's depth + 1
    /// (falling back to 1) rather than trusting a persisted field.
    pub fn load_from_sessions(&self, sessions: &[crate::session::types::SessionMetadata]) {
        self.edges.write().unwrap().clear();
        self.child_to_parent.write().unwrap().clear();
        self.depths.write().unwrap().clear();
        for session in sessions {
            if let Some(ref relationship) = session.relationship {
                if let Some(ref parent_id) = relationship.parent_session_id {
                    let depth = self.get_depth(parent_id).map(|d| d + 1).unwrap_or(1);
                    if let Err(e) = self.register_child(parent_id, &session.session_id, depth) {
                        log::warn!(
                            "Failed to register child session {} under {} in tree during load: {:?}",
                            session.session_id,
                            parent_id,
                            e
                        );
                    }
                }
            }
        }
        for session in sessions {
            if session.relationship.is_some() {
                continue;
            }
            let Some(parent_id) = lineage_rebuild_parent_session_id(session) else {
                continue;
            };
            if parent_id == session.session_id {
                continue;
            }
            let depth = self.get_depth(&parent_id).map(|d| d + 1).unwrap_or(1);
            if let Err(e) = self.register_child(&parent_id, &session.session_id, depth) {
                log::warn!(
                    "Lineage rebuild failed for session {} under {}: {:?}",
                    session.session_id,
                    parent_id,
                    e
                );
            }
        }
    }
}

/// Recover the lost parent session id of a session whose `SessionRelationship`
/// was never persisted (crash window). Honors the `session-<parent>` creator
/// marker in `created_by`, plus free-form `parentSessionId` / `createdBy`
/// custom-metadata keys. Non-marker creator values are not lineage facts.
fn lineage_rebuild_parent_session_id(
    session: &crate::session::types::SessionMetadata,
) -> Option<String> {
    if let Some(serde_json::Value::Object(metadata)) = session.custom_metadata.as_ref() {
        if let Some(parent_id) = metadata
            .get("parentSessionId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(parent_id.to_string());
        }
    }
    session
        .created_by
        .as_deref()
        .and_then(creator_marker_parent_session_id)
        .or_else(|| {
            session
                .custom_metadata
                .as_ref()
                .and_then(|value| value.get("createdBy"))
                .and_then(|value| value.as_str())
                .and_then(creator_marker_parent_session_id)
        })
}

/// Parse the `session-<parent_session_id>` creator marker. Returns None for any
/// other shape so non-lineage creator values are never mistaken for parentage.
fn creator_marker_parent_session_id(marker: &str) -> Option<String> {
    let parent_id = marker.trim().strip_prefix("session-")?;
    let parent_id = parent_id.trim();
    (!parent_id.is_empty()).then(|| parent_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_query_child() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "child-1", 1).unwrap();
        assert_eq!(mgr.get_children("root"), vec!["child-1"]);
        assert_eq!(mgr.get_parent("child-1"), Some("root".to_string()));
        assert_eq!(mgr.get_depth("child-1"), Some(1));
    }

    #[test]
    fn depth_calculation_five_levels() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "l1", 1).unwrap();
        mgr.register_child("l1", "l2", 2).unwrap();
        mgr.register_child("l2", "l3", 3).unwrap();
        mgr.register_child("l3", "l4", 4).unwrap();
        mgr.register_child("l4", "l5", 5).unwrap();
        assert_eq!(mgr.subtree_depth("root"), 5);
    }

    #[test]
    fn remove_subtree_cascading() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        mgr.register_child("a", "b", 2).unwrap();
        mgr.remove_subtree("a");
        assert!(mgr.get_children("a").is_empty());
        assert!(mgr.get_parent("a").is_none());
    }

    #[test]
    fn walk_ancestors_from_leaf() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        mgr.register_child("a", "b", 2).unwrap();
        let ancestors = mgr.walk_ancestors("b");
        assert_eq!(ancestors, vec!["a", "root"]);
    }

    #[test]
    fn test_register_child_rejects_cycle() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("A", "B", 1).unwrap();
        mgr.register_child("B", "C", 2).unwrap();
        let result = mgr.register_child("C", "A", 3);
        assert!(matches!(
            result,
            Err(SessionTreeError::CycleDetected { .. })
        ));
    }

    #[test]
    fn test_register_child_rejects_self_reference() {
        let mgr = SessionTreeManager::new(5);
        let result = mgr.register_child("A", "A", 1);
        assert!(matches!(result, Err(SessionTreeError::SelfReference(_))));
    }

    #[test]
    fn test_register_child_clamps_excessive_depth() {
        let mgr = SessionTreeManager::new(5);
        let result = mgr.register_child("A", "B", 6);
        assert!(result.is_ok());
        assert_eq!(mgr.get_depth("B"), Some(5));
    }
}
