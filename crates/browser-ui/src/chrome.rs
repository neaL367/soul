//! Unified Chrome model aggregating tab strip, toolbar, omnibox, and bookmarks bar.

use crate::bookmarks_bar::BookmarksBarModel;
use crate::omnibox::{OmniboxEngine, OmniboxModel};
use crate::tab_strip::TabStripModel;
use crate::toolbar::ToolbarModel;
use browser_core::{TabId, TabManager};
use storage::{BookmarkEntry, HistoryEntry};

/// High-level user actions dispatched from the browser chrome UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChromeAction {
    /// Navigate active tab to a specific URL string.
    Navigate(String),
    /// Navigate active tab backward in history.
    Back,
    /// Navigate active tab forward in history.
    Forward,
    /// Reload current active tab page.
    Reload,
    /// Cancel current active tab network load.
    Stop,
    /// Open a new blank tab.
    NewTab,
    /// Close an existing tab by ID.
    CloseTab(TabId),
    /// Focus and switch to an existing tab by ID.
    SelectTab(TabId),
    /// Toggle pinned status for a tab by ID.
    TogglePinTab(TabId),
    /// Toggle bookmark star for current active page.
    ToggleBookmark,
    /// Toggle visibility of the bookmarks bar.
    ToggleBookmarksBar,
    /// User typed into the omnibox input field.
    OmniboxInput(String),
    /// User confirmed omnibox input (Enter key).
    OmniboxSubmit,
}

/// Aggregated state model orchestrating all browser chrome components.
#[derive(Debug, Clone, Default)]
pub struct ChromeModel {
    /// Tab strip state (tabs, active selection, pins).
    pub tab_strip: TabStripModel,
    /// Navigation toolbar buttons and loading state.
    pub toolbar: ToolbarModel,
    /// Omnibox input text and suggestion popup.
    pub omnibox: OmniboxModel,
    /// Quick bookmarks bar.
    pub bookmarks_bar: BookmarksBarModel,
    /// Omnibox autocompletion engine.
    pub omnibox_engine: OmniboxEngine,
}

impl ChromeModel {
    /// Creates a new `ChromeModel` with default sub-models.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab_strip: TabStripModel::new(),
            toolbar: ToolbarModel::new(),
            omnibox: OmniboxModel::new(),
            bookmarks_bar: BookmarksBarModel::new(),
            omnibox_engine: OmniboxEngine::new(None),
        }
    }

    /// Synchronizes the Chrome UI models with the `TabManager` state.
    pub fn sync_with_tab_manager(&mut self, manager: &TabManager, is_bookmarked: bool) {
        if let Some(active_tab) = manager.active_tab() {
            self.tab_strip.select_tab(active_tab.id);
            self.toolbar
                .update_from_controller(&active_tab.controller, is_bookmarked);

            if let Some(url) = active_tab.controller.state().current_url() {
                self.omnibox.set_text(url.to_string());
            }
        }
    }

    /// Handles an omnibox text change event, refreshing suggestions against history and bookmarks.
    pub fn handle_omnibox_input(
        &mut self,
        text: String,
        history: &[HistoryEntry],
        bookmarks: &[BookmarkEntry],
    ) {
        let suggestions = self
            .omnibox_engine
            .generate_suggestions(&text, history, bookmarks);
        self.omnibox.set_text(text);
        self.omnibox.set_suggestions(suggestions);
    }

    /// Evaluates the target navigation URL from current omnibox state upon Enter submission.
    #[must_use]
    pub fn resolve_omnibox_submission(&self) -> String {
        let target = self.omnibox.target_url();
        if target.starts_with("http://") || target.starts_with("https://") {
            target
        } else if !target.contains(' ') && (target.contains('.') || target.starts_with("localhost"))
        {
            format!("https://{target}")
        } else {
            let encoded = target.trim();
            format!("https://duckduckgo.com/?q={encoded}")
        }
    }
}
