//! Integration tests for Windows Named Pipe IPC framing and transmission.

use ipc::{
    BrowserToRendererMsg, IpcMessage, MessageId, MessagePayload, accept_named_pipe_server,
    connect_named_pipe_client,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_named_pipe_ipc_message_roundtrip() {
    let pipe_name = format!(r"\\.\pipe\soul-test-pipe-{}", std::process::id());

    let pipe_name_clone = pipe_name.clone();
    let server_task = tokio::spawn(async move {
        let mut server_transport = accept_named_pipe_server(&pipe_name_clone)
            .await
            .expect("Accept named pipe server failed");
        server_transport.recv().await
    });

    sleep(Duration::from_millis(50)).await;

    let mut client_transport = connect_named_pipe_client(&pipe_name)
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
