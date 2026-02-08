//! ReadLints tool - get LSP diagnostic information
//!
//! Responsibilities:
//! - Get LSP diagnostic information for files or directories (errors, warnings, hints)
//! - Support filtering by severity
//! - Provide friendly output format

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::infrastructure::get_workspace_path;
use crate::service::lsp::get_workspace_manager;
use crate::util::errors::{BitFunError, BitFunResult};

/// ReadLints tool
pub struct ReadLintsTool;

impl ReadLintsTool {
    pub fn new() -> Self {
        Self
    }
}

/// Diagnostic severity filter
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SeverityFilter {
    Error,
    Warning,
    Info,
    Hint,
    All,
}

impl Default for SeverityFilter {
    fn default() -> Self {
        SeverityFilter::All
    }
}

/// ReadLints input parameters
#[derive(Debug, Deserialize)]
struct ReadLintsInput {
    /// File or directory path
    path: String,

    /// Severity filter
    #[serde(default)]
    severity: SeverityFilter,

    /// Maximum number of diagnostics to return per file
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize {
    50
}

/// Diagnostic output structure
#[derive(Debug, Serialize)]
struct ReadLintsOutput {
    /// Path type
    path_type: String,

    /// Queried path
    path: String,

    /// Diagnostic results (organized by file)
    diagnostics: HashMap<String, FileDiagnostics>,

    /// Summary information
    summary: DiagnosticSummary,

    /// Warning messages
    warnings: Vec<String>,
}

/// Diagnostic information for a single file
#[derive(Debug, Serialize)]
struct FileDiagnostics {
    /// File relative path
    file_path: String,

    /// Language type
    language: Option<String>,

    /// LSP server status
    lsp_status: String,

    /// Diagnostic list
    items: Vec<Diagnostic>,

    /// Statistics
    error_count: usize,
    warning_count: usize,
    info_count: usize,
    hint_count: usize,
}

/// Diagnostic item
#[derive(Debug, Clone, Serialize)]
struct Diagnostic {
    /// Severity: 1=Error, 2=Warning, 3=Info, 4=Hint
    severity: u8,

    /// Severity text
    severity_text: String,

    /// Line number (starting from 1)
    line: u32,

    /// Column number (starting from 1)
    column: u32,

    /// Diagnostic message
    message: String,

    /// Error code
    code: Option<String>,

    /// Source
    source: Option<String>,
}

/// Diagnostic summary
#[derive(Debug, Serialize)]
struct DiagnosticSummary {
    total_files: usize,
    files_with_issues: usize,
    total_diagnostics: usize,
    error_count: usize,
    warning_count: usize,
    info_count: usize,
    hint_count: usize,
}

#[async_trait]
impl Tool for ReadLintsTool {
    fn name(&self) -> &str {
        "ReadLints"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Read linter errors and warnings from LSP diagnostics for files or directories.

IMPORTANT PREREQUISITES:
- This tool ONLY works after the LSP server has started and analyzed the file/directory
- Files are automatically synced to LSP when modified by tools (FileWrite, FileEdit, etc.)
- There is a ~500ms delay after file modifications to ensure LSP analysis is complete

Usage Guidelines:
- Use this tool to understand code quality issues, errors, and warnings AFTER editing code
- Specify a file path OR a directory path (not both)
- For directories, returns diagnostics for all analyzed files within
- Results include: severity, line number, message, and error code

When to use:
- After writing or editing code to check for errors
- Before suggesting fixes to understand existing issues
- During code review to identify quality problems
- To help debug compilation or runtime errors

When NOT to use:
- Immediately after file operations (wait for the tool to return first)
- For files that haven't been opened or analyzed by LSP yet
- For languages without LSP support

Example usage:
1. AI calls FileWrite to create file.rs
2. AI waits for FileWrite to complete
3. AI calls ReadLints("file.rs") to check for errors
4. AI sees "error: cannot find value `x`" and fixes it

Severity levels:
- "error": Only show errors (compilation failures)
- "warning": Show warnings (potential issues)
- "info": Show informational messages
- "hint": Show hints and suggestions
- "all" (default): Show everything"#
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path or directory path to get diagnostics for. Can be relative or absolute."
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "info", "hint", "all"],
                    "description": "Filter diagnostics by severity level. Default is 'all'."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of diagnostics to return per file. Default is 50."
                }
            },
            "required": ["path"]
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // 1. Parse input
        let params: ReadLintsInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        // 2. Get workspace path
        let workspace = get_workspace_path().ok_or_else(|| {
            BitFunError::tool("Workspace not set. Please open a workspace first.".to_string())
        })?;

        // 3. Parse path (supports relative and absolute paths)
        let path = if Path::new(&params.path).is_absolute() {
            PathBuf::from(&params.path)
        } else {
            workspace.join(&params.path)
        };

        if !path.exists() {
            return Err(BitFunError::tool(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        // 4. Wait for file listener to complete synchronization (give LSP time to analyze)
        // This delay ensures LspFileSync debounce window (300ms) + analysis time
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 5. Get workspace LSP manager
        let workspace_manager = get_workspace_manager(workspace.clone())
            .await
            .map_err(|e| {
                BitFunError::tool(format!(
                    "LSP manager not found for workspace: {}. Error: {}",
                    workspace.display(),
                    e
                ))
            })?;

        // 6. Get diagnostics based on path type
        let result = if path.is_file() {
            self.get_file_diagnostics(&path, &workspace, &workspace_manager, &params)
                .await?
        } else if path.is_dir() {
            self.get_directory_diagnostics(&path, &workspace, &workspace_manager, &params)
                .await?
        } else {
            return Err(BitFunError::tool(format!(
                "Path is neither a file nor directory: {}",
                path.display()
            )));
        };

        // 7. Return result
        let result_json = serde_json::to_value(&result)
            .map_err(|e| BitFunError::tool(format!("Failed to serialize result: {}", e)))?;

        Ok(vec![ToolResult::Result {
            data: result_json,
            result_for_assistant: Some(self.format_summary(&result)),
        }])
    }
}

impl ReadLintsTool {
    /// Get diagnostics for a single file
    async fn get_file_diagnostics(
        &self,
        path: &Path,
        workspace: &Path,
        manager: &std::sync::Arc<crate::service::lsp::WorkspaceLspManager>,
        params: &ReadLintsInput,
    ) -> BitFunResult<ReadLintsOutput> {
        let uri = format!("file://{}", path.display());
        let relative_path = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .display()
            .to_string();

        // Detect language
        let language = self.detect_language(path);

        // Get diagnostic information (prefer cache, since just synced)
        let raw_diagnostics = manager.get_diagnostics(&uri).await.unwrap_or_default();

        // Parse and filter diagnostics
        let diagnostics =
            self.parse_diagnostics(&raw_diagnostics, &params.severity, params.max_results);

        // Statistics
        let (error_count, warning_count, info_count, hint_count) =
            Self::count_by_severity(&diagnostics);

        // Check LSP status
        let lsp_status = if let Some(lang) = &language {
            let state = manager.get_server_state(lang).await;
            format!("{:?}", state.status)
        } else {
            "unknown".to_string()
        };

        let file_diag = FileDiagnostics {
            file_path: relative_path.clone(),
            language,
            lsp_status,
            items: diagnostics.clone(),
            error_count,
            warning_count,
            info_count,
            hint_count,
        };

        let mut diagnostics_map = HashMap::new();
        let has_issues = !diagnostics.is_empty();
        if has_issues {
            diagnostics_map.insert(relative_path, file_diag);
        }

        let summary = DiagnosticSummary {
            total_files: 1,
            files_with_issues: if has_issues { 1 } else { 0 },
            total_diagnostics: diagnostics.len(),
            error_count,
            warning_count,
            info_count,
            hint_count,
        };

        Ok(ReadLintsOutput {
            path_type: "file".to_string(),
            path: params.path.clone(),
            diagnostics: diagnostics_map,
            summary,
            warnings: vec![],
        })
    }

    /// Get directory diagnostics (recursive)
    async fn get_directory_diagnostics(
        &self,
        _path: &Path,
        _workspace: &Path,
        _manager: &std::sync::Arc<crate::service::lsp::WorkspaceLspManager>,
        params: &ReadLintsInput,
    ) -> BitFunResult<ReadLintsOutput> {
        // TODO: Implement recursive directory retrieval
        // Current simplified implementation: prompt user to specify a specific file
        Ok(ReadLintsOutput {
            path_type: "directory".to_string(),
            path: params.path.clone(),
            diagnostics: HashMap::new(),
            summary: DiagnosticSummary {
                total_files: 0,
                files_with_issues: 0,
                total_diagnostics: 0,
                error_count: 0,
                warning_count: 0,
                info_count: 0,
                hint_count: 0,
            },
            warnings: vec![
                "Directory diagnostics not yet implemented. Please specify a file path instead."
                    .to_string(),
            ],
        })
    }

    /// Parse LSP diagnostic information
    fn parse_diagnostics(
        &self,
        raw: &[serde_json::Value],
        filter: &SeverityFilter,
        max_results: usize,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for diag in raw.iter().take(max_results) {
            // Parse severity
            let severity = diag.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) as u8;

            // Filter
            if !self.matches_filter(severity, filter) {
                continue;
            }

            let severity_text = match severity {
                1 => "Error",
                2 => "Warning",
                3 => "Info",
                4 => "Hint",
                _ => "Unknown",
            }
            .to_string();

            // Parse position
            let range = diag.get("range");
            let start = range.and_then(|r| r.get("start"));
            let line = start
                .and_then(|s| s.get("line"))
                .and_then(|l| l.as_u64())
                .unwrap_or(0) as u32
                + 1; // LSP starts from 0, we start from 1
            let column = start
                .and_then(|s| s.get("character"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0) as u32
                + 1;

            // Parse message
            let message = diag
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)")
                .to_string();

            // Parse code and source
            let code = diag
                .get("code")
                .and_then(|c| c.as_str().or_else(|| c.as_i64().map(|_| "")))
                .map(|s| s.to_string());

            let source = diag
                .get("source")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());

            diagnostics.push(Diagnostic {
                severity,
                severity_text,
                line,
                column,
                message,
                code,
                source,
            });
        }

        diagnostics
    }

    /// Check if diagnostic matches filter
    fn matches_filter(&self, severity: u8, filter: &SeverityFilter) -> bool {
        match filter {
            SeverityFilter::Error => severity == 1,
            SeverityFilter::Warning => severity == 2,
            SeverityFilter::Info => severity == 3,
            SeverityFilter::Hint => severity == 4,
            SeverityFilter::All => true,
        }
    }

    /// Count by severity
    fn count_by_severity(diagnostics: &[Diagnostic]) -> (usize, usize, usize, usize) {
        let mut error = 0;
        let mut warning = 0;
        let mut info = 0;
        let mut hint = 0;

        for diag in diagnostics {
            match diag.severity {
                1 => error += 1,
                2 => warning += 1,
                3 => info += 1,
                4 => hint += 1,
                _ => {}
            }
        }

        (error, warning, info, hint)
    }

    /// Detect file language
    fn detect_language(&self, path: &Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| match ext {
                "rs" => Some("rust"),
                "ts" => Some("typescript"),
                "tsx" => Some("typescriptreact"),
                "js" => Some("javascript"),
                "jsx" => Some("javascriptreact"),
                "py" => Some("python"),
                "go" => Some("go"),
                "java" => Some("java"),
                "c" => Some("c"),
                "cpp" | "cc" | "cxx" => Some("cpp"),
                "h" | "hpp" => Some("cpp"),
                _ => None,
            })
            .map(|s| s.to_string())
    }

    /// Format summary information
    fn format_summary(&self, output: &ReadLintsOutput) -> String {
        let summary = &output.summary;

        if summary.total_diagnostics == 0 {
            return format!("No issues found in {}", output.path);
        }

        let mut parts = vec![format!(
            "Found {} issue(s) in {}",
            summary.total_diagnostics, output.path
        )];

        if summary.error_count > 0 {
            parts.push(format!(" {} error(s)", summary.error_count));
        }
        if summary.warning_count > 0 {
            parts.push(format!(" {} warning(s)", summary.warning_count));
        }
        if summary.info_count > 0 {
            parts.push(format!(" {} info", summary.info_count));
        }
        if summary.hint_count > 0 {
            parts.push(format!(" {} hint(s)", summary.hint_count));
        }

        // Add summary of first few errors
        if let Some((_, file_diag)) = output.diagnostics.iter().next() {
            parts.push("\nTop issues:".to_string());
            for (i, diag) in file_diag.items.iter().take(3).enumerate() {
                parts.push(format!(
                    "  {}. [{}] Line {}: {}",
                    i + 1,
                    diag.severity_text,
                    diag.line,
                    if diag.message.chars().count() > 80 {
                        format!("{}...", diag.message.chars().take(80).collect::<String>())
                    } else {
                        diag.message.clone()
                    }
                ));
            }

            if file_diag.items.len() > 3 {
                parts.push(format!("  ... and {} more", file_diag.items.len() - 3));
            }
        }

        parts.join("\n")
    }
}
