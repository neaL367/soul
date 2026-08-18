//! IPC-backed network client implementation: request demux, response assembly,
//! and the named-pipe I/O loop that owns the transport.

use super::NetworkClientConfig;
use crate::error::NetworkError;
use crate::types::{HttpRequest, HttpResponse};
use ipc::{
    BrowserToNetworkMsg, InMemoryTransport, IpcMessage, MessageId, MessagePayload,
    NetworkToBrowserMsg,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, UnboundedSender};
use url::Url;

/// Outbound message sink of an IPC client.
pub(super) enum Outbound {
    /// In-memory transport; `send` is `&self`.
    Memory(Arc<InMemoryTransport>),
    /// Channel feeding a pipe I/O task that owns the stream.
    Proxy(mpsc::UnboundedSender<IpcMessage>),
}

/// Shared IPC client state: outbound sink, in-flight request demux, and counters.
pub struct IpcNetworkClient {
    outbound: Outbound,
    pending: Mutex<HashMap<u64, UnboundedSender<NetworkToBrowserMsg>>>,
    next_request_id: AtomicU64,
    config: NetworkClientConfig,
}

impl IpcNetworkClient {
    /// Creates an IPC client with the given outbound sink and configuration.
    pub(super) fn new(outbound: Outbound, config: NetworkClientConfig) -> Self {
        Self {
            outbound,
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            config,
        }
    }

    /// Executes one request over the IPC contract, demultiplexing concurrent
    /// requests by `request_id` and bounding the exchange by the configured timeout.
    pub(super) async fn fetch(
        &self,
        request: &HttpRequest,
        document_origin: Option<&Url>,
    ) -> Result<HttpResponse, NetworkError> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<NetworkToBrowserMsg>();
        self.pending
            .lock()
            .map_err(|_| NetworkError::TransportError("pending lock poisoned".into()))?
            .insert(request_id, response_tx);

        let message = IpcMessage::new(
            MessageId(request_id),
            MessagePayload::BrowserToNetwork(BrowserToNetworkMsg::FetchRequest {
                request_id,
                url: request.url.to_string(),
                method: request.method.as_str().to_string(),
                headers: request.headers.clone(),
                body: request.body.as_ref().map(|body| body.to_vec()),
                document_origin: document_origin.map(ToString::to_string),
            }),
        );

        if let Err(error) = self.send(message).await {
            self.remove_pending(request_id);
            return Err(error);
        }

        let outcome = tokio::time::timeout(
            self.config.timeout,
            assemble_response(request_id, &mut response_rx),
        )
        .await;
        self.remove_pending(request_id);

        match outcome {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => {
                let _ = self.send(cancel_message(request_id)).await;
                Err(error)
            }
            Err(_) => {
                let _ = self.send(cancel_message(request_id)).await;
                Err(NetworkError::Timeout)
            }
        }
    }

    /// Sends one message on the client's outbound sink.
    async fn send(&self, message: IpcMessage) -> Result<(), NetworkError> {
        match &self.outbound {
            Outbound::Memory(transport) => transport
                .send(message)
                .await
                .map_err(|e| NetworkError::TransportError(e.to_string())),
            Outbound::Proxy(tx) => tx
                .send(message)
                .map_err(|_| NetworkError::TransportError("network pipe closed".into())),
        }
    }

    /// Routes an inbound network message to its pending request channel.
    fn route(&self, message: &IpcMessage) {
        let MessagePayload::NetworkToBrowser(network_msg) = &message.payload else {
            return;
        };
        let request_id = network_request_id(network_msg);
        let Ok(pending) = self.pending.lock() else {
            return;
        };
        let routed = pending
            .get(&request_id)
            .is_some_and(|sender| sender.send(network_msg.clone()).is_ok());
        if !routed {
            tracing::debug!(request_id, "Dropping unroutable network message");
        }
    }

    /// Removes the demux entry for a finished or failed request.
    fn remove_pending(&self, request_id: u64) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&request_id);
        }
    }
}

/// Builds the cancellation message for a request id.
const fn cancel_message(request_id: u64) -> IpcMessage {
    IpcMessage::new(
        MessageId(request_id),
        MessagePayload::BrowserToNetwork(BrowserToNetworkMsg::CancelRequest { request_id }),
    )
}

/// Extracts the request id shared by every `NetworkToBrowserMsg` variant.
const fn network_request_id(message: &NetworkToBrowserMsg) -> u64 {
    match message {
        NetworkToBrowserMsg::ResponseHeaders { request_id, .. }
        | NetworkToBrowserMsg::ResponseBodyChunk { request_id, .. }
        | NetworkToBrowserMsg::ResponseComplete { request_id }
        | NetworkToBrowserMsg::ResponseFailed { request_id, .. } => *request_id,
    }
}

/// Assembles a full `HttpResponse` from the streamed response messages.
async fn assemble_response(
    request_id: u64,
    response_rx: &mut mpsc::UnboundedReceiver<NetworkToBrowserMsg>,
) -> Result<HttpResponse, NetworkError> {
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut final_url: Option<Url> = None;
    let mut set_cookies: Vec<String> = Vec::new();
    let mut status_code: u16 = 0;
    let mut body: Vec<u8> = Vec::new();

    loop {
        let message = response_rx
            .recv()
            .await
            .ok_or_else(|| NetworkError::TransportError("network service disconnected".into()))?;
        match message {
            NetworkToBrowserMsg::ResponseHeaders {
                request_id: message_id,
                status_code: code,
                headers: response_headers,
                final_url: url,
                set_cookies: cookies,
            } => {
                if message_id != request_id {
                    return Err(protocol_mismatch(request_id, message_id));
                }
                status_code = code;
                headers = response_headers.into_iter().collect();
                final_url = Some(Url::parse(&url).map_err(|e| {
                    NetworkError::TransportError(format!("invalid final URL: {e}"))
                })?);
                set_cookies = cookies;
            }
            NetworkToBrowserMsg::ResponseBodyChunk {
                request_id: message_id,
                data,
            } => {
                if message_id != request_id {
                    return Err(protocol_mismatch(request_id, message_id));
                }
                body.extend_from_slice(&data);
            }
            NetworkToBrowserMsg::ResponseComplete {
                request_id: message_id,
            } => {
                if message_id != request_id {
                    return Err(protocol_mismatch(request_id, message_id));
                }
                break;
            }
            NetworkToBrowserMsg::ResponseFailed {
                request_id: message_id,
                error,
            } => {
                if message_id != request_id {
                    return Err(protocol_mismatch(request_id, message_id));
                }
                return Err(NetworkError::Remote(error));
            }
        }
    }

    let url = final_url
        .ok_or_else(|| NetworkError::TransportError("response missing headers message".into()))?;
    let mime_type = headers.get("content-type").map_or_else(
        || "text/html".to_string(),
        |content_type| {
            content_type
                .split(';')
                .next()
                .unwrap_or(content_type)
                .trim()
                .to_string()
        },
    );

    Ok(HttpResponse {
        url,
        status_code,
        headers,
        set_cookies,
        body: body.into(),
        mime_type,
    })
}

/// Constructs a protocol-violation error for a mismatched request id.
fn protocol_mismatch(expected: u64, actual: u64) -> NetworkError {
    NetworkError::TransportError(format!(
        "response request id mismatch: expected {expected}, got {actual}"
    ))
}

/// Routes inbound in-memory transport messages to their pending requests.
pub(super) async fn demux_loop(mut inbound: InMemoryTransport, client: Arc<IpcNetworkClient>) {
    while let Ok(Some(message)) = inbound.recv().await {
        client.route(&message);
    }
    tracing::debug!("Network client demux loop exited");
}

/// Owns a named-pipe transport: writes outbound frames and routes inbound frames.
pub(super) async fn pipe_io_loop(
    mut transport: ipc::AsyncStreamTransport<tokio::net::windows::named_pipe::NamedPipeClient>,
    mut outbound: mpsc::UnboundedReceiver<IpcMessage>,
    client: Arc<IpcNetworkClient>,
) {
    loop {
        tokio::select! {
            outgoing = outbound.recv() => {
                let Some(message) = outgoing else { break; };
                if let Err(error) = transport.send(&message).await {
                    tracing::warn!(%error, "Network pipe write failed");
                    break;
                }
            }
            incoming = transport.recv() => {
                match incoming {
                    Ok(Some(message)) => client.route(&message),
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(%error, "Network pipe read failed");
                        break;
                    }
                }
            }
        }
    }
    tracing::debug!("Network pipe I/O loop exited");
}
