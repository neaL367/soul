//! Real GPUI implementation of the `ChromeBackend` trait for Windows 11.
//!
//! The backend opens genuine native windows through GPUI and presents engine
//! frames (`ViewportFrame::SoftwareRgba`) as window content. GPUI windows can
//! only be created inside the GPUI application event loop, so `open_window` and
//! `update_viewport` stage state, and `run` launches the loop and materializes
//! the staged windows.

use browser_ui::{
    ChromeBackend, ChromeConfig, ChromeError, ChromeEvent, ViewportFrame, WindowId, WindowSpec,
};
use gpui::{
    App, AppContext, Bounds, Context, ImageCacheError, ImageSource, IntoElement, ParentElement,
    Render, RenderImage, SharedString, Styled, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, img, px, size,
};
use image::{Frame as ImageFrame, RgbaImage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Boxed callback function for chrome event handling.
pub type EventHandlerCallback = Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>;

/// Thread-safe shared slot for the active event handler callback.
pub type SharedEventHandler = Arc<Mutex<Option<EventHandlerCallback>>>;

/// Per-window state shared between the backend and the live GPUI view.
#[derive(Default)]
struct WindowSharedState {
    /// Window title shown in the native title bar.
    title: String,
    /// Initial window width in logical pixels.
    width: u32,
    /// Initial window height in logical pixels.
    height: u32,
    /// Latest engine frame converted to a GPU image, if any.
    frame: Option<Arc<RenderImage>>,
}

/// Backend-wide state shared with every live view.
#[derive(Default)]
struct BackendSharedState {
    windows: HashMap<WindowId, WindowSharedState>,
}

/// Concrete `ChromeBackend` implementation using `GPUI`.
pub struct GpuiChromeBackend {
    next_window_id: u64,
    state: Arc<Mutex<BackendSharedState>>,
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
            next_window_id: 1,
            state: Arc::new(Mutex::new(BackendSharedState::default())),
            event_handler: Arc::new(Mutex::new(None)),
        }
    }

    /// Converts an engine software frame into a GPU-resident `RenderImage`.
    fn frame_to_render_image(frame: ViewportFrame) -> Option<Arc<RenderImage>> {
        match frame {
            ViewportFrame::SoftwareRgba {
                width,
                height,
                pixels,
            } => {
                let rgba = RgbaImage::from_raw(width, height, pixels)?;
                Some(Arc::new(RenderImage::new(vec![ImageFrame::new(rgba)])))
            }
            // DXGI shared-handle frames are not presentable by the software backend.
            ViewportFrame::DxgiSharedHandle { .. } => None,
        }
    }

    /// Locks shared backend state, mapping lock poisoning to a backend error.
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, BackendSharedState>, ChromeError> {
        self.state
            .lock()
            .map_err(|_| ChromeError::Other("backend state lock poisoned".to_string()))
    }
}

impl ChromeBackend for GpuiChromeBackend {
    fn init(&mut self, config: ChromeConfig) -> Result<(), ChromeError> {
        tracing::info!(app_name = %config.app_name, "Initializing GPUI chrome backend");
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn open_window(&mut self, spec: WindowSpec) -> Result<WindowId, ChromeError> {
        let window_id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        tracing::info!(
            window_id = window_id.0,
            title = %spec.title,
            width = spec.width,
            height = spec.height,
            "Registering GPUI browser window"
        );

        let mut state = self.lock_state()?;
        state.windows.insert(
            window_id,
            WindowSharedState {
                title: spec.title,
                width: spec.width,
                height: spec.height,
                frame: None,
            },
        );
        Ok(window_id)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn close_window(&mut self, window_id: WindowId) -> Result<(), ChromeError> {
        let mut state = self.lock_state()?;
        if state.windows.remove(&window_id).is_some() {
            tracing::info!(window_id = window_id.0, "Closing GPUI browser window");
            drop(state);
            if let Some(handler) = self.event_handler.lock().ok().as_deref()
                && let Some(handler) = handler
            {
                handler(ChromeEvent::WindowCloseRequested {
                    window_id: window_id.0,
                });
            }
            Ok(())
        } else {
            Err(ChromeError::WindowNotFound(window_id))
        }
    }

    #[allow(clippy::significant_drop_tightening)]
    fn update_viewport(
        &mut self,
        window_id: WindowId,
        frame: ViewportFrame,
    ) -> Result<(), ChromeError> {
        let render_image = Self::frame_to_render_image(frame);

        let mut state = self.lock_state()?;
        let window = state
            .windows
            .get_mut(&window_id)
            .ok_or(ChromeError::WindowNotFound(window_id))?;
        window.frame = render_image;
        tracing::debug!(window_id = window_id.0, "Viewport frame staged");
        Ok(())
    }

    fn set_event_handler(&mut self, handler: Box<dyn Fn(ChromeEvent) + Send + Sync + 'static>) {
        *self.event_handler.lock().unwrap() = Some(handler);
    }

    #[allow(clippy::cast_precision_loss)]
    fn run(self: Box<Self>) -> Result<(), ChromeError> {
        let state = self.state.clone();
        let event_handler = self.event_handler.clone();

        // Snapshot staged window specs before entering the (blocking) GPUI loop.
        let staged: Vec<(WindowId, String, u32, u32)> = self
            .lock_state()?
            .windows
            .iter()
            .map(|(id, w)| (*id, w.title.clone(), w.width, w.height))
            .collect();

        tracing::info!(windows = staged.len(), "Launching GPUI application loop");

        gpui_platform::application().run(move |cx: &mut App| {
            // MVP: emit a close event when any window closes. The backend is
            // single-window today; a handle->WindowId map lands with M2 input routing.
            let close_handler = event_handler.clone();
            let close_ids: Vec<WindowId> = staged.iter().map(|(id, _, _, _)| *id).collect();
            let _ = cx.on_window_closed(move |_app, _window_id| {
                if let Some(handler) = close_handler.lock().ok().as_deref()
                    && let Some(handler) = handler
                {
                    for id in &close_ids {
                        handler(ChromeEvent::WindowCloseRequested { window_id: id.0 });
                    }
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
                let _ = cx.open_window(options, move |_window, cx| {
                    cx.new(move |_| PageView {
                        window_id,
                        state: view_state,
                    })
                });
            }
        });

        Ok(())
    }
}

/// Root view of a browser window: renders the latest engine frame full-window.
struct PageView {
    window_id: WindowId,
    state: Arc<Mutex<BackendSharedState>>,
}

impl PageView {
    /// Snapshot of the latest staged frame for this window.
    fn current_frame(&self) -> Option<Arc<RenderImage>> {
        let guard = self.state.lock().ok()?;
        guard
            .windows
            .get(&self.window_id)
            .and_then(|w| w.frame.clone())
    }
}

impl Render for PageView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let source: ImageSource = self.current_frame().map_or_else(
            || ImageSource::Custom(Arc::new(|_window: &mut Window, _app: &mut App| None)),
            |render_image| {
                ImageSource::Custom(Arc::new(move |_window: &mut Window, _app: &mut App| {
                    Some(Ok::<Arc<RenderImage>, ImageCacheError>(
                        render_image.clone(),
                    ))
                }))
            },
        );
        div().size_full().child(img(source))
    }
}
