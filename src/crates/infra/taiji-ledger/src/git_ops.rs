//! Git 语义编排 — Agent 状态管理的 git 操作映射
//!
//! 来源: modules/ledger/接口设计.md §2 — 功德簿语义编排

use std::path::{Path, PathBuf};

use git2::{Commit, Repository};
use taiji_types::agent::AgentId;

use crate::error::GitError;

/// Git 语义编排 — Agent 状态管理的 git 操作映射
pub struct GitLedger {
    repo_path: PathBuf,
    repo: Repository,
}

impl GitLedger {
    pub fn new(repo_path: &Path) -> Result<Self, GitError> {
        let repo = if repo_path.join(".git").exists() {
            Repository::open(repo_path)?
        } else {
            std::fs::create_dir_all(repo_path).map_err(|e| GitError::Io(e.to_string()))?;
            Repository::init(repo_path)?
        };
        Ok(Self { repo_path: repo_path.to_path_buf(), repo })
    }

    /// commit = 任务完成自动提交
    pub async fn commit(&self, agent_id: &AgentId, message: &str) -> Result<String, GitError> {
        let sig = self.signature()?;
        let mut index = self.repo.index()?;
        let state_path = format!("agents/{}/state.json", agent_id);
        let parent = Path::new(&state_path).parent()
            .ok_or_else(|| GitError::Io(format!("state_path 无父目录: {}", state_path)))?;
        std::fs::create_dir_all(self.repo_path.join(parent))
            .map_err(|e| GitError::Io(e.to_string()))?;
        let content = serde_json::json!({
            "agent_id": agent_id, "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let content_str = serde_json::to_string_pretty(&content)
            .map_err(|e| GitError::Io(e.to_string()))?;
        std::fs::write(self.repo_path.join(&state_path), &content_str)
            .map_err(|e| GitError::Io(e.to_string()))?;
        index.add_path(Path::new(&state_path))?;
        let tree_oid = index.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;
        let parent_commit = self.find_parent();
        let oid = if let Some(ref parent) = parent_commit {
            self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[parent])?
        } else {
            self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?
        };
        Ok(oid.to_string())
    }

    /// reincarnate = 转世重生
    pub async fn reincarnate(&self, target_commit: &str) -> Result<(), GitError> {
        let oid = git2::Oid::from_str(target_commit)
            .map_err(|e| GitError::CommandFailed(format!("invalid commit: {}", e)))?;
        let commit = self.repo.find_commit(oid)?;

        // 使用 Hard reset 到 commit 对象
        self.repo.reset(commit.as_object(), git2::ResetType::Hard, None)?;
        Ok(())
    }

    /// cherry_pick = 夺舍
    pub async fn cherry_pick(&self, commit_id: &str) -> Result<(), GitError> {
        let oid = git2::Oid::from_str(commit_id)
            .map_err(|e| GitError::CommandFailed(format!("invalid commit: {}", e)))?;
        let commit = self.repo.find_commit(oid)?;
        self.repo.cherrypick(&commit, None)?;

        // 检查冲突
        let mut index = self.repo.index()?;
        if index.has_conflicts() {
            return Err(GitError::MergeConflict("cherry-pick conflict".into()));
        }

        let sig = self.signature()?;
        let msg = match commit.summary() {
            Ok(Some(s)) => format!("cherry-pick: {}", s),
            _ => format!("cherry-pick: {}", commit_id),
        };
        let tree_oid = index.write_tree()?;
        let tree_obj = self.repo.find_tree(tree_oid)?;
        let parent = self.find_parent();
        if let Some(ref p) = parent {
            self.repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree_obj, &[p])?;
        } else {
            self.repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree_obj, &[])?;
        };
        self.repo.cleanup_state()?;
        Ok(())
    }

    /// fork_branch = 身外化身
    pub async fn fork_branch(&self, branch_name: &str) -> Result<(), GitError> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        self.repo.branch(branch_name, &commit, false)?;
        Ok(())
    }

    /// get_graph = 功德簿 log
    pub fn get_graph(&self) -> Result<String, GitError> {
        // 空仓库无 HEAD 时直接返回
        if self.repo.head().is_err() {
            return Ok("(empty ledger)".into());
        }
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::TIME)?;
        revwalk.push_head()?;
        let mut graph = String::new();
        for oid in revwalk {
            let oid = oid?;
            if let Ok(commit) = self.repo.find_commit(oid) {
                let short = &oid.to_string()[..7];
                let msg = match commit.summary() {
                    Ok(Some(s)) => s.to_string(),
                    _ => "(no message)".to_string(),
                };
                graph.push_str(&format!("* {} - {}\n", short, msg));
            }
        }
        if graph.is_empty() { graph = "(empty ledger)".into(); }
        Ok(graph)
    }

    fn signature(&self) -> Result<git2::Signature<'static>, GitError> {
        git2::Signature::now("ledger", "ledger@lvpa.local")
            .map_err(|e| GitError::Git2(e.message().to_string()))
    }

    fn find_parent(&self) -> Option<Commit<'_>> {
        self.repo.head().ok().and_then(|h| h.peel_to_commit().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_repo(name: &str) -> (GitLedger, PathBuf) {
        let dir = env::temp_dir().join(format!("taiji_ledger_test_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        (GitLedger::new(&dir).unwrap(), dir)
    }

    #[tokio::test]
    async fn test_git_commit() {
        let (ledger, dir) = setup_repo("commit");
        let agent = AgentId::new();
        let hash = ledger.commit(&agent, "test commit").await.unwrap();
        assert!(!hash.is_empty());
        assert!(ledger.repo.find_commit(git2::Oid::from_str(&hash).unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_git_fork_branch() {
        let (ledger, dir) = setup_repo("fork");
        ledger.commit(&AgentId::new(), "initial").await.unwrap();
        ledger.fork_branch("child-agent").await.unwrap();
        assert!(ledger.repo.find_branch("child-agent", git2::BranchType::Local).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_git_cherry_pick() {
        let (ledger, dir) = setup_repo("cherry");
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();
        let hash = ledger.commit(&agent1, "agent1 work").await.unwrap();
        ledger.commit(&agent2, "agent2 work").await.unwrap();
        // Cherry-pick from agent1 (different file, no conflict)
        ledger.cherry_pick(&hash).await.unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_git_get_graph() {
        let (ledger, dir) = setup_repo("graph");
        let agent = AgentId::new();
        assert_eq!(ledger.get_graph().unwrap(), "(empty ledger)");
        ledger.commit(&agent, "first").await.unwrap();
        ledger.commit(&agent, "second").await.unwrap();
        let g = ledger.get_graph().unwrap();
        assert!(g.contains("first") && g.contains("second"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_git_reincarnate() {
        let (ledger, dir) = setup_repo("reincarnate");
        let agent = AgentId::new();
        let h1 = ledger.commit(&agent, "v1").await.unwrap();
        ledger.commit(&agent, "v2").await.unwrap();
        ledger.reincarnate(&h1).await.unwrap();
        assert_eq!(ledger.repo.head().unwrap().peel_to_commit().unwrap().id().to_string(), h1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
