pub(crate) const V2_PERMISSION_TOOL_NAMES: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "Delete",
    "Bash",
    "ExecCommand",
    "Git",
    "WebFetch",
    "WebSearch",
    "Skill",
    "Task",
    "LaunchReviewAgent",
];

pub(crate) fn uses_v2_permission(tool_name: &str) -> bool {
    V2_PERMISSION_TOOL_NAMES.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::agentic::tools::implementations::{
        BashTool, ExecCommandTool, GitTool, LaunchReviewAgentTool, SkillTool, TaskTool,
        WebFetchTool, WebSearchTool,
    };
    use serde_json::{json, Value};

    #[test]
    fn command_and_integration_tools_use_v2_while_unmigrated_controls_keep_legacy_gate() {
        for tool_name in [
            "Bash",
            "ExecCommand",
            "Git",
            "WebFetch",
            "WebSearch",
            "Skill",
            "Task",
            "LaunchReviewAgent",
        ] {
            assert!(uses_v2_permission(tool_name), "{tool_name}");
        }
        for tool_name in ["ExecControl", "WriteStdin", "ComputerUse"] {
            assert!(!uses_v2_permission(tool_name), "{tool_name}");
        }
    }

    #[test]
    fn command_and_integration_tools_emit_non_empty_v2_resources() {
        let context = ToolUseContext::for_tool_listing(None, None);
        let bash = BashTool::new();
        let exec = ExecCommandTool::new();
        let git = GitTool::new();
        let fetch = WebFetchTool::new();
        let search = WebSearchTool::new();
        let skill = SkillTool::new();
        let task = TaskTool::new();
        let launch_review = LaunchReviewAgentTool::new();
        let cases: Vec<(&dyn Tool, Value, &str, &str)> = vec![
            (
                &bash,
                json!({ "command": "git status" }),
                "bash",
                "git status",
            ),
            (&exec, json!({ "cmd": "cargo test" }), "bash", "cargo test"),
            (&git, json!({ "operation": "status" }), "git", "git status"),
            (
                &fetch,
                json!({ "url": "https://example.com/docs" }),
                "webfetch",
                "https://example.com/docs",
            ),
            (
                &search,
                json!({ "query": "rust permission model" }),
                "websearch",
                "rust permission model",
            ),
            (&skill, json!({ "command": "pdf" }), "skill", "pdf"),
            (
                &task,
                json!({
                    "action": "spawn",
                    "subagent_type": "Explore",
                    "description": "inspect",
                    "prompt": "inspect"
                }),
                "task",
                "Explore",
            ),
            (
                &task,
                json!({ "action": "spawn", "fork_context": true }),
                "task",
                "fork_context",
            ),
            (
                &task,
                json!({ "action": "send_input", "session_id": "session-7" }),
                "task",
                "send_input:session-7",
            ),
            (
                &task,
                json!({ "action": "cancel", "session_id": "session-7" }),
                "task",
                "cancel:session-7",
            ),
            (
                &launch_review,
                json!({ "subagent_type": "ReviewSecurity" }),
                "task",
                "ReviewSecurity",
            ),
        ];

        for (tool, input, expected_action, expected_resource) in cases {
            let intents = tool
                .permission_intents(&input, &context)
                .expect("permission intent");
            assert_eq!(intents.len(), 1, "{}", tool.name());
            assert_eq!(intents[0].action, expected_action, "{}", tool.name());
            assert_eq!(intents[0].resources, [expected_resource.to_string()]);
        }
    }
}
