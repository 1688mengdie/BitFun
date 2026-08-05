//! Desktop implementation of the Warden model judgement port.
//!
//! Bridges `bitfun_runtime_ports::WardenModelJudgementPort` to a real model
//! call through the desktop `AIClientFactory` (fast model). The judgement
//! prompt embeds the candidate rule ids and the evidence summary; the model
//! response is parsed as JSON into `WardenAuditJudgementResponse`. Any model
//! failure, parse failure, or timeout returns `Err` so the audit caller falls
//! back to the mechanical rule ladder — the judgement port must never block
//! the audit loop on a broken model response.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core_types::Message;
use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, WardenAuditJudgementRequest,
    WardenAuditJudgementResponse, WardenModelJudgementPort,
};

/// Time budget for one judgement model call.
const WARDEN_JUDGEMENT_TIMEOUT: Duration = Duration::from_secs(30);

/// System prompt instructing the model to emit only the judgement JSON.
const WARDEN_JUDGEMENT_SYSTEM_PROMPT: &str = "You are the Warden audit judgement engine \
of an AI agent host. Given one finished agent action (tool call or turn) and a \
list of candidate discipline rules, decide whether the agent deserves a poke \
reminder. Respond with a single JSON object of the shape \
{\"shouldPoke\": bool, \"ruleIds\": [string], \"evidenceRequested\": [string]}. \
\"shouldPoke\" must be false for the first exploratory failure of a scene; \
pokes are for repeated failures of the same kind. Do not include any text \
outside the JSON object.";

/// Desktop implementation of [`WardenModelJudgementPort`] over the global AI
/// client factory.
pub(crate) struct DesktopWardenModelJudgementPort {
    ai_client_factory: Arc<AIClientFactory>,
}

impl std::fmt::Debug for DesktopWardenModelJudgementPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopWardenModelJudgementPort")
            .field("ai_client_factory", &"<AIClientFactory>")
            .finish()
    }
}

impl DesktopWardenModelJudgementPort {
    pub(crate) fn new(ai_client_factory: Arc<AIClientFactory>) -> Self {
        Self { ai_client_factory }
    }

    /// Build the user prompt embedding every judgement input.
    fn judgement_prompt(request: &WardenAuditJudgementRequest) -> String {
        let tool_args = request
            .tool_args
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
            .unwrap_or_else(|| "null".to_string());
        let evidence = request
            .evidence
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
            .unwrap_or_else(|| "null".to_string());
        format!(
            "sessionId: {}\ntoolName: {}\ntoolArgs: {}\ncandidateRuleIds: {}\nevidence: {}",
            request.session_id,
            request.tool_name,
            tool_args,
            request.rule_ids.join(", "),
            evidence
        )
    }
}

#[async_trait]
impl WardenModelJudgementPort for DesktopWardenModelJudgementPort {
    async fn judge_audit(
        &self,
        request: WardenAuditJudgementRequest,
    ) -> PortResult<WardenAuditJudgementResponse> {
        let client = self
            .ai_client_factory
            .get_client_resolved("fast")
            .await
            .map_err(|error| {
                PortError::new(
                    PortErrorKind::Backend,
                    format!("failed to resolve warden judgement model: {error}"),
                )
            })?;

        let messages = vec![
            Message::system(WARDEN_JUDGEMENT_SYSTEM_PROMPT.to_string()),
            Message::user(Self::judgement_prompt(&request)),
        ];

        let response = tokio::time::timeout(
            WARDEN_JUDGEMENT_TIMEOUT,
            client.send_message(messages, None),
        )
        .await
        .map_err(|_| {
            PortError::new(
                PortErrorKind::Timeout,
                "warden judgement timed out; caller falls back to mechanical rules",
            )
        })?
        .map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement model call failed: {error}"),
            )
        })?;

        let text = response.text.trim();
        if text.is_empty() {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "warden judgement model returned an empty response",
            ));
        }

        let json: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement response is not valid JSON: {error}"),
            )
        })?;
        serde_json::from_value(json).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement response does not match the expected shape: {error}"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgement_prompt_embeds_all_inputs() {
        let request = WardenAuditJudgementRequest {
            session_id: "sess-1".to_string(),
            tool_name: "ExecCommand".to_string(),
            tool_args: Some(serde_json::json!({"cmd": "pwd"})),
            rule_ids: vec!["iron-rules-compliance".to_string()],
            evidence: Some(serde_json::json!({"consecutiveFailures": 2})),
        };
        let prompt = DesktopWardenModelJudgementPort::judgement_prompt(&request);
        assert!(prompt.contains("sess-1"));
        assert!(prompt.contains("ExecCommand"));
        assert!(prompt.contains("pwd"));
        assert!(prompt.contains("iron-rules-compliance"));
        assert!(prompt.contains("consecutiveFailures"));
    }

    #[test]
    fn judgement_prompt_handles_missing_optional_inputs() {
        let request = WardenAuditJudgementRequest {
            session_id: "sess-2".to_string(),
            tool_name: "Read".to_string(),
            tool_args: None,
            rule_ids: Vec::new(),
            evidence: None,
        };
        let prompt = DesktopWardenModelJudgementPort::judgement_prompt(&request);
        assert!(prompt.contains("toolName: Read"));
        assert!(prompt.contains("toolArgs: null"));
        assert!(prompt.contains("candidateRuleIds: "));
        assert!(prompt.contains("evidence: null"));
    }
}
