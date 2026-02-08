use super::util::resolve_path;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::ai_rules::get_global_ai_rules_service;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::path::Path;
use tool_runtime::fs::read_file::read_file;

/// File read tool
pub struct FileReadTool {
    /// Maximum number of lines to read
    default_max_lines_to_read: usize,
    /// Maximum line length
    max_line_chars: usize,
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            default_max_lines_to_read: 2000,
            max_line_chars: 2000,
        }
    }

    /// Create FileReadTool with custom configuration
    pub fn with_config(default_max_lines_to_read: usize, max_line_chars: usize) -> Self {
        Self {
            default_max_lines_to_read,
            max_line_chars,
        }
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            r#"Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path.
- By default, it reads up to {} lines starting from the beginning of the file. 
- You can optionally specify a start_line and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters.
- Any lines longer than {} characters will be truncated.
- Results are returned using cat -n format, with line numbers starting at 1
- This tool can only read files, not directories. To read a directory, use an ls command via the Bash tool.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
"#,
            self.default_max_lines_to_read, self.max_line_chars
        ))
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "start_line": {
                    "type": "number",
                    "description": "The line number to start reading from. Only provide if the file is too large to read at once"
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once."
                }
            },
            "required": ["file_path"],
            "additionalProperties": false
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

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if file_path.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("file_path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            let path = Path::new(file_path);
            if !path.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("File does not exist: {}", file_path)),
                    error_code: Some(404),
                    meta: None,
                };
            }

            if !path.is_file() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path is not a file: {}", file_path)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("file_path is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Reading file: {}", file_path)
            } else {
                format!("Read {}", file_path)
            }
        } else {
            "Reading file".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("file_path is required".to_string()))?;

        let start_line = input
            .get("start_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_max_lines_to_read as u64) as usize;

        let resolved_path = resolve_path(file_path);

        let read_file_result = read_file(&resolved_path, start_line, limit, self.max_line_chars)
            .map_err(|e| BitFunError::tool(e))?;

        // Get matching file-specific rules
        let file_rules = match get_global_ai_rules_service().await {
            Ok(rules_service) => rules_service.get_rules_for_file(&resolved_path).await,
            Err(e) => {
                debug!("Failed to get AIRulesService: {}", e);
                crate::service::ai_rules::FileRulesResult {
                    matched_count: 0,
                    formatted_content: None,
                }
            }
        };

        // Build result string
        let mut result_for_assistant = format!(
            "Read lines {}-{} from {} ({} total lines)\n<file_content>\n{}\n</file_content>",
            read_file_result.start_line,
            read_file_result.end_line,
            resolved_path,
            read_file_result.total_lines,
            read_file_result.content
        );

        // If there are matching rules, append to result
        if let Some(rules_content) = &file_rules.formatted_content {
            result_for_assistant.push_str("\n\n");
            result_for_assistant.push_str(rules_content);
        }

        let lines_read = read_file_result.end_line - read_file_result.start_line + 1;

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved_path,
                "content": read_file_result.content,
                "total_lines": read_file_result.total_lines,
                "lines_read": lines_read,
                "start_line": read_file_result.start_line,
                "size": read_file_result.content.len(),
                "matched_rules_count": file_rules.matched_count
            }),
            result_for_assistant: Some(result_for_assistant),
        };

        Ok(vec![result])
    }
}
