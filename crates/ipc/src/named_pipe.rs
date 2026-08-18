//! Windows Named Pipe asynchronous server and client transports.

use crate::error::IpcError;
use crate::transport::AsyncStreamTransport;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

static PIPE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Formats a valid Windows named pipe path (`\\.\pipe\soul-<prefix>-<pid>-<seq>`).
#[must_use]
pub fn generate_pipe_name(prefix: &str) -> String {
    let id = PIPE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    format!(r"\\.\pipe\soul-{prefix}-{pid}-{id}")
}

/// Creates a Windows Named Pipe server and waits for a client to connect.
///
/// # Errors
///
/// Returns `IpcError::Io` if pipe creation or connection fails.
pub async fn accept_named_pipe_server(
    pipe_name: &str,
) -> Result<AsyncStreamTransport<NamedPipeServer>, IpcError> {
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(pipe_name)?;
    server.connect().await?;
    Ok(AsyncStreamTransport::new(server))
}

/// Asynchronously connects a Windows Named Pipe client to `pipe_name`.
///
/// # Errors
///
/// Returns `IpcError::Io` if connection fails.
pub async fn connect_named_pipe_client(
    pipe_name: &str,
) -> Result<AsyncStreamTransport<NamedPipeClient>, IpcError> {
    let client = ClientOptions::new().open(pipe_name)?;
    Ok(AsyncStreamTransport::new(client))
}
