//! Integration tests for Windows Named Pipe IPC framing and transmission.

use ipc::{
    AsyncStreamTransport, BrowserToRendererMsg, IpcMessage, MessageId, MessagePayload,
    accept_named_pipe_server, connect_named_pipe_client,
};
use std::time::Duration;
use tokio::net::windows::named_pipe::NamedPipeClient;
use tokio::time::sleep;

/// Connects a named-pipe client with retries so the test does not race the
/// server's accept, which on Windows can transiently fail before the server
/// reaches its connect. Mirrors the retry pattern used by the network client
/// tests.
async fn connect_with_retry(
    pipe_name: &str,
    attempts: u32,
) -> Result<AsyncStreamTransport<NamedPipeClient>, ipc::IpcError> {
    let mut last_error = None;
    for _ in 0..attempts {
        match connect_named_pipe_client(pipe_name).await {
            Ok(transport) => return Ok(transport),
            Err(error) => last_error = Some(error),
        }
        sleep(Duration::from_millis(50)).await;
    }
    Err(last_error.expect("at least one attempt ran"))
}

#[tokio::test]
async fn test_named_pipe_ipc_message_roundtrip() {
    let pipe_name = ipc::generate_pipe_name("test-pipe");

    let pipe_name_clone = pipe_name.clone();
    let server_task = tokio::spawn(async move {
        let mut server_transport = accept_named_pipe_server(&pipe_name_clone)
            .await
            .expect("Accept named pipe server failed");
        server_transport.recv().await
    });

    let mut client_transport = connect_with_retry(&pipe_name, 10)
        .await
        .expect("Connect named pipe client failed");

    let ping_msg = IpcMessage::new(
        MessageId(1),
        MessagePayload::BrowserToRenderer(BrowserToRendererMsg::Navigate {
            tab_id: 42,
            url: "https://example.com/live".to_string(),
        }),
    );
    client_transport
        .send(&ping_msg)
        .await
        .expect("Send message failed");

    let received = server_task
        .await
        .expect("Join server task failed")
        .expect("Recv failed");
    assert_eq!(received, Some(ping_msg));
}
