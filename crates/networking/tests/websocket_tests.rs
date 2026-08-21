//! Integration tests for RFC 6455 WebSocket framing, masking, and handshake.

use networking::websocket::{
    Frame, OpCode, build_handshake_request, compute_accept_key, generate_websocket_key,
    validate_handshake_response,
};

#[test]
fn test_frame_encode_and_decode_roundtrip_text() {
    let original = Frame::text("Hello, WebSocket!".to_string());
    let mask = [0x12, 0x34, 0x56, 0x78];
    let wire_bytes = original.encode_client(mask);

    let (decoded, consumed) = Frame::decode(&wire_bytes).expect("frame decodes");
    assert_eq!(consumed, wire_bytes.len());
    assert_eq!(decoded.opcode, OpCode::Text);
    assert!(decoded.fin);
    assert_eq!(
        String::from_utf8(decoded.payload).unwrap(),
        "Hello, WebSocket!"
    );
}

#[test]
fn test_frame_encode_and_decode_roundtrip_binary() {
    let payload: Vec<u8> = (0..=255u8).collect();
    let original = Frame::binary(payload.clone());
    let mask = [0xAA, 0xBB, 0xCC, 0xDD];
    let wire_bytes = original.encode_client(mask);

    let (decoded, consumed) = Frame::decode(&wire_bytes).expect("binary frame decodes");
    assert_eq!(consumed, wire_bytes.len());
    assert_eq!(decoded.opcode, OpCode::Binary);
    assert_eq!(decoded.payload, payload);
}

#[test]
fn test_control_frames_ping_pong_close() {
    let ping = Frame::ping(b"heartbeat".to_vec());
    let mask = [1, 2, 3, 4];
    let wire = ping.encode_client(mask);
    let (decoded_ping, _) = Frame::decode(&wire).expect("ping decodes");
    assert_eq!(decoded_ping.opcode, OpCode::Ping);
    assert_eq!(decoded_ping.payload, b"heartbeat");

    let close = Frame::close(Some(1000), "Normal Closure");
    let wire_close = close.encode_client(mask);
    let (decoded_close, _) = Frame::decode(&wire_close).expect("close decodes");
    assert_eq!(decoded_close.opcode, OpCode::Close);
    assert_eq!(
        u16::from_be_bytes([decoded_close.payload[0], decoded_close.payload[1]]),
        1000
    );
}

#[test]
fn test_handshake_accept_key_rfc6455_vector() {
    // Official test vector from RFC 6455 §1.3:
    // Key: dGhlIHNhbXBsZSBub25jZQ==
    // Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
    let test_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let accept = compute_accept_key(test_key);
    assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[test]
fn test_handshake_request_and_response_validation() {
    let key = generate_websocket_key();
    let req = build_handshake_request("example.com", "/chat", &key);
    let req_str = String::from_utf8(req).unwrap();
    assert!(req_str.starts_with("GET /chat HTTP/1.1\r\n"));
    assert!(req_str.contains("Upgrade: websocket\r\n"));
    assert!(req_str.contains(&format!("Sec-WebSocket-Key: {key}\r\n")));

    let accept = compute_accept_key(&key);
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );

    let consumed = validate_handshake_response(resp.as_bytes(), &key).expect("handshake valid");
    assert_eq!(consumed, resp.len());
}
