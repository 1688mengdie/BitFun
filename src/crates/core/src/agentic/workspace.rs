use std::path::{Path, PathBuf};

/// Session-bound workspace information used during agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceBinding {
    pub workspace_id: Option<String>,
    pub root_path: PathBuf,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root_path: PathBuf) -> Self {
        Self {
            workspace_id,
            root_path,
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn root_path_string(&self) -> String {
        self.root_path.to_string_lossy().to_string()
    }
}
