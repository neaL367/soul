//! Commands and handle types for the single-owner navigation driver.

use std::sync::mpsc::{self, SyncSender};

/// Commands accepted from the Soul toolbar and omnibox.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationCommand {
    /// Navigate to user-entered URL or search text.
    Navigate(String),
    /// Traverse one entry backward.
    Back,
    /// Traverse one entry forward.
    Forward,
    /// Re-fetch current URL.
    Reload,
    /// Scroll the active page without refetching it.
    Scroll {
        /// Vertical document-space delta in logical pixels.
        delta_y: f32,
    },
    /// Resize the active viewport dimensions.
    Resize {
        /// New window/viewport width.
        width: u32,
        /// New window/viewport height.
        height: u32,
    },
    /// Open a new blank tab and make it active.
    NewTab,
    /// Select a tab by its current tab-strip index.
    SelectTab {
        /// Zero-based tab-strip index.
        tab_index: usize,
    },
    /// Close a tab by its current tab-strip index.
    CloseTab {
        /// Zero-based tab-strip index.
        tab_index: usize,
    },
}

/// Capacity of the bounded navigation command channel. Bounding it prevents an
/// unbounded queue from accumulating behind a slow worker; the UI side uses
/// `try_send`, so a full channel drops the newest command (acceptable for
/// high-frequency scroll events) instead of blocking the UI thread.
pub const COMMAND_CHANNEL_CAPACITY: usize = 64;

/// Handle for sending navigation commands to one controller-owning worker.
#[derive(Clone)]
pub struct NavigationDriver {
    pub(super) sender: SyncSender<NavigationCommand>,
}

impl NavigationDriver {
    /// Attempts to enqueue a command without blocking.
    ///
    /// # Errors
    ///
    /// Returns `TrySendError::Full` when the bounded channel is full or
    /// `TrySendError::Closed` when the navigation worker has exited.
    pub fn send(
        &self,
        command: NavigationCommand,
    ) -> Result<(), mpsc::TrySendError<NavigationCommand>> {
        self.sender.try_send(command)
    }
}
