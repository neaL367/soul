//! Download item descriptor and transfer state machine.

use std::path::PathBuf;

/// Transfer and lifecycle state of a download operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    /// Initial queued state before network connection is established.
    Queued,
    /// Active download stream transfer in progress.
    Downloading {
        /// Number of bytes received so far.
        received_bytes: u64,
        /// Total file size in bytes if provided by `Content-Length`.
        total_bytes: Option<u64>,
    },
    /// Download is temporarily paused.
    Paused,
    /// Download completed and verified on disk.
    Completed,
    /// Download failed with an error message.
    Failed(String),
    /// Download was cancelled by user.
    Cancelled,
}

/// Metadata and state representation of an active or historical file download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadItem {
    /// Unique download task ID.
    pub id: u64,
    /// Origin URL of the downloadable asset.
    pub url: String,
    /// Inferred or specified local file name.
    pub file_name: String,
    /// Target filesystem destination path.
    pub destination_path: PathBuf,
    /// Current transfer progress state.
    pub state: DownloadState,
    /// Estimated download transfer rate in bytes per second.
    pub speed_bps: u64,
}

impl DownloadItem {
    /// Creates a new `DownloadItem` in the initial `Queued` state.
    #[must_use]
    pub fn new(id: u64, url: String, destination_path: PathBuf) -> Self {
        let file_name = destination_path.file_name().map_or_else(
            || "download".to_string(),
            |s| s.to_string_lossy().to_string(),
        );

        Self {
            id,
            url,
            file_name,
            destination_path,
            state: DownloadState::Queued,
            speed_bps: 0,
        }
    }
}
