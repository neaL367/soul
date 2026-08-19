//! Shared state and frame bridge between Soul backend lifecycle and GPUI views.

use gpui::RenderImage;
use image::{Frame as ImageFrame, RgbaImage};
use soul_ui::{
    HitTestMap, SoulError, SoulEvent, TabStripModel, ViewportFrame, WindowId, WindowSpec,
};
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
    pub(crate) title: String,
    /// Initial window width in logical pixels.
    pub(crate) width: u32,
    /// Initial window height in logical pixels.
    pub(crate) height: u32,
    /// Latest engine frame converted to a GPUI image.
    pub(crate) frame: Option<Arc<RenderImage>>,
    /// Interactive page regions associated with the latest frame.
    pub(crate) hit_test_map: HitTestMap,
    /// Current document-space page scroll offset.
    pub(crate) page_scroll_y: f32,
    /// Backend-neutral tab-strip snapshot rendered by the GPUI chrome.
    pub(crate) tab_strip: TabStripModel,
    /// Monotonic bump counter signaling a frame/state change to a live view.
    pub(crate) generation: u64,
}

/// Backend-wide state shared with every live view.
#[derive(Default)]
pub struct BackendSharedState {
    pub(crate) windows: HashMap<WindowId, WindowSharedState>,
}

/// Thread-safe handle for pushing frames into a live GPUI window after `run()`.
#[derive(Clone)]
pub struct SoulBackendHandle {
    pub(crate) state: Arc<Mutex<BackendSharedState>>,
}

/// Marks a window's shared state as changed so a live view can re-render.
const fn touch(window: &mut WindowSharedState) {
    window.generation = window.generation.wrapping_add(1);
}

impl SoulBackendHandle {
    /// Updates a live window frame without mutable backend access.
    ///
    /// # Errors
    ///
    /// Returns `SoulError` when target window is missing, the frame cannot be
    /// converted for presentation, or the state lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_viewport(
        &self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), SoulError> {
        let render_image = frame_to_render_image(frame)?;
        let mut state = lock_state(&self.state)?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.frame = Some(render_image);
        touch(window);
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
        let mut state = lock_state(&self.state)?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.hit_test_map = hit_test_map;
        touch(window);
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
        let render_image = frame_to_render_image(frame)?;
        let mut state = lock_state(&self.state)?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.frame = Some(render_image);
        window.hit_test_map = hit_test_map;
        window.page_scroll_y = page_scroll_y.max(0.0);
        touch(window);
        Ok(())
    }

    /// Replaces the tab-strip snapshot shown by a live window.
    ///
    /// # Errors
    ///
    /// Returns `SoulError` when target window is missing or state lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_tab_strip(
        &self,
        window_id: WindowId,
        tab_strip: TabStripModel,
    ) -> Result<(), SoulError> {
        let mut state = lock_state(&self.state)?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.tab_strip = tab_strip;
        touch(window);
        Ok(())
    }

    /// Clears the current page frame and interaction state for a blank tab.
    ///
    /// # Errors
    ///
    /// Returns `SoulError` when target window is missing or state lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn clear_page_state(&self, window_id: WindowId) -> Result<(), SoulError> {
        let mut state = lock_state(&self.state)?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(SoulError::WindowNotFound(window_id))?;
        window.frame = None;
        window.hit_test_map = HitTestMap::default();
        window.page_scroll_y = 0.0;
        touch(window);
        Ok(())
    }

    /// Returns whether `window_id` currently holds a rendered frame.
    #[must_use]
    pub fn has_frame(&self, window_id: WindowId) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state
            .windows
            .get(&window_id)
            .is_some_and(|window| window.frame.is_some())
    }
}

/// Converts an engine software frame into a GPU-resident GPUI image.
///
/// # Errors
///
/// Returns `SoulError::PresentationFailed` when the frame uses an unsupported
/// transport or its pixel buffer does not match the declared dimensions.
pub fn frame_to_render_image(frame: ViewportFrame) -> Result<Arc<RenderImage>, SoulError> {
    match frame {
        ViewportFrame::SoftwareRgba {
            width,
            height,
            pixels,
        } => {
            let rgba = RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
                SoulError::PresentationFailed(format!(
                    "software frame {width}x{height} does not match pixel buffer length"
                ))
            })?;
            Ok(Arc::new(RenderImage::new(vec![ImageFrame::new(rgba)])))
        }
        // DXGI frames require a separate native interop path.
        ViewportFrame::DxgiSharedHandle { .. } => Err(SoulError::PresentationFailed(
            "DXGI shared-handle frames are not supported by the GPUI software path".to_string(),
        )),
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
        tab_strip: initial_tab_strip(),
        generation: 0,
    }
}

fn initial_tab_strip() -> TabStripModel {
    let mut tabs = TabStripModel::new();
    tabs.add_tab(soul_ui::TabId(1), "New Tab".to_string(), true);
    tabs
}
