//! Navigation controller state machine driving tab lifecycles.

use super::history::NavigationHistory;
use super::state::{NavigationError, NavigationId, NavigationState};
use url::Url;

/// Controls the lifecycle and state transitions for a single browser tab.
pub struct NavigationController {
    next_id: u64,
    state: NavigationState,
    history: NavigationHistory,
    history_traversal: bool,
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
            history_traversal: false,
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
        self.history_traversal = false;
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
                if !self.history_traversal {
                    self.history.push(committed_url);
                }
                self.history_traversal = false;
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
        self.history.go_back().map(|url| {
            let id = self.navigate_url(url);
            self.history_traversal = true;
            id
        })
    }

    /// Triggers forward navigation in history if available.
    pub fn go_forward(&mut self) -> Option<NavigationId> {
        self.history.go_forward().map(|url| {
            let id = self.navigate_url(url);
            self.history_traversal = true;
            id
        })
    }

    /// Triggers reload of the current URL.
    pub fn reload(&mut self) -> Option<NavigationId> {
        self.state
            .current_url()
            .cloned()
            .map(|url| self.navigate_url(url))
    }
}
