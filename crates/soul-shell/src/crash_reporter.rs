//! Crash reporting, minidump diagnostics capture, breadcrumb tracking, and failure persistence.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Thread-safe ring buffer capturing the most recent navigation and subsystem event breadcrumbs.
#[derive(Debug)]
pub struct BreadcrumbTracker {
    capacity: usize,
    events: Mutex<VecDeque<String>>,
}

impl BreadcrumbTracker {
    /// Returns the global singleton `BreadcrumbTracker`.
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<BreadcrumbTracker> = OnceLock::new();
        GLOBAL.get_or_init(|| Self::with_capacity(50))
    }

    /// Creates a new `BreadcrumbTracker` with bounded capacity.
    #[must_use]
    pub const fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            events: Mutex::new(VecDeque::new()),
        }
    }

    /// Records a new breadcrumb event, evicting the oldest entry if capacity is reached.
    pub fn record(&self, event: impl Into<String>) {
        if let Ok(mut lock) = self.events.lock() {
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(event.into());
        }
    }

    /// Returns a snapshot copy of all currently recorded breadcrumbs.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.events
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clears all recorded breadcrumbs.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.events.lock() {
            lock.clear();
        }
    }
}

/// Structured crash report recording unhandled failure diagnostics and environmental state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    /// Timestamp (UNIX epoch seconds) when crash occurred.
    pub timestamp: u64,
    /// Reason or panic message.
    pub reason: String,
    /// Process subsystem name (e.g. "renderer", "gpu", "network").
    pub subsystem: String,
    /// Detailed callstack or minidump diagnostic details.
    pub diagnostics: String,
    /// Chronological list of user action and navigation breadcrumbs leading up to crash.
    pub breadcrumbs: Vec<String>,
    /// Host operating system version.
    pub os_version: String,
    /// Browser build version.
    pub browser_version: String,
}

impl CrashReport {
    /// Creates a new `CrashReport`, automatically capturing recent breadcrumbs from the global tracker.
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
            breadcrumbs: BreadcrumbTracker::global().snapshot(),
            os_version: "Windows 11".to_string(),
            browser_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Attaches explicit custom breadcrumbs to the report.
    #[must_use]
    pub fn with_breadcrumbs(mut self, breadcrumbs: Vec<String>) -> Self {
        self.breadcrumbs = breadcrumbs;
        self
    }

    /// Sets custom OS version metadata.
    #[must_use]
    pub fn with_os_version(mut self, os_version: impl Into<String>) -> Self {
        self.os_version = os_version.into();
        self
    }

    /// Sets custom browser build version.
    #[must_use]
    pub fn with_browser_version(mut self, version: impl Into<String>) -> Self {
        self.browser_version = version.into();
        self
    }

    /// Serializes the crash report into a formatted textual log.
    #[must_use]
    pub fn serialize_log(&self) -> String {
        use std::fmt::Write;

        let mut out = format!(
            "timestamp: {}\nos_version: {}\nbrowser_version: {}\nsubsystem: {}\nreason: {}\ndiagnostics: {}\nbreadcrumbs:\n",
            self.timestamp,
            self.os_version,
            self.browser_version,
            self.subsystem,
            self.reason,
            self.diagnostics
        );
        for (i, b) in self.breadcrumbs.iter().enumerate() {
            let _ = writeln!(out, "  [{i}] {b}");
        }
        out
    }

    /// Persists the crash report to disk under `report_dir` and prunes older logs.
    ///
    /// # Errors
    /// Returns `std::io::Error` on disk write failure.
    pub fn persist_to_disk(&self, report_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(report_dir)?;
        let filename = format!("crash_{}_{}.log", self.timestamp, self.subsystem);
        let filepath = report_dir.join(filename);
        std::fs::write(&filepath, self.serialize_log())?;

        // Maintain bounded disk storage: keep maximum 20 crash reports
        let _ = prune_old_reports(report_dir, 20);

        Ok(filepath)
    }
}

/// Prunes older crash report logs in `report_dir` if the total file count exceeds `max_retained`.
///
/// # Errors
///
/// Returns `std::io::Error` if directory traversal fails.
pub fn prune_old_reports(report_dir: &Path, max_retained: usize) -> std::io::Result<usize> {
    if !report_dir.exists() {
        return Ok(0);
    }

    let mut log_files = Vec::new();
    for entry in std::fs::read_dir(report_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            && let Ok(meta) = entry.metadata()
        {
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            log_files.push((path, modified));
        }
    }

    let total = log_files.len();
    if total <= max_retained {
        return Ok(0);
    }

    // Sort oldest first
    log_files.sort_by_key(|(_, modified)| *modified);
    let to_remove = total.saturating_sub(max_retained);
    let mut removed_count = 0;

    for (path, _) in log_files.iter().take(to_remove) {
        if std::fs::remove_file(path).is_ok() {
            removed_count += 1;
        }
    }

    Ok(removed_count)
}
