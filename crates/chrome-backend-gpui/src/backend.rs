//! GPUI implementation of the `ChromeBackend` trait for Windows 11.

use browser_ui::{
    ChromeBackend, ChromeConfig, ChromeError, ChromeEvent, ViewportFrame, WindowId, WindowSpec,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// State of an active browser window in `GPUI` chrome.
#[derive(Debug, Clone)]
pub struct GpuiWindowState {
    /// Window configuration.
    pub spec: WindowSpec,
    /// Latest presented viewport frame.
    pub current_frame: Option<ViewportFrame>,
}

/// Boxed callback function for chrome event handling.
pub type EventHandlerCallback = Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>;

/// Thread-safe shared slot for the active event handler callback.
pub type SharedEventHandler = Arc<Mutex<Option<EventHandlerCallback>>>;

/// Concrete `ChromeBackend` implementation using `GPUI`.
pub struct GpuiChromeBackend {
    config: ChromeConfig,
    next_window_id: u64,
    windows: Arc<Mutex<HashMap<WindowId, GpuiWindowState>>>,
    event_handler: SharedEventHandler,
}

impl Default for GpuiChromeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuiChromeBackend {
    /// Creates a new uninitialized `GPUI` chrome backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ChromeConfig::default(),
            next_window_id: 1,
            windows: Arc::new(Mutex::new(HashMap::new())),
            event_handler: Arc::new(Mutex::new(None)),
        }
    }

    /// Emits a chrome event to the registered application event handler.
    pub fn emit_event(&self, event: ChromeEvent) {
        if let Some(ref handler) = *self.event_handler.lock().unwrap() {
            handler(event);
        }
    }
}

impl ChromeBackend for GpuiChromeBackend {
    fn init(&mut self, config: ChromeConfig) -> Result<(), ChromeError> {
        tracing::info!(app_name = %config.app_name, "Initializing GPUI chrome backend");
        self.config = config;
        Ok(())
    }

    fn open_window(&mut self, spec: WindowSpec) -> Result<WindowId, ChromeError> {
        let window_id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        tracing::info!(
            window_id = window_id.0,
            title = %spec.title,
            width = spec.width,
            height = spec.height,
            "Opening GPUI browser window"
        );

        let state = GpuiWindowState {
            spec,
            current_frame: None,
        };

        self.windows.lock().unwrap().insert(window_id, state);
        Ok(window_id)
    }

    fn close_window(&mut self, window_id: WindowId) -> Result<(), ChromeError> {
        let mut windows = self.windows.lock().unwrap();
        if windows.remove(&window_id).is_some() {
            tracing::info!(window_id = window_id.0, "Closing GPUI browser window");
            drop(windows);
            self.emit_event(ChromeEvent::WindowCloseRequested {
                window_id: window_id.0,
            });
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
        let mut windows = self.windows.lock().unwrap();
        if let Some(window_state) = windows.get_mut(&window_id) {
            window_state.current_frame = Some(frame);
            Ok(())
        } else {
            Err(ChromeError::WindowNotFound(window_id))
        }
    }

    fn set_event_handler(&mut self, handler: Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>) {
        *self.event_handler.lock().unwrap() = Some(handler);
    }

    fn run(self: Box<Self>) -> Result<(), ChromeError> {
        tracing::info!("GPUI application event loop running");
        Ok(())
    }
}
