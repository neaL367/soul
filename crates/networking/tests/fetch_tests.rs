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

/// Multi-connection mock server: serves `/start` with a 302 to `/final` and
/// `/final` with a 200 body. Accepts up to two connections.
async fn spawn_redirect_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let response = if req_str.contains("GET /start ") {
                "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                let body = "<html><body><h1>Redirected!</h1></body></html>";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            };
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    (addr, handle)
}

#[tokio::test]
async fn test_http_client_follows_redirects() {
    let (addr, server_handle) = spawn_redirect_server().await;
    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/start", addr.port())).unwrap();

    let response = client.fetch(&url).await.expect("redirect chain failed");

    // Final response carries the resolved URL and the 200 body.
    assert_eq!(response.status_code, 200);
    assert_eq!(
        response.url.as_str(),
        format!("http://127.0.0.1:{}/final", addr.port())
    );
    assert!(response.text().unwrap().contains("Redirected!"));

    let _ = server_handle.await;
}

/// Cross-origin redirect bypass test: same-origin request redirects to a second
/// local origin that responds WITHOUT CORS headers. The final response URL must
/// be the one checked, so the request must be rejected with `CorsViolation`.
#[tokio::test]
async fn test_cors_enforced_on_final_url_after_cross_origin_redirect() {
    let (addr_final, server_handle_final) = spawn_mock_http_server(|_req| {
        let body = "redirected content";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let final_port = addr_final.port();
    let (addr_origin, server_handle_origin) = spawn_mock_http_server(move |_req| {
        let target = format!("http://127.0.0.1:{final_port}/final");
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    })
    .await;

    let client = HttpClient::default();
    let request_url =
        Url::parse(&format!("http://127.0.0.1:{}/start", addr_origin.port())).unwrap();
    let doc_origin = Url::parse(&format!("http://127.0.0.1:{}", addr_origin.port())).unwrap();

    let err = client
        .fetch_with_security_context(&HttpRequest::get(request_url), Some(&doc_origin))
        .await
        .expect_err("cross-origin redirect without CORS headers must be rejected");
    assert!(matches!(err, networking::NetworkError::CorsViolation(_)));

    let _ = server_handle_origin.await;
    let _ = server_handle_final.await;
}

#[tokio::test]
async fn test_http_client_redirect_loop_fails() {
    // Self-referencing Location header: /loop -> /loop forever.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let response =
                "HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_string();
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/loop", addr.port())).unwrap();

    let err = client
        .fetch(&url)
        .await
        .expect_err("redirect loop must fail");
    assert!(matches!(err, networking::NetworkError::TooManyRedirects(_)));

    handle.abort();
}

#[tokio::test]
async fn test_http1_gzip_decompression_success() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let original_html = "<html><body><h1>Gzip Compressed Content Delivered!</h1></body></html>";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original_html.as_bytes()).unwrap();
    let compressed_bytes = encoder.finish().unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let compressed_clone = compressed_bytes.clone();
    let server_handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await.unwrap_or(0);

            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                compressed_clone.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(&compressed_clone).await;
            let _ = socket.flush().await;
        }
    });

    let client = HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/compressed.html", addr.port())).unwrap();
    let response = client.fetch(&url).await.expect("fetch compressed failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.text().unwrap(), original_html);

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_http_client_timeout_bounds_hung_servers() {
    // A server that accepts the connection but never responds must be bounded
    // by the client's configured timeout instead of hanging the fetch forever.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = [0u8; 2048];
        let _ = socket.read(&mut buf).await;
        // Never write a response; hold the connection open.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    });

    let config = networking::HttpClientConfig {
        timeout: std::time::Duration::from_millis(200),
        ..networking::HttpClientConfig::default()
    };
    let client = HttpClient::new(config);
    let url = Url::parse(&format!("http://{addr}/hung")).unwrap();

    let err = client
        .fetch(&url)
        .await
        .expect_err("hung server must trigger the client timeout");
    assert!(matches!(err, networking::NetworkError::Timeout));

    server_handle.abort();
}
