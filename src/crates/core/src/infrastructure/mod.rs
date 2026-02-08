//! Infrastructure module
//!
//! Provides low-level services: AI clients, storage, event system, workspace path

pub mod ai;
pub mod debug_log;
pub mod events;
pub mod filesystem;
pub mod storage;
pub mod workspace_path;

pub use ai::AIClient;
pub use events::BackendEventManager;
pub use filesystem::{
    file_watcher, get_path_manager_arc, initialize_file_watcher, try_get_path_manager_arc,
    FileInfo, FileOperationOptions, FileOperationService, FileReadResult, FileSearchResult,
    FileTreeNode, FileTreeOptions, FileTreeService, FileTreeStatistics, FileWriteResult,
    PathManager, SearchMatchType,
};
// pub use storage::{};
pub use workspace_path::{get_workspace_path, set_workspace_path};
