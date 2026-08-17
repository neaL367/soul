//! Shared fixtures and mock HTTP servers for integration tests.
#![allow(dead_code)]

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawns a single-shot mock HTTP server responding to one request.
pub async fn spawn_mock_http_server<F>(handler: F) -> (SocketAddr, tokio::task::JoinHandle<()>)
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);
            let response_bytes = handler(&req_str);
            let _ = socket.write_all(response_bytes.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    (addr, handle)
}

/// Standard HTML test fixture.
pub const fn fixture_html() -> &'static str {
    r#"<!DOCTYPE html>
    <html>
    <head><title>Fixture</title></head>
    <body style="background-color: #ffffff;">
        <h1 aria-label="Main Heading">Page Title</h1>
        <p>Introductory paragraph</p>
        <button aria-label="Submit Button">Click</button>
        <a href="/next" style="display: block;">Next</a>
        <img src="missing.png" aria-label="Hero Image">
    </body>
    </html>"#
}

/// Constructs a basic HTTP 200 HTML response.
pub fn http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Multi-connection mock server serving a styled page plus a red PNG image.
pub async fn spawn_media_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let red_pixels: Vec<u8> = (0..8 * 8).flat_map(|_| [255u8, 0, 0, 255]).collect();
    let logo_png = image_decode::encode_png(&red_pixels, 8, 8).expect("encode test PNG");

    let page_html = r#"<!DOCTYPE html>
    <html>
    <head>
        <style>
            body { background-color: #00ff00; }
            p { color: #0000ff; }
        </style>
    </head>
    <body>
        <h1>Styled Page</h1>
        <img src="/logo.png">
        <p>Paragraph</p>
    </body>
    </html>"#;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let response = if req_str.contains("GET /logo.png ") {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    logo_png.len()
                )
                .into_bytes()
                .into_iter()
                .chain(logo_png.clone())
                .collect::<Vec<u8>>()
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    page_html.len(),
                    page_html
                )
                .into_bytes()
            };
            let _ = socket.write_all(&response).await;
            let _ = socket.flush().await;
        }
    });

    (addr, handle)
}

/// Helper to read a specific pixel from an RGBA buffer.
pub fn pixel_at(buffer: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]]
}
