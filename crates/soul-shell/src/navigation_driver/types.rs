//! Commands and handle types for the single-owner navigation driver.

use std::sync::mpsc::{self, Sender};

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

/// Handle for sending navigation commands to one controller-owning worker.
#[derive(Clone)]
pub struct NavigationDriver {
    pub(super) sender: Sender<NavigationCommand>,
}

impl NavigationDriver {
    /// Sends a command to the navigation worker.
    ///
    /// # Errors
    ///
    /// Returns `SendError` if the navigation worker has exited.
    pub fn send(
        &self,
        command: NavigationCommand,
    ) -> Result<(), mpsc::SendError<NavigationCommand>> {
        self.sender.send(command)
    }
}
