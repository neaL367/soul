//! Inter-process communication protocol, codecs, transports, and message dispatch.

pub mod codec;
pub mod dispatcher;
pub mod error;
pub mod messages;
pub mod named_pipe;
pub mod transport;

pub use codec::{decode_message, encode_message};
pub use dispatcher::IpcDispatcher;
pub use error::IpcError;
pub use messages::{
    BrowserToNetworkMsg, BrowserToRendererMsg, IpcMessage, MessageId, MessagePayload,
    NetworkToBrowserMsg, RendererToBrowserMsg,
};
pub use named_pipe::{accept_named_pipe_server, connect_named_pipe_client, generate_pipe_name};
pub use transport::{
    AsyncStreamReadHalf, AsyncStreamTransport, AsyncStreamWriteHalf, InMemoryTransport,
};
