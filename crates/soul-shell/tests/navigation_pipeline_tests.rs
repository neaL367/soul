//! End-to-end integration tests for the navigation rendering pipeline:
//! fetch → parse → style cascade → layout → paint → rasterization → accessibility.

mod test_helpers;

use self::test_helpers::{
    fixture_html, http_response, pixel_at, spawn_media_server, spawn_mock_http_server,
};
use soul_shell::engine::{
    A11yNode, A11yRole, RenderOptions, a11y_lines, has_visible_pixels, navigate_and_render,
    render_html_to_buffer,
};
use soul_ui::HitTestTarget;
use url::Url;

fn collect_roles(node: &A11yNode, roles: &mut Vec<A11yRole>) {
    roles.push(node.role);
    for child in &node.children {
        collect_roles(child, roles);
    }
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
    assert_eq!(result.title, "Fixture");
    assert!(result.navigation_id.0 > 0);
    assert!(has_visible_pixels(&result.pixel_buffer));
    assert!(
        result
            .hit_test_map
            .regions
            .iter()
            .any(|region| region.target == HitTestTarget::Link("/next".to_string()))
    );
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

/// Author `<style>` sheets must apply, and `<img>` subresources must be fetched
/// through the security-checked path, decoded, and rasterized.
#[tokio::test]
async fn test_styles_and_images_render_end_to_end() {
    common::init_tracing();
    let (addr, server_handle) = spawn_media_server().await;
    let url = Url::parse(&format!("http://127.0.0.1:{}/page", addr.port())).unwrap();
    let options = RenderOptions {
        width: 640,
        height: 480,
    };

    let result = navigate_and_render(url, options)
        .await
        .expect("navigation failed");
    let buffer = &result.pixel_buffer.data;
    let width = result.pixel_buffer.width;

    // Author <style> applied: body background is green (#00ff00) inside the body box.
    let bg = pixel_at(buffer, width, 10, 10);
    assert!(
        bg[1] > 200 && bg[0] < 100,
        "expected green background, got {bg:?}"
    );

    // The image was fetched, decoded, and drawn: red pixels exist somewhere.
    let has_red = buffer
        .chunks_exact(4)
        .any(|px| px[0] > 200 && px[1] < 100 && px[2] < 100);
    assert!(has_red, "no red image pixels found in frame");

    // Image role present in the accessibility tree.
    let tree = result.a11y_tree.expect("a11y tree expected");
    let mut roles = Vec::new();
    collect_roles(&tree, &mut roles);
    assert!(roles.contains(&A11yRole::Image));

    server_handle.abort();
}
