//! Navigation lifecycle state and error types.

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

    /// Returns `true` if a navigation request or document parsing is currently in progress.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(
            self,
            Self::Navigating { .. } | Self::ResponseReceived { .. } | Self::DomReady { .. }
        )
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
