//! Browser profile management and Private Browsing ephemeral sessions.

use std::path::PathBuf;

/// Profile mode governing persistence and session isolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserProfile {
    /// Standard profile persisting history, cookies, and `LocalStorage` to disk.
    Standard {
        /// Profile root data directory.
        data_dir: PathBuf,
    },
    /// Private Browsing mode: 100% ephemeral in-memory storage, zero disk trace.
    PrivateBrowsing,
}

impl BrowserProfile {
    /// Returns `true` if this profile operates in ephemeral private browsing mode.
    #[must_use]
    pub const fn is_ephemeral(&self) -> bool {
        matches!(self, Self::PrivateBrowsing)
    }

    /// Returns the on-disk data directory path if not ephemeral.
    #[must_use]
    pub const fn data_dir(&self) -> Option<&PathBuf> {
        match self {
            Self::Standard { data_dir } => Some(data_dir),
            Self::PrivateBrowsing => None,
        }
    }
}
