//! Raw GPUI Soul window view: toolbar, omnibox, input routing, and page frame.

mod input;
mod tabs;

use crate::layout::TOOLBAR_HEIGHT;
use crate::state::{BackendSharedState, SharedEventHandler};
use crate::toolbar::action_button;
use gpui::{
    App, Context, FocusHandle, ImageCacheError, ImageSource, InteractiveElement, IntoElement,
    ParentElement, Render, RenderImage, StatefulInteractiveElement, Styled, Task, Window, div, img,
    px, rgb,
};
use soul_ui::{InputRouter, SoulEvent, TabStripModel, WindowId};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Poll cadence for background frame pushes. No re-render is scheduled unless
/// the shared state generation counter changed since the last tick, so idle
/// windows spend the interval asleep instead of re-rendering every 100 ms.
const FRAME_POLL_INTERVAL: Duration = Duration::from_millis(33);

/// Live root view for one Soul native window.
pub struct PageView {
    pub(super) window_id: WindowId,
    pub(super) state: Arc<Mutex<BackendSharedState>>,
    pub(super) event_handler: SharedEventHandler,
    pub(super) input_router: Arc<Mutex<InputRouter>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) omnibox: String,
    pub(super) omnibox_focused: bool,
    pub(super) last_window_metrics: Option<(u32, u32, f32)>,
    pub(super) poll_task: Option<Task<()>>,
}

impl PageView {
    /// Creates a view and installs the input-router subscriber.
    pub fn new(
        window_id: WindowId,
        state: Arc<Mutex<BackendSharedState>>,
        event_handler: SharedEventHandler,
        cx: &Context<Self>,
    ) -> Self {
        let input_router = Arc::new(Mutex::new(InputRouter::default()));
        let input_handler = event_handler.clone();
        if let Ok(mut router) = input_router.lock() {
            router.subscribe(move |routed_window_id, event| {
                if let Ok(handler) = input_handler.lock()
                    && let Some(handler) = handler.as_deref()
                {
                    handler(SoulEvent::InputRouted {
                        window_id: routed_window_id.0,
                        event: event.clone(),
                    });
                }
            });
        }

        Self {
            window_id,
            state,
            event_handler,
            input_router,
            focus_handle: cx.focus_handle(),
            omnibox: String::new(),
            omnibox_focused: false,
            last_window_metrics: None,
            poll_task: None,
        }
    }

    /// Snapshot of latest staged page frame.
    fn current_frame(&self) -> Option<Arc<RenderImage>> {
        let guard = self.state.lock().ok()?;
        guard
            .windows
            .get(&self.window_id)
            .and_then(|window| window.frame.clone())
    }

    /// Snapshot of the current backend-neutral tab-strip model.
    fn current_tabs(&self) -> TabStripModel {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .windows
                    .get(&self.window_id)
                    .map(|w| w.tab_strip.clone())
            })
            .unwrap_or_default()
    }

    /// Emits a Soul event to browser-shell's event handler.
    pub(super) fn emit_event(&self, event: SoulEvent) {
        if let Ok(handler) = self.event_handler.lock()
            && let Some(handler) = handler.as_deref()
        {
            handler(event);
        }
    }

    /// Emits resize/DPI changes and updates input coordinate conversion.
    fn emit_window_metrics(&mut self, window: &Window) {
        let bounds = window.bounds();
        let width = u32::from(bounds.size.width).max(1);
        let height = u32::from(bounds.size.height).max(1);
        let scale_factor = window.scale_factor();
        let metrics = (width, height, scale_factor);
        if self.last_window_metrics == Some(metrics) {
            return;
        }
        self.last_window_metrics = Some(metrics);
        if let Ok(mut router) = self.input_router.lock() {
            router.set_scale_factor(f64::from(scale_factor));
        }
        self.emit_event(SoulEvent::WindowResized {
            window_id: self.window_id.0,
            width,
            height,
            scale_factor,
        });
    }

    /// Polls the shared state generation counter so frames pushed from a
    /// background navigation task reach GPUI. Only calls `cx.notify()` when the
    /// counter moved, so nothing is re-rendered while the window is idle.
    fn start_frame_poll(&mut self, cx: &Context<Self>) {
        if self.poll_task.is_some() {
            return;
        }
        self.poll_task = Some(cx.spawn(async move |view, cx| {
            let mut last_generation: Option<u64> = None;
            loop {
                if view
                    .update(cx, |this, cx| {
                        let generation = this.state.lock().ok().and_then(|state| {
                            state
                                .windows
                                .get(&this.window_id)
                                .map(|window| window.generation)
                        });
                        if last_generation != generation {
                            last_generation = generation;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(FRAME_POLL_INTERVAL).await;
            }
        }));
    }
}

impl Render for PageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.emit_window_metrics(window);
        self.start_frame_poll(cx);

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

        let omnibox_text = if self.omnibox.is_empty() {
            "Enter address".to_string()
        } else {
            self.omnibox.clone()
        };
        let window_id = self.window_id.0;
        div()
            .size_full()
            .flex_col()
            .capture_any_mouse_down(cx.listener(Self::on_mouse_down))
            .capture_any_mouse_up(cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .bg(rgb(0x001e_1e2e))
            .child(self.tab_strip_element(cx))
            .child(
                div()
                    .w_full()
                    .h(px(TOOLBAR_HEIGHT))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .flex()
                    .bg(rgb(0x0018_1825))
                    .child(action_button(
                        window_id,
                        self.event_handler.clone(),
                        "Back",
                        SoulEvent::NavigateBack { window_id },
                    ))
                    .child(action_button(
                        window_id,
                        self.event_handler.clone(),
                        "Forward",
                        SoulEvent::NavigateForward { window_id },
                    ))
                    .child(action_button(
                        window_id,
                        self.event_handler.clone(),
                        "Reload",
                        SoulEvent::Reload {
                            window_id,
                            bypass_cache: false,
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .h(px(30.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .rounded_md()
                            .bg(rgb(0x0031_3244))
                            .text_color(rgb(0x00cd_d6f4))
                            .id("omnibox")
                            .track_focus(&self.focus_handle)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.omnibox_focused = true;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(Self::on_key_down))
                            .child(omnibox_text),
                    )
                    .child(action_button(
                        window_id,
                        self.event_handler.clone(),
                        "New tab",
                        SoulEvent::NewTabRequested { window_id },
                    )),
            )
            .child(div().flex_1().size_full().child(img(source)))
    }
}
