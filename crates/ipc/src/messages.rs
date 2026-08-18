//! Typed IPC command and event message contracts between browser subsystems.

use serde::{Deserialize, Serialize};

/// Monotonically increasing message identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MessageId(pub u64);

/// Commands sent from the Browser UI / Core process to a Renderer process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BrowserToRendererMsg {
    /// Command to navigate a tab viewport to a target URL.
    Navigate {
        /// Target tab identifier.
        tab_id: u64,
        /// Destination URL.
        url: String,
    },
    /// Dispatched mouse interaction event.
    InputMouse {
        /// Target tab identifier.
        tab_id: u64,
        /// Horizontal position in logical pixels.
        x: f64,
        /// Vertical position in logical pixels.
        y: f64,
        /// Mouse button code (0 = Left, 1 = Right, 2 = Middle).
        button: Option<u8>,
        /// Whether the button is pressed down.
        is_down: bool,
    },
    /// Dispatched keyboard interaction event.
    InputKey {
        /// Target tab identifier.
        tab_id: u64,
        /// Key identifier string.
        key: String,
        /// Whether the key is pressed down.
        is_down: bool,
    },
    /// Viewport window resize command.
    ResizeViewport {
        /// Target tab identifier.
        tab_id: u64,
        /// Width in physical pixels.
        width: u32,
        /// Height in physical pixels.
        height: u32,
        /// Device display scaling factor.
        scale_factor: f64,
    },
    /// Sets resource and execution tier for tab throttling.
    SetTier {
        /// Target tab identifier.
        tab_id: u64,
        /// Tier level (0 = Active, 1 = Background, 2 = Frozen).
        tier: u8,
    },
    /// Evaluates a JavaScript code snippet in page context.
    EvalScript {
        /// Target tab identifier.
        tab_id: u64,
        /// Script text to execute.
        script: String,
    },
}

/// Events emitted from a Renderer process back to the Browser UI / Core process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RendererToBrowserMsg {
    /// Lifecycle update for active page navigation.
    NavigationStateChanged {
        /// Source tab identifier.
        tab_id: u64,
        /// Active URL string.
        url: String,
        /// HTTP status code if response received.
        status_code: Option<u16>,
        /// Whether the document is still loading.
        is_loading: bool,
    },
    /// Page document title updated.
    TitleChanged {
        /// Source tab identifier.
        tab_id: u64,
        /// Extracted title string.
        title: String,
    },
    /// Rasterized pixel buffer ready for presentation.
    FrameReady {
        /// Source tab identifier.
        tab_id: u64,
        /// Buffer width in pixels.
        width: u32,
        /// Buffer height in pixels.
        height: u32,
        /// Premultiplied 8-bit RGBA pixel byte array.
        pixel_data: Vec<u8>,
    },
    /// Console output emitted from JavaScript runtime.
    ConsoleLog {
        /// Source tab identifier.
        tab_id: u64,
        /// Log severity level (`info`, `warn`, `error`).
        level: String,
        /// Log message payload.
        message: String,
    },
}

/// Requests sent from the Browser process to the Network process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserToNetworkMsg {
    /// In-flight HTTP request initiation.
    FetchRequest {
        /// Unique request sequence ID.
        request_id: u64,
        /// Request target URL.
        url: String,
        /// HTTP method (`GET`, `POST`, etc.).
        method: String,
        /// HTTP request headers.
        headers: Vec<(String, String)>,
        /// Optional request body payload (carried for POST/PUT/DELETE).
        body: Option<Vec<u8>>,
        /// Serialized origin URL of the requesting document; enables
        /// mixed-content and CORS enforcement in the Network process.
        document_origin: Option<String>,
    },
    /// Cancels an in-flight network request.
    CancelRequest {
        /// Request sequence ID to cancel.
        request_id: u64,
    },
}

/// Network responses streamed back from the Network process to the Browser process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkToBrowserMsg {
    /// Response status code and initial headers received.
    ResponseHeaders {
        /// Request sequence ID.
        request_id: u64,
        /// HTTP status code.
        status_code: u16,
        /// Response headers.
        headers: Vec<(String, String)>,
        /// Final response URL after following any redirects.
        final_url: String,
        /// Raw `Set-Cookie` header values.
        set_cookies: Vec<String>,
    },
    /// Incremental chunk of response body bytes.
    ResponseBodyChunk {
        /// Request sequence ID.
        request_id: u64,
        /// Raw response payload bytes.
        data: Vec<u8>,
    },
    /// Entire response payload has finished streaming successfully.
    ResponseComplete {
        /// Request sequence ID.
        request_id: u64,
    },
    /// Network request failed with an error.
    ResponseFailed {
        /// Request sequence ID.
        request_id: u64,
        /// Error description.
        error: String,
    },
}

/// Unified payload wrapper for all IPC message protocols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Browser-to-Renderer command.
    BrowserToRenderer(BrowserToRendererMsg),
    /// Renderer-to-Browser event.
    RendererToBrowser(RendererToBrowserMsg),
    /// Browser-to-Network command.
    BrowserToNetwork(BrowserToNetworkMsg),
    /// Network-to-Browser event.
    NetworkToBrowser(NetworkToBrowserMsg),
}

/// Standardized envelope wrapping an IPC message with metadata and correlation tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcMessage {
    /// Monotonic message sequence ID.
    pub id: MessageId,
    /// Optional correlation identifier linking requests and responses.
    pub correlation_id: Option<MessageId>,
    /// Underlying protocol message payload.
    pub payload: MessagePayload,
}

impl IpcMessage {
    /// Creates a new `IpcMessage` with an assigned ID and no correlation.
    #[must_use]
    pub const fn new(id: MessageId, payload: MessagePayload) -> Self {
        Self {
            id,
            correlation_id: None,
            payload,
        }
    }

    /// Creates a response `IpcMessage` correlated to a request message ID.
    #[must_use]
    pub const fn response_to(
        id: MessageId,
        request_id: MessageId,
        payload: MessagePayload,
    ) -> Self {
        Self {
            id,
            correlation_id: Some(request_id),
            payload,
        }
    }
}
