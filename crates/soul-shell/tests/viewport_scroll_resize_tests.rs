//! Integration tests for viewport operations: new tab page rendering, retained scrolling, and dynamic resizing.

mod test_helpers;

use self::test_helpers::{http_response, spawn_mock_http_server};
use soul_shell::engine::{RenderOptions, navigate_and_render};
use soul_shell::local_page::render_new_tab_frame;
use soul_ui::{SoulBackend, ViewportFrame};
use std::fmt::Write as _;
use url::Url;

/// Newly-created tabs use the same HTML → CSS → layout → paint → raster path
/// as the start page instead of presenting an uninitialized viewport.
#[test]
fn test_new_tab_page_renders_visible_pixels() {
    let frame = render_new_tab_frame(RenderOptions {
        width: 320,
        height: 240,
    })
    .expect("new tab page should render");

    let ViewportFrame::SoftwareRgba {
        width,
        height,
        pixels,
    } = frame
    else {
        panic!("new tab page must use the software-raster frame");
    };
    assert_eq!((width, height), (320, 240));
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel[3] > 0),
        "new tab page should contain opaque pixels"
    );
}

/// Window resize command updates the active viewport frame.
#[tokio::test]
async fn test_window_resize_command_updates_viewport() {
    let mut backend = soul_backend_gpui::GpuiSoulBackend::new();
    let handle = backend.shared_handle();
    let window_id = backend
        .open_window(soul_ui::WindowSpec {
            width: 320,
            height: 240,
            ..Default::default()
        })
        .expect("open window");

    let driver = soul_shell::navigation_driver::NavigationDriver::spawn(
        handle.clone(),
        window_id,
        RenderOptions {
            width: 320,
            height: 240,
        },
        None,
    );

    let mut initial_ok = false;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let state = handle.state.lock().unwrap();
        if let Some(win) = state.windows.get(&window_id)
            && win.frame.is_some()
        {
            initial_ok = true;
            break;
        }
    }
    assert!(initial_ok, "initial frame expected");

    driver
        .send(soul_shell::navigation_driver::NavigationCommand::Resize {
            width: 640,
            height: 480,
        })
        .unwrap();

    let mut resized_ok = false;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let state = handle.state.lock().unwrap();
        if let Some(win) = state.windows.get(&window_id)
            && win.frame.is_some()
        {
            resized_ok = true;
            break;
        }
    }
    assert!(resized_ok, "resized frame expected");
}

#[tokio::test]
async fn test_scroll_updates_retained_viewport_without_refetch() {
    let mut paragraphs = String::new();
    for i in 0..20 {
        let _ = write!(
            paragraphs,
            "<p style=\"margin: 16px 0;\">Paragraph {i} with enough text to create a tall document that exceeds viewport bounds.</p>"
        );
    }
    let tall_page = format!(
        "<!DOCTYPE html><html><body style=\"margin: 0; padding: 16px;\">{paragraphs}</body></html>"
    );

    let (addr, server_handle) = spawn_mock_http_server(move |_req| http_response(&tall_page)).await;
    let url = Url::parse(&format!("http://127.0.0.1:{}/scroll", addr.port())).unwrap();
    let options = RenderOptions {
        width: 320,
        height: 100,
    };

    let mut result = navigate_and_render(url, options)
        .await
        .expect("tall page should render");

    assert!(
        result.document_height > 100.0,
        "document height {} must exceed viewport height 100",
        result.document_height
    );
    assert!(result.scroll_y.abs() < 0.001);
    assert_eq!(result.pixel_buffer.height, 100);

    let first_viewport = result.pixel_buffer.data.clone();
    result.scroll_by(50.0, options.height);
    assert!((result.scroll_y - 50.0).abs() < 0.001);
    assert_eq!(result.pixel_buffer.height, 100);
    assert_ne!(result.pixel_buffer.data, first_viewport);
    result.scroll_by(10_000.0, options.height);
    assert!((result.scroll_y - (result.document_height - 100.0)).abs() < 0.001);
    result.scroll_by(-10_000.0, options.height);
    assert!(result.scroll_y.abs() < 0.001);

    let _ = server_handle.await;
}
