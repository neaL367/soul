//! Integration tests for inline JavaScript, Web Storage, and `fetch()` DOM mutations.

mod test_helpers;

use self::test_helpers::{http_response, spawn_mock_http_server};
use soul_shell::engine::{RenderOptions, a11y_lines, navigate_and_render};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// Inline scripts execute before the document is styled, laid out, and painted.
#[tokio::test]
async fn test_inline_script_mutation_reaches_rendered_accessibility_tree() {
    let page = r#"<!DOCTYPE html>
    <html><body>
        <h1 id="message">Initial</h1>
        <script>
            document.getElementById("message").setTextContent("Updated by script");
        </script>
    </body></html>"#;
    let (addr, server_handle) = spawn_mock_http_server(move |_req| http_response(page)).await;
    let url = Url::parse(&format!("http://127.0.0.1:{}/scripted", addr.port())).unwrap();

    let result = navigate_and_render(
        url,
        RenderOptions {
            width: 320,
            height: 240,
        },
    )
    .await
    .expect("scripted page should render");
    let tree = result.a11y_tree.expect("a11y tree expected");
    let mut lines = Vec::new();
    a11y_lines(&tree, &mut lines);
    assert!(
        lines.iter().any(|line| line.contains("Updated by script")),
        "script mutation was not reflected in the rendered document: {lines:?}"
    );

    let _ = server_handle.await;
}

/// Inline scripts can use localStorage, sessionStorage, and `fetch()` to mutate the DOM.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_inline_script_web_storage_and_fetch() {
    let data_json = r#"{"msg":"from_fetch"}"#;
    let page = r#"<!DOCTYPE html>
    <html><body>
        <h1 id="status">Initial</h1>
        <script>
            localStorage.setItem("user", "Alice");
            sessionStorage.setItem("session", "Active");
            let u = localStorage.getItem("user");
            let s = sessionStorage.getItem("session");
            fetch("/data.json")
                .then(res => res.text())
                .then(text => {
                    document.getElementById("status").setTextContent(u + ":" + s + ":" + text);
                });
        </script>
    </body></html>"#;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let page_clone = page.to_string();

    let server_handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let response = if req_str.contains("GET /data.json ") {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    data_json.len(),
                    data_json
                )
            } else {
                http_response(&page_clone)
            };
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
            drop(socket);
        }
    });

    let url = Url::parse(&format!("http://127.0.0.1:{}/app", addr.port())).unwrap();
    let result = navigate_and_render(
        url,
        RenderOptions {
            width: 320,
            height: 240,
        },
    )
    .await
    .expect("app page should render");

    let tree = result.a11y_tree.expect("a11y tree expected");
    let mut lines = Vec::new();
    a11y_lines(&tree, &mut lines);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Alice:Active:{\"msg\":\"from_fetch\"}")),
        "storage and fetch mutations were not reflected in the rendered document: {lines:?}"
    );

    server_handle.abort();
    let _ = server_handle.await;
}

/// External `<script src="...">` files are fetched through the security-checked
/// path and execute in document order before layout.
#[tokio::test]
async fn test_external_script_executed_end_to_end() {
    let script_code =
        r#"document.getElementById("target").setTextContent("Loaded from external script");"#;
    let page_html = r#"<!DOCTYPE html>
    <html>
    <head>
        <script src="/scripts/app.js"></script>
    </head>
    <body>
        <h1 id="target">Original</h1>
    </body>
    </html>"#;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let script_copy = script_code.to_string();
    let page_copy = page_html.to_string();

    let server_handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 4096];
            let n = AsyncReadExt::read(&mut socket, &mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let response = if req_str.contains("GET /scripts/app.js ") {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    script_copy.len(),
                    script_copy
                )
            } else {
                http_response(&page_copy)
            };
            let _ = AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
            let _ = AsyncWriteExt::flush(&mut socket).await;
            drop(socket);
        }
    });

    let url = Url::parse(&format!("http://127.0.0.1:{}/external_js", addr.port())).unwrap();
    let result = navigate_and_render(
        url,
        RenderOptions {
            width: 320,
            height: 240,
        },
    )
    .await
    .expect("page with external JS should render");

    let tree = result.a11y_tree.expect("a11y tree expected");
    let mut lines = Vec::new();
    a11y_lines(&tree, &mut lines);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Loaded from external script")),
        "external script execution did not update the DOM: {lines:?}"
    );

    server_handle.abort();
}

/// Multiple `Set-Cookie` headers are extracted and parsed into RFC 6265bis Cookie records.
#[tokio::test]
async fn test_cookie_parse_and_storage_extraction() {
    let raw_cookies = [
        "session_id=xyz123; Path=/; HttpOnly; SameSite=Strict",
        "theme=dark; Path=/cookies; Max-Age=3600; SameSite=Lax",
    ];

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = AsyncReadExt::read(&mut socket, &mut buf).await;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nSet-Cookie: {}\r\nSet-Cookie: {}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
                raw_cookies[0], raw_cookies[1]
            );
            let _ = AsyncWriteExt::write_all(&mut socket, resp.as_bytes()).await;
            let _ = AsyncWriteExt::flush(&mut socket).await;
        }
    });

    let client = networking::HttpClient::default();
    let url = Url::parse(&format!("http://127.0.0.1:{}/cookies", addr.port())).unwrap();
    let response = client.fetch(&url).await.expect("fetch failed");

    assert_eq!(response.set_cookies.len(), 2);

    let db = storage::StorageDatabase::open_in_memory().unwrap();
    let jar = storage::CookieJar::new(db);

    for raw in &response.set_cookies {
        let cookie = storage::Cookie::parse(raw, &url).expect("valid cookie");
        jar.set_cookie(&cookie).expect("save cookie");
    }

    let active_cookies = jar.get_cookies_for_url(url.as_str(), 0).unwrap();
    assert!(!active_cookies.is_empty());
    assert!(active_cookies.iter().any(|c| c.name == "theme"));

    let _ = server_handle.await;
}
