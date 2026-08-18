//! Browser-side network client facade over the IPC contract.
//!
//! [`NetworkClient`] is the single network access point used by the browser
//! engine. It exposes the same async surface as the direct HTTP client while
//! routing requests through [`NetworkService`] as typed
//! [`BrowserToNetworkMsg`]/[`NetworkToBrowserMsg`] messages, so the transport
//! (in-memory channel vs. Windows named pipe) can be swapped without touching
//! call sites — the property that makes the eventual network-process split a
//! transport change rather than a rewrite (ADR-2/ADR-5).

mod ipc_client;

use crate::client::HttpClient;
use crate::error::NetworkError;
use crate::types::{HttpRequest, HttpResponse};
use ipc::{InMemoryTransport, IpcMessage};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use url::Url;

pub use ipc_client::IpcNetworkClient;

/// Configuration for the browser-side network client facade.
#[derive(Debug, Clone)]
pub struct NetworkClientConfig {
    /// Per-request timeout budget applied while assembling IPC responses.
    pub timeout: Duration,
    /// In-process transport channel capacity.
    pub channel_capacity: usize,
}

impl Default for NetworkClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            channel_capacity: 64,
        }
    }
}

/// Browser-facing network access point wrapping either the direct in-process
/// HTTP client or the IPC network service behind one message-shaped API.
#[derive(Clone)]
pub enum NetworkClient {
    /// Direct in-process HTTP client (headless/CLI/test path).
    Direct(HttpClient),
    /// IPC client routing requests through the `NetworkService`.
    Ipc(Arc<IpcNetworkClient>),
}

impl NetworkClient {
    /// Creates a client using the direct HTTP client.
    #[must_use]
    pub fn direct() -> Self {
        Self::Direct(HttpClient::default())
    }

    /// Spawns an in-process `NetworkService` and returns an IPC client wired to it.
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime context, because spawning the
    /// service and demux tasks requires one.
    pub async fn ipc_in_process() -> Self {
        Self::ipc_in_process_with_config(NetworkClientConfig::default()).await
    }

    /// Spawns an in-process `NetworkService` with the given client configuration.
    ///
    /// The `async` keyword is load-bearing: it forces callers to enter a Tokio
    /// runtime context, which the spawned service and demux tasks require.
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime context, because spawning the
    /// service and demux tasks requires one.
    #[allow(clippy::unused_async)]
    pub async fn ipc_in_process_with_config(config: NetworkClientConfig) -> Self {
        let (browser_outbound, service_in) = InMemoryTransport::pair(config.channel_capacity);
        let (service_outbound, browser_inbound) = InMemoryTransport::pair(config.channel_capacity);
        let service_outbound = Arc::new(service_outbound);

        tokio::spawn(crate::service::NetworkService::new().run(service_in, service_outbound));

        let client = Arc::new(IpcNetworkClient::new(
            ipc_client::Outbound::Memory(Arc::new(browser_outbound)),
            config,
        ));
        tokio::spawn(ipc_client::demux_loop(browser_inbound, client.clone()));
        Self::Ipc(client)
    }

    /// Connects an IPC client to a running named-pipe `NetworkService`.
    ///
    /// The pipe is owned by a single I/O task that multiplexes outbound frames
    /// and inbound responses.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the named pipe cannot be connected.
    pub async fn ipc_named_pipe(
        config: NetworkClientConfig,
        pipe_name: &str,
    ) -> Result<Self, NetworkError> {
        let transport = ipc::connect_named_pipe_client(pipe_name)
            .await
            .map_err(|e| NetworkError::TransportError(e.to_string()))?;
        let (tx, rx) = mpsc::unbounded_channel::<IpcMessage>();

        let client = Arc::new(IpcNetworkClient::new(
            ipc_client::Outbound::Proxy(tx),
            config,
        ));
        tokio::spawn(ipc_client::pipe_io_loop(transport, rx, client.clone()));
        Ok(Self::Ipc(client))
    }

    /// Fetches a URL with a standard HTTP GET request, without a security context.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the request fails or the timeout budget is exceeded.
    pub async fn fetch(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        self.fetch_with_security_context(&HttpRequest::get(url.clone()), None)
            .await
    }

    /// Executes an HTTP request with document origin security checks (mixed
    /// content and CORS) applied at the network boundary.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if mixed content, CORS, transport, or timeout checks fail.
    pub async fn fetch_with_security_context(
        &self,
        request: &HttpRequest,
        document_origin: Option<&Url>,
    ) -> Result<HttpResponse, NetworkError> {
        match self {
            Self::Direct(client) => {
                client
                    .fetch_with_security_context(request, document_origin)
                    .await
            }
            Self::Ipc(inner) => inner.fetch(request, document_origin).await,
        }
    }
}
