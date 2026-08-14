//! Navigation toolbar model (Back, Forward, Reload, Stop, Bookmark).

use soul_core::NavigationController;

/// State model managing the navigation buttons and toolbar state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ToolbarModel {
    /// Whether the back navigation button is enabled.
    pub can_go_back: bool,
    /// Whether the forward navigation button is enabled.
    pub can_go_forward: bool,
    /// Whether the active tab is loading (shows Stop instead of Reload).
    pub is_loading: bool,
    /// Whether the current page is saved in the bookmarks store.
    pub is_bookmarked: bool,
}

impl ToolbarModel {
    /// Creates a new `ToolbarModel` with default inactive states.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            can_go_back: false,
            can_go_forward: false,
            is_loading: false,
            is_bookmarked: false,
        }
    }

    /// Updates the toolbar buttons based on the active tab's navigation controller and bookmark state.
    pub fn update_from_controller(
        &mut self,
        controller: &NavigationController,
        is_bookmarked: bool,
    ) {
        self.can_go_back = controller.history().can_go_back();
        self.can_go_forward = controller.history().can_go_forward();
        self.is_loading = controller.state().is_loading();
        self.is_bookmarked = is_bookmarked;
    }

    /// Explicitly updates the loading state.
    pub const fn set_loading(&mut self, is_loading: bool) {
        self.is_loading = is_loading;
    }

    /// Explicitly updates the bookmark star state.
    pub const fn set_bookmarked(&mut self, is_bookmarked: bool) {
        self.is_bookmarked = is_bookmarked;
    }
}
