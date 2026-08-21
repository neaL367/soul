//! Integration tests for WHATWG WebSocket JavaScript API bindings.

use javascript::JsRuntime;
use std::sync::Arc;
use web_api::{
    CLOSED, MockWebSocketSession, WebSocketFactory, WebSocketSession, register_websocket,
};

#[test]
fn test_js_websocket_constants_and_constructor() {
    let mut runtime = JsRuntime::new();
    register_websocket(&mut runtime.context, None).expect("register succeeds");

    let val = runtime
        .eval("WebSocket.CONNECTING === 0 && WebSocket.OPEN === 1 && WebSocket.CLOSING === 2 && WebSocket.CLOSED === 3")
        .expect("eval succeeds");
    assert_eq!(val, "true");

    let val_inst = runtime
        .eval("const ws = new WebSocket('ws://example.com/socket'); ws.url;")
        .expect("eval succeeds");
    assert_eq!(val_inst.trim_matches('"'), "ws://example.com/socket");

    let val_state = runtime.eval("ws.readyState;").expect("eval succeeds");
    assert_eq!(val_state, "1"); // OPEN in mock
}

#[test]
fn test_js_websocket_send_and_close() {
    let mut runtime = JsRuntime::new();

    let session = Arc::new(MockWebSocketSession::new());
    let session_clone = session.clone();

    let factory: WebSocketFactory =
        Arc::new(move |_url, _protocols| Ok(session_clone.clone() as Arc<dyn WebSocketSession>));

    register_websocket(&mut runtime.context, Some(factory)).expect("register succeeds");

    runtime
        .eval("const ws = new WebSocket('ws://echo.websocket.events'); ws.send('hello server'); ws.send('second message');")
        .expect("send succeeds");

    let sent = session.sent_messages();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], "hello server");
    assert_eq!(sent[1], "second message");

    runtime
        .eval("ws.close(1000, 'done');")
        .expect("close succeeds");

    assert_eq!(session.ready_state(), CLOSED);
}
