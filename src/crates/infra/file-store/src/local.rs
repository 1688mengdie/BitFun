//! file-store（储物阁）— 本地文件系统实现
//!
//! 来源: gbrain storage/local.ts (MIT)

use tokio::io::AsyncRead;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::FileStoreError;
use crate::meta::FileMeta;
use crate::path::sanitize_path;
use crate::storage::FileStorage;

/// 本地文件存储实现
///
/// 基于 tokio::fs 的本地文件系统后端。
pub struct LocalFileStorage {
    /// 根目录
    root: PathBuf,
}

impl LocalFileStorage {
    /// 创建本地文件存储
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
        }
    }

    /// 将相对路径解析为绝对路径（安全检查：防止路径遍历攻击）
    fn resolve_path(&self, path: &str) -> Result<PathBuf, FileStoreError> {
        // 按路径组件逐段清洗，保留目录结构
        let safe_path: PathBuf = Path::new(path)
            .components()
            .map(|c| sanitize_path(&c.as_os_str().to_string_lossy()))
            .collect();
        let full = self.root.join(&safe_path);

        // 安全检查：确保解析后的路径仍在 root 下
        if !full.starts_with(&self.root) {
            return Err(FileStoreError::InvalidPath(
                "路径遍历攻击被阻止".into(),
            ));
        }
        Ok(full)
    }

    /// 确保父目录存在
    async fn ensure_parent_dir(&self, path: &Path) -> Result<(), FileStoreError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| FileStoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// 计算 SHA256 哈希
    async fn compute_hash(&self, path: &PathBuf) -> Result<String, FileStoreError> {
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;
        let hash = Sha256::digest(&data);
        Ok(format!("{:x}", hash))
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn write(&self, path: &str, data: &[u8]) -> Result<FileMeta, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        self.ensure_parent_dir(&full_path).await?;
        tokio::fs::write(&full_path, data)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;
        self.metadata(path).await
    }

    async fn write_stream(
        &self,
        path: &str,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        size_hint: Option<u64>,
    ) -> Result<FileMeta, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        self.ensure_parent_dir(&full_path).await?;

        let mut file = tokio::fs::File::create(&full_path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;

        let mut reader = reader;
        tokio::io::copy(&mut reader, &mut file)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;

        file.flush()
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;

        let _ = size_hint;
        self.metadata(path).await
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        if !full_path.exists() {
            return Err(FileStoreError::NotFound(path.into()));
        }
        tokio::fs::read(&full_path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))
    }

    async fn read_stream(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        if !full_path.exists() {
            return Err(FileStoreError::NotFound(path.into()));
        }
        let file = tokio::fs::File::open(&full_path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;
        Ok(Box::new(file))
    }

    async fn delete(&self, path: &str) -> Result<bool, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        if !full_path.exists() {
            return Ok(false);
        }
        tokio::fs::remove_file(&full_path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;
        Ok(true)
    }

    async fn exists(&self, path: &str) -> Result<bool, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        Ok(full_path.exists())
    }

    async fn metadata(&self, path: &str) -> Result<FileMeta, FileStoreError> {
        let full_path = self.resolve_path(path)?;
        let meta = tokio::fs::metadata(&full_path)
            .await
            .map_err(|e| FileStoreError::NotFound(format!("{}: {}", path, e)))?;

        // 计算 SHA256 哈希
        let content_hash = if meta.is_file() {
            self.compute_hash(&full_path).await?
        } else {
            String::new()
        };

        Ok(FileMeta {
            path: path.to_string(),
            size: meta.len(),
            mime_type: mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string(),
            content_hash,
            last_modified: meta
                .modified()
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(|_| Utc::now()),
            is_dir: meta.is_dir(),
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<FileMeta>, FileStoreError> {
        let dir = self.resolve_path(prefix)?;
        let mut result = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?
        {
            let path_str = entry.path().to_string_lossy().to_string();
            // 转换为相对路径
            let root_str = self.root.to_string_lossy();
            if let Some(rel) = path_str.strip_prefix(root_str.as_ref()) {
                let rel_path = rel.trim_start_matches('\\').trim_start_matches('/');
                if let Ok(meta) = self.metadata(rel_path).await {
                    result.push(meta);
                }
            }
        }
        Ok(result)
    }

    async fn get_url(&self, _path: &str) -> Result<String, FileStoreError> {
        Err(FileStoreError::InvalidPath(
            "本地文件系统不支持 get_url，请使用 S3/MinIO 后端".into(),
        ))
    }

    async fn copy(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError> {
        let from_path = self.resolve_path(from)?;
        let to_path = self.resolve_path(to)?;
        self.ensure_parent_dir(&to_path).await?;
        tokio::fs::copy(&from_path, &to_path)
            .await
            .map_err(|e| FileStoreError::Io(e.to_string()))?;
        self.metadata(to).await
    }

    async fn move_file(&self, from: &str, to: &str) -> Result<FileMeta, FileStoreError> {
        let result = self.copy(from, to).await?;
        self.delete(from).await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_file_storage_write_read() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        let data = b"hello world";
        let meta = store.write("test.txt", data).await.unwrap();
        assert_eq!(meta.size, 11);

        let read_data = store.read("test.txt").await.unwrap();
        assert_eq!(read_data, data);

        assert!(store.exists("test.txt").await.unwrap());

        assert!(store.delete("test.txt").await.unwrap());
        assert!(!store.exists("test.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_local_file_storage_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        let data = b"nested content";
        let meta = store.write("dir1/dir2/file.txt", data).await.unwrap();
        assert_eq!(meta.size, data.len() as u64);

        let read_data = store.read("dir1/dir2/file.txt").await.unwrap();
        assert_eq!(read_data, data);
    }

    #[tokio::test]
    async fn test_local_file_storage_list() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        store.write("dir/a.txt", b"aaa").await.unwrap();
        store.write("dir/b.txt", b"bbb").await.unwrap();
        store.write("dir/sub/c.txt", b"ccc").await.unwrap();

        let files = store.list("dir").await.unwrap();
        // list 只列出直接子条目
        assert_eq!(files.len(), 3);
    }

    #[tokio::test]
    async fn test_local_file_storage_copy_move() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        store.write("source.txt", b"content").await.unwrap();

        // 复制
        let copied = store.copy("source.txt", "target.txt").await.unwrap();
        assert_eq!(copied.path, "target.txt");
        assert!(store.exists("target.txt").await.unwrap());

        // 移动
        let moved = store.move_file("source.txt", "moved.txt").await.unwrap();
        assert_eq!(moved.path, "moved.txt");
        assert!(!store.exists("source.txt").await.unwrap());
        assert!(store.exists("moved.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_local_file_storage_not_found() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        let result = store.read("nonexistent.txt").await;
        assert!(matches!(result, Err(FileStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_local_file_storage_metadata() {
        let tmp = TempDir::new().unwrap();
        let store = LocalFileStorage::new(tmp.path());

        store.write("meta_test.txt", b"content").await.unwrap();
        let meta = store.metadata("meta_test.txt").await.unwrap();

        assert_eq!(meta.path, "meta_test.txt");
        assert_eq!(meta.size, 7);
        assert!(!meta.content_hash.is_empty());
        assert!(!meta.is_dir);
    }

    #[test]
    fn test_sanitize_path_in_storage() {
        assert_eq!(sanitize_path("normal.txt"), "normal.txt");
        assert_eq!(sanitize_path("../escape"), ".._escape");
        assert_eq!(sanitize_path("a/b:c"), "a_b_c");
    }
}
