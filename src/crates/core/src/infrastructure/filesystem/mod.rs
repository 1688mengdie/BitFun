//! Filesystem infrastructure
//!
//! File operations, file tree building, file watching, and path management.

pub mod file_tree;
pub mod file_operations;
pub mod file_watcher;
pub mod path_manager;

pub use path_manager::{
    PathManager,
    StorageLevel,
    CacheType,
    get_path_manager_arc,
    try_get_path_manager_arc,
};
pub use file_tree::{
    FileTreeService,
    FileTreeNode,
    FileTreeOptions,
    FileTreeStatistics,
    FileSearchResult,
    SearchMatchType,
};
pub use file_operations::{
    FileOperationService,
    FileOperationOptions,
    FileInfo,
    FileReadResult,
    FileWriteResult,
};
#[cfg(feature = "tauri-support")]
pub use file_watcher::{start_file_watch, stop_file_watch, get_watched_paths};
pub use file_watcher::initialize_file_watcher;
