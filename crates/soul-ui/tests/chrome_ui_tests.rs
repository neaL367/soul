//! Integration tests for browser chrome view models, tab strip, omnibox, and toolbar.

use soul_core::{NavigationController, TabId, TabManager};
use soul_ui::{OmniboxEngine, OmniboxSuggestionType, SoulModel, TabStripModel, ToolbarModel};
use storage::{BookmarkEntry, HistoryEntry};

#[test]
fn test_tab_strip_lifecycle_and_pinning() {
    let mut strip = TabStripModel::new();

    let tab1 = TabId(1);
    let tab2 = TabId(2);
    let tab3 = TabId(3);

    strip.add_tab(tab1, "Tab 1".to_string(), true);
    strip.add_tab(tab2, "Tab 2".to_string(), false);
    strip.add_tab(tab3, "Tab 3".to_string(), false);

    assert_eq!(strip.len(), 3);
    assert_eq!(strip.active_tab_id(), Some(tab1));

    // Pin tab 2
    strip.toggle_pinned(tab2);
    assert!(strip.tabs()[0].is_pinned);
    assert_eq!(strip.tabs()[0].id, tab2);

    // Remove active tab (tab1), adjacent tab becomes active
    let next_active = strip.remove_tab(tab1);
    assert!(next_active.is_some());
    assert_eq!(strip.len(), 2);
}

#[test]
fn test_toolbar_state_sync() {
    let mut controller = NavigationController::new();

    let mut toolbar = ToolbarModel::new();
    toolbar.update_from_controller(&controller, false);
    assert!(!toolbar.can_go_back);
    assert!(!toolbar.can_go_forward);

    // Navigate to url1, then url2
    let nav1 = controller.navigate("https://example.com/1").unwrap();
    controller.handle_response(nav1, 200, "text/html".to_string());
    controller.handle_dom_ready(nav1);
    controller.handle_loaded(nav1);

    let nav2 = controller.navigate("https://example.com/2").unwrap();
    controller.handle_response(nav2, 200, "text/html".to_string());
    controller.handle_dom_ready(nav2);
    controller.handle_loaded(nav2);

    toolbar.update_from_controller(&controller, true);
    assert!(toolbar.can_go_back);
    assert!(!toolbar.can_go_forward);
    assert!(toolbar.is_bookmarked);

    // Go back
    controller.go_back();
    toolbar.update_from_controller(&controller, false);
    assert!(!toolbar.can_go_back);
    assert!(toolbar.can_go_forward);
}

#[test]
fn test_omnibox_engine_scoring_and_ranking() {
    let engine = OmniboxEngine::new(None);

    let history = vec![
        HistoryEntry {
            id: 1,
            url: "https://rust-lang.org/learn".to_string(),
            title: Some("Learn Rust".to_string()),
            visit_count: 15,
            last_visited_at: 1000,
        },
        HistoryEntry {
            id: 2,
            url: "https://crates.io".to_string(),
            title: Some("Rust Package Registry".to_string()),
            visit_count: 5,
            last_visited_at: 900,
        },
    ];

    let bookmarks = vec![BookmarkEntry {
        id: 1,
        url: "https://github.com/neaL367/soul".to_string(),
        title: "Soul Engine".to_string(),
        folder: None,
        created_at: 800,
    }];

    // 1. Direct URL detection
    let suggestions = engine.generate_suggestions("https://github.com", &history, &bookmarks);
    assert_eq!(
        suggestions[0].suggestion_type,
        OmniboxSuggestionType::DirectUrl
    );

    // 2. Query matching bookmark
    let suggestions2 = engine.generate_suggestions("soul", &history, &bookmarks);
    assert_eq!(
        suggestions2[0].suggestion_type,
        OmniboxSuggestionType::Bookmark
    );
    assert_eq!(suggestions2[0].url, "https://github.com/neaL367/soul");

    // 3. Query matching history
    let suggestions3 = engine.generate_suggestions("learn", &history, &bookmarks);
    assert_eq!(
        suggestions3[0].suggestion_type,
        OmniboxSuggestionType::History
    );
    assert_eq!(suggestions3[0].url, "https://rust-lang.org/learn");

    // 4. Fallback search
    let suggestions4 = engine.generate_suggestions("something novel", &history, &bookmarks);
    assert_eq!(
        suggestions4.last().unwrap().suggestion_type,
        OmniboxSuggestionType::Search
    );
}

#[test]
fn test_chrome_model_aggregation() {
    let mut chrome = SoulModel::new();
    let mut manager = TabManager::new();

    let tab_id = manager.create_tab();
    let tab = manager.get_tab_mut(tab_id).unwrap();
    let _nav = tab.controller.navigate("https://rust-lang.org").unwrap();

    chrome.sync_with_tab_manager(&manager, false);
    assert_eq!(chrome.omnibox.text, "https://rust-lang.org/");

    // Omnibox submission resolution
    chrome.omnibox.set_text("github.com".to_string());
    assert_eq!(chrome.resolve_omnibox_submission(), "https://github.com");

    chrome.omnibox.set_text("rust async patterns".to_string());
    assert_eq!(
        chrome.resolve_omnibox_submission(),
        "https://duckduckgo.com/?q=rust async patterns"
    );
}
