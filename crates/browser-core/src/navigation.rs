//! Navigation state machine, URL resolution, and session history management.

use std::time::SystemTime;
use thiserror::Error;
use url::Url;

/// Unique identifier assigned to each navigation sequence to prevent out-of-order commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NavigationId(pub u64);

/// States of a tab's navigation lifecycle per the architecture plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationState {
    /// Initial idle state before any navigation has started.
    Init,
    /// Network request or fetch is in-flight.
    Navigating {
        /// Active navigation identifier.
        id: NavigationId,
        /// Destination URL.
        url: Url,
    },
    /// Response headers and status code received from network.
    ResponseReceived {
        /// Active navigation identifier.
        id: NavigationId,
        /// Final response URL (after any redirects).
        url: Url,
        /// HTTP status code.
        status_code: u16,
        /// MIME content type.
        mime_type: String,
    },
    /// HTML parsed into DOM tree and initial styles computed.
    DomReady {
        /// Active navigation identifier.
        id: NavigationId,
        /// Page URL.
        url: Url,
    },
    /// Full page and sub-resources loaded.
    Loaded {
        /// Active navigation identifier.
        id: NavigationId,
        /// Page URL.
        url: Url,
    },
    /// Navigation failed or was canceled.
    Failed {
        /// Active navigation identifier.
        id: NavigationId,
        /// Attempted URL.
        url: Url,
        /// Human-readable error description.
        error: String,
    },
}

impl NavigationState {
    /// Returns the active `NavigationId` if a navigation has occurred.
    #[must_use]
    pub const fn navigation_id(&self) -> Option<NavigationId> {
        match self {
            Self::Init => None,
            Self::Navigating { id, .. }
            | Self::ResponseReceived { id, .. }
            | Self::DomReady { id, .. }
            | Self::Loaded { id, .. }
            | Self::Failed { id, .. } => Some(*id),
        }
    }

    /// Returns the current or destination URL if available.
    #[must_use]
    pub const fn current_url(&self) -> Option<&Url> {
        match self {
            Self::Init => None,
            Self::Navigating { url, .. }
            | Self::ResponseReceived { url, .. }
            | Self::DomReady { url, .. }
            | Self::Loaded { url, .. }
            | Self::Failed { url, .. } => Some(url),
        }
    }
}

/// Errors originating during URL resolution or navigation processing.
#[derive(Debug, Error)]
pub enum NavigationError {
    /// Invalid URL syntax.
    #[error("Failed to parse URL '{0}': {1}")]
    InvalidUrl(String, url::ParseError),

    /// Navigation operation was canceled.
    #[error("Navigation canceled")]
    Canceled,

    /// General navigation error.
    #[error("Navigation error: {0}")]
    Other(String),
}

/// Single entry in a tab's back/forward session history stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Target URL.
    pub url: Url,
    /// Page title if loaded.
    pub title: Option<String>,
    /// Timestamp when this entry was created.
    pub timestamp: SystemTime,
}

impl HistoryEntry {
    /// Creates a new history entry for a URL.
    #[must_use]
    pub fn new(url: Url) -> Self {
        Self {
            url,
            title: None,
            timestamp: SystemTime::now(),
        }
    }
}

/// Session navigation history managing back and forward stacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavigationHistory {
    entries: Vec<HistoryEntry>,
    current_index: Option<usize>,
}

impl NavigationHistory {
    /// Creates an empty session navigation history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if back navigation is available.
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.current_index.is_some_and(|idx| idx > 0)
    }

    /// Returns true if forward navigation is available.
    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        self.current_index
            .is_some_and(|idx| idx + 1 < self.entries.len())
    }

    /// Returns the currently active history entry.
    #[must_use]
    pub fn current_entry(&self) -> Option<&HistoryEntry> {
        self.current_index.and_then(|idx| self.entries.get(idx))
    }

    /// Pushes a new committed URL onto the history stack, truncating forward entries.
    pub fn push(&mut self, url: Url) {
        if let Some(idx) = self.current_index {
            self.entries.truncate(idx + 1);
        } else {
            self.entries.clear();
        }
        self.entries.push(HistoryEntry::new(url));
        self.current_index = Some(self.entries.len() - 1);
    }

    /// Steps backward in history and returns the destination URL.
    pub fn go_back(&mut self) -> Option<Url> {
        match self.current_index {
            Some(idx) if idx > 0 => {
                let new_idx = idx - 1;
                self.current_index = Some(new_idx);
                Some(self.entries[new_idx].url.clone())
            }
            _ => None,
        }
    }

    /// Steps forward in history and returns the destination URL.
    pub fn go_forward(&mut self) -> Option<Url> {
        match self.current_index {
            Some(idx) if idx + 1 < self.entries.len() => {
                let new_idx = idx + 1;
                self.current_index = Some(new_idx);
                Some(self.entries[new_idx].url.clone())
            }
            _ => None,
        }
    }
}

/// Controls the lifecycle and state transitions for a single browser tab.
pub struct NavigationController {
    next_id: u64,
    state: NavigationState,
    history: NavigationHistory,
}

impl Default for NavigationController {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationController {
    /// Creates a new navigation controller in the `Init` state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            state: NavigationState::Init,
            history: NavigationHistory::new(),
        }
    }

    /// Returns the current state of navigation.
    #[must_use]
    pub const fn state(&self) -> &NavigationState {
        &self.state
    }

    /// Returns a reference to the session history.
    #[must_use]
    pub const fn history(&self) -> &NavigationHistory {
        &self.history
    }

    /// Initiates navigation to a raw URL string with automatic scheme normalization.
    pub fn navigate(&mut self, url_str: &str) -> Result<NavigationId, NavigationError> {
        let normalized = if url_str.starts_with("http://")
            || url_str.starts_with("https://")
            || url_str.starts_with("about:")
            || url_str.starts_with("file://")
        {
            url_str.to_string()
        } else {
            format!("https://{url_str}")
        };

        let parsed = Url::parse(&normalized)
            .map_err(|err| NavigationError::InvalidUrl(url_str.to_string(), err))?;

        Ok(self.navigate_url(parsed))
    }

    /// Initiates navigation to an already parsed `Url`.
    pub fn navigate_url(&mut self, url: Url) -> NavigationId {
        let id = NavigationId(self.next_id);
        self.next_id += 1;

        tracing::info!(navigation_id = id.0, url = %url, "Starting navigation");
        self.state = NavigationState::Navigating { id, url };
        id
    }

    /// Cancels the in-flight navigation if one is active.
    pub fn cancel(&mut self) {
        if let NavigationState::Navigating { id, ref url } = self.state {
            tracing::info!(navigation_id = id.0, url = %url, "Canceling navigation");
            self.state = NavigationState::Failed {
                id,
                url: url.clone(),
                error: "Navigation canceled by user".to_string(),
            };
        }
    }

    /// Handles HTTP response received from the network.
    ///
    /// Returns `true` if the event matched the active `NavigationId` and was applied,
    /// or `false` if the event carried a stale `NavigationId` and was discarded.
    pub fn handle_response(
        &mut self,
        id: NavigationId,
        status_code: u16,
        mime_type: String,
    ) -> bool {
        match &self.state {
            NavigationState::Navigating { id: active_id, url } if *active_id == id => {
                let url = url.clone();
                self.state = NavigationState::ResponseReceived {
                    id,
                    url,
                    status_code,
                    mime_type,
                };
                true
            }
            _ => {
                tracing::debug!(event_id = id.0, "Discarding stale response event");
                false
            }
        }
    }

    /// Handles DOM ready notification from the HTML parser / style engine.
    pub fn handle_dom_ready(&mut self, id: NavigationId) -> bool {
        match &self.state {
            NavigationState::ResponseReceived {
                id: active_id, url, ..
            } if *active_id == id => {
                let url = url.clone();
                self.state = NavigationState::DomReady { id, url };
                true
            }
            _ => {
                tracing::debug!(event_id = id.0, "Discarding stale DOM ready event");
                false
            }
        }
    }

    /// Handles full page load complete notification.
    pub fn handle_loaded(&mut self, id: NavigationId) -> bool {
        match &self.state {
            NavigationState::DomReady { id: active_id, url } if *active_id == id => {
                let committed_url = url.clone();
                self.state = NavigationState::Loaded {
                    id,
                    url: committed_url.clone(),
                };
                self.history.push(committed_url);
                true
            }
            _ => {
                tracing::debug!(event_id = id.0, "Discarding stale loaded event");
                false
            }
        }
    }

    /// Handles navigation error notification.
    pub fn handle_error(&mut self, id: NavigationId, error: String) -> bool {
        match self.state.navigation_id() {
            Some(active_id) if active_id == id => {
                if let Some(url) = self.state.current_url().cloned() {
                    self.state = NavigationState::Failed { id, url, error };
                    return true;
                }
                false
            }
            _ => {
                tracing::debug!(event_id = id.0, "Discarding stale error event");
                false
            }
        }
    }

    /// Triggers backward navigation in history if available.
    pub fn go_back(&mut self) -> Option<NavigationId> {
        self.history.go_back().map(|url| self.navigate_url(url))
    }

    /// Triggers forward navigation in history if available.
    pub fn go_forward(&mut self) -> Option<NavigationId> {
        self.history.go_forward().map(|url| self.navigate_url(url))
    }

    /// Triggers reload of the current URL.
    pub fn reload(&mut self) -> Option<NavigationId> {
        self.state
            .current_url()
            .cloned()
            .map(|url| self.navigate_url(url))
    }
}
