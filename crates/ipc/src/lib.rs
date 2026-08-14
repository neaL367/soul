//! Inter-Process Communication (IPC) message definitions, framing codecs, and transports.

pub mod codec;
pub mod dispatcher;
pub mod error;
pub mod messages;
pub mod transport;

pub use codec::{MAX_FRAME_SIZE, decode_message, encode_message};
pub use dispatcher::IpcDispatcher;
pub use error::IpcError;
pub use messages::{
    BrowserToNetworkMsg, BrowserToRendererMsg, IpcMessage, MessageId, MessagePayload,
    NetworkToBrowserMsg, RendererToBrowserMsg,
};
pub use transport::{AsyncStreamTransport, InMemoryTransport};
