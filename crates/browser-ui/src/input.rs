//! Input event models, coordinates, and keyboard/mouse state representations.

/// Position represented in device-independent logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalPosition {
    /// Horizontal coordinate in logical pixels.
    pub x: f64,
    /// Vertical coordinate in logical pixels.
    pub y: f64,
}

impl LogicalPosition {
    /// Creates a new logical position.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Position represented in physical device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicalPosition {
    /// Horizontal coordinate in physical pixels.
    pub x: f64,
    /// Vertical coordinate in physical pixels.
    pub y: f64,
}

impl PhysicalPosition {
    /// Creates a new physical position.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Converts physical position to logical position using a scale factor.
    #[must_use]
    pub fn to_logical(self, scale_factor: f64) -> LogicalPosition {
        if scale_factor <= 0.0 {
            LogicalPosition::new(self.x, self.y)
        } else {
            LogicalPosition::new(self.x / scale_factor, self.y / scale_factor)
        }
    }
}

/// Size represented in device-independent logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalSize {
    /// Width in logical pixels.
    pub width: f64,
    /// Height in logical pixels.
    pub height: f64,
}

impl LogicalSize {
    /// Creates a new logical size.
    #[must_use]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// Size represented in physical device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

impl PhysicalSize {
    /// Creates a new physical size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Keyboard modifier flags active during an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct KeyModifiers {
    /// Shift key active.
    pub shift: bool,
    /// Control key active.
    pub ctrl: bool,
    /// Alt key active.
    pub alt: bool,
    /// Meta / Windows key active.
    pub meta: bool,
}

impl KeyModifiers {
    /// Returns true if no modifier keys are active.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Primary (left) mouse button.
    Left,
    /// Secondary (right) mouse button.
    Right,
    /// Middle mouse button (wheel click).
    Middle,
    /// Back navigation button.
    Back,
    /// Forward navigation button.
    Forward,
    /// Other button identified by integer code.
    Other(u16),
}

/// Lifecycle phase of a mouse interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MousePhase {
    /// Mouse button pressed down.
    Down,
    /// Mouse button released.
    Up,
    /// Mouse cursor moved.
    Move,
    /// Mouse cursor entered window or element.
    Enter,
    /// Mouse cursor left window or element.
    Leave,
}

/// Structured mouse input event.
#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    /// Position of the mouse in logical coordinates.
    pub position: LogicalPosition,
    /// Physical position on the display.
    pub physical_position: PhysicalPosition,
    /// Button associated with the event (if any).
    pub button: Option<MouseButton>,
    /// Phase of the mouse action.
    pub phase: MousePhase,
    /// Number of sequential clicks (1 for single, 2 for double, 3 for triple).
    pub click_count: u32,
    /// Active keyboard modifiers.
    pub modifiers: KeyModifiers,
}

/// Unit mode for wheel scroll increments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelDeltaMode {
    /// Delta measured in logical pixels.
    #[default]
    Pixel,
    /// Delta measured in text lines.
    Line,
    /// Delta measured in view pages.
    Page,
}

/// Structured mouse wheel or trackpad scroll event.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelEvent {
    /// Cursor position where the scroll occurred.
    pub position: LogicalPosition,
    /// Horizontal scroll delta.
    pub delta_x: f64,
    /// Vertical scroll delta.
    pub delta_y: f64,
    /// Unit of the scroll delta.
    pub delta_mode: WheelDeltaMode,
    /// Active keyboard modifiers.
    pub modifiers: KeyModifiers,
}

/// Lifecycle phase of a keyboard key interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    /// Key pressed down.
    Down,
    /// Key released.
    Up,
    /// Key repeating due to being held down.
    Repeat,
}

/// Structured keyboard event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    /// Standard key identifier (e.g., "Enter", "a", "Escape").
    pub key: String,
    /// Physical key code (e.g., `KeyA`, `Digit1`).
    pub code: String,
    /// Action phase of the key.
    pub phase: KeyPhase,
    /// Active keyboard modifiers.
    pub modifiers: KeyModifiers,
    /// Printable text generated by the keypress if applicable.
    pub text: Option<String>,
}

/// Unified input event enum dispatched to windows and viewports.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Mouse movement or button event.
    Mouse(MouseEvent),
    /// Wheel or trackpad scrolling.
    Wheel(WheelEvent),
    /// Keyboard key event.
    Keyboard(KeyboardEvent),
}
