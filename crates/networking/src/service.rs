//! Network service handling IPC requests, with in-process security enforcement.
//!
//! The service owns an [`HttpClient`] and executes browser requests received as
//! [`BrowserToNetworkMsg`] commands over either the in-memory transport
//! ([`NetworkService::run`]) or a Windows named pipe
//! ([`NetworkService::run_named_pipe`]). Responses are streamed back as
//! [`NetworkToBrowserMsg`] messages: headers, then body chunks, then completion
//! (or failure).
//!
//! Mixed-content and CORS enforcement runs here, inside the network process,
//! using the `document_origin` carried on each [`BrowserToNetworkMsg::FetchRequest`].

use crate::client::HttpClient;
use crate::error::NetworkError;
use crate::types::{HttpMethod, HttpRequest, HttpResponse};
use bytes::Bytes;
use ipc::{
    AsyncStreamWriteHalf, BrowserToNetworkMsg, InMemoryTransport, IpcMessage, MessageId,
    MessagePayload, NetworkToBrowserMsg,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use url::Url;

/// Network process service handling HTTP requests dispatched across IPC boundaries.
#[derive(Default)]
pub struct NetworkService {
    client: HttpClient,
    active_requests: HashMap<u64, JoinHandle<()>>,
}

impl NetworkService {
    /// Creates a new `NetworkService`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: HttpClient::default(),
            active_requests: HashMap::new(),
        }
    }

    /// Asynchronously runs the network service message loop over the in-memory transport.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if IPC transport encounters an unrecoverable failure.
    pub async fn run(
        mut self,
        mut in_rx: InMemoryTransport,
        out_tx: Arc<InMemoryTransport>,
    ) -> Result<(), NetworkError> {
        while let Some(msg) = in_rx
            .recv()
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?
        {
            if let MessagePayload::BrowserToNetwork(command) = msg.payload {
                self.handle_message(command, out_tx.clone());
            }
        }
        Ok(())
    }

    /// Dispatches one browser command over the in-memory transport. Fetch requests
    /// run on spawned tasks so concurrent requests progress independently and can
    /// be cancelled by request id.
    pub fn handle_message(&mut self, msg: BrowserToNetworkMsg, out_tx: Arc<InMemoryTransport>) {
        self.prune_finished();
        match msg {
            BrowserToNetworkMsg::FetchRequest {
                request_id,
                url,
                method,
                headers,
                body,
                document_origin,
            } => {
                let client = self.client.clone();
                let handle = tokio::spawn(async move {
                    let result =
                        execute_request(&client, &url, &method, &headers, body, document_origin)
                            .await;
                    stream_result(request_id, result, &out_tx).await;
                });
                self.active_requests.insert(request_id, handle);
            }
            BrowserToNetworkMsg::CancelRequest { request_id } => {
                if let Some(handle) = self.active_requests.remove(&request_id) {
                    handle.abort();
                }
            }
        }
    }

    /// Asynchronously runs the network service message loop over a Windows Named Pipe.
    ///
    /// The pipe is split into framed read/write halves: the read loop dispatches
    /// requests while spawned fetch workers stream responses through the shared
    /// write half, so cancellation and concurrency behave exactly as they do on
    /// the in-memory transport.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if pipe connection or transport error occurs.
    pub async fn run_named_pipe(mut self, pipe_name: &str) -> Result<(), NetworkError> {
        let transport = ipc::accept_named_pipe_server(pipe_name)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;
        let (mut reader, writer) = transport.split();
        let writer = Arc::new(Mutex::new(writer));

        while let Some(msg) = reader
            .recv()
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?
        {
            if let MessagePayload::BrowserToNetwork(command) = msg.payload {
                self.handle_pipe_message(command, writer.clone());
            }
        }
        Ok(())
    }

    /// Dispatches one browser command over the named-pipe write half.
    fn handle_pipe_message(
        &mut self,
        msg: BrowserToNetworkMsg,
        writer: Arc<Mutex<AsyncStreamWriteHalf<NamedPipeServer>>>,
    ) {
        self.prune_finished();
        match msg {
            BrowserToNetworkMsg::FetchRequest {
                request_id,
                url,
                method,
                headers,
                body,
                document_origin,
            } => {
                let client = self.client.clone();
                let handle = tokio::spawn(async move {
                    let result =
                        execute_request(&client, &url, &method, &headers, body, document_origin)
                            .await;
                    let messages = match result {
                        Ok(response) => response_messages(request_id, &response),
                        Err(error) => vec![failure_message(request_id, &error.to_string())],
                    };
                    let mut writer = writer.lock().await;
                    for message in &messages {
                        if let Err(error) = writer.send(message).await {
                            tracing::warn!(%error, "Failed to stream response to network pipe");
                            break;
                        }
                    }
                });
                self.active_requests.insert(request_id, handle);
            }
            BrowserToNetworkMsg::CancelRequest { request_id } => {
                if let Some(handle) = self.active_requests.remove(&request_id) {
                    handle.abort();
                }
            }
        }
    }

    /// Drops bookkeeping entries for fetch tasks that already completed.
    fn prune_finished(&mut self) {
        self.active_requests
            .retain(|_, handle| !handle.is_finished());
    }
}

/// Executes a request from its IPC message fields with mixed-content and CORS
/// enforcement against the serialized document origin.
async fn execute_request(
    client: &HttpClient,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<Vec<u8>>,
    document_origin: Option<String>,
) -> Result<HttpResponse, NetworkError> {
    let url =
        Url::parse(url).map_err(|e| NetworkError::Other(format!("invalid request URL: {e}")))?;
    let method = match method.to_ascii_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "HEAD" => HttpMethod::Head,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        other => return Err(NetworkError::UnsupportedMethod(other.to_string())),
    };
    let document_origin = document_origin
        .as_deref()
        .map(Url::parse)
        .transpose()
        .map_err(|e| NetworkError::Other(format!("invalid document origin: {e}")))?;

    let request = HttpRequest {
        url,
        method,
        headers: headers.to_vec(),
        body: body.map(Bytes::from),
    };
    client
        .fetch_with_security_context(&request, document_origin.as_ref())
        .await
}

/// Streams a completed request outcome into an in-memory transport.
async fn stream_result(
    request_id: u64,
    result: Result<HttpResponse, NetworkError>,
    out_tx: &Arc<InMemoryTransport>,
) {
    let messages = match result {
        Ok(response) => response_messages(request_id, &response),
        Err(error) => vec![failure_message(request_id, &error.to_string())],
    };
    for message in &messages {
        if out_tx.send(message.clone()).await.is_err() {
            tracing::warn!(request_id, "Network response channel closed");
            break;
        }
    }
}

/// Builds the streaming response message sequence for a completed request.
fn response_messages(request_id: u64, response: &HttpResponse) -> Vec<IpcMessage> {
    let headers = response
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    vec![
        IpcMessage::new(
            MessageId(request_id),
            MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseHeaders {
                request_id,
                status_code: response.status_code,
                headers,
                final_url: response.url.to_string(),
                set_cookies: response.set_cookies.clone(),
            }),
        ),
        IpcMessage::new(
            MessageId(request_id),
            MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseBodyChunk {
                request_id,
                data: response.body.to_vec(),
            }),
        ),
        IpcMessage::new(
            MessageId(request_id),
            MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseComplete { request_id }),
        ),
    ]
}

/// Builds the failure message for a request that could not be completed.
fn failure_message(request_id: u64, error: &str) -> IpcMessage {
    IpcMessage::new(
        MessageId(request_id),
        MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseFailed {
            request_id,
            error: error.to_string(),
        }),
    )
}
