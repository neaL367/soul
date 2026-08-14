//! Download manager subsystem providing resumable HTTP file transfers and MOTW file security.

pub mod error;
pub mod item;
pub mod manager;
pub mod motw;

pub use error::DownloadError;
pub use item::{DownloadItem, DownloadState};
pub use manager::DownloadManager;
pub use motw::attach_zone_identifier;
