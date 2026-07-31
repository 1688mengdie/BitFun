//! file-store（储物阁）— 文件/资源存储模块
//!
//! 提供统一文件存储接口 FileStorage trait，支持多种后端：
//! - LocalFileStorage：本地文件系统
//! - S3FileStorage：AWS S3 / MinIO（feature-gated）
//!
//! 来源: gbrain storage.ts (MIT) / biliup upload.rs + util.rs (MIT)

pub mod storage;
pub mod config;
pub mod meta;
pub mod error;
pub mod local;
pub mod path;
pub mod pipeline;

#[cfg(feature = "s3")]
pub mod s3_storage;

pub use storage::{FileStorage, create_file_storage};
pub use config::{FileStoreConfig, FileStoreKind, S3Config};
pub use meta::FileMeta;
pub use error::FileStoreError;
pub use local::LocalFileStorage;
pub use path::{sanitize_path, FilePathGenerator};
pub use pipeline::{UploadTask, UploadResult, UploadPipeline};
