//! Session navigation history management.

use std::time::SystemTime;
use url::Url;

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
