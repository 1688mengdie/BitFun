//! PlanRead tool implementation
//!
//! Reads a plan file from the workspace plans directory and returns its
//! structured content (YAML frontmatter: name/overview/todos + markdown body).

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

/// YAML frontmatter structure for Plan files (mirror of the CreatePlan
/// writer; fields are optional so older or hand-edited files stay readable).
#[derive(Debug, Deserialize)]
struct PlanFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    todos: Vec<TodoItem>,
}

/// Todo item structure (mirror of the CreatePlan writer).
#[derive(Debug, Deserialize)]
struct TodoItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// PlanRead tool - read plan file
pub struct PlanReadTool;

impl PlanReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanReadTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a plan file body into its YAML frontmatter and markdown body.
fn parse_plan_file(content: &str) -> BitFunResult<(PlanFrontmatter, String)> {
    let trimmed = content.trim_start();
    let after_open = trimmed
        .strip_prefix("---")
        .ok_or_else(|| BitFunError::tool("Plan file is missing the YAML frontmatter opener '---'"))?;
    let end = after_open.find("\n---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter closer '---'")
    })?;
    let yaml_part = &after_open[..end];
    let body_start = end + "\n---".len();
    let body = after_open[body_start..]
        .trim_start_matches(['\n', '\r'])
        .to_string();

    let frontmatter: PlanFrontmatter = serde_yaml::from_str(yaml_part).map_err(|error| {
        BitFunError::tool(format!(
            "Failed to parse plan YAML frontmatter: {}",
            error
        ))
    })?;
    Ok((frontmatter, body))
}

#[async_trait]
impl Tool for PlanReadTool {
    fn name(&self) -> &str {
        "PlanRead"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"Read a plan file from the current workspace plans directory (or an absolute plan file path). The input accepts the plan file name (for example "my_plan_1234abcd.plan.md") or a full path to a .plan.md file. Returns the parsed YAML frontmatter (name, overview, todos with id/content/status/dependencies) plus the raw markdown body. Read-only: does not modify any files."###
            .to_string())
    }

    fn short_description(&self) -> String {
        "Read and parse a plan file from the workspace plans directory.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // 2026-08-04 user calibration: the plan tool family is a commander
        // staple; Direct so no GetToolSpec unlock round-trip is needed
        // (mirrored by `shared_coding_mode_tool_exposure_overrides()`).
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["plan_file"],
            "properties": {
                "plan_file": {
                    "type": "string",
                    "description": "Plan file name (e.g. my_plan_1234abcd.plan.md) or an absolute path to a .plan.md file"
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let plan_file = input
            .get("plan_file")
            .and_then(|value| value.as_str())
            .ok_or(BitFunError::validation(
                "Missing required field: plan_file",
            ))?;
        let plan_file = plan_file.trim();
        if plan_file.is_empty() {
            return Err(BitFunError::validation(
                "Missing required field: plan_file",
            ));
        }

        let plan_path = resolve_plan_path(plan_file, context).await?;
        let content = fs::read_to_string(&plan_path)
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to read plan file: {}", error)))?;

        let (frontmatter, body) = parse_plan_file(&content)?;

        let todos = frontmatter
            .todos
            .into_iter()
            .map(|todo| {
                json!({
                    "id": todo.id.unwrap_or_default(),
                    "content": todo.content.unwrap_or_default(),
                    "status": todo.status.unwrap_or_else(|| "pending".to_string()),
                    "dependencies": todo.dependencies
                })
            })
            .collect::<Vec<_>>();

        let plan_reference =
            context.build_runtime_artifact_reference(&format!("plans/{}", plan_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))?;

        let result = json!({
            "success": true,
            "plan_file_name": plan_path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
            "plan_file_path": plan_reference,
            "name": frontmatter.name,
            "overview": frontmatter.overview,
            "todos": todos,
            "body": body
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: None,
            image_attachments: None,
        }])
    }
}

/// Resolve the plan file argument to a concrete filesystem path. Absolute
/// paths (or paths containing a separator) are used as-is; bare file names
/// are resolved against the current workspace plans directory.
async fn resolve_plan_path(plan_file: &str, context: &ToolUseContext) -> BitFunResult<PathBuf> {
    let supplied = PathBuf::from(plan_file);
    let looks_like_path = plan_file.contains('/') || plan_file.contains('\\');
    if supplied.is_absolute() || looks_like_path {
        // Note: extension() only returns the last suffix ("md" for
        // "xxx.plan.md"), so validate the full file name suffix instead.
        let file_name = supplied
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !file_name.ends_with(".plan.md") {
            return Err(BitFunError::tool(format!(
                "Plan file must end with .plan.md: {}",
                plan_file
            )));
        }
        if !supplied.exists() {
            return Err(BitFunError::tool(format!(
                "Plan file not found: {}",
                plan_file
            )));
        }
        return Ok(supplied);
    }

    let runtime_context = context.ensure_current_workspace_runtime().await?;
    let plans_dir = runtime_context.plans_dir.clone();
    let plan_path = plans_dir.join(plan_file);
    if !plan_path.exists() {
        return Err(BitFunError::tool(format!(
            "Plan file not found in plans directory: {} (plans_dir={})",
            plan_file,
            plans_dir.to_string_lossy()
        )));
    }
    Ok(plan_path)
}

#[cfg(test)]
mod tests {
    use super::parse_plan_file;

    #[test]
    fn parse_plan_file_reads_frontmatter_and_body() {
        let content = "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n---\n\n# My Plan\n\nBody text here.\n";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("My Plan"));
        assert_eq!(frontmatter.overview.as_deref(), Some("An overview"));
        assert_eq!(frontmatter.todos.len(), 1);
        assert_eq!(frontmatter.todos[0].id.as_deref(), Some("setup-auth"));
        assert_eq!(frontmatter.todos[0].content.as_deref(), Some("Set up auth"));
        assert_eq!(frontmatter.todos[0].status.as_deref(), Some("pending"));
        assert!(frontmatter.todos[0].dependencies.is_empty());
        assert!(body.contains("Body text here."));
    }

    #[test]
    fn parse_plan_file_round_trips_create_plan_writer_format() {
        // Mirror the exact layout emitted by create_plan_tool.rs
        // `generate_plan_file_content` (---\n<yaml>---\n\n<body>).
        let content = "---\nname: deploy-api\noverview: Deploy the API service\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n- id: implement-ui\n  content: Implement the UI\n  status: pending\n  dependencies:\n  - setup-auth\n---\n\n# deploy-api\n\n## Steps\n\n1. Auth\n2. UI\n";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("deploy-api"));
        assert_eq!(frontmatter.todos.len(), 2);
        assert_eq!(frontmatter.todos[1].id.as_deref(), Some("implement-ui"));
        assert_eq!(
            frontmatter.todos[1].dependencies,
            vec!["setup-auth".to_string()]
        );
        assert!(body.starts_with("# deploy-api"));
        assert!(body.contains("1. Auth"));
    }

    #[test]
    fn parse_plan_file_missing_delimiters_errors() {
        assert!(parse_plan_file("no frontmatter here").is_err());
        assert!(parse_plan_file("---\nname: x").is_err());
    }

    #[test]
    fn parse_plan_file_tolerates_missing_optional_fields() {
        let content = "---\nname: Minimal\n---\n\nBody";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("Minimal"));
        assert!(frontmatter.overview.is_none());
        assert!(frontmatter.todos.is_empty());
        assert!(body.contains("Body"));
    }

    use super::resolve_plan_path;
    use crate::agentic::tools::framework::ToolUseContext;
    use bitfun_runtime_ports::ToolRuntimeHandles;
    use std::collections::HashMap;
    use tool_runtime::context::PrimaryModelFacts;
    use uuid::Uuid;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn resolve_plan_path_absolute_plan_md_suffix_succeeds() {
        // Regression: xxx.plan.md must be accepted via absolute path
        // (extension() alone would report only "md").
        let dir = std::env::temp_dir().join(format!("plan-read-resolve-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should be created");
        let plan_path = dir.join("my_plan_1234abcd.plan.md");
        std::fs::write(&plan_path, "---\nname: Test\n---\n\nBody").expect("write plan file");
        let result = resolve_plan_path(plan_path.to_str().unwrap(), &empty_context()).await;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.expect("absolute .plan.md path must resolve"), plan_path);
    }

    #[tokio::test]
    async fn resolve_plan_path_rejects_wrong_suffix() {
        let error = resolve_plan_path("C:/tmp/not_a_plan.md", &empty_context())
            .await
            .expect_err("non-.plan.md absolute path must error");
        let message = error.to_string();
        assert!(
            message.contains("Plan file must end with .plan.md"),
            "unexpected error: {}",
            message
        );
    }
}
