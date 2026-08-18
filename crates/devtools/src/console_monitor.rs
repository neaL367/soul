//! Console log capture and streaming for Developer Tools.

use serde::{Deserialize, Serialize};

/// Captured console message entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleEntry {
    /// Severity level (`log`, `info`, `warn`, `error`).
    pub level: String,
    /// Message content.
    pub message: String,
}

/// In-memory console log collector.
#[derive(Default)]
pub struct ConsoleMonitor {
    messages: Vec<ConsoleEntry>,
}

impl ConsoleMonitor {
    /// Creates a new `ConsoleMonitor`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Appends a new console message.
    pub fn log(&mut self, level: &str, message: &str) {
        self.messages.push(ConsoleEntry {
            level: level.to_string(),
            message: message.to_string(),
        });
    }

    /// Clears all recorded console messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Returns all collected console messages.
    #[must_use]
    pub fn get_messages(&self) -> &[ConsoleEntry] {
        &self.messages
    }
}
