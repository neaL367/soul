//! `ChromeBackend` trait and backend-agnostic view logic for tabs, omnibox, toolbar, menus, settings, and downloads UI.

pub mod backend;
pub mod event;
pub mod input;
pub mod input_router;

pub use backend::{ChromeBackend, ChromeConfig, ChromeError, ViewportFrame, WindowId, WindowSpec};
pub use event::ChromeEvent;
pub use input::{
    InputEvent, KeyModifiers, KeyPhase, KeyboardEvent, LogicalPosition, LogicalSize, MouseButton,
    MouseEvent, MousePhase, PhysicalPosition, PhysicalSize, WheelDeltaMode, WheelEvent,
};
pub use input_router::InputRouter;
