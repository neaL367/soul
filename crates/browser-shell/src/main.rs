//! Browser application entry point, wiring chrome, core state machines, and rendering engines together.

use browser_ui::{ChromeBackend, ChromeConfig, WindowSpec};
use chrome_backend_gpui::GpuiChromeBackend;

fn main() {
    common::init_tracing();
    tracing::info!("Soul Browser starting up (Milestone 1)...");

    let mut backend = Box::new(GpuiChromeBackend::new());

    if let Err(err) = backend.init(ChromeConfig {
        app_name: "Soul Browser".to_string(),
        resource_dir: None,
    }) {
        tracing::error!(%err, "Failed to initialize chrome backend");
        return;
    }

    backend.set_event_handler(Box::new(|event| {
        tracing::info!(?event, "Received chrome event");
    }));

    let window_spec = WindowSpec {
        title: "Soul Browser - New Tab".to_string(),
        width: 1280,
        height: 800,
        min_width: Some(400),
        min_height: Some(300),
        resizable: true,
        decorated: true,
    };

    match backend.open_window(window_spec) {
        Ok(window_id) => {
            tracing::info!(window_id = window_id.0, "Initial browser window opened");
        }
        Err(err) => {
            tracing::error!(%err, "Failed to open initial browser window");
            return;
        }
    }

    if let Err(err) = backend.run() {
        tracing::error!(%err, "Chrome backend runtime error");
    }
}
