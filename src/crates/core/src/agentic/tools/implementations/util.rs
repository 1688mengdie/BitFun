use crate::infrastructure::get_workspace_path;
use log::warn;
use std::path::Path;
use std::path::{Component, PathBuf};

pub fn normalize_path(path: &str) -> String {
    let path = Path::new(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {} // Ignore "."
            Component::ParentDir => {
                // Handle ".."
                if !components.is_empty() {
                    components.pop();
                }
            }
            c => components.push(c),
        }
    }
    components
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .to_string()
}

pub fn resolve_path(path: &str) -> String {
    if Path::new(path).is_absolute() {
        normalize_path(path)
    } else {
        // Relative paths need to be resolved based on workspace path
        match get_workspace_path() {
            Some(workspace_path) => {
                normalize_path(&workspace_path.join(path).to_string_lossy().to_string())
            }
            None => {
                warn!(
                    "Workspace path not set, using current directory to resolve relative path: {}",
                    path
                );
                path.to_string()
            }
        }
    }
}
