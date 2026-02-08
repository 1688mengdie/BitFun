//! File system service module
//!
//! Integrates file operations, file tree building, search, and related functionality.

pub mod factory;
pub mod service;
pub mod types;

pub use factory::FileSystemServiceFactory;
pub use service::FileSystemService;
pub use types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};
