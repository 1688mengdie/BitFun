//! Desktop-side implementation of the ACP client runtime port.
//!
//! Bridges `bitfun_runtime_ports::AcpClientPort` to the real `AcpClientService`
//! owned by the desktop host. The desktop host may depend on `runtime-ports`
//! (unlike `bitfun-acp`'s `client` feature, whose capability domain excludes
//! the runtime-ports edge), so this is the legitimate injection point of the
//! ACP port used by `SessionControl` / `SessionMessage`.

use std::sync::Arc;

use async_trait::async_trait;
use bitfun_acp::client::AcpClientStreamEvent;
use bitfun_acp::AcpClientService;
use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::service::remote_ssh::workspace_state::get_effective_session_path;
use bitfun_runtime_ports::{
    acp_backend_error, acp_flow_client_id_from_session_id, AcpClientBitfunMessageRequest,
    AcpClientCreateRequest, AcpClientCreateResult, AcpClientListResult, AcpClientMessageRequest,
    AcpClientMessageResult, AcpClientPort, AcpClientStreamChunk, AcpClientStreamChunkSink,
    AcpClientSummary, PortError, PortErrorKind, PortResult, RuntimeServiceCapability,
    RuntimeServicePort,
};

/// Desktop implementation of [`AcpClientPort`] over the real ACP client service.
pub(crate) struct DesktopAcpClientPort {
    acp_client_service: Option<Arc<AcpClientService>>,
    coordinator: Option<Arc<ConversationCoordinator>>,
}

impl std::fmt::Debug for DesktopAcpClientPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopAcpClientPort")
            .field(
                "acp_client_service",
                &self
                    .acp_client_service
                    .as_ref()
                    .map(|_| "<AcpClientService>"),
            )
            .field(
                "coordinator",
                &self
                    .coordinator
                    .as_ref()
                    .map(|_| "<ConversationCoordinator>"),
            )
            .finish()
    }
}

impl DesktopAcpClientPort {
    pub(crate) fn new(
        acp_client_service: Option<Arc<AcpClientService>>,
        coordinator: Option<Arc<ConversationCoordinator>>,
    ) -> Self {
        Self {
            acp_client_service,
            coordinator,
        }
    }

    fn service(&self) -> PortResult<&Arc<AcpClientService>> {
        self.acp_client_service
            .as_ref()
            .ok_or_else(|| acp_backend_error("ACP client service not initialized"))
    }
}

impl RuntimeServicePort for DesktopAcpClientPort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::AcpClient
    }
}

#[async_trait]
impl AcpClientPort for DesktopAcpClientPort {
    async fn create_session(
        &self,
        request: AcpClientCreateRequest,
    ) -> PortResult<AcpClientCreateResult> {
        let service = self.service()?.clone();
        let session_storage_path =
            get_effective_session_path(&request.workspace_path, None, None).await;
        let response = service
            .create_flow_session_record(
                &session_storage_path,
                &request.workspace_path,
                &request.client_id,
                request.session_name,
            )
            .await
            .map_err(|error| acp_backend_error(format!("failed to create ACP session: {error}")))?;
        if let Err(error) = service
            .start_client_for_session(
                &request.client_id,
                &response.session_id,
                Some(&request.workspace_path),
                request.remote_connection_id.as_deref(),
            )
            .await
        {
            let _ = service
                .delete_flow_session_record(&session_storage_path, &response.session_id)
                .await;
            return Err(acp_backend_error(format!(
                "failed to start ACP client for session: {error}"
            )));
        }
        Ok(AcpClientCreateResult {
            session_id: response.session_id,
            session_name: response.session_name,
            agent_type: response.agent_type,
        })
    }

    async fn list_clients(&self) -> PortResult<AcpClientListResult> {
        let infos =
            self.service()?.list_clients().await.map_err(|error| {
                acp_backend_error(format!("failed to list ACP clients: {error}"))
            })?;
        Ok(AcpClientListResult {
            clients: infos
                .into_iter()
                .map(|info| AcpClientSummary {
                    client_id: info.id,
                    name: info.name,
                    status: format!("{:?}", info.status),
                    session_count: info.session_count,
                    readonly: info.readonly,
                })
                .collect(),
        })
    }

    async fn send_message_stream(
        &self,
        request: AcpClientMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult> {
        let service = self.service()?.clone();
        let client_id = acp_flow_client_id_from_session_id(&request.session_id).ok_or_else(|| {
            PortError::new(
                PortErrorKind::InvalidRequest,
                format!(
                    "session_id '{}' is not an ACP flow session id (expected acp_<client_id>_<uuid>)",
                    request.session_id
                ),
            )
        })?;
        let mut response = String::new();
        service
            .prompt_agent_stream(
                &client_id,
                request.message,
                request.workspace_path,
                None,
                request.session_id.clone(),
                None,
                request.timeout_seconds,
                |event| forward_acp_stream_event(event, &mut response, &chunk_sink),
            )
            .await
            .map_err(|error| acp_backend_error(format!("ACP agent failed: {error}")))?;
        Ok(AcpClientMessageResult {
            session_id: request.session_id,
            response,
        })
    }

    async fn send_message_to_bitfun_session_stream(
        &self,
        request: AcpClientBitfunMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult> {
        let service = self.service()?.clone();
        let mut response = String::new();
        service
            .prompt_agent_stream(
                &request.client_id,
                request.message,
                request.workspace_path,
                None,
                request.bitfun_session_id.clone(),
                None,
                request.timeout_seconds,
                |event| forward_acp_stream_event(event, &mut response, &chunk_sink),
            )
            .await
            .map_err(|error| acp_backend_error(format!("ACP agent failed: {error}")))?;
        Ok(AcpClientMessageResult {
            session_id: request.bitfun_session_id,
            response,
        })
    }
}

/// Translate a native `AcpClientStreamEvent` into the boundary
/// `AcpClientStreamChunk` sequence pushed into `chunk_sink`, accumulating the
/// full response text from `AgentText` chunks.
fn forward_acp_stream_event(
    event: AcpClientStreamEvent,
    response: &mut String,
    chunk_sink: &AcpClientStreamChunkSink,
) -> bitfun_core::util::errors::BitFunResult<()> {
    match event {
        AcpClientStreamEvent::AgentText(text) => {
            response.push_str(&text);
            let _ = chunk_sink.send(AcpClientStreamChunk::Text { text });
        }
        AcpClientStreamEvent::AgentThought(text) => {
            let _ = chunk_sink.send(AcpClientStreamChunk::Thought { text });
        }
        AcpClientStreamEvent::Completed => {
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
        }
        AcpClientStreamEvent::Cancelled => {
            let _ = chunk_sink.send(AcpClientStreamChunk::Cancelled);
        }
        _ => {}
    }
    Ok(())
}
