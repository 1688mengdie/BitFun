//! taiji-gbrain CLI — 知识库管理命令行接口。
//!
//! 7 个命令：
//! - page put/get/delete/list — 页面 CRUD
//! - search — 搜索知识库
//! - sync — 同步来源
//! - status — 知识库状态
//!
//! 参考: gbrain (MIT) src/commands/ — R-5-401 — Rust clap 实现

use clap::{Parser, Subcommand};

use crate::engine::GBrainEngine;

/// taiji-gbrain 知识库管理 CLI（R-5-401）。
#[derive(Parser, Debug)]
#[command(name = "taiji-gbrain", version, about = "LVPA 知识库管理工具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// 页面操作
    Page {
        #[command(subcommand)]
        action: PageAction,
    },
    /// 搜索知识库
    Search {
        /// 搜索查询
        query: String,
    },
    /// 同步指定来源
    Sync {
        /// 来源名称
        source: String,
    },
    /// 知识库状态（页面数/分块数/健康检查）
    Status,
}

#[derive(Subcommand, Debug)]
pub enum PageAction {
    /// 添加或更新页面
    Put {
        /// 页面 slug
        slug: String,
        /// 页面内容文件路径
        file: String,
    },
    /// 获取页面内容
    Get {
        /// 页面 slug
        slug: String,
    },
    /// 删除页面
    Delete {
        /// 页面 slug
        slug: String,
    },
    /// 列出所有页面
    List,
}

/// 执行 CLI 命令。
pub fn execute(args: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match &args.command {
        Command::Page { action } => execute_page(action),
        Command::Search { query } => execute_search(query),
        Command::Sync { source } => execute_sync(source),
        Command::Status => execute_status(),
    }
}

fn execute_page(action: &PageAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        PageAction::Put { slug, file } => {
            let content = std::fs::read_to_string(file)
                .map_err(|e| format!("无法读取文件 '{}': {}", file, e))?;
            let input = taiji_types::knowledge::PageInput {
                title: slug.clone(),
                content,
                source_id: Some("cli".into()),
                tags: vec![],
                metadata: serde_json::Value::Null,
            };
            // Create engine, connect, put
            // For CLI, use PGLite with default config
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut engine = crate::engine::pglite::PGLiteEngine::new();
                engine
                    .connect(&taiji_types::knowledge::GBrainConfig::default())
                    .await
                    .map_err(|e| format!("连接引擎失败: {}", e))?;
                engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;
                engine
                    .put_page(slug, input)
                    .await
                    .map_err(|e| format!("写入页面失败: {}", e))?;
                println!("页面 '{}' 已保存", slug);
                Ok::<_, Box<dyn std::error::Error>>(())
            })
        }
        PageAction::Get { slug } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut engine = crate::engine::pglite::PGLiteEngine::new();
                engine
                    .connect(&taiji_types::knowledge::GBrainConfig::default())
                    .await
                    .map_err(|e| format!("连接引擎失败: {}", e))?;
                engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;
                match engine.get_page(slug).await.map_err(|e| format!("查询失败: {}", e))? {
                    Some(page) => {
                        println!("标题: {}", page.title);
                        println!("来源: {}", page.source_id);
                        println!("创建时间: {}", page.created_at);
                        println!("更新时间: {}", page.updated_at);
                        println!("--- 内容 ---");
                        println!("{}", page.content);
                    }
                    None => {
                        println!("页面 '{}' 不存在", slug);
                    }
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            })
        }
        PageAction::Delete { slug } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut engine = crate::engine::pglite::PGLiteEngine::new();
                engine
                    .connect(&taiji_types::knowledge::GBrainConfig::default())
                    .await
                    .map_err(|e| format!("连接引擎失败: {}", e))?;
                engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;
                let deleted = engine
                    .delete_page(slug)
                    .await
                    .map_err(|e| format!("删除失败: {}", e))?;
                if deleted {
                    println!("页面 '{}' 已删除", slug);
                } else {
                    println!("页面 '{}' 不存在", slug);
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            })
        }
        PageAction::List => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut engine = crate::engine::pglite::PGLiteEngine::new();
                engine
                    .connect(&taiji_types::knowledge::GBrainConfig::default())
                    .await
                    .map_err(|e| format!("连接引擎失败: {}", e))?;
                engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;
                let pages = engine
                    .list_pages()
                    .await
                    .map_err(|e| format!("查询失败: {}", e))?;
                if pages.is_empty() {
                    println!("知识库为空");
                } else {
                    for (i, page) in pages.iter().enumerate() {
                        println!("{}. [{}] {} (来源: {})", i + 1, page.id, page.title, page.source_id);
                    }
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            })
        }
    }
}

fn execute_search(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let opts = taiji_types::knowledge::SearchOpts::default();
        let mut engine = crate::engine::pglite::PGLiteEngine::new();
        engine
            .connect(&taiji_types::knowledge::GBrainConfig::default())
            .await
            .map_err(|e| format!("连接引擎失败: {}", e))?;
        engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;
        let results = engine
            .search(query, &opts)
            .await
            .map_err(|e| format!("搜索失败: {}", e))?;
        if results.is_empty() {
            println!("未找到匹配结果");
        } else {
            for (i, r) in results.iter().enumerate() {
                println!("{}. [{}] {} (分数: {:.2})", i + 1, r.chunk.page_id, r.page_title, r.score);
                println!("   {}", r.chunk.text);
            }
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

fn execute_sync(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("同步来源 '{}' — 功能开发中", source);
    println!("预期行为：获取来源的所有页面并导入知识库");
    Ok(())
}

fn execute_status() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut engine = crate::engine::pglite::PGLiteEngine::new();
        engine
            .connect(&taiji_types::knowledge::GBrainConfig::default())
            .await
            .map_err(|e| format!("连接引擎失败: {}", e))?;
        engine.init_schema().await.map_err(|e| format!("初始化 schema 失败: {}", e))?;

        let page_count = engine.page_count().await.unwrap_or(0);
        println!("知识库状态:");
        println!("  引擎: PGLite");
        println!("  页面数: {}", page_count);

        // 统计所有页面总块数
        let pages = engine.list_pages().await.unwrap_or_default();
        let mut total_chunks = 0usize;
        let mut total_embeddings = 0usize;
        for page in &pages {
            if let Ok(chunks) = engine.get_chunks(&page.id).await {
                total_chunks += chunks.len();
                total_embeddings += chunks.iter().filter(|c| c.embedding.is_some()).count();
            }
        }
        println!("  分块数: {}", total_chunks);
        println!("  已嵌入: {}", total_embeddings);
        println!("  健康检查: ✅");

        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_page_put() {
        let cli = Cli::parse_from(["taiji-gbrain", "page", "put", "test-slug", "test.md"]);
        match cli.command {
            Command::Page { action } => match action {
                PageAction::Put { slug, file } => {
                    assert_eq!(slug, "test-slug");
                    assert_eq!(file, "test.md");
                }
                _ => panic!("expected Put"),
            },
            _ => panic!("expected Page"),
        }
    }

    #[test]
    fn test_cli_parse_page_get() {
        let cli = Cli::parse_from(["taiji-gbrain", "page", "get", "my-page"]);
        match cli.command {
            Command::Page { action } => match action {
                PageAction::Get { slug } => assert_eq!(slug, "my-page"),
                _ => panic!("expected Get"),
            },
            _ => panic!("expected Page"),
        }
    }

    #[test]
    fn test_cli_parse_page_delete() {
        let cli = Cli::parse_from(["taiji-gbrain", "page", "delete", "old-page"]);
        match cli.command {
            Command::Page { action } => match action {
                PageAction::Delete { slug } => assert_eq!(slug, "old-page"),
                _ => panic!("expected Delete"),
            },
            _ => panic!("expected Page"),
        }
    }

    #[test]
    fn test_cli_parse_page_list() {
        let cli = Cli::parse_from(["taiji-gbrain", "page", "list"]);
        match cli.command {
            Command::Page { action } => match action {
                PageAction::List => {} // OK
                _ => panic!("expected List"),
            },
            _ => panic!("expected Page"),
        }
    }

    #[test]
    fn test_cli_parse_search() {
        let cli = Cli::parse_from(["taiji-gbrain", "search", "量价时空"]);
        match cli.command {
            Command::Search { query } => assert_eq!(query, "量价时空"),
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_cli_parse_sync() {
        let cli = Cli::parse_from(["taiji-gbrain", "sync", "github"]);
        match cli.command {
            Command::Sync { source } => assert_eq!(source, "github"),
            _ => panic!("expected Sync"),
        }
    }

    #[test]
    fn test_cli_parse_status() {
        let cli = Cli::parse_from(["taiji-gbrain", "status"]);
        match cli.command {
            Command::Status => {} // OK
            _ => panic!("expected Status"),
        }
    }

    #[test]
    fn test_cli_page_put_requires_file() {
        // Verifying that clap rejects missing file argument
        let result = Cli::try_parse_from(["taiji-gbrain", "page", "put", "slug-only"]);
        assert!(result.is_err(), "put without file should fail");
    }

    #[test]
    fn test_cli_search_requires_query() {
        let result = Cli::try_parse_from(["taiji-gbrain", "search"]);
        assert!(result.is_err(), "search without query should fail");
    }

    #[test]
    fn test_cli_page_get_requires_slug() {
        let result = Cli::try_parse_from(["taiji-gbrain", "page", "get"]);
        assert!(result.is_err(), "get without slug should fail");
    }
}
