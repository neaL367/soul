//! Asynchronous file download manager coordinating disk streaming and progress updates.

use crate::disposition::sanitize_filename;
use crate::error::DownloadError;
use crate::item::{DownloadItem, DownloadState};
use crate::motw::attach_zone_identifier;
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use networking::HttpClient;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use url::Url;

/// Persist a progress snapshot to the item map at most every 64 KiB received,
/// so a large download does not contend on the lock for every frame.
const PROGRESS_UPDATE_INTERVAL_BYTES: u64 = 64 * 1024;

/// Download manager orchestrating file streaming, MOTW tagging, and transfer lifecycle.
#[derive(Clone)]
pub struct DownloadManager {
    downloads: Arc<Mutex<HashMap<u64, DownloadItem>>>,
    next_id: Arc<AtomicU64>,
    client: HttpClient,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadManager {
    /// Creates a new `DownloadManager`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            downloads: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            client: HttpClient::default(),
        }
    }

    /// Registers and begins an asynchronous file download.
    ///
    /// The response body is streamed to disk incrementally (never buffered
    /// whole in memory) and the download is only considered complete for a
    /// successful 2xx status. The Mark of the Web zone identifier is attached
    /// from the final, post-redirect URL.
    ///
    /// # Errors
    ///
    /// Returns `DownloadError` if URL parsing fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn start_download(
        &self,
        url_str: &str,
        destination: PathBuf,
    ) -> Result<u64, DownloadError> {
        let parsed_url =
            Url::parse(url_str).map_err(|e| DownloadError::InvalidUrl(e.to_string()))?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Sanitize the file name component at the boundary so a caller passing
        // a raw server-supplied `Content-Disposition` name can never write
        // outside the chosen directory or use illegal Windows characters.
        let destination = sanitize_destination(&destination);
        let item = DownloadItem::new(id, url_str.to_string(), destination.clone());
        {
            let mut lock = self.downloads.lock().await;
            lock.insert(id, item);
        }

        self.spawn_download_task(id, parsed_url, destination, 0);
        Ok(id)
    }

    /// Pauses an active or queued downloading item.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn pause_download(&self, id: u64) -> bool {
        let mut lock = self.downloads.lock().await;
        if let Some(d) = lock.get_mut(&id)
            && matches!(
                d.state,
                DownloadState::Downloading { .. } | DownloadState::Queued
            )
        {
            d.state = DownloadState::Paused;
            return true;
        }
        false
    }

    /// Resumes a paused download, requesting remaining bytes via HTTP Range request.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn resume_download(&self, id: u64) -> bool {
        let (url_str, destination, received_bytes) = {
            let mut lock = self.downloads.lock().await;
            let Some(d) = lock.get_mut(&id) else {
                return false;
            };
            if !matches!(d.state, DownloadState::Paused) {
                return false;
            }
            d.state = DownloadState::Queued;
            let existing_len = tokio::fs::metadata(&d.destination_path)
                .await
                .map_or(0, |m| m.len());
            (d.url.clone(), d.destination_path.clone(), existing_len)
        };

        Url::parse(&url_str).is_ok_and(|parsed_url| {
            self.spawn_download_task(id, parsed_url, destination, received_bytes);
            true
        })
    }

    /// Cancels an active or paused download and deletes any partially downloaded file.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn cancel_download(&self, id: u64) -> bool {
        let dest = {
            let mut lock = self.downloads.lock().await;
            if let Some(d) = lock.get_mut(&id) {
                d.state = DownloadState::Cancelled;
                Some(d.destination_path.clone())
            } else {
                None
            }
        };

        if let Some(path) = dest {
            let _ = tokio::fs::remove_file(path).await;
            true
        } else {
            false
        }
    }

    fn spawn_download_task(
        &self,
        id: u64,
        parsed_url: Url,
        destination: PathBuf,
        resume_offset: u64,
    ) {
        let downloads_clone = self.downloads.clone();
        let client_clone = self.client.clone();
        let url_copy = parsed_url.to_string();

        tokio::spawn(async move {
            let started = Instant::now();
            match client_clone.fetch_streaming(&parsed_url).await {
                Ok(stream) => {
                    if !(200..300).contains(&stream.status_code) {
                        tracing::warn!(
                            status = stream.status_code,
                            url = %url_copy,
                            "Download rejected on non-success HTTP status"
                        );
                        Self::mark_failed(
                            &downloads_clone,
                            id,
                            format!("HTTP {}", stream.status_code),
                        )
                        .await;
                        return;
                    }

                    let total_bytes = stream.content_length().map(|c| c + resume_offset);
                    let final_url = stream.url.to_string();
                    Self::mark_downloading(&downloads_clone, id, resume_offset, total_bytes).await;

                    match Self::write_stream_to_disk(
                        stream.into_body(),
                        &destination,
                        &downloads_clone,
                        id,
                        total_bytes,
                        resume_offset > 0,
                    )
                    .await
                    {
                        Ok(received) => {
                            let total_received = resume_offset + received;
                            let elapsed_ms = u64::try_from(started.elapsed().as_millis())
                                .unwrap_or(1)
                                .max(1);
                            let speed_bps = total_received.saturating_mul(1000) / elapsed_ms;

                            // MOTW must reflect the final, post-redirect URL.
                            let _ = attach_zone_identifier(&destination, &final_url);

                            let mut lock = downloads_clone.lock().await;
                            if let Some(d) = lock.get_mut(&id)
                                && !matches!(
                                    d.state,
                                    DownloadState::Cancelled | DownloadState::Paused
                                )
                            {
                                d.speed_bps = speed_bps;
                                d.state = DownloadState::Completed;
                            }
                        }
                        Err(err) => {
                            let _ = tokio::fs::remove_file(&destination).await;
                            Self::mark_failed(&downloads_clone, id, err).await;
                        }
                    }
                }
                Err(e) => {
                    Self::mark_failed(&downloads_clone, id, e.to_string()).await;
                }
            }
        });
    }

    /// Streams a response body to `destination` in bounded-memory frames,
    /// updating the item's progress every `PROGRESS_UPDATE_INTERVAL_BYTES`.
    async fn write_stream_to_disk(
        mut body: BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>,
        destination: &PathBuf,
        downloads: &Arc<Mutex<HashMap<u64, DownloadItem>>>,
        id: u64,
        total_bytes: Option<u64>,
        append: bool,
    ) -> Result<u64, String> {
        let mut file = if append {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(destination)
                .await
                .map_err(|e| format!("Failed to open download file for appending: {e}"))?
        } else {
            File::create(destination)
                .await
                .map_err(|e| format!("Failed to create download file: {e}"))?
        };

        let mut received: u64 = 0;
        let mut next_update = PROGRESS_UPDATE_INTERVAL_BYTES;

        while let Some(frame) = body
            .frame()
            .await
            .transpose()
            .map_err(|e| format!("Failed to read download body: {e}"))?
        {
            // Check if paused or cancelled mid-transfer
            {
                let lock = downloads.lock().await;
                if let Some(item) = lock.get(&id)
                    && matches!(item.state, DownloadState::Paused | DownloadState::Cancelled)
                {
                    return Ok(received);
                }
            }

            if let Ok(data) = frame.into_data() {
                file.write_all(&data)
                    .await
                    .map_err(|e| format!("Failed to write download data: {e}"))?;
                received += data.len() as u64;
                if received >= next_update {
                    Self::mark_downloading(downloads, id, received, total_bytes).await;
                    next_update += PROGRESS_UPDATE_INTERVAL_BYTES;
                }
            }
        }

        file.flush()
            .await
            .map_err(|e| format!("Failed to flush download file: {e}"))?;
        Ok(received)
    }

    async fn mark_downloading(
        downloads: &Arc<Mutex<HashMap<u64, DownloadItem>>>,
        id: u64,
        received: u64,
        total: Option<u64>,
    ) {
        let mut lock = downloads.lock().await;
        if let Some(d) = lock.get_mut(&id)
            && !matches!(d.state, DownloadState::Paused | DownloadState::Cancelled)
        {
            d.state = DownloadState::Downloading {
                received_bytes: received,
                total_bytes: total,
            };
        }
    }

    async fn mark_failed(downloads: &Arc<Mutex<HashMap<u64, DownloadItem>>>, id: u64, err: String) {
        let mut lock = downloads.lock().await;
        if let Some(d) = lock.get_mut(&id) {
            d.state = DownloadState::Failed(err);
        }
    }

    /// Retrieves a cloned snapshot of a download item by ID.
    pub async fn get_download(&self, id: u64) -> Option<DownloadItem> {
        let lock = self.downloads.lock().await;
        lock.get(&id).cloned()
    }

    /// Returns a list of all active and completed downloads.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn list_downloads(&self) -> Vec<DownloadItem> {
        let lock = self.downloads.lock().await;
        let mut list: Vec<DownloadItem> = lock.values().cloned().collect();
        list.sort_by_key(|d| d.id);
        list
    }
}

/// Replaces the file name component of `destination` with its sanitized form,
/// keeping the caller-chosen directory unchanged.
fn sanitize_destination(destination: &Path) -> PathBuf {
    let name = destination
        .file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| "download".to_string(), sanitize_filename);
    destination
        .parent()
        .map_or_else(|| PathBuf::from(&name), |parent| parent.join(&name))
}
