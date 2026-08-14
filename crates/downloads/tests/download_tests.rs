//! Integration tests for `DownloadManager` and Mark of the Web zone identifier tagging.

use downloads::{DownloadItem, DownloadManager, DownloadState, attach_zone_identifier};
use std::fs;
use std::path::PathBuf;

#[tokio::test]
async fn test_download_item_state_creation() {
    let item = DownloadItem::new(
        1,
        "https://example.com/archive.zip".to_string(),
        PathBuf::from("C:\\Users\\test\\Downloads\\archive.zip"),
    );
    assert_eq!(item.id, 1);
    assert_eq!(item.file_name, "archive.zip");
    assert_eq!(item.state, DownloadState::Queued);
}

#[tokio::test]
async fn test_download_manager_list_and_query() {
    let manager = DownloadManager::new();
    let dest = std::env::temp_dir().join("soul_test_dl.bin");

    let dl_id = manager
        .start_download("http://127.0.0.1:9/invalid", dest.clone())
        .await
        .unwrap();
    assert_eq!(dl_id, 1);

    let item = manager.get_download(dl_id).await;
    assert!(item.is_some());
    assert_eq!(item.unwrap().id, 1);

    let list = manager.list_downloads().await;
    assert_eq!(list.len(), 1);
}

#[test]
fn test_motw_zone_identifier_attachment() {
    let temp_file = std::env::temp_dir().join("soul_motw_test.txt");
    fs::write(&temp_file, b"test content").unwrap();

    let res = attach_zone_identifier(&temp_file, "https://example.com/file.txt");
    // Windows NTFS ADS will succeed
    assert!(res.is_ok());

    let _ = fs::remove_file(temp_file);
}
