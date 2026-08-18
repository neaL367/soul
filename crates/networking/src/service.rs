//! Out-of-process Network service handler processing IPC requests.

use crate::client::HttpClient;
use crate::error::NetworkError;
use ipc::{
    BrowserToNetworkMsg, InMemoryTransport, IpcMessage, MessageId, MessagePayload,
    NetworkToBrowserMsg,
};
use std::collections::HashMap;
use std::sync::Arc;
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

    /// Handles an incoming `BrowserToNetworkMsg`, executing the request asynchronously.
    pub fn handle_message(&mut self, msg: BrowserToNetworkMsg, transport: Arc<InMemoryTransport>) {
        match msg {
            BrowserToNetworkMsg::FetchRequest {
                request_id,
                url,
                method: _,
                headers: _,
            } => {
                let client = self.client.clone();
                let transport_clone = transport;

                let handle = tokio::spawn(async move {
                    let parsed_url = match Url::parse(&url) {
                        Ok(u) => u,
                        Err(e) => {
                            let fail_msg = IpcMessage::new(
                                MessageId(request_id),
                                MessagePayload::NetworkToBrowser(
                                    NetworkToBrowserMsg::ResponseFailed {
                                        request_id,
                                        error: e.to_string(),
                                    },
                                ),
                            );
                            let _ = transport_clone.send(fail_msg).await;
                            return;
                        }
                    };

                    match client.fetch(&parsed_url).await {
                        Ok(response) => {
                            let headers = response
                                .headers
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();

                            let header_msg = IpcMessage::new(
                                MessageId(request_id),
                                MessagePayload::NetworkToBrowser(
                                    NetworkToBrowserMsg::ResponseHeaders {
                                        request_id,
                                        status_code: response.status_code,
                                        headers,
                                    },
                                ),
                            );
                            let _ = transport_clone.send(header_msg).await;

                            let chunk_msg = IpcMessage::new(
                                MessageId(request_id),
                                MessagePayload::NetworkToBrowser(
                                    NetworkToBrowserMsg::ResponseBodyChunk {
                                        request_id,
                                        data: response.body.to_vec(),
                                    },
                                ),
                            );
                            let _ = transport_clone.send(chunk_msg).await;

                            let complete_msg = IpcMessage::new(
                                MessageId(request_id),
                                MessagePayload::NetworkToBrowser(
                                    NetworkToBrowserMsg::ResponseComplete { request_id },
                                ),
                            );
                            let _ = transport_clone.send(complete_msg).await;
                        }
                        Err(e) => {
                            let fail_msg = IpcMessage::new(
                                MessageId(request_id),
                                MessagePayload::NetworkToBrowser(
                                    NetworkToBrowserMsg::ResponseFailed {
                                        request_id,
                                        error: e.to_string(),
                                    },
                                ),
                            );
                            let _ = transport_clone.send(fail_msg).await;
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

    /// Asynchronously runs the network service message loop.
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
            if let MessagePayload::BrowserToNetwork(net_cmd) = msg.payload {
                self.handle_message(net_cmd, out_tx.clone());
            }
        }
        Ok(())
    }

    /// Asynchronously runs the network service over a Windows Named Pipe.
    ///
    /// # Errors
    /// Returns `NetworkError` if pipe connection or transport error occurs.
    pub async fn run_named_pipe(self, pipe_name: &str) -> Result<(), NetworkError> {
        let mut transport = ipc::accept_named_pipe_server(pipe_name)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

        while let Some(msg) = transport
            .recv()
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?
        {
            if let MessagePayload::BrowserToNetwork(BrowserToNetworkMsg::FetchRequest {
                request_id,
                url,
                method: _,
                headers: _,
            }) = msg.payload
            {
                let client = self.client.clone();
                let parsed_url = match Url::parse(&url) {
                    Ok(u) => u,
                    Err(e) => {
                        let fail_msg = IpcMessage::new(
                            MessageId(request_id),
                            MessagePayload::NetworkToBrowser(
                                NetworkToBrowserMsg::ResponseFailed {
                                    request_id,
                                    error: e.to_string(),
                                },
                            ),
                        );
                        transport
                            .send(&fail_msg)
                            .await
                            .map_err(|e| NetworkError::TransportError(e.to_string()))?;
                        continue;
                    }
                };

                match client.fetch(&parsed_url).await {
                    Ok(response) => {
                        let headers = response
                            .headers
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();

                        let header_msg = IpcMessage::new(
                            MessageId(request_id),
                            MessagePayload::NetworkToBrowser(
                                NetworkToBrowserMsg::ResponseHeaders {
                                    request_id,
                                    status_code: response.status_code,
                                    headers,
                                },
                            ),
                        );
                        transport
                            .send(&header_msg)
                            .await
                            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

                        let chunk_msg = IpcMessage::new(
                            MessageId(request_id),
                            MessagePayload::NetworkToBrowser(
                                NetworkToBrowserMsg::ResponseBodyChunk {
                                    request_id,
                                    data: response.body.to_vec(),
                                },
                            ),
                        );
                        transport
                            .send(&chunk_msg)
                            .await
                            .map_err(|e| NetworkError::TransportError(e.to_string()))?;

                        let complete_msg = IpcMessage::new(
                            MessageId(request_id),
                            MessagePayload::NetworkToBrowser(
                                NetworkToBrowserMsg::ResponseComplete { request_id },
                            ),
                        );
                        transport
                            .send(&complete_msg)
                            .await
                            .map_err(|e| NetworkError::TransportError(e.to_string()))?;
                    }
                    Err(e) => {
                        let fail_msg = IpcMessage::new(
                            MessageId(request_id),
                            MessagePayload::NetworkToBrowser(
                                NetworkToBrowserMsg::ResponseFailed {
                                    request_id,
                                    error: e.to_string(),
                                },
                            ),
                        );
                        transport
                            .send(&fail_msg)
                            .await
                            .map_err(|e| NetworkError::TransportError(e.to_string()))?;
                    }
                }
            }
        }

        Ok(())
    }
}
