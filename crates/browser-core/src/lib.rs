//! Tab, window, navigation, session, profile, and permission state machines.

pub mod navigation;
pub mod tab;

pub use navigation::{
    HistoryEntry, NavigationController, NavigationError, NavigationHistory, NavigationId,
    NavigationState,
};
pub use tab::{Tab, TabId, TabManager, TabTier};
