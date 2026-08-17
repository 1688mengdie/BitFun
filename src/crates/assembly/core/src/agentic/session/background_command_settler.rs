//! Settles sessions back to `Idle` when a background ExecCommand child process
//! that pinned the session to `Processing` exits.
//!
//! R-WF-25: the turn-completion path keeps the session `Processing` while a
//! background command is still running (keep_processing_turns marker). This
//! subscriber listens for the mirrored `BackgroundCommandLifecycleChanged`
//! agentic events and, once no `Running` background command remains for the
//! session, transitions it back to `Idle` and clears the marker. The watchdog
//! spawned at pin time is the fallback if a lifecycle event is missed.

use super::SessionManager;
use crate::agentic::core::SessionState;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use log::{debug, warn};
use std::sync::Arc;

/// Settles a keep-processing session back to `Idle` after its background
/// command exits.
pub struct BackgroundCommandSettlerSubscriber {
    session_manager: Arc<SessionManager>,
}

impl BackgroundCommandSettlerSubscriber {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for BackgroundCommandSettlerSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        let AgenticEvent::BackgroundCommandLifecycleChanged { session_id, status } = event else {
            return Ok(());
        };
        if status == "running" {
            return Ok(());
        }

        let Some(turn_id) = self.session_manager.keep_processing_turn(session_id) else {
            return Ok(());
        };

        // Double-check the registry: only settle when no Running command
        // remains for the session (another child could still be alive).
        let response = tool_runtime::background_command_output::background_command_output_capture()
            .list(tool_runtime::background_command_output::ListBackgroundCommandOutputRequest {
                agent_session_id: Some(session_id.clone()),
            })
            .await;
        if response
            .activities
            .iter()
            .any(|metadata| metadata.status == tool_runtime::background_command_output::BackgroundCommandOutputStatus::Running)
        {
            debug!(
                "Background command lifecycle settled but another command still running; keeping Processing: session_id={}",
                session_id
            );
            return Ok(());
        }

        debug!(
            "Background command settled; transitioning session back to Idle: session_id={}, turn_id={}",
            session_id, turn_id
        );
        if let Err(error) = self
            .session_manager
            .update_session_state_for_turn_if_processing(
                session_id,
                &turn_id,
                SessionState::Idle,
            )
            .await
        {
            warn!(
                "Failed to settle session to Idle after background command exit: session_id={}, error={}",
                session_id, error
            );
        }
        self.session_manager.clear_keep_processing_turn(session_id);

        Ok(())
    }
}
