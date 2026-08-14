//! Integration tests for the navigation state machine, race condition prevention, and tab management.

use soul_core::{NavigationController, NavigationId, NavigationState, TabManager, TabTier};
use url::Url;

#[test]
fn test_happy_path_navigation_state_machine() {
    let mut controller = NavigationController::new();
    assert_eq!(*controller.state(), NavigationState::Init);

    let id = controller
        .navigate("example.com")
        .expect("navigation initiation failed");
    assert_eq!(id, NavigationId(1));

    let expected_url = Url::parse("https://example.com/").unwrap();
    assert_eq!(
        *controller.state(),
        NavigationState::Navigating {
            id,
            url: expected_url.clone(),
        }
    );

    // 1. Response received
    assert!(controller.handle_response(id, 200, "text/html".to_string()));
    assert_eq!(
        *controller.state(),
        NavigationState::ResponseReceived {
            id,
            url: expected_url.clone(),
            status_code: 200,
            mime_type: "text/html".to_string(),
        }
    );

    // 2. DOM ready
    assert!(controller.handle_dom_ready(id));
    assert_eq!(
        *controller.state(),
        NavigationState::DomReady {
            id,
            url: expected_url.clone(),
        }
    );

    // 3. Loaded
    assert!(controller.handle_loaded(id));
    assert_eq!(
        *controller.state(),
        NavigationState::Loaded {
            id,
            url: expected_url.clone(),
        }
    );

    // History committed
    assert_eq!(
        controller.history().current_entry().unwrap().url,
        expected_url
    );
}

#[test]
fn test_navigation_race_condition_stale_events_discarded() {
    let mut controller = NavigationController::new();

    // Start navigation 1 to site A
    let id_1 = controller.navigate("https://site-a.com").unwrap();

    // User rapidly navigates to site B before site A completes
    let id_2 = controller.navigate("https://site-b.com").unwrap();
    assert_ne!(id_1, id_2);

    let url_b = Url::parse("https://site-b.com").unwrap();
    assert_eq!(
        *controller.state(),
        NavigationState::Navigating {
            id: id_2,
            url: url_b.clone(),
        }
    );

    // Late network response arrives for site A (stale id_1)
    let applied_stale_response = controller.handle_response(id_1, 200, "text/html".to_string());
    assert!(
        !applied_stale_response,
        "Stale response should be discarded"
    );
    assert_eq!(
        *controller.state(),
        NavigationState::Navigating {
            id: id_2,
            url: url_b.clone(),
        }
    );

    // Valid response arrives for site B (id_2)
    assert!(controller.handle_response(id_2, 200, "text/html".to_string()));
    assert!(
        matches!(controller.state(), NavigationState::ResponseReceived { id, .. } if *id == id_2)
    );

    // Stale DOM ready for site A arrives
    assert!(!controller.handle_dom_ready(id_1));

    // Valid DOM ready for site B arrives
    assert!(controller.handle_dom_ready(id_2));

    // Valid Loaded for site B arrives
    assert!(controller.handle_loaded(id_2));
    assert_eq!(controller.history().current_entry().unwrap().url, url_b);
}

#[test]
fn test_back_forward_history_navigation() {
    let mut controller = NavigationController::new();

    let id1 = controller.navigate("https://page-1.com").unwrap();
    controller.handle_response(id1, 200, "text/html".to_string());
    controller.handle_dom_ready(id1);
    controller.handle_loaded(id1);

    let id2 = controller.navigate("https://page-2.com").unwrap();
    controller.handle_response(id2, 200, "text/html".to_string());
    controller.handle_dom_ready(id2);
    controller.handle_loaded(id2);

    let id3 = controller.navigate("https://page-3.com").unwrap();
    controller.handle_response(id3, 200, "text/html".to_string());
    controller.handle_dom_ready(id3);
    controller.handle_loaded(id3);

    assert!(controller.history().can_go_back());
    assert!(!controller.history().can_go_forward());

    // Go back to page 2
    let _ = controller.go_back().unwrap();
    assert_eq!(
        controller.state().current_url().unwrap().as_str(),
        "https://page-2.com/"
    );
    assert!(controller.history().can_go_forward());

    // Go back to page 1
    let _ = controller.go_back().unwrap();
    assert_eq!(
        controller.state().current_url().unwrap().as_str(),
        "https://page-1.com/"
    );
    assert!(!controller.history().can_go_back());

    // Go forward to page 2
    let _ = controller.go_forward().unwrap();
    assert_eq!(
        controller.state().current_url().unwrap().as_str(),
        "https://page-2.com/"
    );

    // Navigate to page 4 while at page 2 -> forward history truncated
    let id4 = controller.navigate("https://page-4.com").unwrap();
    controller.handle_response(id4, 200, "text/html".to_string());
    controller.handle_dom_ready(id4);
    controller.handle_loaded(id4);

    assert!(!controller.history().can_go_forward());
    assert!(controller.history().can_go_back());
}

#[test]
fn test_tab_manager_lifecycle_and_tiering() {
    let mut manager = TabManager::new();
    assert_eq!(manager.tab_count(), 0);

    let tab1 = manager.create_tab();
    let tab2 = manager.create_tab();
    let tab3 = manager.create_tab();

    assert_eq!(manager.tab_count(), 3);
    assert_eq!(manager.active_tab_id(), Some(tab3));

    // Tab 3 is active, Tabs 1 and 2 should be in Background tier
    assert_eq!(manager.tabs()[0].tier, TabTier::Background);
    assert_eq!(manager.tabs()[1].tier, TabTier::Background);
    assert_eq!(manager.tabs()[2].tier, TabTier::Active);

    // Switch to Tab 1
    assert!(manager.select_tab(tab1));
    assert_eq!(manager.active_tab_id(), Some(tab1));
    assert_eq!(manager.tabs()[0].tier, TabTier::Active);
    assert_eq!(manager.tabs()[2].tier, TabTier::Background);

    // Close Tab 2
    assert!(manager.close_tab(tab2));
    assert_eq!(manager.tab_count(), 2);

    // Close active Tab 1 -> Tab 3 becomes active
    assert!(manager.close_tab(tab1));
    assert_eq!(manager.tab_count(), 1);
    assert_eq!(manager.active_tab_id(), Some(tab3));
}
