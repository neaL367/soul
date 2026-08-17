//! Event types delivered between the browser UI and core state machines.

use crate::input::InputEvent;

/// Events originating from user interaction with the browser chrome or native window.
#[derive(Debug, Clone, PartialEq)]
pub enum SoulEvent {
    /// Input event routed from GPUI into Soul's backend-agnostic input model.
    InputRouted {
        /// ID of the window receiving input.
        window_id: u64,
        /// Normalized input event with logical and physical coordinates.
        event: InputEvent,
    },
    /// Page hyperlink activated by a pointer click.
    LinkActivated {
        /// ID of the window receiving the click.
        window_id: u64,
        /// Destination URL from the anchor's `href` attribute.
        url: String,
    },
    /// Window close requested by the user or OS.
    WindowCloseRequested {
        /// ID of the window requesting closure.
        window_id: u64,
    },
    /// Window resized to new logical dimensions.
    WindowResized {
        /// ID of the resized window.
        window_id: u64,
        /// New width in logical pixels.
        width: u32,
        /// New height in logical pixels.
        height: u32,
        /// Device pixel ratio (scale factor).
        scale_factor: f32,
    },
    /// Window gained or lost input focus.
    WindowFocusChanged {
        /// ID of the window whose focus changed.
        window_id: u64,
        /// Whether the window is currently focused.
        is_focused: bool,
    },
    /// Omnibox URL or search query submitted.
    OmniboxSubmitted {
        /// ID of the window containing the omnibox.
        window_id: u64,
        /// Raw text input entered by the user.
        input: String,
    },
    /// Back navigation button clicked.
    NavigateBack {
        /// ID of the window where back was clicked.
        window_id: u64,
    },
    /// Forward navigation button clicked.
    NavigateForward {
        /// ID of the window where forward was clicked.
        window_id: u64,
    },
    /// Reload page button clicked.
    Reload {
        /// ID of the window where reload was clicked.
        window_id: u64,
        /// Whether to bypass local cache.
        bypass_cache: bool,
    },
    /// New tab button clicked.
    NewTabRequested {
        /// ID of the window where new tab was requested.
        window_id: u64,
    },
    /// Tab close button clicked.
    TabCloseRequested {
        /// ID of the window containing the tab.
        window_id: u64,
        /// Index or ID of the tab to close.
        tab_index: usize,
    },
    /// Tab selection changed.
    TabSelected {
        /// ID of the window containing the tab.
        window_id: u64,
        /// Newly active tab index.
        tab_index: usize,
    },
}
