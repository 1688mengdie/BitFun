//! Data conversion utilities
//!
//! Used to convert between frontend data structures and persisted data structures.

use super::types::*;
use serde_json::Value;

/// Creates `SessionMetadata` from the frontend session configuration.
pub fn create_session_metadata(
    session_id: String,
    session_name: String,
    agent_type: String,
    model_name: String,
    terminal_session_id: Option<String>,
) -> SessionMetadata {
    let mut metadata = SessionMetadata::new(session_id, session_name, agent_type, model_name);

    metadata.terminal_session_id = terminal_session_id;
    metadata.snapshot_session_id = Some(metadata.session_id.clone());

    metadata
}

/// Builds `DialogTurnData` from event data.
pub fn build_dialog_turn_from_events(
    turn_id: String,
    turn_index: usize,
    session_id: String,
    user_message_content: String,
    model_rounds: Vec<ModelRoundFromEvents>,
) -> DialogTurnData {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let user_message = UserMessageData {
        id: format!("{}_user", turn_id),
        content: user_message_content,
        timestamp: now,
        metadata: None,
    };

    let model_rounds_data: Vec<ModelRoundData> = model_rounds
        .into_iter()
        .enumerate()
        .map(|(idx, round)| convert_model_round(round, idx, turn_id.clone()))
        .collect();

    let start_time = model_rounds_data
        .first()
        .map(|r| r.start_time)
        .unwrap_or(now);

    let end_time = model_rounds_data.last().and_then(|r| r.end_time);

    let duration_ms = end_time.map(|end| end.saturating_sub(start_time));

    DialogTurnData {
        turn_id: turn_id.clone(),
        turn_index,
        session_id,
        timestamp: now,
        user_message,
        model_rounds: model_rounds_data,
        start_time,
        end_time,
        duration_ms,
        status: if end_time.is_some() {
            TurnStatus::Completed
        } else {
            TurnStatus::InProgress
        },
    }
}

/// Model round data collected from events
#[derive(Debug, Clone)]
pub struct ModelRoundFromEvents {
    pub id: String,
    pub text_contents: Vec<String>, // Accumulated text content
    pub tool_calls: Vec<ToolCallFromEvents>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub status: String,
}

/// Tool call data collected from events
#[derive(Debug, Clone)]
pub struct ToolCallFromEvents {
    pub id: String,
    pub tool_name: String,
    pub input: Value,
    pub result: Option<Value>,
    pub success: bool,
    pub error: Option<String>,
    pub ai_intent: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
}

fn convert_model_round(
    round: ModelRoundFromEvents,
    round_index: usize,
    turn_id: String,
) -> ModelRoundData {
    let text_items: Vec<TextItemData> = round
        .text_contents
        .into_iter()
        .enumerate()
        .map(|(idx, content)| TextItemData {
            id: format!("{}_text_{}", round.id, idx),
            content,
            is_streaming: false,
            timestamp: round.start_time,
            is_markdown: true,
            order_index: None,
            status: None,
            is_subagent_item: None,
            parent_task_tool_id: None,
            subagent_session_id: None,
        })
        .collect();

    let tool_items: Vec<ToolItemData> = round
        .tool_calls
        .into_iter()
        .enumerate()
        .map(|(idx, tool)| {
            let duration_ms = tool.end_time.map(|end| end.saturating_sub(tool.start_time));

            ToolItemData {
                id: format!("{}_tool_{}", round.id, idx),
                tool_name: tool.tool_name,
                tool_call: ToolCallData {
                    input: tool.input,
                    id: tool.id,
                },
                tool_result: tool.result.map(|result| ToolResultData {
                    result,
                    success: tool.success,
                    error: tool.error,
                    duration_ms,
                }),
                ai_intent: tool.ai_intent,
                start_time: tool.start_time,
                end_time: tool.end_time,
                duration_ms,
                order_index: None,
                status: None,
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
            }
        })
        .collect();

    ModelRoundData {
        id: round.id,
        turn_id,
        round_index,
        timestamp: round.start_time,
        text_items,
        tool_items,
        thinking_items: Vec::new(),
        start_time: round.start_time,
        end_time: round.end_time,
        status: round.status,
    }
}
