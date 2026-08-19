//! GPUI lifecycle implementation of Soul's backend boundary.

pub use crate::state::{EventHandlerCallback, SharedEventHandler, SoulBackendHandle};

use crate::state;
use crate::state::{lock_state, new_event_handler, new_state, window_state};
use crate::view::PageView;
use gpui::{
    App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use soul_ui::{SoulBackend, SoulConfig, SoulError, SoulEvent, ViewportFrame, WindowId, WindowSpec};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Concrete `SoulBackend` implementation using GPUI.
pub struct GpuiSoulBackend {
    next_window_id: u64,
    state: Arc<Mutex<state::BackendSharedState>>,
    event_handler: SharedEventHandler,
}

impl Default for GpuiSoulBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuiSoulBackend {
    /// Creates a new uninitialized GPUI Soul backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_window_id: 1,
            state: new_state(),
            event_handler: new_event_handler(),
        }
    }

    /// Returns a handle usable by navigation tasks after GPUI starts.
    #[must_use]
    pub fn shared_handle(&self) -> SoulBackendHandle {
        SoulBackendHandle {
            state: self.state.clone(),
        }
    }
}

impl SoulBackend for GpuiSoulBackend {
    fn init(&mut self, config: SoulConfig) -> Result<(), SoulError> {
        tracing::info!(app_name = %config.app_name, "Initializing GPUI Soul backend");
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn open_window(&mut self, spec: WindowSpec) -> Result<WindowId, SoulError> {
        let window_id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        tracing::info!(
            window_id = window_id.0,
            title = %spec.title,
            width = spec.width,
            height = spec.height,
            "Registering GPUI Soul window"
        );

        let mut state = lock_state(&self.state)?;
        state.windows.insert(window_id, window_state(spec));
        Ok(window_id)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn close_window(&mut self, window_id: WindowId) -> Result<(), SoulError> {
        let mut state = lock_state(&self.state)?;
        if state.windows.remove(&window_id).is_some() {
            tracing::info!(window_id = window_id.0, "Closing GPUI Soul window");
            drop(state);
            if let Ok(handler) = self.event_handler.lock()
                && let Some(handler) = handler.as_deref()
            {
                handler(SoulEvent::WindowCloseRequested {
                    window_id: window_id.0,
                });
            }
            Ok(())
        } else {
            Err(SoulError::WindowNotFound(window_id))
        }
    }

    #[allow(clippy::significant_drop_tightening)]
    fn update_viewport(
        &mut self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), SoulError> {
        self.shared_handle().update_viewport(window_id, frame)
    }

    fn set_event_handler(&mut self, handler: Box<dyn Fn(SoulEvent) + Send + Sync + 'static>) {
        match self.event_handler.lock() {
            Ok(mut guard) => *guard = Some(handler),
            // Recover from a poisoned lock rather than panicking: the handler
            // still matters more than a one-time panic on another thread.
            Err(poisoned) => *poisoned.into_inner() = Some(handler),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn run(self: Box<Self>) -> Result<(), SoulError> {
        let state = self.state.clone();
        let event_handler = self.event_handler.clone();
        let staged: Vec<(WindowId, String, u32, u32)> = lock_state(&self.state)?
            .windows
            .iter()
            .map(|(id, window)| (*id, window.title.clone(), window.width, window.height))
            .collect();

        tracing::info!(
            windows = staged.len(),
            "Launching GPUI Soul application loop"
        );

        gpui_platform::application().run(move |cx: &mut App| {
            let close_handler = event_handler.clone();
            // Map GPUI window ids (slotmap keys) to Soul window ids so a window
            // close event is reported for the window that actually closed, not
            // fanned out to every window.
            let id_map: Arc<Mutex<HashMap<u64, u64>>> = Arc::default();
            let close_ids = id_map.clone();
            let _ = cx.on_window_closed(move |_app, window_id| {
                let soul_window_id = close_ids
                    .lock()
                    .ok()
                    .and_then(|map| map.get(&window_id.as_u64()).copied());
                if let Some(soul_window_id) = soul_window_id
                    && let Ok(handler) = close_handler.lock()
                    && let Some(handler) = handler.as_deref()
                {
                    handler(SoulEvent::WindowCloseRequested {
                        window_id: soul_window_id,
                    });
                }
            });

            for (window_id, title, width, height) in staged {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(width as f32), px(height as f32)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from(title)),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    is_resizable: true,
                    window_min_size: Some(size(px(400.0), px(300.0))),
                    ..Default::default()
                };

                let view_state = state.clone();
                let view_handler = event_handler.clone();
                let window_id_map = id_map.clone();
                let _ = cx.open_window(options, move |window, cx| {
                    if let Ok(mut map) = window_id_map.lock() {
                        map.insert(window.window_handle().window_id().as_u64(), window_id.0);
                    }
                    cx.new(|cx| PageView::new(window_id, view_state, view_handler, cx))
                });
            }
        });

        Ok(())
    }
}
