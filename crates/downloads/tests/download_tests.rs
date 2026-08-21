//! Integration tests for `DownloadManager`, RFC 6266 Content-Disposition, and MOTW tagging.

use downloads::{
    DownloadItem, DownloadManager, DownloadState, attach_zone_identifier, find_available_path,
    parse_content_disposition_filename, sanitize_filename,
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
fn test_sanitize_reserved_windows_device_names() {
    // Reserved device names must be neutralized so writes never hit NUL/CON/etc.
    assert_eq!(sanitize_filename("NUL"), "_NUL");
    assert_eq!(sanitize_filename("nul.txt"), "_nul.txt");
    assert_eq!(sanitize_filename("CON"), "_CON");
    assert_eq!(sanitize_filename("COM1.log"), "_COM1.log");
    assert_eq!(sanitize_filename("LPT9"), "_LPT9");
    // Ordinary names are untouched.
    assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
    assert_eq!(sanitize_filename("com1data.txt"), "com1data.txt");
}

#[test]
fn test_sanitize_all_dot_names() {
    assert_eq!(sanitize_filename("..."), "download");
    assert_eq!(sanitize_filename(".."), "download");
    assert_eq!(sanitize_filename("...."), "download");
}

#[tokio::test]
async fn test_start_download_sanitizes_destination_filename() {
    let manager = DownloadManager::new();
    // Illegal Windows characters and dot-only names in the file name must be
    // neutralized before the item is recorded or any file is created.
    let raw = std::env::temp_dir().join("..").join("report:final?.pdf");
    let id = manager
        .start_download("http://127.0.0.1:9/invalid", raw)
        .await
        .unwrap();

    let item = manager.get_download(id).await.unwrap();
    assert_eq!(
        item.destination_path.file_name().unwrap().to_str().unwrap(),
        "reportfinal.pdf"
    );
    assert_eq!(item.file_name, "reportfinal.pdf");
}

#[test]
fn test_sanitize_reserved_name_via_content_disposition() {
    let header = "attachment; filename=\"NUL.exe\"";
    assert_eq!(
        parse_content_disposition_filename(header).as_deref(),
        Some("_NUL.exe")
    );
}

#[test]
fn test_find_available_path_never_overwrites() {
    let dir = std::env::temp_dir().join("soul_dl_collision_many");
    let _ = fs::create_dir_all(&dir);

    let base = dir.join("file.bin");
    fs::write(&base, b"0").unwrap();
    for i in 1..5 {
        fs::write(dir.join(format!("file ({i}).bin")), b"0").unwrap();
    }

    // The next free slot is `file (5).bin`; the resolver must not fall back to
    // an existing path and silently overwrite it.
    let resolved = find_available_path(&dir, "file.bin");
    assert_eq!(
        resolved.file_name().unwrap().to_str().unwrap(),
        "file (5).bin"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Spawns a minimal HTTP/1.0 server that serves a fixed body for exactly one
/// connection and returns its `http://127.0.0.1:<port>` URL.
async fn serve_once(status_line: &str, body: &[u8]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = body.to_vec();
    let status_line = status_line.to_string();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let response = format!(
            "{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            status_line,
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.write_all(&body).await;
        let _ = socket.flush().await;
    });

    format!("http://{addr}")
}

#[tokio::test]
async fn test_download_streams_successful_response() {
    let url = serve_once("HTTP/1.1 200 OK", b"streamed payload").await;
    let dest = std::env::temp_dir().join("soul_streamed_download.bin");
    let _ = fs::remove_file(&dest);

    let manager = DownloadManager::new();
    let id = manager.start_download(&url, dest.clone()).await.unwrap();

    // Wait for the spawned transfer to complete.
    let mut item = manager.get_download(id).await.unwrap();
    for _ in 0..100 {
        if item.state == DownloadState::Completed {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if let Some(d) = manager.get_download(id).await {
            item = d;
        }
    }

    assert_eq!(item.state, DownloadState::Completed);
    let on_disk = fs::read(&dest).unwrap();
    assert_eq!(on_disk, b"streamed payload");
    assert!(item.speed_bps > 0);
    let _ = fs::remove_file(&dest);
}

#[tokio::test]
async fn test_download_rejects_non_success_status() {
    let url = serve_once("HTTP/1.1 404 Not Found", b"nope").await;
    let dest = std::env::temp_dir().join("soul_failed_download.bin");
    let _ = fs::remove_file(&dest);

    let manager = DownloadManager::new();
    let id = manager.start_download(&url, dest.clone()).await.unwrap();

    let mut item = manager.get_download(id).await.unwrap();
    for _ in 0..100 {
        if matches!(item.state, DownloadState::Failed(_)) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if let Some(d) = manager.get_download(id).await {
            item = d;
        }
    }

    assert!(
        matches!(&item.state, DownloadState::Failed(msg) if msg.starts_with("HTTP 404")),
        "expected HTTP 404 failure, got {:?}",
        item.state
    );
    // The error body must not have been written, and no partial file left behind.
    assert!(
        !dest.exists(),
        "non-success download must not create a file"
    );
    let _ = fs::remove_file(&dest);
}

#[tokio::test]
async fn test_download_pause_resume_and_cancel_lifecycle() {
    let manager = DownloadManager::new();
    let dest = std::env::temp_dir().join("soul_pause_resume_test.bin");
    let _ = fs::remove_file(&dest);

    let id = manager
        .start_download("http://127.0.0.1:9/invalid", dest.clone())
        .await
        .unwrap();

    // Test pause
    let paused = manager.pause_download(id).await;
    assert!(paused);

    let resumed = manager.resume_download(id).await;
    assert!(resumed);

    // Test cancel
    let cancelled = manager.cancel_download(id).await;
    assert!(cancelled);
    let item = manager.get_download(id).await.unwrap();
    assert_eq!(item.state, DownloadState::Cancelled);
    assert!(!dest.exists());
}
