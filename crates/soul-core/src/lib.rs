//! Browser core state machines, tab management, navigation, and profiles.

pub mod navigation;
pub mod profile;
pub mod tab;

pub use navigation::{
    NavigationController, NavigationError, NavigationHistory, NavigationId, NavigationState,
};
pub use profile::BrowserProfile;
pub use tab::{PageScrollState, Tab, TabId, TabManager, TabTier};
