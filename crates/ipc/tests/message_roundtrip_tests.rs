//! Round-trip tests covering every IPC message variant plus framing edge cases.

use ipc::{
    AsyncStreamTransport, BrowserToNetworkMsg, BrowserToRendererMsg, IpcMessage, MessageId,
    MessagePayload, NetworkToBrowserMsg, PROTOCOL_VERSION, RendererToBrowserMsg, decode_message,
    encode_message,
};
use tokio::io::AsyncWriteExt;

/// Encodes `msg` and decodes it back, asserting it survives unchanged.
fn assert_roundtrip(msg: &IpcMessage) {
    assert_eq!(msg.version, PROTOCOL_VERSION);
    let encoded = encode_message(msg).expect("encoding failed");
    let (decoded, consumed) = decode_message(&encoded)
        .expect("decoding failed")
        .expect("incomplete frame");
    assert_eq!(consumed, encoded.len());
    assert_eq!(&decoded, msg);
}

const fn payload_b2r(variant: BrowserToRendererMsg) -> MessagePayload {
    MessagePayload::BrowserToRenderer(variant)
}

const fn payload_r2b(variant: RendererToBrowserMsg) -> MessagePayload {
    MessagePayload::RendererToBrowser(variant)
}

const fn payload_b2n(variant: BrowserToNetworkMsg) -> MessagePayload {
    MessagePayload::BrowserToNetwork(variant)
}

const fn payload_n2b(variant: NetworkToBrowserMsg) -> MessagePayload {
    MessagePayload::NetworkToBrowser(variant)
}

#[test]
fn test_roundtrip_all_browser_to_renderer_variants() {
    for payload in [
        payload_b2r(BrowserToRendererMsg::Navigate {
            tab_id: 1,
            url: "https://example.com".to_string(),
        }),
        payload_b2r(BrowserToRendererMsg::InputMouse {
            tab_id: 1,
            x: 12.5,
            y: 30.0,
            button: Some(0),
            is_down: true,
        }),
        payload_b2r(BrowserToRendererMsg::InputKey {
            tab_id: 1,
            key: "Enter".to_string(),
            is_down: true,
        }),
        payload_b2r(BrowserToRendererMsg::ResizeViewport {
            tab_id: 1,
            width: 800,
            height: 600,
            scale_factor: 1.5,
        }),
        payload_b2r(BrowserToRendererMsg::SetTier { tab_id: 1, tier: 2 }),
        payload_b2r(BrowserToRendererMsg::EvalScript {
            tab_id: 1,
            script: "1 + 1".to_string(),
        }),
    ] {
        assert_roundtrip(&IpcMessage::new(MessageId(1), payload));
    }
}

#[test]
fn test_roundtrip_all_renderer_to_browser_variants() {
    for payload in [
        payload_r2b(RendererToBrowserMsg::NavigationStateChanged {
            tab_id: 1,
            url: "https://example.com".to_string(),
            status_code: Some(200),
            is_loading: true,
        }),
        payload_r2b(RendererToBrowserMsg::TitleChanged {
            tab_id: 1,
            title: "Hello".to_string(),
        }),
        payload_r2b(RendererToBrowserMsg::FrameReady {
            tab_id: 1,
            width: 4,
            height: 4,
            pixel_data: vec![0; 64],
        }),
        payload_r2b(RendererToBrowserMsg::ConsoleLog {
            tab_id: 1,
            level: "info".to_string(),
            message: "log".to_string(),
        }),
    ] {
        assert_roundtrip(&IpcMessage::new(MessageId(1), payload));
    }
}

#[test]
fn test_roundtrip_all_browser_to_network_variants() {
    for payload in [
        payload_b2n(BrowserToNetworkMsg::FetchRequest {
            request_id: 7,
            url: "https://example.com/data".to_string(),
            method: "POST".to_string(),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: Some(vec![1, 2, 3]),
            document_origin: Some("https://example.com".to_string()),
        }),
        payload_b2n(BrowserToNetworkMsg::CancelRequest { request_id: 7 }),
    ] {
        assert_roundtrip(&IpcMessage::new(MessageId(1), payload));
    }
}

#[test]
fn test_roundtrip_all_network_to_browser_variants() {
    for payload in [
        payload_n2b(NetworkToBrowserMsg::ResponseHeaders {
            request_id: 7,
            status_code: 200,
            headers: vec![("content-type".to_string(), "text/html".to_string())],
            final_url: "https://example.com/".to_string(),
            set_cookies: vec!["session=abc; Path=/".to_string()],
        }),
        payload_n2b(NetworkToBrowserMsg::ResponseBodyChunk {
            request_id: 7,
            data: vec![1, 2, 3],
        }),
        payload_n2b(NetworkToBrowserMsg::ResponseComplete { request_id: 7 }),
        payload_n2b(NetworkToBrowserMsg::ResponseFailed {
            request_id: 7,
            error: "timeout".to_string(),
        }),
    ] {
        assert_roundtrip(&IpcMessage::new(MessageId(1), payload));
    }
}

#[test]
fn test_decode_rejects_oversized_frame_header() {
    // A 4-byte header advertising a payload far above the limit.
    let mut frame = Vec::new();
    frame.extend_from_slice(&(u32::MAX).to_be_bytes());
    frame.extend_from_slice(&[0u8; 8]);

    let err = decode_message(&frame).expect_err("oversized frame must be rejected");
    assert!(matches!(err, ipc::IpcError::FrameTooLarge { .. }), "{err}");
}

#[test]
fn test_decode_returns_none_for_truncated_header() {
    assert_eq!(decode_message(&[0, 0]).unwrap(), None);
}

#[tokio::test]
async fn test_clean_eof_returns_none() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    drop(client_io); // peer closes without sending anything.
    let mut transport = AsyncStreamTransport::new(server_io);
    assert_eq!(transport.recv().await.unwrap(), None);
}

#[tokio::test]
async fn test_partial_frame_eof_returns_connection_closed() {
    let (mut client_io, server_io) = tokio::io::duplex(4096);

    // Write a valid frame header claiming a 10-byte payload, then only 3 bytes
    // and close the peer mid-frame.
    client_io.write_all(&[0, 0, 0, 10, 1, 2, 3]).await.unwrap();
    drop(client_io);

    let mut transport = AsyncStreamTransport::new(server_io);
    let err = transport.recv().await.expect_err("partial frame must fail");
    assert!(matches!(err, ipc::IpcError::ConnectionClosed), "{err}");
}

#[tokio::test]
async fn test_split_half_clean_eof_returns_none() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    drop(client_io);
    let (mut reader, _writer) = AsyncStreamTransport::new(server_io).split();
    assert_eq!(reader.recv().await.unwrap(), None);
}
