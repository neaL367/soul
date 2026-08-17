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
