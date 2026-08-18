//! End-to-end tests for the browser-side `NetworkClient` facade over both the
//! in-process transport and the named-pipe transport, against local servers.

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use networking::{HttpRequest, NetworkClient, NetworkClientConfig, NetworkError};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// Spawns a single-shot HTTP server; `handler` maps the raw request line to a response.
async fn spawn_mock_http_server<F>(handler: F) -> (SocketAddr, tokio::task::JoinHandle<()>)
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

/// Spawns a hyper server echoing the request method and body back in the response body.
async fn spawn_echo_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service = service_fn(|req: hyper::Request<hyper::body::Incoming>| async move {
            let method = req.method().to_string();
            let body = req.into_body().collect().await.unwrap().to_bytes();
            let echo = format!("{method}:{}", String::from_utf8_lossy(&body));
            Ok::<_, hyper::Error>(
                hyper::Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(Full::new(Bytes::from(echo)))
                    .unwrap(),
            )
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    (addr, handle)
}

/// Multi-connection server: serves `/start` with a 302 to `/final` and `/final`
/// with a 200 body carrying a `Set-Cookie` header.
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
                let body = "redirected final body";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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

/// Spawns a hyper server accepting many connections, each echoing its request path.
async fn spawn_path_echo_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let io = TokioIo::new(stream);
            let service = service_fn(|req| async move {
                let path = req.uri().path().to_string();
                Ok::<_, hyper::Error>(
                    hyper::Response::builder()
                        .status(200)
                        .header("content-type", "text/plain")
                        .body(Full::new(Bytes::from(format!("body:{path}"))))
                        .unwrap(),
                )
            });
            let conn = hyper::server::conn::http1::Builder::new().serve_connection(io, service);
            tokio::spawn(async move {
                let _ = conn.await;
            });
        }
    });

    (addr, handle)
}

/// Spawns a hyper server that delays its response by `delay`.
async fn spawn_slow_server(delay: Duration) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service = service_fn(|_req| async move {
            tokio::time::sleep(delay).await;
            Ok::<_, hyper::Error>(
                hyper::Response::builder()
                    .status(200)
                    .body(Full::new(Bytes::from("late body")))
                    .unwrap(),
            )
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    (addr, handle)
}

/// Connects a named-pipe client with retries so the test does not race the server.
async fn connect_pipe_with_retry(pipe_name: &str) -> Result<NetworkClient, NetworkError> {
    let config = NetworkClientConfig::default();
    let mut last_error = None;
    for _ in 0..20 {
        match NetworkClient::ipc_named_pipe(config.clone(), pipe_name).await {
            Ok(client) => return Ok(client),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(last_error.expect("retry loop always runs at least once"))
}

#[tokio::test]
async fn test_ipc_client_end_to_end_get() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        let body = "hello from ipc network client";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = NetworkClient::ipc_in_process().await;
    let url = Url::parse(&format!("http://{addr}/index.html")).unwrap();
    let response = client.fetch(&url).await.expect("ipc fetch failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.mime_type, "text/plain");
    assert_eq!(response.url, url);
    assert_eq!(response.text().unwrap(), "hello from ipc network client");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_ipc_client_post_body_roundtrip() {
    let (addr, server_handle) = spawn_echo_server().await;

    let client = NetworkClient::ipc_in_process().await;
    let url = Url::parse(&format!("http://{addr}/submit")).unwrap();
    let request = HttpRequest {
        url,
        method: networking::types::HttpMethod::Post,
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: Some("payload-bytes".to_string().into()),
    };

    let response = client
        .fetch_with_security_context(&request, None)
        .await
        .expect("ipc POST failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.text().unwrap(), "POST:payload-bytes");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_ipc_client_redirect_final_url_and_set_cookies() {
    let (addr, server_handle) = spawn_redirect_server().await;

    let client = NetworkClient::ipc_in_process().await;
    let url = Url::parse(&format!("http://{addr}/start")).unwrap();
    let response = client.fetch(&url).await.expect("ipc redirect failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.url.to_string(), format!("http://{addr}/final"));
    assert_eq!(response.set_cookies, vec!["session=abc; Path=/"]);
    assert_eq!(response.text().unwrap(), "redirected final body");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_ipc_client_security_blocks_mixed_content() {
    let (addr, _server_handle) = spawn_mock_http_server(|_req| "unreachable".to_string()).await;

    let client = NetworkClient::ipc_in_process().await;
    let doc_origin = Url::parse("https://example.com").unwrap();
    let url = Url::parse(&format!("http://{addr}/insecure.png")).unwrap();
    let request = HttpRequest::get(url);

    let err = client
        .fetch_with_security_context(&request, Some(&doc_origin))
        .await
        .expect_err("mixed content must be blocked");
    assert!(
        err.to_string().contains("Mixed content"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_ipc_client_cors_blocked_without_allow_header() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        let body = "cross origin data";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = NetworkClient::ipc_in_process().await;
    let doc_origin = Url::parse("http://example.com").unwrap();
    let url = Url::parse(&format!("http://{addr}/data")).unwrap();
    let request = HttpRequest::get(url);

    let err = client
        .fetch_with_security_context(&request, Some(&doc_origin))
        .await
        .expect_err("cross-origin response without CORS header must be blocked");
    assert!(err.to_string().contains("CORS"), "unexpected error: {err}");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_ipc_client_cors_allowed_with_allow_header() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        let body = "cross origin data";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let client = NetworkClient::ipc_in_process().await;
    let doc_origin = Url::parse("http://example.com").unwrap();
    let url = Url::parse(&format!("http://{addr}/data")).unwrap();
    let request = HttpRequest::get(url);

    let response = client
        .fetch_with_security_context(&request, Some(&doc_origin))
        .await
        .expect("CORS-allowed fetch must succeed");
    assert_eq!(response.text().unwrap(), "cross origin data");

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_ipc_client_concurrent_requests() {
    let (addr, server_handle) = spawn_path_echo_server().await;

    let client = NetworkClient::ipc_in_process().await;
    let url_one = Url::parse(&format!("http://{addr}/one")).unwrap();
    let url_two = Url::parse(&format!("http://{addr}/two")).unwrap();

    let (one, two) = tokio::join!(client.fetch(&url_one), client.fetch(&url_two));
    assert_eq!(one.unwrap().text().unwrap(), "body:/one");
    assert_eq!(two.unwrap().text().unwrap(), "body:/two");

    // The echo server loops forever; detach rather than await it.
    drop(server_handle);
}

#[tokio::test]
async fn test_ipc_client_timeout() {
    let (addr, _server_handle) = spawn_slow_server(Duration::from_secs(10)).await;

    let config = NetworkClientConfig {
        timeout: Duration::from_millis(200),
        ..NetworkClientConfig::default()
    };
    let client = NetworkClient::ipc_in_process_with_config(config).await;
    let url = Url::parse(&format!("http://{addr}/slow")).unwrap();

    let err = client
        .fetch(&url)
        .await
        .expect_err("slow response must exceed the client timeout");
    assert!(
        matches!(err, NetworkError::Timeout),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_named_pipe_client_end_to_end() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        let body = "hello over the named pipe";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await;

    let pipe_name = ipc::generate_pipe_name("net-client-e2e");
    let server_pipe = pipe_name.clone();
    tokio::spawn(async move {
        let service = networking::NetworkService::new();
        let _ = service.run_named_pipe(&server_pipe).await;
    });

    let client = connect_pipe_with_retry(&pipe_name)
        .await
        .expect("pipe connect failed");
    let url = Url::parse(&format!("http://{addr}/pipe")).unwrap();
    let response = client.fetch(&url).await.expect("pipe fetch failed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.text().unwrap(), "hello over the named pipe");

    let _ = server_handle.await;
}
