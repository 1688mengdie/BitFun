use super::{build_structured_compression_reminder, CompressionFallbackOptions};
use crate::agentic::core::{
    render_system_reminder, CompressedMessageRole, CompressionEntry, CompressionPayload, Message,
    MessageSemanticKind, ToolCall, ToolResult,
};
use serde_json::json;

fn default_options() -> CompressionFallbackOptions {
    CompressionFallbackOptions {
        max_tokens: 10_000,
        user_chars: 120,
        assistant_chars: 120,
        tool_arg_chars: 80,
        tool_command_chars: 80,
    }
}

#[test]
fn clears_tool_results_from_compressed_history() {
    let assistant = Message::assistant_with_tools(
        "Checking file".to_string(),
        vec![ToolCall {
            tool_id: "tool_1".to_string(),
            tool_name: "Read".to_string(),
            arguments: json!({
                "file_path": "/tmp/demo.rs",
                "start_line": 1,
                "limit": 20
            }),
            is_error: false,
        }],
    );
    let tool_result = Message::tool_result(ToolResult {
        tool_id: "tool_1".to_string(),
        tool_name: "Read".to_string(),
        result: json!({"content": "ignored"}),
        result_for_assistant: Some("Read succeeded with file preview".to_string()),
        is_error: false,
        duration_ms: None,
        image_attachments: None,
    });

    let reminder = build_structured_compression_reminder(
        vec![vec![
            Message::user("inspect".to_string()),
            assistant,
            tool_result,
        ]],
        &default_options(),
    );

    let turn = match &reminder.payload.entries[0] {
        CompressionEntry::Turn { messages, .. } => messages,
        _ => panic!("expected turn entry"),
    };
    let assistant_message = turn
        .iter()
        .find(|message| message.role == CompressedMessageRole::Assistant)
        .expect("assistant message");

    assert_eq!(assistant_message.tool_calls.len(), 1);
    assert!(!reminder.model_text.contains("Tool result:"));
    assert!(reminder.model_text.contains("All tool results have been cleared"));
}

#[test]
fn reuses_existing_compression_payload_atomically() {
    let prior_summary = "Previous conversation summary".to_string();
    let reminder_message = Message::user(render_system_reminder(&prior_summary))
        .with_semantic_kind(MessageSemanticKind::InternalReminder)
        .with_compression_payload(CompressionPayload::from_summary(prior_summary.clone()));

    let reminder =
        build_structured_compression_reminder(vec![vec![reminder_message]], &default_options());

    assert!(matches!(
        &reminder.payload.entries[0],
        CompressionEntry::ModelSummary { text } if text == &prior_summary
    ));
}
