//! End-to-end integration tests: local HTTP server → navigation state machine →
//! security-checked fetch → parse → style → layout → paint → raster → a11y tree.

use browser_shell::engine::{
    A11yRole, RenderOptions, a11y_lines, has_visible_pixels, navigate_and_render,
    render_html_to_buffer,
};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// Spawns a single-shot mock HTTP server responding to one request.
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

const fn fixture_html() -> &'static str {
    r#"<!DOCTYPE html>
    <html>
    <head><title>Fixture</title></head>
    <body style="background-color: #ffffff;">
        <h1 aria-label="Main Heading">Page Title</h1>
        <p>Introductory paragraph</p>
        <button aria-label="Submit Button">Click</button>
        <img src="missing.png" aria-label="Hero Image">
    </body>
    </html>"#
}

fn http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// The full connected path: real HTTP fetch over a local socket, then the whole
/// rendering pipeline, ending in a rasterized frame and an accessibility tree.
#[tokio::test]
async fn test_end_to_end_navigation_and_render() {
    let (addr, server_handle) = spawn_mock_http_server(|req| {
        assert!(req.contains("GET /index.html HTTP/1.1"));
        http_response(fixture_html())
    })
    .await;

    let url = Url::parse(&format!("http://127.0.0.1:{}/index.html", addr.port())).unwrap();
    let options = RenderOptions {
        width: 640,
        height: 480,
    };

    let result = navigate_and_render(url.clone(), options)
        .await
        .expect("end-to-end navigation failed");

    assert_eq!(result.status_code, 200);
    assert_eq!(result.url, url);
    assert!(result.navigation_id.0 > 0);
    assert!(has_visible_pixels(&result.pixel_buffer));
    assert_eq!(
        (result.pixel_buffer.width, result.pixel_buffer.height),
        (640, 480)
    );

    // Accessibility tree carries semantic roles from the laid-out document.
    let tree = result.a11y_tree.expect("a11y tree expected");
    assert_eq!(tree.role, A11yRole::Document);
    let mut roles = Vec::new();
    collect_roles(&tree, &mut roles);
    for expected in [
        A11yRole::Heading,
        A11yRole::Paragraph,
        A11yRole::Button,
        A11yRole::Image,
    ] {
        assert!(
            roles.contains(&expected),
            "missing role {expected:?} in {roles:?}"
        );
    }

    // Accessibility lines are human-readable for dump output.
    let mut lines = Vec::new();
    a11y_lines(&tree, &mut lines);
    assert!(lines.len() >= 4);
    assert!(lines.iter().any(|l| l.contains("Heading")));

    let _ = server_handle.await;
}

/// Rasterized frames encode to valid PNG (screenshot/golden-test foundation).
#[tokio::test]
async fn test_render_result_encodes_png() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| http_response(fixture_html())).await;
    let url = Url::parse(&format!("http://127.0.0.1:{}/", addr.port())).unwrap();

    let result = navigate_and_render(url, RenderOptions::default())
        .await
        .expect("navigation failed");

    let png = result.encode_png().expect("PNG encode failed");
    assert!(png.len() > 100, "PNG too small: {} bytes", png.len());
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let _ = server_handle.await;
}

/// Non-2xx responses fail the navigation with the HTTP status surfaced.
#[tokio::test]
async fn test_http_error_status_fails_navigation() {
    let (addr, server_handle) = spawn_mock_http_server(|_req| {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    })
    .await;

    let url = Url::parse(&format!("http://127.0.0.1:{}/missing", addr.port())).unwrap();
    let err = navigate_and_render(url, RenderOptions::default())
        .await
        .expect_err("404 must fail navigation");
    assert!(err.to_string().contains("404"), "unexpected error: {err}");

    let _ = server_handle.await;
}

/// In-memory rendering path (start page) also produces visible pixels and roles.
#[test]
fn test_render_html_to_buffer_inline_styles() {
    let options = RenderOptions {
        width: 320,
        height: 240,
    };
    let (buffer, tree, timings) =
        render_html_to_buffer(fixture_html(), options).expect("render failed");

    assert!(has_visible_pixels(&buffer));
    assert!(timings.total() > std::time::Duration::ZERO);
    assert!(tree.is_some());
}

fn collect_roles(node: &browser_shell::engine::A11yNode, roles: &mut Vec<A11yRole>) {
    roles.push(node.role);
    for child in &node.children {
        collect_roles(child, roles);
    }
}
