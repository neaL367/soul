//! Bookmarks bar model and items for quick-access bookmark display.

use storage::BookmarkEntry;

/// A single bookmark item or folder displayed in the quick bookmarks bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkBarItem {
    /// Unique database ID.
    pub id: i64,
    /// Item title text.
    pub title: String,
    /// Item destination URL string.
    pub url: String,
    /// Whether this item represents a folder grouping.
    pub is_folder: bool,
}

/// State model managing the bookmarks bar beneath the navigation toolbar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookmarksBarModel {
    /// List of quick bookmark bar items.
    pub items: Vec<BookmarkBarItem>,
    /// Whether the bookmarks bar is currently visible in the UI.
    pub is_visible: bool,
}

impl BookmarksBarModel {
    /// Creates a new `BookmarksBarModel` with default visibility.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            is_visible: true,
        }
    }

    /// Updates the bar items from a slice of bookmark database entries.
    pub fn update_from_entries(&mut self, entries: &[BookmarkEntry]) {
        self.items = entries
            .iter()
            .map(|e| BookmarkBarItem {
                id: e.id,
                title: e.title.clone(),
                url: e.url.clone(),
                is_folder: e.folder.is_some(),
            })
            .collect();
    }

    /// Toggles the visibility of the bookmarks bar.
    pub const fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }
}
