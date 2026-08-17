//! Raw GPUI Soul window view: toolbar, omnibox, input routing, and page frame.

use crate::state::{BackendSharedState, SharedEventHandler};
use gpui::{
    App, Context, FocusHandle, ImageCacheError, ImageSource, InteractiveElement, IntoElement,
    KeyDownEvent, Modifiers, MouseButton as GpuiMouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, ParentElement, Render, RenderImage, ScrollDelta,
    ScrollWheelEvent, StatefulInteractiveElement, Styled, Task, Window, div, img, px, rgb,
};
use soul_ui::{
    InputRouter, KeyModifiers, KeyPhase, MouseButton, MousePhase, PhysicalPosition, SoulEvent,
    WheelDeltaMode, WindowId,
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

    /// Routes GPUI mouse-up through `InputRouter`.
    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Ok(mut router) = self.input_router.lock() {
            router.handle_mouse_button(
                self.window_id,
                Self::mouse_button(event.button),
                MousePhase::Up,
                PhysicalPosition::new(f64::from(event.position.x), f64::from(event.position.y)),
            );
        }
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

    /// Builds a raw GPUI button for Soul toolbar actions.
    fn action_button(
        window_id: u64,
        event_handler: SharedEventHandler,
        label: &'static str,
        event: SoulEvent,
    ) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(0x0045_475a))
            .text_color(rgb(0x00cd_d6f4))
            .cursor_pointer()
            .id(label)
            .on_click(move |_, _, _| {
                if let Ok(handler) = event_handler.lock()
                    && let Some(handler) = handler.as_deref()
                {
                    let event = match event.clone() {
                        SoulEvent::NavigateBack { .. } => SoulEvent::NavigateBack { window_id },
                        SoulEvent::NavigateForward { .. } => {
                            SoulEvent::NavigateForward { window_id }
                        }
                        SoulEvent::Reload { bypass_cache, .. } => SoulEvent::Reload {
                            window_id,
                            bypass_cache,
                        },
                        other => other,
                    };
                    handler(event);
                }
            })
            .child(label)
    }
}

impl Render for PageView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(
                div()
                    .w_full()
                    .h(px(44.0))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .flex()
                    .bg(rgb(0x0018_1825))
                    .child(Self::action_button(
                        window_id,
                        self.event_handler.clone(),
                        "Back",
                        SoulEvent::NavigateBack { window_id },
                    ))
                    .child(Self::action_button(
                        window_id,
                        self.event_handler.clone(),
                        "Forward",
                        SoulEvent::NavigateForward { window_id },
                    ))
                    .child(Self::action_button(
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
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x0089_b4fa))
                            .text_color(rgb(0x001e_1e2e))
                            .child("New tab"),
                    ),
            )
            .child(div().flex_1().size_full().child(img(source)))
    }
}
