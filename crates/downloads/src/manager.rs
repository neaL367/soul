//! Asynchronous file download manager coordinating disk writes and progress updates.

use crate::error::DownloadError;
use crate::item::{DownloadItem, DownloadState};
use crate::motw::attach_zone_identifier;
use networking::HttpClient;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use url::Url;

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

        let item = DownloadItem::new(id, url_str.to_string(), destination.clone());
        {
            let mut lock = self.downloads.lock().await;
            lock.insert(id, item);
        }

        let downloads_clone = self.downloads.clone();
        let client_clone = self.client.clone();
        let url_copy = url_str.to_string();

        tokio::spawn(async move {
            match client_clone.fetch(&parsed_url).await {
                Ok(response) => {
                    let total_bytes = response.body.len() as u64;

                    if let Ok(mut file) = File::create(&destination).await
                        && file.write_all(&response.body).await.is_ok()
                        && file.flush().await.is_ok()
                    {
                        // Attach Mark of the Web zone identifier
                        let _ = attach_zone_identifier(&destination, &url_copy);

                        let mut lock = downloads_clone.lock().await;
                        if let Some(d) = lock.get_mut(&id) {
                            d.state = DownloadState::Completed;
                            d.speed_bps = total_bytes;
                        }
                        return;
                    }

                    let mut lock = downloads_clone.lock().await;
                    if let Some(d) = lock.get_mut(&id) {
                        d.state = DownloadState::Failed("Disk write error occurred".to_string());
                    }
                }
                Err(e) => {
                    let mut lock = downloads_clone.lock().await;
                    if let Some(d) = lock.get_mut(&id) {
                        d.state = DownloadState::Failed(e.to_string());
                    }
                }
            }
        });

        Ok(id)
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
