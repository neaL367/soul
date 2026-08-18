//! Asynchronous in-memory and stream-framed IPC transports.

use crate::codec::{decode_message, encode_message};
use crate::error::IpcError;
use crate::messages::IpcMessage;
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

/// In-process bidirectional asynchronous channel transport.
#[derive(Debug)]
pub struct InMemoryTransport {
    sender: mpsc::Sender<IpcMessage>,
    receiver: mpsc::Receiver<IpcMessage>,
}

impl InMemoryTransport {
    /// Creates a linked pair of bidirectional in-memory transports.
    #[must_use]
    pub fn pair(capacity: usize) -> (Self, Self) {
        let (tx1, rx1) = mpsc::channel(capacity);
        let (tx2, rx2) = mpsc::channel(capacity);

        (
            Self {
                sender: tx1,
                receiver: rx2,
            },
            Self {
                sender: tx2,
                receiver: rx1,
            },
        )
    }

    /// Asynchronously sends an `IpcMessage` to the paired transport.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::ChannelClosed` if the receiving peer was dropped.
    pub async fn send(&self, message: IpcMessage) -> Result<(), IpcError> {
        self.sender
            .send(message)
            .await
            .map_err(|_| IpcError::ChannelClosed)
    }

    /// Asynchronously receives the next `IpcMessage` from the paired transport.
    ///
    /// Returns `Ok(Some(msg))` when a message is received, or `Ok(None)` if the channel was closed.
    ///
    /// # Errors
    ///
    /// Always returns `Ok` on in-memory channel poll.
    pub async fn recv(&mut self) -> Result<Option<IpcMessage>, IpcError> {
        Ok(self.receiver.recv().await)
    }
}

/// Asynchronous stream transport framing messages over any `AsyncRead + AsyncWrite` stream (e.g. Named Pipe or TCP).
#[derive(Debug)]
pub struct AsyncStreamTransport<S> {
    stream: S,
    read_buffer: BytesMut,
}

/// Framing-aware read half produced by [`AsyncStreamTransport::split`].
#[derive(Debug)]
pub struct AsyncStreamReadHalf<S> {
    read: tokio::io::ReadHalf<S>,
    read_buffer: BytesMut,
}

/// Framing-aware write half produced by [`AsyncStreamTransport::split`].
#[derive(Debug)]
pub struct AsyncStreamWriteHalf<S> {
    write: tokio::io::WriteHalf<S>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncStreamTransport<S> {
    /// Splits the transport into independent framed read and write halves so a
    /// service can read requests from one task while spawned workers write
    /// responses concurrently.
    #[must_use]
    pub fn split(self) -> (AsyncStreamReadHalf<S>, AsyncStreamWriteHalf<S>) {
        let (read, write) = tokio::io::split(self.stream);
        (
            AsyncStreamReadHalf {
                read,
                read_buffer: self.read_buffer,
            },
            AsyncStreamWriteHalf { write },
        )
    }
}

impl<S: AsyncRead + Unpin> AsyncStreamReadHalf<S> {
    /// Asynchronously reads and decodes the next `IpcMessage` from the stream.
    ///
    /// Returns `Ok(Some(msg))` when a complete frame is read, or `Ok(None)` on clean EOF.
    ///
    /// # Errors
    ///
    /// Returns `IpcError` if reading, framing, or decoding fails.
    pub async fn recv(&mut self) -> Result<Option<IpcMessage>, IpcError> {
        loop {
            if let Some((msg, consumed)) = decode_message(&self.read_buffer)? {
                let _ = self.read_buffer.split_to(consumed);
                return Ok(Some(msg));
            }

            let mut chunk = [0u8; 4096];
            let n = self.read.read(&mut chunk).await?;
            if n == 0 {
                if self.read_buffer.is_empty() {
                    return Ok(None);
                }
                return Err(IpcError::ConnectionClosed);
            }
            self.read_buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncStreamWriteHalf<S> {
    /// Asynchronously writes a length-prefixed encoded `IpcMessage` to the stream.
    ///
    /// # Errors
    ///
    /// Returns `IpcError` if encoding or network writing fails.
    pub async fn send(&mut self, message: &IpcMessage) -> Result<(), IpcError> {
        let frame_bytes = encode_message(message)?;
        self.write.write_all(&frame_bytes).await?;
        self.write.flush().await?;
        Ok(())
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncStreamTransport<S> {
    /// Creates a new `AsyncStreamTransport` wrapping an I/O stream with an initial 8KB read buffer.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            read_buffer: BytesMut::with_capacity(8192),
        }
    }

    /// Asynchronously writes a length-prefixed encoded `IpcMessage` to the stream.
    ///
    /// # Errors
    ///
    /// Returns `IpcError` if encoding or network writing fails.
    pub async fn send(&mut self, message: &IpcMessage) -> Result<(), IpcError> {
        let frame_bytes = encode_message(message)?;
        self.stream.write_all(&frame_bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Asynchronously reads and decodes the next `IpcMessage` from the stream.
    ///
    /// Returns `Ok(Some(msg))` when a complete frame is read, or `Ok(None)` on clean EOF.
    ///
    /// # Errors
    ///
    /// Returns `IpcError` if reading, framing, or decoding fails.
    pub async fn recv(&mut self) -> Result<Option<IpcMessage>, IpcError> {
        loop {
            if let Some((msg, consumed)) = decode_message(&self.read_buffer)? {
                let _ = self.read_buffer.split_to(consumed);
                return Ok(Some(msg));
            }

            let mut chunk = [0u8; 4096];
            let n = self.stream.read(&mut chunk).await?;
            if n == 0 {
                if self.read_buffer.is_empty() {
                    return Ok(None);
                }
                return Err(IpcError::ConnectionClosed);
            }
            self.read_buffer.extend_from_slice(&chunk[..n]);
        }
    }
}
