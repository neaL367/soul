//! Raw GPUI Soul window view: toolbar, omnibox, input routing, and page frame.

use crate::state::{BackendSharedState, SharedEventHandler};
use crate::toolbar::action_button;
use gpui::{
    App, Context, FocusHandle, ImageCacheError, ImageSource, InteractiveElement, IntoElement,
    KeyDownEvent, Modifiers, MouseButton as GpuiMouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, ParentElement, Render, RenderImage, ScrollDelta,
    ScrollWheelEvent, StatefulInteractiveElement, Styled, Task, Window, div, img, px, rgb,
};
use soul_ui::{
    HitTestTarget, InputRouter, KeyModifiers, KeyPhase, MouseButton, MousePhase, PhysicalPosition,
    SoulEvent, TabItem, TabStripModel, WheelDeltaMode, WindowId,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Live root view for one Soul native window.
pub struct PageView {
    window_id: WindowId,
    state: Arc<Mutex<BackendSharedState>>,
    event_handler: SharedEventHandler,
    input_router: Arc<Mutex<InputRouter>>,
    focus_handle: FocusHandle,
    omnibox: String,
    omnibox_focused: bool,
    last_window_metrics: Option<(u32, u32, f32)>,
    poll_task: Option<Task<()>>,
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
    fn emit_event(&self, event: SoulEvent) {
        if let Ok(handler) = self.event_handler.lock()
            && let Some(handler) = handler.as_deref()
        {
            handler(event);
        }
    }

    /// Converts GPUI modifiers into Soul modifiers.
    const fn modifiers(modifiers: Modifiers) -> KeyModifiers {
        KeyModifiers {
            shift: modifiers.shift,
            ctrl: modifiers.control,
            alt: modifiers.alt,
            meta: modifiers.platform,
        }
    }

    /// Converts GPUI button into Soul's backend-neutral button.
    const fn mouse_button(button: GpuiMouseButton) -> MouseButton {
        match button {
            GpuiMouseButton::Left => MouseButton::Left,
            GpuiMouseButton::Right => MouseButton::Right,
            GpuiMouseButton::Middle => MouseButton::Middle,
            GpuiMouseButton::Navigate(NavigationDirection::Back) => MouseButton::Back,
            GpuiMouseButton::Navigate(NavigationDirection::Forward) => MouseButton::Forward,
        }
    }

    /// Routes GPUI mouse-down through `InputRouter`.
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if let Ok(mut router) = self.input_router.lock() {
            router.handle_mouse_button(
                self.window_id,
                Self::mouse_button(event.button),
                MousePhase::Down,
                PhysicalPosition::new(f64::from(event.position.x), f64::from(event.position.y)),
            );
        }
    }

    /// Routes GPUI mouse-up through `InputRouter` and activates page links.
    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let x = f32::from(event.position.x);
        let y = f32::from(event.position.y);
        if let Ok(mut router) = self.input_router.lock() {
            router.handle_mouse_button(
                self.window_id,
                Self::mouse_button(event.button),
                MousePhase::Up,
                PhysicalPosition::new(f64::from(x), f64::from(y)),
            );
        }
        // Toolbar occupies first 44 logical pixels; translate remaining clicks
        // into page coordinates before hit-testing layout regions.
        if y > 44.0
            && let Some(HitTestTarget::Link(url)) = self.hit_test(x, y - 44.0)
        {
            self.emit_event(SoulEvent::LinkActivated {
                window_id: self.window_id.0,
                url,
            });
        }
    }

    /// Finds a page target at client coordinates.
    fn hit_test(&self, x: f32, y: f32) -> Option<HitTestTarget> {
        let guard = self.state.lock().ok()?;
        guard.windows.get(&self.window_id).and_then(|window| {
            window
                .hit_test_map
                .hit_test(x, y + window.page_scroll_y)
                .cloned()
        })
    }

    /// Routes GPUI mouse movement through `InputRouter`.
    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if let Ok(mut router) = self.input_router.lock() {
            router.handle_mouse_move(
                self.window_id,
                PhysicalPosition::new(f64::from(event.position.x), f64::from(event.position.y)),
            );
        }
    }

    /// Routes GPUI wheel input through `InputRouter`.
    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let (delta_x, delta_y, mode) = match event.delta {
            ScrollDelta::Pixels(point) => (
                f64::from(point.x),
                f64::from(point.y),
                WheelDeltaMode::Pixel,
            ),
            ScrollDelta::Lines(point) => {
                (f64::from(point.x), f64::from(point.y), WheelDeltaMode::Line)
            }
        };
        if let Ok(mut router) = self.input_router.lock() {
            router.handle_wheel(self.window_id, delta_x, delta_y, mode);
        }
    }

    /// Routes GPUI key input before applying omnibox editing behavior.
    fn route_key(&self, event: &KeyDownEvent) {
        if let Ok(mut router) = self.input_router.lock() {
            let phase = if event.is_held {
                KeyPhase::Repeat
            } else {
                KeyPhase::Down
            };
            let key = event.keystroke.key.clone();
            router.handle_key(
                self.window_id,
                key.clone(),
                key,
                phase,
                Self::modifiers(event.keystroke.modifiers),
                event.keystroke.key_char.clone(),
            );
        }
    }

    /// Handles raw keystrokes for the from-scratch omnibox.
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.route_key(event);
        if !self.omnibox_focused {
            return;
        }

        match event.keystroke.key.as_str() {
            "enter" => {
                self.emit_event(SoulEvent::OmniboxSubmitted {
                    window_id: self.window_id.0,
                    input: self.omnibox.clone(),
                });
                self.omnibox_focused = false;
            }
            "backspace" => {
                self.omnibox.pop();
            }
            "escape" => {
                self.omnibox_focused = false;
            }
            _ => {
                if let Some(character) = &event.keystroke.key_char {
                    self.omnibox.push_str(character);
                }
            }
        }
        cx.notify();
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

    /// Starts low-frequency polling so background navigation frames reach GPUI.
    fn start_frame_poll(&mut self, cx: &Context<Self>) {
        if self.poll_task.is_some() {
            return;
        }
        self.poll_task = Some(cx.spawn(async move |view, cx| {
            loop {
                if view.update(cx, |_view, cx| cx.notify()).is_err() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
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
                    .h(px(44.0))
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

impl PageView {
    fn tab_strip_element(&self, cx: &Context<Self>) -> impl IntoElement {
        let tab_items = self.current_tabs().tabs().to_vec();
        div()
            .w_full()
            .h(px(32.0))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(rgb(0x0018_1825))
            .children(
                tab_items
                    .into_iter()
                    .enumerate()
                    .map(|(tab_index, tab)| self.tab_element(tab_index, tab)),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x0045_475a))
                    .text_color(rgb(0x00cd_d6f4))
                    .cursor_pointer()
                    .id("new-tab")
                    .on_click(cx.listener(|this, _, _, _| {
                        this.emit_event(SoulEvent::NewTabRequested {
                            window_id: this.window_id.0,
                        });
                    }))
                    .child("+"),
            )
    }
}

impl PageView {
    fn tab_element(&self, tab_index: usize, tab: TabItem) -> impl IntoElement {
        let event_handler = self.event_handler.clone();
        let close_event_handler = self.event_handler.clone();
        let window_id = self.window_id.0;
        let background = if tab.is_active {
            0x0031_3244
        } else {
            0x0024_2636
        };
        div()
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(rgb(background))
            .text_color(rgb(0x00cd_d6f4))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_1()
            .id(format!("tab-{tab_index}"))
            .on_click(move |_, _, _| {
                if let Ok(handler) = event_handler.lock()
                    && let Some(handler) = handler.as_deref()
                {
                    handler(SoulEvent::TabSelected {
                        window_id,
                        tab_index,
                    });
                }
            })
            .child(if tab.is_loading {
                format!("{} …", tab.title)
            } else {
                tab.title
            })
            .child(
                div()
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .id(format!("close-tab-{tab_index}"))
                    .on_click(move |_, _, _| {
                        if let Ok(handler) = close_event_handler.lock()
                            && let Some(handler) = handler.as_deref()
                        {
                            handler(SoulEvent::TabCloseRequested {
                                window_id,
                                tab_index,
                            });
                        }
                    })
                    .child("×"),
            )
    }
}
