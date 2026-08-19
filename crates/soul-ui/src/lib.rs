//! Chrome backend trait, platform abstraction, input routing, and Chrome UI models.

pub mod backend;
pub mod bookmarks_bar;
pub mod event;
pub mod hit_test;
pub mod input;
pub mod input_router;
pub mod omnibox;
pub mod soul;
pub mod tab_strip;
pub mod toolbar;

pub use backend::{SoulBackend, SoulConfig, SoulError, ViewportFrame, WindowId, WindowSpec};
pub use bookmarks_bar::{BookmarkBarItem, BookmarksBarModel};
pub use event::SoulEvent;
pub use hit_test::{HitTestMap, HitTestRegion, HitTestTarget};
pub use input::{
    InputEvent, KeyModifiers, KeyPhase, KeyboardEvent, LogicalPosition, LogicalSize, MouseButton,
    MouseEvent, MousePhase, PhysicalPosition, PhysicalSize, WheelDeltaMode, WheelEvent,
};
pub use input_router::InputRouter;
pub use omnibox::{
    OmniboxEngine, OmniboxModel, OmniboxSuggestion, OmniboxSuggestionType, looks_like_url,
};
pub use soul::{SoulAction, SoulModel};
pub use soul_core::TabId;
pub use tab_strip::{TabItem, TabStripModel};
pub use toolbar::ToolbarModel;
