//! Workspace path management
//!
//! Provides global workspace path set/get

use std::path::PathBuf;
use std::sync::RwLock;

static GLOBAL_WORKSPACE_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

pub fn set_workspace_path(workspace_path: Option<PathBuf>) {
    if let Ok(mut path) = GLOBAL_WORKSPACE_PATH.write() {
        *path = workspace_path;
    }
}

pub fn get_workspace_path() -> Option<PathBuf> {
    GLOBAL_WORKSPACE_PATH
        .read()
        .ok()
        .and_then(|path| path.clone())
}
