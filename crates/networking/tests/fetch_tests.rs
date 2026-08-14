//! Integration tests for HTTP/1.1 fetch client against a local test server.

use networking::{HttpClient, HttpRequest, HttpResponse};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

async fn spawn_mock_http_server<F>(handler: F) -> (SocketAddr, tokio::task::JoinHandle<()>)
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let response_bytes = handler(&req_str);
            let _ = socket.write_all(response_bytes.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    (addr, handle)
}

#[tokio::test]
async fn test_http1_get_success() {
    let (addr, server_handle) = spawn_mock_http_server(|req| {
        assert!(req.contains("GET /index.html HTTP/1.1"));
        assert!(req.contains("host: 127.0.0.1"));
        let body = "<!DOCTYPE html><html><body><h1>Hello Soul Engine!</h1></body></html>";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/index.html", addr.port())).unwrap();

    let response: HttpResponse = client.fetch(&url).await.expect("fetch failed");

    assert_eq!(response.status_code, 200);
    assert!(response.is_success());
    assert_eq!(response.mime_type, "text/html");

    let text = response.text().expect("decode text failed");
    assert_eq!(
        text,
        "<!DOCTYPE html><html><body><h1>Hello Soul Engine!</h1></body></html>"
    );

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_http1_404_not_found() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        let body = "Not Found";
        format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/missing.html", addr.port())).unwrap();

    let response = client.fetch(&url).await.expect("fetch failed");

    assert_eq!(response.status_code, 404);
    assert!(!response.is_success());
    assert_eq!(response.mime_type, "text/plain");
    assert_eq!(response.text().unwrap(), "Not Found");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_http1_custom_headers_and_lookup() {
    let (addr, server_handle) = spawn_mock_http_server(|req| {
        assert!(req.contains("x-custom-auth: SecretBrowserToken"));
        let body = "Authenticated";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nX-Server-Time: 123456\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/api/data", addr.port())).unwrap();

    let request = HttpRequest::get(url).with_header("X-Custom-Auth", "SecretBrowserToken");

    let response = client
        .fetch_request(&request)
        .await
        .expect("fetch_request failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.header("X-Server-Time"), Some("123456"));
    assert_eq!(response.header("x-server-time"), Some("123456"));
    assert_eq!(response.text().unwrap(), "Authenticated");

    let _ = server_handle.await;
}
