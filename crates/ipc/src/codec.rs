//! Length-prefixed framing and JSON serialization codec for IPC streams.

use crate::error::IpcError;
use crate::messages::IpcMessage;

/// Maximum allowable frame size (64 Megabytes) to prevent memory exhaustion attacks.
pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;

/// Serializes an `IpcMessage` into a length-prefixed binary frame.
///
/// Format: `[4-byte big-endian payload length] [JSON serialized payload]`
///
/// # Errors
///
/// Returns `IpcError::Serialization` if serialization fails, or `IpcError::FrameTooLarge` if the payload exceeds 64MB.
#[allow(clippy::cast_possible_truncation)]
pub fn encode_message(message: &IpcMessage) -> Result<Vec<u8>, IpcError> {
    let payload_bytes = serde_json::to_vec(message)?;
    let payload_len = payload_bytes.len();

    if payload_len > MAX_FRAME_SIZE {
        return Err(IpcError::FrameTooLarge {
            size: payload_len,
            max: MAX_FRAME_SIZE,
        });
    }

    let len_header = (payload_len as u32).to_be_bytes();
    let mut frame = Vec::with_capacity(4 + payload_len);
    frame.extend_from_slice(&len_header);
    frame.extend_from_slice(&payload_bytes);
    Ok(frame)
}

/// Attempts to decode an `IpcMessage` from a byte buffer slice.
///
/// Returns `Ok(Some((message, consumed_bytes)))` if a complete frame was decoded,
/// or `Ok(None)` if more bytes are required to assemble the frame.
///
/// # Errors
///
/// Returns `IpcError::FrameTooLarge` if the header specifies a frame exceeding 64MB,
/// or `IpcError::Serialization` if deserialization fails.
pub fn decode_message(buffer: &[u8]) -> Result<Option<(IpcMessage, usize)>, IpcError> {
    if buffer.len() < 4 {
        return Ok(None);
    }

    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&buffer[0..4]);
    let payload_len = u32::from_be_bytes(len_bytes) as usize;

    if payload_len > MAX_FRAME_SIZE {
        return Err(IpcError::FrameTooLarge {
            size: payload_len,
            max: MAX_FRAME_SIZE,
        });
    }

    let total_frame_len = 4 + payload_len;
    if buffer.len() < total_frame_len {
        return Ok(None);
    }

    let payload_slice = &buffer[4..total_frame_len];
    let message: IpcMessage = serde_json::from_slice(payload_slice)?;
    Ok(Some((message, total_frame_len)))
}
