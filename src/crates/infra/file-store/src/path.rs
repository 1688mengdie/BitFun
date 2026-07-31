//! file-store（储物阁）— 文件路径工具
//!
//! 来源: biliup util.rs:91-102 (MIT)
//! 来源: biliup util.rs:30-58 (Recorder 文件名模板)

use std::path::PathBuf;

use chrono::Utc;

/// 清洗文件路径中的非法字符（Windows 兼容）
///
/// 来源: util.rs:91-102
pub fn sanitize_path(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('_'),
            c if c.is_control() => out.push('_'),
            _ => out.push(ch),
        }
    }
    let out = out.trim_end_matches([' ', '.']).to_string();
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

/// 文件路径生成器
///
/// 来源: util.rs:30-58（Recorder 文件名模板设计）
pub struct FilePathGenerator {
    /// 基础目录
    base_dir: PathBuf,
    /// 文件名模板（支持 {date} / {time} / {name} 占位符）
    template: String,
}

impl FilePathGenerator {
    /// 创建新的路径生成器
    pub fn new(base_dir: impl Into<PathBuf>, template: &str) -> Self {
        Self {
            base_dir: base_dir.into(),
            template: template.to_string(),
        }
    }

    /// 生成完整路径
    pub fn generate(&self, name: &str) -> PathBuf {
        let filename = self
            .template
            .replace("{name}", name)
            .replace("{date}", &Utc::now().format("%Y-%m-%d").to_string())
            .replace("{time}", &Utc::now().format("%H%M%S").to_string());
        self.base_dir.join(sanitize_path(&filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path_normal() {
        assert_eq!(sanitize_path("hello world"), "hello world");
    }

    #[test]
    fn test_sanitize_path_special_chars() {
        assert_eq!(sanitize_path("file:name?.txt"), "file_name_.txt");
        assert_eq!(sanitize_path("a<b>c|d\"e"), "a_b_c_d_e");
    }

    #[test]
    fn test_sanitize_path_empty() {
        assert_eq!(sanitize_path("  "), "_");
    }

    #[test]
    fn test_sanitize_path_trailing_dot() {
        assert_eq!(sanitize_path("file."), "file");
        assert_eq!(sanitize_path("file  "), "file");
    }

    #[test]
    fn test_file_path_generator() {
        let gen = FilePathGenerator::new("/tmp", "{date}_{name}");
        let path = gen.generate("test.txt");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("test.txt"));
        assert!(path_str.starts_with("/tmp"));
    }
}
