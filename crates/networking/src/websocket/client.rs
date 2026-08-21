//! Async WebSocket client connection over TCP/TLS with RFC 6455 support.

use super::frame::{Frame, OpCode};
use super::handshake::{
    build_handshake_request, generate_websocket_key, validate_handshake_response,
};
use crate::client::create_default_tls_config;
use crate::error::NetworkError;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use url::Url;

/// High-level WebSocket incoming message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketMessage {
    /// UTF-8 Text message.
    Text(String),
    /// Binary data payload.
    Binary(Vec<u8>),
    /// Connection close signal.
    Close(u16, String),
    /// Ping control frame.
    Ping(Vec<u8>),
    /// Pong control frame.
    Pong(Vec<u8>),
}

enum StreamKind {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl StreamKind {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf).await,
            Self::Tls(s) => s.read(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.write_all(buf).await,
            Self::Tls(s) => s.write_all(buf).await,
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush().await,
            Self::Tls(s) => s.flush().await,
        }
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.shutdown().await,
            Self::Tls(s) => s.shutdown().await,
        }
    }
}

/// Established active WebSocket client session.
pub struct WebSocketClient {
    stream: StreamKind,
    read_buffer: Vec<u8>,
    is_closed: bool,
}

impl WebSocketClient {
    /// Connects to a `ws://` or `wss://` endpoint and performs the RFC 6455 handshake.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if connection, TLS negotiation, or handshake fails.
    pub async fn connect(url_str: &str) -> Result<Self, NetworkError> {
        let parsed = Url::parse(url_str).map_err(|e| NetworkError::Other(e.to_string()))?;
        let is_secure = match parsed.scheme() {
            "ws" => false,
            "wss" => true,
            s => return Err(NetworkError::UnsupportedScheme(s.to_string())),
        };

        let host = parsed
            .host_str()
            .ok_or_else(|| NetworkError::MissingHost(url_str.to_string()))?;
        let port = parsed
            .port_or_known_default()
            .unwrap_or(if is_secure { 443 } else { 80 });

        let addr = format!("{host}:{port}");
        let tcp = TcpStream::connect(&addr)
            .await
            .map_err(|e| NetworkError::ConnectionFailed(addr.clone(), e))?;

        let mut stream = if is_secure {
            let config = create_default_tls_config();
            let server_name = rustls_pki_types::ServerName::try_from(host.to_string())
                .map_err(|e| NetworkError::TlsError(e.to_string()))?;
            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let tls = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| NetworkError::TlsError(e.to_string()))?;
            StreamKind::Tls(Box::new(tls))
        } else {
            StreamKind::Plain(tcp)
        };

        // Handshake
        let key = generate_websocket_key();
        let path = if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        };
        let req_bytes = build_handshake_request(host, path, &key);

        stream
            .write_all(&req_bytes)
            .await
            .map_err(|e| NetworkError::ConnectionFailed(addr.clone(), e))?;
        stream
            .flush()
            .await
            .map_err(|e| NetworkError::ConnectionFailed(addr.clone(), e))?;

        let mut buf = vec![0u8; 4096];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| NetworkError::ConnectionFailed(addr.clone(), e))?;
        if n == 0 {
            return Err(NetworkError::WebSocketProtocol(
                "server closed connection during handshake".into(),
            ));
        }

        let consumed = validate_handshake_response(&buf[..n], &key)?;
        let read_buffer = buf[consumed..n].to_vec();

        Ok(Self {
            stream,
            read_buffer,
            is_closed: false,
        })
    }

    /// Sends a text message frame to the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if writing to the stream fails.
    pub async fn send_text(&mut self, text: &str) -> Result<(), NetworkError> {
        let frame = Frame::text(text.to_string());
        self.send_frame(&frame).await
    }

    /// Sends a binary message frame to the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if writing to the stream fails.
    pub async fn send_binary(&mut self, bytes: &[u8]) -> Result<(), NetworkError> {
        let frame = Frame::binary(bytes.to_vec());
        self.send_frame(&frame).await
    }

    /// Sends a close control frame and marks the connection as closed.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if writing to the stream fails.
    pub async fn close(&mut self, code: u16, reason: &str) -> Result<(), NetworkError> {
        if self.is_closed {
            return Ok(());
        }
        let frame = Frame::close(Some(code), reason);
        let _ = self.send_frame(&frame).await;
        self.is_closed = true;
        let _ = self.stream.shutdown().await;
        Ok(())
    }

    async fn send_frame(&mut self, frame: &Frame) -> Result<(), NetworkError> {
        if self.is_closed {
            return Err(NetworkError::WebSocketProtocol(
                "connection already closed".into(),
            ));
        }
        // Random 4-byte client mask
        let mask: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
        let encoded = frame.encode_client(mask);
        self.stream
            .write_all(&encoded)
            .await
            .map_err(NetworkError::IoError)?;
        self.stream.flush().await.map_err(NetworkError::IoError)?;
        Ok(())
    }

    /// Reads the next incoming WebSocket message from the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if parsing fails or stream disconnects.
    pub async fn receive_message(&mut self) -> Result<Option<WebSocketMessage>, NetworkError> {
        if self.is_closed {
            return Ok(None);
        }

        loop {
            if let Ok((frame, consumed)) = Frame::decode(&self.read_buffer) {
                self.read_buffer.drain(..consumed);
                match frame.opcode {
                    OpCode::Text => {
                        let s = String::from_utf8(frame.payload).map_err(|_| {
                            NetworkError::WebSocketProtocol("invalid utf-8 text frame".into())
                        })?;
                        return Ok(Some(WebSocketMessage::Text(s)));
                    }
                    OpCode::Binary => {
                        return Ok(Some(WebSocketMessage::Binary(frame.payload)));
                    }
                    OpCode::Ping => {
                        let pong = Frame::pong(frame.payload.clone());
                        let _ = self.send_frame(&pong).await;
                        return Ok(Some(WebSocketMessage::Ping(frame.payload)));
                    }
                    OpCode::Pong => {
                        return Ok(Some(WebSocketMessage::Pong(frame.payload)));
                    }
                    OpCode::Close => {
                        self.is_closed = true;
                        let code = if frame.payload.len() >= 2 {
                            u16::from_be_bytes([frame.payload[0], frame.payload[1]])
                        } else {
                            1000
                        };
                        let reason = if frame.payload.len() > 2 {
                            String::from_utf8_lossy(&frame.payload[2..]).into_owned()
                        } else {
                            String::new()
                        };
                        return Ok(Some(WebSocketMessage::Close(code, reason)));
                    }
                    OpCode::Continuation => {}
                }
            }

            let mut tmp = [0u8; 4096];
            let n = self
                .stream
                .read(&mut tmp)
                .await
                .map_err(NetworkError::IoError)?;
            if n == 0 {
                self.is_closed = true;
                return Ok(None);
            }
            self.read_buffer.extend_from_slice(&tmp[..n]);
        }
    }
}
