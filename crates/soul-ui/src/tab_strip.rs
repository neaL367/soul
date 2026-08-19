//! Tab strip model, tab items, pinning, and reordering state management.

use soul_core::TabId;

/// Visual representation of a single browser tab in the tab strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    /// Unique tab identifier.
    pub id: TabId,
    /// Tab title text displayed in the tab strip.
    pub title: String,
    /// Whether this tab is currently the active focused tab.
    pub is_active: bool,
    /// Whether this tab is currently performing network navigation or loading.
    pub is_loading: bool,
    /// Whether this tab is pinned to the front of the tab strip.
    pub is_pinned: bool,
}

/// State model managing the collection of tabs in the browser chrome tab strip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabStripModel {
    tabs: Vec<TabItem>,
    active_tab_id: Option<TabId>,
}

impl TabStripModel {
    /// Creates a new empty `TabStripModel`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }

    /// Adds a new tab item to the tab strip.
    pub fn add_tab(&mut self, id: TabId, title: String, is_active: bool) {
        if is_active {
            for tab in &mut self.tabs {
                tab.is_active = false;
            }
            self.active_tab_id = Some(id);
        }

        self.tabs.push(TabItem {
            id,
            title,
            is_active,
            is_loading: false,
            is_pinned: false,
        });
    }

    /// Removes a tab by ID, activating an adjacent tab if the removed tab was active.
    pub fn remove_tab(&mut self, id: TabId) -> Option<TabId> {
        let index = self.tabs.iter().position(|t| t.id == id)?;
        let was_active = self.tabs[index].is_active;
        self.tabs.remove(index);

        if self.tabs.is_empty() {
            self.active_tab_id = None;
            return None;
        }

        if was_active {
            let new_active_index = if index >= self.tabs.len() {
                self.tabs.len() - 1
            } else {
                index
            };
            let new_active_id = self.tabs[new_active_index].id;
            self.select_tab(new_active_id);
            Some(new_active_id)
        } else {
            self.active_tab_id
        }
    }

    /// Focuses and activates a specific tab by ID.
    pub fn select_tab(&mut self, id: TabId) {
        let mut found = false;
        for tab in &mut self.tabs {
            if tab.id == id {
                tab.is_active = true;
                found = true;
            } else {
                tab.is_active = false;
            }
        }
        if found {
            self.active_tab_id = Some(id);
        }
    }

    /// Sets the loading state for a given tab.
    pub fn set_loading(&mut self, id: TabId, is_loading: bool) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.is_loading = is_loading;
        }
    }

    /// Updates the title of a tab.
    pub fn set_title(&mut self, id: TabId, title: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.title = title;
        }
    }

    /// Toggles the pinned status of a tab, keeping pinned tabs sorted before unpinned tabs.
    pub fn toggle_pinned(&mut self, id: TabId) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs[index].is_pinned = !self.tabs[index].is_pinned;
            self.sort_pinned_tabs();
        }
    }

    /// Reorders a tab from one index to another, preserving the invariant that
    /// pinned tabs always precede unpinned tabs.
    pub fn move_tab(&mut self, from_index: usize, to_index: usize) {
        if from_index >= self.tabs.len() || to_index >= self.tabs.len() || from_index == to_index {
            return;
        }
        let tab = self.tabs.remove(from_index);
        let mut target = to_index;
        let pinned_left = self.tabs.iter().filter(|t| t.is_pinned).count();
        if tab.is_pinned {
            // Keep a pinned tab inside the leading pinned run whenever another
            // pinned tab still anchors it; the sole pinned tab may go anywhere.
            if pinned_left > 0 {
                target = target.min(pinned_left - 1);
            }
        } else {
            // An unpinned tab must never be inserted before the pinned run.
            target = target.max(pinned_left);
        }
        self.tabs.insert(target.min(self.tabs.len()), tab);
    }

    /// Returns the currently active tab item if any.
    #[must_use]
    pub fn active_tab(&self) -> Option<&TabItem> {
        self.tabs.iter().find(|t| t.is_active)
    }

    /// Returns the active tab ID.
    #[must_use]
    pub const fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Returns a slice of all tab items.
    #[must_use]
    pub fn tabs(&self) -> &[TabItem] {
        &self.tabs
    }

    /// Returns the total number of tabs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Returns `true` if there are no tabs in the strip.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    fn sort_pinned_tabs(&mut self) {
        let mut pinned = Vec::new();
        let mut unpinned = Vec::new();
        for tab in self.tabs.drain(..) {
            if tab.is_pinned {
                pinned.push(tab);
            } else {
                unpinned.push(tab);
            }
        }
        pinned.append(&mut unpinned);
        self.tabs = pinned;
    }
}
