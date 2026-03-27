use crate::agentic::core::{
    CompressedMessage, CompressedMessageRole, CompressionEntry, CompressionPayload,
};
use serde_json::{json, Value};

pub(super) fn render_payload_for_model(payload: &CompressionPayload) -> String {
    if payload.entries.is_empty() {
        return [
            "Earlier conversation has been condensed for context management.",
            "The omitted history could not be kept within the available context budget.",
        ]
        .join("\n\n");
    }

    let mut sections = vec![
        "Earlier conversation has been condensed for context management.".to_string(),
        "The history below is a partial record. Message text, tool arguments, and task lists may have been truncated or omitted during compression. All tool results have been cleared from this compressed history. Treat it as approximate historical context rather than a complete verbatim transcript.".to_string(),
    ];

    for (index, entry) in payload.entries.iter().enumerate() {
        match entry {
            CompressionEntry::ModelSummary { text } => {
                sections.push(format!(
                    "Earlier summarized history {}:\n{}",
                    index + 1,
                    text
                ));
            }
            CompressionEntry::Turn { messages, todo, .. } => {
                let mut lines = vec![format!("Historical turn {}:", index + 1)];
                for message in messages {
                    render_compressed_message(&mut lines, message);
                }
                if let Some(todo) = todo {
                    lines.push("Latest task list for this turn:".to_string());
                    if todo.todos.is_empty() {
                        if let Some(summary) = todo.summary.as_ref() {
                            lines.push(format!("- {}", summary));
                        }
                    } else {
                        for todo_item in &todo.todos {
                            lines.push(format!("- [{}] {}", todo_item.status, todo_item.content));
                        }
                        if let Some(summary) = todo.summary.as_ref() {
                            lines.push(format!("Task list note: {}", summary));
                        }
                    }
                }
                sections.push(lines.join("\n"));
            }
        }
    }

    sections.join("\n\n")
}

fn render_compressed_message(lines: &mut Vec<String>, message: &CompressedMessage) {
    let role_label = match message.role {
        CompressedMessageRole::User => "User",
        CompressedMessageRole::Assistant => "Assistant",
    };

    if let Some(text) = message.text.as_ref() {
        lines.push(format!("{role_label}: {text}"));
    } else {
        lines.push(format!("{role_label}:"));
    }

    for tool_call in &message.tool_calls {
        let mut rendered = tool_call.tool_name.clone();
        if let Some(arguments) = tool_call.arguments.as_ref() {
            rendered.push(' ');
            rendered.push_str(&render_tool_arguments(arguments));
        }
        if tool_call.is_error {
            rendered.push_str(" [error]");
        }
        lines.push(format!("Tool call: {}", rendered));
    }
}

fn render_tool_arguments(arguments: &Value) -> String {
    if arguments.is_null() {
        return "{}".to_string();
    }
    serde_json::to_string(arguments).unwrap_or_else(|_| json!({}).to_string())
}
