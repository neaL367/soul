//! Chrome backend trait, platform abstraction, input routing, and Chrome UI models.

pub mod backend;
pub mod bookmarks_bar;
pub mod chrome;
pub mod event;
pub mod input;
pub mod input_router;
pub mod omnibox;
pub mod tab_strip;
pub mod toolbar;

pub use backend::{ChromeBackend, ChromeConfig, ChromeError, ViewportFrame, WindowId, WindowSpec};
pub use bookmarks_bar::{BookmarkBarItem, BookmarksBarModel};
pub use chrome::{ChromeAction, ChromeModel};
pub use event::ChromeEvent;
pub use input::{
    InputEvent, KeyModifiers, KeyPhase, KeyboardEvent, LogicalPosition, LogicalSize, MouseButton,
    MouseEvent, MousePhase, PhysicalPosition, PhysicalSize, WheelDeltaMode, WheelEvent,
};
pub use input_router::InputRouter;
pub use omnibox::{OmniboxEngine, OmniboxModel, OmniboxSuggestion, OmniboxSuggestionType};
pub use tab_strip::{TabItem, TabStripModel};
pub use toolbar::ToolbarModel;
