//! Input routing, coordinate transformation, and event dispatch logic.

use crate::backend::WindowId;
use crate::input::{
    InputEvent, KeyModifiers, KeyPhase, KeyboardEvent, LogicalPosition, MouseButton, MouseEvent,
    MousePhase, PhysicalPosition, WheelDeltaMode, WheelEvent,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Threshold duration for grouping consecutive mouse clicks into double/triple clicks.
pub const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);

/// Maximum pixel distance threshold for multi-click grouping.
pub const MULTI_CLICK_DISTANCE_THRESHOLD: f64 = 5.0;

/// Callback subscriber for routed input events.
pub type InputSubscriber = Box<dyn Fn(WindowId, &InputEvent) + Send + Sync>;

/// State tracker and event dispatcher for window input.
pub struct InputRouter {
    scale_factor: f64,
    cursor_position: LogicalPosition,
    active_modifiers: KeyModifiers,
    pressed_buttons: HashSet<MouseButton>,
    last_click_time: Option<Instant>,
    last_click_button: Option<MouseButton>,
    last_click_pos: Option<LogicalPosition>,
    click_count: u32,
    subscribers: Vec<InputSubscriber>,
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl InputRouter {
    /// Creates a new `InputRouter` with the given display scale factor.
    #[must_use]
    pub fn new(scale_factor: f64) -> Self {
        Self {
            scale_factor: if scale_factor > 0.0 {
                scale_factor
            } else {
                1.0
            },
            cursor_position: LogicalPosition::default(),
            active_modifiers: KeyModifiers::default(),
            pressed_buttons: HashSet::new(),
            last_click_time: None,
            last_click_button: None,
            last_click_pos: None,
            click_count: 0,
            subscribers: Vec::new(),
        }
    }

    /// Sets the device pixel ratio / scale factor for coordinate conversion.
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        if scale_factor > 0.0 {
            self.scale_factor = scale_factor;
        }
    }

    /// Returns the current scale factor.
    #[must_use]
    pub const fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Returns the current logical cursor position.
    #[must_use]
    pub const fn cursor_position(&self) -> LogicalPosition {
        self.cursor_position
    }

    /// Returns the currently active keyboard modifiers.
    #[must_use]
    pub const fn active_modifiers(&self) -> KeyModifiers {
        self.active_modifiers
    }

    /// Returns true if a specific mouse button is currently held down.
    #[must_use]
    pub fn is_button_pressed(&self, button: &MouseButton) -> bool {
        self.pressed_buttons.contains(button)
    }

    /// Registers a subscriber callback to receive routed input events.
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(WindowId, &InputEvent) + Send + Sync + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    /// Handles a raw physical mouse move and routes the resulting logical event.
    pub fn handle_mouse_move(&mut self, window_id: WindowId, physical_pos: PhysicalPosition) {
        let logical_pos = physical_pos.to_logical(self.scale_factor);
        self.cursor_position = logical_pos;

        let event = InputEvent::Mouse(MouseEvent {
            position: logical_pos,
            physical_position: physical_pos,
            button: None,
            phase: MousePhase::Move,
            click_count: 0,
            modifiers: self.active_modifiers,
        });

        self.dispatch(window_id, &event);
    }

    /// Handles a raw mouse button press or release.
    pub fn handle_mouse_button(
        &mut self,
        window_id: WindowId,
        button: MouseButton,
        phase: MousePhase,
        physical_pos: PhysicalPosition,
    ) {
        let logical_pos = physical_pos.to_logical(self.scale_factor);
        self.cursor_position = logical_pos;

        match phase {
            MousePhase::Down => {
                self.pressed_buttons.insert(button);
                let now = Instant::now();

                let is_sequential = self
                    .last_click_time
                    .is_some_and(|t| now.duration_since(t) <= MULTI_CLICK_INTERVAL)
                    && self.last_click_button == Some(button)
                    && self.last_click_pos.is_some_and(|p| {
                        (p.x - logical_pos.x).hypot(p.y - logical_pos.y)
                            <= MULTI_CLICK_DISTANCE_THRESHOLD
                    });

                if is_sequential {
                    self.click_count += 1;
                } else {
                    self.click_count = 1;
                }

                self.last_click_time = Some(now);
                self.last_click_button = Some(button);
                self.last_click_pos = Some(logical_pos);
            }
            MousePhase::Up => {
                self.pressed_buttons.remove(&button);
            }
            _ => {}
        }

        let event = InputEvent::Mouse(MouseEvent {
            position: logical_pos,
            physical_position: physical_pos,
            button: Some(button),
            phase,
            click_count: self.click_count,
            modifiers: self.active_modifiers,
        });

        self.dispatch(window_id, &event);
    }

    /// Handles a raw scroll/wheel event.
    pub fn handle_wheel(
        &mut self,
        window_id: WindowId,
        delta_x: f64,
        delta_y: f64,
        delta_mode: WheelDeltaMode,
    ) {
        let event = InputEvent::Wheel(WheelEvent {
            position: self.cursor_position,
            delta_x,
            delta_y,
            delta_mode,
            modifiers: self.active_modifiers,
        });

        self.dispatch(window_id, &event);
    }

    /// Handles a raw keyboard event and updates active modifier state.
    pub fn handle_key(
        &mut self,
        window_id: WindowId,
        key: String,
        code: String,
        phase: KeyPhase,
        modifiers: KeyModifiers,
        text: Option<String>,
    ) {
        self.active_modifiers = modifiers;

        let event = InputEvent::Keyboard(KeyboardEvent {
            key,
            code,
            phase,
            modifiers,
            text,
        });

        self.dispatch(window_id, &event);
    }

    fn dispatch(&self, window_id: WindowId, event: &InputEvent) {
        for subscriber in &self.subscribers {
            subscriber(window_id, event);
        }
    }
}
