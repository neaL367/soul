//! Integration tests for `DownloadManager`, RFC 6266 Content-Disposition, and MOTW tagging.

use downloads::{
    DownloadItem, DownloadManager, DownloadState, attach_zone_identifier, find_available_path,
    parse_content_disposition_filename,
};
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
    assert!(res.is_ok());

    let _ = fs::remove_file(temp_file);
}

#[test]
fn test_rfc6266_content_disposition_parsing() {
    // Standard ASCII filename
    let header1 = "attachment; filename=\"report_2026.pdf\"";
    assert_eq!(
        parse_content_disposition_filename(header1).as_deref(),
        Some("report_2026.pdf")
    );

    // RFC 5987 / RFC 6266 UTF-8 percent-encoded filename*
    let header2 = "attachment; filename*=UTF-8''my%20document%20%28final%29.pdf";
    assert_eq!(
        parse_content_disposition_filename(header2).as_deref(),
        Some("my document (final).pdf")
    );

    // Sanitizes traversal and invalid characters
    let header3 = "attachment; filename=\"../../evil<script>:foo.exe\"";
    assert_eq!(
        parse_content_disposition_filename(header3).as_deref(),
        Some("evilscriptfoo.exe")
    );
}

#[test]
fn test_find_available_path_collision_resolution() {
    let dir = std::env::temp_dir().join("soul_dl_collision_test");
    let _ = fs::create_dir_all(&dir);

    let base_file = dir.join("document.pdf");
    fs::write(&base_file, b"existing").unwrap();

    let resolved = find_available_path(&dir, "document.pdf");
    assert_eq!(
        resolved.file_name().unwrap().to_str().unwrap(),
        "document (1).pdf"
    );

    let _ = fs::remove_file(&base_file);
    let _ = fs::remove_dir_all(&dir);
}
