//! Integration tests for IPC transports, message framing codec, and dispatcher.

use ipc::{
    AsyncStreamTransport, BrowserToNetworkMsg, BrowserToRendererMsg, InMemoryTransport,
    IpcDispatcher, IpcMessage, MessageId, MessagePayload, NetworkToBrowserMsg,
    RendererToBrowserMsg, decode_message, encode_message,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn test_in_memory_bidirectional_exchange() {
    let (transport_a, mut transport_b) = InMemoryTransport::pair(16);

    let msg1 = IpcMessage::new(
        MessageId(1),
        MessagePayload::BrowserToRenderer(BrowserToRendererMsg::Navigate {
            tab_id: 10,
            url: "https://rust-lang.org".to_string(),
        }),
    );

    transport_a.send(msg1.clone()).await.unwrap();
    let received = transport_b.recv().await.unwrap().unwrap();
    assert_eq!(received.id, MessageId(1));

    if let MessagePayload::BrowserToRenderer(BrowserToRendererMsg::Navigate { tab_id, url }) =
        received.payload
    {
        assert_eq!(tab_id, 10);
        assert_eq!(url, "https://rust-lang.org");
    } else {
        panic!("unexpected message payload");
    }
}

#[test]
fn test_length_prefixed_codec_roundtrip() {
    let msg = IpcMessage::new(
        MessageId(42),
        MessagePayload::BrowserToNetwork(BrowserToNetworkMsg::FetchRequest {
            request_id: 101,
            url: "https://crates.io/api/v1/crates".to_string(),
            method: "GET".to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
        }),
    );

    let encoded = encode_message(&msg).expect("encoding failed");
    assert!(encoded.len() > 4);

    // Decode full frame
    let (decoded, consumed) = decode_message(&encoded)
        .expect("decoding failed")
        .expect("incomplete frame");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, msg);

    // Decode incomplete frame
    let partial = &encoded[..encoded.len() - 5];
    let incomplete = decode_message(partial).unwrap();
    assert!(incomplete.is_none());
}

#[tokio::test]
async fn test_async_stream_transport_duplex() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    let mut client_transport = AsyncStreamTransport::new(client_io);
    let mut server_transport = AsyncStreamTransport::new(server_io);

    let msg = IpcMessage::new(
        MessageId(99),
        MessagePayload::RendererToBrowser(RendererToBrowserMsg::TitleChanged {
            tab_id: 5,
            title: "Soul Home".to_string(),
        }),
    );

    client_transport.send(&msg).await.unwrap();
    let received = server_transport.recv().await.unwrap().unwrap();
    assert_eq!(received.id, MessageId(99));

    if let MessagePayload::RendererToBrowser(RendererToBrowserMsg::TitleChanged { tab_id, title }) =
        received.payload
    {
        assert_eq!(tab_id, 5);
        assert_eq!(title, "Soul Home");
    } else {
        panic!("unexpected payload");
    }
}

#[tokio::test]
async fn test_ipc_dispatcher_correlation_and_handlers() {
    let mut dispatcher = IpcDispatcher::new();
    let handler_call_count = Arc::new(AtomicUsize::new(0));

    let count_clone = handler_call_count.clone();
    dispatcher.register_handler(move |_msg| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });

    let req_id = MessageId(100);
    let response_rx = dispatcher.expect_response(req_id);

    let response_msg = IpcMessage::response_to(
        MessageId(101),
        req_id,
        MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseComplete { request_id: 55 }),
    );

    dispatcher.dispatch(response_msg.clone()).unwrap();

    let received_response = response_rx.await.unwrap();
    assert_eq!(received_response.id, MessageId(101));
    assert_eq!(received_response.correlation_id, Some(req_id));

    // Correlated message went directly to response waiter, handler not called
    assert_eq!(handler_call_count.load(Ordering::SeqCst), 0);

    // Uncorrelated message triggers general handler
    let general_msg = IpcMessage::new(
        MessageId(200),
        MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseComplete { request_id: 56 }),
    );
    dispatcher.dispatch(general_msg).unwrap();
    assert_eq!(handler_call_count.load(Ordering::SeqCst), 1);
}
