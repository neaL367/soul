//! Backend-agnostic browser chrome trait and window specification types.

use crate::event::ChromeEvent;
use thiserror::Error;

/// Unique identifier for a native browser chrome window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// Specification used to configure a newly requested native window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSpec {
    /// Window title displayed in the OS title bar.
    pub title: String,
    /// Initial width in logical pixels.
    pub width: u32,
    /// Initial height in logical pixels.
    pub height: u32,
    /// Minimum width in logical pixels.
    pub min_width: Option<u32>,
    /// Minimum height in logical pixels.
    pub min_height: Option<u32>,
    /// Whether the window allows resizing.
    pub resizable: bool,
    /// Whether the window has native OS title bar and decorations.
    pub decorated: bool,
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self {
            title: "Soul Browser".to_string(),
            width: 1280,
            height: 800,
            min_width: Some(400),
            min_height: Some(300),
            resizable: true,
            decorated: true,
        }
    }
}

/// Viewport frame data to be presented inside the web page area of the chrome.
#[derive(Debug, Clone)]
pub enum ViewportFrame {
    /// CPU software pixel buffer (RGBA format) for M10a.
    SoftwareRgba {
        /// Buffer width in pixels.
        width: u32,
        /// Buffer height in pixels.
        height: u32,
        /// Raw pixel bytes in 8-bit RGBA order.
        pixels: Vec<u8>,
    },
    /// `Direct3D11` / `DXGI` shared texture handle for M10b.
    DxgiSharedHandle {
        /// Width of the GPU texture in pixels.
        width: u32,
        /// Height of the GPU texture in pixels.
        height: u32,
        /// Raw Windows `HANDLE` cast to `usize` for transport.
        handle: usize,
    },
}

/// Global configuration options for initializing the chrome backend.
#[derive(Debug, Clone, Default)]
pub struct ChromeConfig {
    /// Application name.
    pub app_name: String,
    /// Custom asset directory path if applicable.
    pub resource_dir: Option<std::path::PathBuf>,
}

/// Error type for chrome backend operations.
#[derive(Debug, Error)]
pub enum ChromeError {
    /// Backend initialization failed.
    #[error("Failed to initialize chrome backend: {0}")]
    InitializationFailed(String),

    /// Window creation failed.
    #[error("Failed to create native window: {0}")]
    WindowCreationFailed(String),

    /// Window not found for the given ID.
    #[error("Window ID not found: {0:?}")]
    WindowNotFound(WindowId),

    /// Framebuffer presentation error.
    #[error("Failed to present viewport frame: {0}")]
    PresentationFailed(String),

    /// General backend error.
    #[error("Chrome backend error: {0}")]
    Other(String),
}

/// Trait defining the interface between the browser application and the desktop UI framework.
///
/// This trait isolates the rest of the codebase from the chosen UI framework (`GPUI`),
/// allowing mock implementations for testing or backend replacement without engine rewrites.
pub trait ChromeBackend: Send + Sync + 'static {
    /// Initializes the desktop UI framework runtime.
    fn init(&mut self, config: ChromeConfig) -> Result<(), ChromeError>;

    /// Creates and opens a new native window.
    fn open_window(&mut self, spec: WindowSpec) -> Result<WindowId, ChromeError>;

    /// Closes a previously opened window.
    fn close_window(&mut self, window_id: WindowId) -> Result<(), ChromeError>;

    /// Updates the rendered content frame inside a window's web viewport.
    fn update_viewport(
        &mut self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), ChromeError>;

    /// Registers a callback handler to receive events emitted by user interactions with chrome.
    fn set_event_handler(&mut self, handler: Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>);

    /// Runs the application message loop until all windows are closed or shutdown is triggered.
    fn run(self: Box<Self>) -> Result<(), ChromeError>;
}
