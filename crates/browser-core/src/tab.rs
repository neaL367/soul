//! Tab models, tiered lifecycle management, and collection orchestration.

use crate::navigation::NavigationController;

/// Unique identifier for a browser tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);

/// Memory and execution tier of a browser tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabTier {
    /// Actively focused and rendering.
    #[default]
    Active,
    /// Background tab with throttled timers and reduced resources.
    Background,
    /// Frozen tab with discarded render buffers and suspended scripts.
    Frozen,
}

/// Represents an individual tab inside the browser window.
pub struct Tab {
    /// Unique tab ID.
    pub id: TabId,
    /// Navigation controller managing this tab's state machine.
    pub controller: NavigationController,
    /// Tab title displayed in the tab strip.
    pub title: String,
    /// Current execution and resource tier.
    pub tier: TabTier,
}

impl Tab {
    /// Creates a new tab with default state.
    #[must_use]
    pub fn new(id: TabId) -> Self {
        Self {
            id,
            controller: NavigationController::new(),
            title: "New Tab".to_string(),
            tier: TabTier::Active,
        }
    }
}

/// Manages the collection of open tabs and active selection.
pub struct TabManager {
    next_tab_id: u64,
    tabs: Vec<Tab>,
    active_tab_id: Option<TabId>,
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TabManager {
    /// Creates a new empty `TabManager`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_tab_id: 1,
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }

    /// Creates and appends a new tab, setting it as active.
    pub fn create_tab(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;

        let tab = Tab::new(id);
        self.tabs.push(tab);
        self.select_tab(id);
        id
    }

    /// Returns the currently active tab ID if one exists.
    #[must_use]
    pub const fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Returns the number of open tabs.
    #[must_use]
    pub const fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns a slice of all open tabs.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Selects the tab with the specified ID and updates tab tiers.
    pub fn select_tab(&mut self, id: TabId) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab_id = Some(id);
            for tab in &mut self.tabs {
                if tab.id == id {
                    tab.tier = TabTier::Active;
                } else if tab.tier == TabTier::Active {
                    tab.tier = TabTier::Background;
                }
            }
            true
        } else {
            false
        }
    }

    /// Closes the tab with the specified ID.
    pub fn close_tab(&mut self, id: TabId) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(pos);
            if self.active_tab_id == Some(id) {
                if self.tabs.is_empty() {
                    self.active_tab_id = None;
                } else {
                    let new_pos = pos.min(self.tabs.len() - 1);
                    let new_id = self.tabs[new_pos].id;
                    self.select_tab(new_id);
                }
            }
            true
        } else {
            false
        }
    }

    /// Returns a reference to the active tab.
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    /// Returns a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let active_id = self.active_tab_id;
        active_id.and_then(|id| self.tabs.iter_mut().find(|t| t.id == id))
    }
}
