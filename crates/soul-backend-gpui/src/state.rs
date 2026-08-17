//! Shared state and frame bridge between Soul backend lifecycle and GPUI views.

use gpui::RenderImage;
use image::{Frame as ImageFrame, RgbaImage};
use soul_ui::{HitTestMap, SoulError, SoulEvent, ViewportFrame, WindowId, WindowSpec};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Boxed callback function for Soul UI events.
pub type EventHandlerCallback = Box<dyn Fn(SoulEvent) + Send + Sync + 'static>;

/// Thread-safe shared slot for the active event handler callback.
pub type SharedEventHandler = Arc<Mutex<Option<EventHandlerCallback>>>;

/// Per-window state shared between backend lifecycle and live GPUI view.
#[derive(Default)]
pub struct WindowSharedState {
    /// Window title shown in native title bar.
    pub title: String,
    /// Initial window width in logical pixels.
    pub width: u32,
    /// Initial window height in logical pixels.
    pub height: u32,
    /// Latest engine frame converted to a GPUI image.
    pub frame: Option<Arc<RenderImage>>,
    /// Interactive page regions associated with the latest frame.
    pub hit_test_map: HitTestMap,
    /// Current document-space page scroll offset.
    pub page_scroll_y: f32,
}

/// Backend-wide state shared with every live view.
#[derive(Default)]
pub struct BackendSharedState {
    pub windows: HashMap<WindowId, WindowSharedState>,
}

/// Thread-safe handle for pushing frames into a live GPUI window after `run()`.
#[derive(Clone)]
pub struct SoulBackendHandle {
    /// Shared state used by live navigation tasks to push frames.
    pub state: Arc<Mutex<BackendSharedState>>,
}

impl SoulBackendHandle {
    /// Updates a live window frame without mutable backend access.
    ///
    /// # Errors
    ///
    /// Returns `SoulError` when target window is missing or state lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_viewport(
        &self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), SoulError> {
        let render_image = frame_to_render_image(frame);
        let mut state = self
            .state
            .lock()
            .map_err(|_| SoulError::Other("backend state lock poisoned".to_string()))?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.frame = render_image;
        Ok(())
    }

    /// Replaces hit-test regions associated with the current frame.
    ///
    /// # Errors
    ///
    /// Returns `SoulError` when target window is missing or state lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_hit_test_map(
        &self,
        window_id: WindowId,
        hit_test_map: HitTestMap,
    ) -> Result<(), SoulError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SoulError::Other("backend state lock poisoned".to_string()))?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.hit_test_map = hit_test_map;
        Ok(())
    }

    /// Atomically publishes a frame, hit-test map, and scroll offset.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_page_state(
        &self,
        window_id: WindowId,
        frame: ViewportFrame,
        hit_test_map: HitTestMap,
        page_scroll_y: f32,
    ) -> Result<(), SoulError> {
        let render_image = frame_to_render_image(frame);
        let mut state = self
            .state
            .lock()
            .map_err(|_| SoulError::Other("backend state lock poisoned".to_string()))?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.frame = render_image;
        window.hit_test_map = hit_test_map;
        window.page_scroll_y = page_scroll_y.max(0.0);
        Ok(())
    }
}

/// Converts an engine software frame into a GPU-resident GPUI image.
pub fn frame_to_render_image(frame: ViewportFrame) -> Option<Arc<RenderImage>> {
    match frame {
        ViewportFrame::SoftwareRgba {
            width,
            height,
            pixels,
        } => {
            let rgba = RgbaImage::from_raw(width, height, pixels)?;
            Some(Arc::new(RenderImage::new(vec![ImageFrame::new(rgba)])))
        }
        // DXGI frames require a separate native interop path.
        ViewportFrame::DxgiSharedHandle { .. } => None,
    }
}

/// Locks shared backend state, mapping poisoning to a backend error.
pub fn lock_state(
    state: &Arc<Mutex<BackendSharedState>>,
) -> Result<std::sync::MutexGuard<'_, BackendSharedState>, SoulError> {
    state
        .lock()
        .map_err(|_| SoulError::Other("backend state lock poisoned".to_string()))
}

/// Creates shared backend state.
pub fn new_state() -> Arc<Mutex<BackendSharedState>> {
    Arc::new(Mutex::new(BackendSharedState::default()))
}

/// Creates a shared event-handler slot.
pub fn new_event_handler() -> SharedEventHandler {
    Arc::new(Mutex::new(None))
}

/// Converts a window specification into shared view state.
pub fn window_state(spec: WindowSpec) -> WindowSharedState {
    WindowSharedState {
        title: spec.title,
        width: spec.width,
        height: spec.height,
        frame: None,
        hit_test_map: HitTestMap::default(),
        page_scroll_y: 0.0,
    }
}
