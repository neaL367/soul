//! Integration tests for the `ChromeBackend` trait contract and event dispatch.

use browser_ui::{
    ChromeBackend, ChromeConfig, ChromeError, ChromeEvent, ViewportFrame, WindowId, WindowSpec,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct MockChromeBackend {
    next_id: u64,
    windows: HashMap<WindowId, WindowSpec>,
    viewports: HashMap<WindowId, ViewportFrame>,
    event_handler: Option<Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>>,
}

impl MockChromeBackend {
    fn new() -> Self {
        Self {
            next_id: 1,
            windows: HashMap::new(),
            viewports: HashMap::new(),
            event_handler: None,
        }
    }
}

impl ChromeBackend for MockChromeBackend {
    fn init(&mut self, _config: ChromeConfig) -> Result<(), ChromeError> {
        Ok(())
    }

    fn open_window(&mut self, spec: WindowSpec) -> Result<WindowId, ChromeError> {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        self.windows.insert(id, spec);
        Ok(id)
    }

    fn close_window(&mut self, window_id: WindowId) -> Result<(), ChromeError> {
        if self.windows.remove(&window_id).is_some() {
            self.viewports.remove(&window_id);
            if let Some(ref handler) = self.event_handler {
                handler(ChromeEvent::WindowCloseRequested {
                    window_id: window_id.0,
                });
            }
            Ok(())
        } else {
            Err(ChromeError::WindowNotFound(window_id))
        }
    }

    fn update_viewport(
        &mut self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), ChromeError> {
        if !self.windows.contains_key(&window_id) {
            return Err(ChromeError::WindowNotFound(window_id));
        }
        self.viewports.insert(window_id, frame);
        Ok(())
    }

    fn set_event_handler(&mut self, handler: Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>) {
        self.event_handler = Some(handler);
    }

    fn run(self: Box<Self>) -> Result<(), ChromeError> {
        Ok(())
    }
}

#[test]
fn test_mock_backend_window_lifecycle() {
    let mut backend = MockChromeBackend::new();
    backend
        .init(ChromeConfig {
            app_name: "TestBrowser".to_string(),
            resource_dir: None,
        })
        .expect("init failed");

    let spec = WindowSpec {
        title: "Test Window".to_string(),
        width: 1024,
        height: 768,
        ..Default::default()
    };

    let window_id = backend.open_window(spec).expect("open_window failed");
    assert_eq!(window_id, WindowId(1));

    // Update viewport with software frame
    let frame = ViewportFrame::SoftwareRgba {
        width: 100,
        height: 100,
        pixels: vec![255; 100 * 100 * 4],
    };
    backend
        .update_viewport(window_id, frame)
        .expect("update_viewport failed");

    // Close window
    backend
        .close_window(window_id)
        .expect("close_window failed");

    // Verify closing again fails
    assert!(backend.close_window(window_id).is_err());
}

#[test]
fn test_mock_backend_event_handler() {
    let mut backend = MockChromeBackend::new();
    let received_events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&received_events);

    backend.set_event_handler(Box::new(move |event| {
        events_clone.lock().unwrap().push(event);
    }));

    let id = backend.open_window(WindowSpec::default()).unwrap();
    backend.close_window(id).unwrap();

    let events = received_events.lock().unwrap().clone();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        ChromeEvent::WindowCloseRequested { window_id: id.0 }
    );
}
