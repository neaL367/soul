//! RFC 6455 WebSocket frame encoding, decoding, opcodes, and masking.

use crate::error::NetworkError;

/// WebSocket frame opcodes (RFC 6455 §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    /// Continuation frame (0x0).
    Continuation,
    /// UTF-8 text frame (0x1).
    Text,
    /// Binary data frame (0x2).
    Binary,
    /// Connection close control frame (0x8).
    Close,
    /// Ping control frame (0x9).
    Ping,
    /// Pong control frame (0xA).
    Pong,
}

impl OpCode {
    /// Returns the 4-bit opcode integer value.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Continuation => 0x0,
            Self::Text => 0x1,
            Self::Binary => 0x2,
            Self::Close => 0x8,
            Self::Ping => 0x9,
            Self::Pong => 0xA,
        }
    }

    /// Parses an opcode from a 4-bit integer.
    #[must_use]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b & 0x0F {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }

    /// Returns `true` if this is a control frame (Close, Ping, Pong).
    #[must_use]
    pub const fn is_control(self) -> bool {
        matches!(self, Self::Close | Self::Ping | Self::Pong)
    }
}

/// A parsed or constructed WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// `true` if this is the final fragment in a message.
    pub fin: bool,
    /// Frame opcode.
    pub opcode: OpCode,
    /// Frame payload bytes (unmasked).
    pub payload: Vec<u8>,
}

impl Frame {
    /// Creates a complete text frame.
    #[must_use]
    pub const fn text(payload: String) -> Self {
        Self {
            fin: true,
            opcode: OpCode::Text,
            payload: payload.into_bytes(),
        }
    }

    /// Creates a complete binary frame.
    #[must_use]
    pub const fn binary(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: OpCode::Binary,
            payload,
        }
    }

    /// Creates a ping control frame.
    #[must_use]
    pub const fn ping(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: OpCode::Ping,
            payload,
        }
    }

    /// Creates a pong control frame.
    #[must_use]
    pub const fn pong(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            opcode: OpCode::Pong,
            payload,
        }
    }

    /// Creates a close control frame with an optional status code.
    #[must_use]
    pub fn close(code: Option<u16>, reason: &str) -> Self {
        let mut payload = Vec::new();
        if let Some(c) = code {
            payload.extend_from_slice(&c.to_be_bytes());
            payload.extend_from_slice(reason.as_bytes());
        }
        Self {
            fin: true,
            opcode: OpCode::Close,
            payload,
        }
    }

    /// Encodes this frame for client-to-server transmission (always masked with `mask_key`).
    #[must_use]
    pub fn encode_client(&self, mask_key: [u8; 4]) -> Vec<u8> {
        let mut encoded = Vec::new();
        let b0 = if self.fin { 0x80 } else { 0x00 } | self.opcode.as_u8();
        encoded.push(b0);

        let len = self.payload.len();
        if len < 126 {
            #[allow(clippy::cast_possible_truncation)]
            encoded.push(0x80 | (len as u8));
        } else if len <= 0xFFFF {
            encoded.push(0x80 | 0x7E);
            #[allow(clippy::cast_possible_truncation)]
            encoded.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            encoded.push(0x80 | 0x7F);
            encoded.extend_from_slice(&(len as u64).to_be_bytes());
        }

        encoded.extend_from_slice(&mask_key);

        let mut masked = self.payload.clone();
        for (i, byte) in masked.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }
        encoded.extend_from_slice(&masked);

        encoded
    }

    /// Decodes a frame from incoming raw wire bytes.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if frame headers are invalid or incomplete.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), NetworkError> {
        if bytes.len() < 2 {
            return Err(NetworkError::WebSocketProtocol("frame too short".into()));
        }

        let b0 = bytes[0];
        let fin = (b0 & 0x80) != 0;
        let opcode = OpCode::from_u8(b0 & 0x0F)
            .ok_or_else(|| NetworkError::WebSocketProtocol("unknown websocket opcode".into()))?;

        let b1 = bytes[1];
        let is_masked = (b1 & 0x80) != 0;
        let mut payload_len = (b1 & 0x7F) as usize;
        let mut offset = 2;

        if payload_len == 126 {
            if bytes.len() < offset + 2 {
                return Err(NetworkError::WebSocketProtocol(
                    "incomplete 16-bit frame length".into(),
                ));
            }
            let len_bytes: [u8; 2] = [bytes[offset], bytes[offset + 1]];
            payload_len = u16::from_be_bytes(len_bytes) as usize;
            offset += 2;
        } else if payload_len == 127 {
            if bytes.len() < offset + 8 {
                return Err(NetworkError::WebSocketProtocol(
                    "incomplete 64-bit frame length".into(),
                ));
            }
            let mut len_bytes = [0u8; 8];
            len_bytes.copy_from_slice(&bytes[offset..offset + 8]);
            #[allow(clippy::cast_possible_truncation)]
            {
                payload_len = u64::from_be_bytes(len_bytes) as usize;
            }
            offset += 8;
        }

        let mask = if is_masked {
            if bytes.len() < offset + 4 {
                return Err(NetworkError::WebSocketProtocol(
                    "incomplete mask key".into(),
                ));
            }
            let mut m = [0u8; 4];
            m.copy_from_slice(&bytes[offset..offset + 4]);
            offset += 4;
            Some(m)
        } else {
            None
        };

        if bytes.len() < offset + payload_len {
            return Err(NetworkError::WebSocketProtocol(
                "incomplete payload data".into(),
            ));
        }

        let mut payload = bytes[offset..offset + payload_len].to_vec();
        if let Some(m) = mask {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= m[i % 4];
            }
        }

        let total_consumed = offset + payload_len;
        Ok((
            Self {
                fin,
                opcode,
                payload,
            },
            total_consumed,
        ))
    }
}
