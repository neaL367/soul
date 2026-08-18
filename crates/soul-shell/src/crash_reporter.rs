//! Crash reporting, minidump diagnostics capture, and failure persistence.

use std::path::{Path, PathBuf};

/// Structured crash report recording unhandled failure diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    /// Timestamp (UNIX epoch seconds) when crash occurred.
    pub timestamp: u64,
    /// Reason or panic message.
    pub reason: String,
    /// Process subsystem name (e.g. "renderer", "gpu", "network").
    pub subsystem: String,
    /// Optional stack trace or minidump path.
    pub diagnostics: String,
}

impl CrashReport {
    /// Creates a new `CrashReport`.
    #[must_use]
    pub fn new(
        reason: impl Into<String>,
        subsystem: impl Into<String>,
        diagnostics: impl Into<String>,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            timestamp,
            reason: reason.into(),
            subsystem: subsystem.into(),
            diagnostics: diagnostics.into(),
        }
    }

    /// Serializes the crash report into a formatted textual log.
    #[must_use]
    pub fn serialize_log(&self) -> String {
        format!(
            "timestamp: {}\nsubsystem: {}\nreason: {}\ndiagnostics: {}\n",
            self.timestamp, self.subsystem, self.reason, self.diagnostics
        )
    }

    /// Persists the crash report to disk under `report_dir`.
    ///
    /// # Errors
    /// Returns `std::io::Error` on disk write failure.
    pub fn persist_to_disk(&self, report_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(report_dir)?;
        let filename = format!("crash_{}_{}.log", self.timestamp, self.subsystem);
        let filepath = report_dir.join(filename);
        std::fs::write(&filepath, self.serialize_log())?;
        Ok(filepath)
    }
}
