//! Desktop-side ACP session lifecycle bridge.
//!
//! `SessionControl` creates `acp__<client>` sessions as plain internal
//! sessions (the external ACP process is never started by the tool itself).
//! This subscriber bridges the core coordinator's agentic lifecycle events
//! back to the ACP client service so the external process lifecycle follows
//! the internal session lifecycle:
//!
//! - `SessionCreated` with an `acp__*` agent type starts the external client
//!   process for that session (idempotent; a running connection is reused).
//! - `SessionDeleted` releases the ACP session, so no external process or
//!   remote session outlives the internal session.
//! - `DialogTurnCancelled` cancels the matching ACP dialog turn when the
//!   internal turn is cancelled (for example through SessionControl cancel).
//!
//! The bridge only touches the ACP client service from the desktop layer;
//! core keeps no dependency on the ACP service.

use std::sync::Arc;

use async_trait::async_trait;
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use bitfun_agent_runtime::event_router::EventSubscriber;
use bitfun_events::AgenticEvent;

/// Routes agentic session lifecycle events to the ACP client service.
pub(crate) struct AcpSessionLifecycleSubscriber {
    acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>,
}

impl AcpSessionLifecycleSubscriber {
    pub(crate) fn new(acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>) -> Self {
        Self {
            acp_client_service,
        }
    }
}

#[async_trait]
impl EventSubscriber for AcpSessionLifecycleSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        match event {
            // Start the external ACP client process when an `acp__<client>`
            // session is created (SessionControl create path). Failure is a
            // warning only: the internal session stays usable for the
            // forwarding tool, and the process can still be started lazily
            // by the first delegated turn.
            AgenticEvent::SessionCreated {
                session_id,
                agent_type,
                workspace_path,
                remote_connection_id,
                ..
            } => {
                let Some(client_id) = agent_type.strip_prefix("acp__") else {
                    return Ok(());
                };
                let Some(service) = self.acp_client_service.as_ref() else {
                    return Ok(());
                };
                if let Err(error) = service
                    .start_client_for_session(
                        client_id,
                        session_id,
                        workspace_path.as_deref(),
                        remote_connection_id.as_deref(),
                    )
                    .await
                {
                    log::warn!(
                        "Failed to start ACP client for session: session_id={}, client_id={}, error={}",
                        session_id,
                        client_id,
                        error
                    );
                }
            }
            // SessionControl delete and the frontend delete both flow through
            // coordinator.delete_session_tree, which emits SessionDeleted.
            // Releasing here is idempotent and complements the frontend
            // delete path's host-effects release.
            AgenticEvent::SessionDeleted { session_id } => {
                if let Some(service) = self.acp_client_service.as_ref() {
                    service.release_bitfun_session(session_id).await;
                }
            }
            // SessionControl cancel flows through runtime.cancel_turn; the
            // coordinator emits DialogTurnCancelled (duplicates are harmless).
            AgenticEvent::DialogTurnCancelled { session_id, .. } => {
                if let Some(service) = self.acp_client_service.as_ref() {
                    if let Err(error) = service.cancel_bitfun_session(session_id).await {
                        log::warn!(
                            "Failed to cancel ACP session after dialog turn cancellation: session_id={}, error={}",
                            session_id,
                            error
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
