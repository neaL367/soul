//! RFC 6455 WebSocket protocol implementation.

pub mod client;
pub mod frame;
pub mod handshake;

pub use client::{WebSocketClient, WebSocketMessage};
pub use frame::{Frame, OpCode};
pub use handshake::{
    build_handshake_request, compute_accept_key, generate_websocket_key,
    validate_handshake_response,
};
