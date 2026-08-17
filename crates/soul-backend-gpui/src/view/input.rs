//! Input conversion and routing handlers for `PageView`.

use super::PageView;
use gpui::{
    Context, KeyDownEvent, Modifiers, MouseButton as GpuiMouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, NavigationDirection, ScrollDelta, ScrollWheelEvent, Window,
};
use soul_ui::{
    HitTestTarget, KeyModifiers, KeyPhase, MouseButton, MousePhase, PhysicalPosition, SoulEvent,
    WheelDeltaMode,
};

impl PageView {
    /// Converts GPUI modifiers into Soul modifiers.
    pub(super) const fn modifiers(modifiers: Modifiers) -> KeyModifiers {
        KeyModifiers {
            shift: modifiers.shift,
            ctrl: modifiers.control,
            alt: modifiers.alt,
            meta: modifiers.platform,
        }
    }

    /// Converts GPUI button into Soul's backend-neutral button.
    pub(super) const fn mouse_button(button: GpuiMouseButton) -> MouseButton {
        match button {
            GpuiMouseButton::Left => MouseButton::Left,
            GpuiMouseButton::Right => MouseButton::Right,
            GpuiMouseButton::Middle => MouseButton::Middle,
            GpuiMouseButton::Navigate(NavigationDirection::Back) => MouseButton::Back,
            GpuiMouseButton::Navigate(NavigationDirection::Forward) => MouseButton::Forward,
        }
    }

    /// Routes GPUI mouse-down through `InputRouter`.
    pub(super) fn on_mouse_down(
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
    pub(super) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
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
    pub(super) fn hit_test(&self, x: f32, y: f32) -> Option<HitTestTarget> {
        let guard = self.state.lock().ok()?;
        guard.windows.get(&self.window_id).and_then(|window| {
            window
                .hit_test_map
                .hit_test(x, y + window.page_scroll_y)
                .cloned()
        })
    }

    /// Routes GPUI mouse movement through `InputRouter`.
    pub(super) fn on_mouse_move(
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
    pub(super) fn on_scroll_wheel(
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
    pub(super) fn route_key(&self, event: &KeyDownEvent) {
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
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
}
