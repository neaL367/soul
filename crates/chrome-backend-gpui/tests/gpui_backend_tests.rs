//! Integration tests for `GpuiChromeBackend`.

use browser_ui::{
    ChromeBackend, ChromeConfig, ChromeError, ChromeEvent, ViewportFrame, WindowId, WindowSpec,
};
use chrome_backend_gpui::GpuiChromeBackend;
use std::sync::{Arc, Mutex};

#[test]
fn test_gpui_backend_lifecycle() {
    let mut backend = GpuiChromeBackend::new();
    backend
        .init(ChromeConfig {
            app_name: "Soul Browser Test".to_string(),
            resource_dir: None,
        })
        .expect("init failed");

    let spec = WindowSpec {
        title: "Soul Browser - Window 1".to_string(),
        width: 1280,
        height: 800,
        min_width: Some(400),
        min_height: Some(300),
        resizable: true,
        decorated: true,
    };

    let window_id = backend.open_window(spec).expect("open_window failed");
    assert_eq!(window_id, WindowId(1));

    let frame = ViewportFrame::SoftwareRgba {
        width: 800,
        height: 600,
        pixels: vec![0; 800 * 600 * 4],
    };
    backend
        .update_viewport(window_id, frame)
        .expect("update_viewport failed");

    backend
        .close_window(window_id)
        .expect("close_window failed");

    assert!(matches!(
        backend.close_window(window_id),
        Err(ChromeError::WindowNotFound(_))
    ));
}

#[test]
fn test_gpui_backend_event_emission() {
    let mut backend = GpuiChromeBackend::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    backend.set_event_handler(Box::new(move |event| {
        events_clone.lock().unwrap().push(event);
    }));

    let id = backend.open_window(WindowSpec::default()).unwrap();
    backend.close_window(id).unwrap();

    let emitted = events.lock().unwrap().clone();
    assert_eq!(emitted.len(), 1);
    assert_eq!(
        emitted[0],
        ChromeEvent::WindowCloseRequested { window_id: id.0 }
    );
}
