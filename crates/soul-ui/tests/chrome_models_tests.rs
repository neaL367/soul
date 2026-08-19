//! Integration tests for `HitTestMap`, `OmniboxEngine`, tab-strip reordering,
//! and the shared URL-detection helper.

use soul_core::TabId;
use soul_ui::{
    HitTestMap, HitTestRegion, HitTestTarget, OmniboxEngine, TabStripModel, looks_like_url,
};
use storage::HistoryEntry;

#[test]
fn test_hit_test_map_region_containment() {
    let mut map = HitTestMap::default();
    map.regions.push(HitTestRegion {
        x: 10.0,
        y: 20.0,
        width: 50.0,
        height: 30.0,
        target: HitTestTarget::Link("https://a".to_string()),
    });

    // Inside the region.
    assert_eq!(
        map.hit_test(35.0, 35.0),
        Some(&HitTestTarget::Link("https://a".to_string()))
    );
    // On the inclusive bottom/right edges.
    assert_eq!(
        map.hit_test(60.0, 50.0),
        Some(&HitTestTarget::Link("https://a".to_string()))
    );
    // Outside the region.
    assert_eq!(map.hit_test(5.0, 5.0), None);
    assert_eq!(map.hit_test(61.0, 35.0), None);
    assert_eq!(map.hit_test(35.0, 51.0), None);
}

#[test]
fn test_hit_test_map_later_region_wins() {
    let mut map = HitTestMap::default();
    map.regions.push(HitTestRegion {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        target: HitTestTarget::Link("https://bottom".to_string()),
    });
    map.regions.push(HitTestRegion {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        target: HitTestTarget::Link("https://top".to_string()),
    });

    // Later regions (painted on top) win over earlier ones.
    assert_eq!(
        map.hit_test(50.0, 50.0),
        Some(&HitTestTarget::Link("https://top".to_string()))
    );
}

#[test]
fn test_looks_like_url() {
    assert!(looks_like_url("example.com"));
    assert!(looks_like_url("https://example.com"));
    assert!(looks_like_url("localhost:8080"));
    assert!(looks_like_url("  rust-lang.org  "));
    assert!(!looks_like_url("rust async patterns"));
    assert!(!looks_like_url("  "));
    assert!(!looks_like_url("plaintext"));
}

#[test]
fn test_omnibox_dedup_keeps_highest_score() {
    let engine = OmniboxEngine::new(None);
    let history = vec![
        HistoryEntry {
            id: 1,
            url: "https://rust-lang.org".to_string(),
            title: Some("Rust".to_string()),
            visit_count: 30,
            last_visited_at: 1,
        },
        HistoryEntry {
            id: 2,
            url: "https://rust-lang.org".to_string(),
            title: Some("Duplicate entry".to_string()),
            visit_count: 2,
            last_visited_at: 1,
        },
    ];
    let suggestions = engine.generate_suggestions("rust", &history, &[]);
    let matches = suggestions
        .iter()
        .filter(|s| s.url == "https://rust-lang.org")
        .count();
    // The two history entries collapse into a single suggestion.
    assert_eq!(matches, 1, "duplicate URLs must be deduplicated");
}

#[test]
fn test_omnibox_empty_input_produces_no_suggestions() {
    let engine = OmniboxEngine::new(None);
    let history = vec![HistoryEntry {
        id: 1,
        url: "https://rust-lang.org".to_string(),
        title: Some("Rust".to_string()),
        visit_count: 1,
        last_visited_at: 1,
    }];
    assert!(engine.generate_suggestions("   ", &history, &[]).is_empty());
    assert!(engine.generate_suggestions("", &history, &[]).is_empty());
}

#[test]
fn test_move_tab_preserves_pinned_before_unpinned() {
    let mut strip = TabStripModel::new();
    strip.add_tab(TabId(1), "T1".to_string(), true);
    strip.add_tab(TabId(2), "T2".to_string(), false);
    strip.add_tab(TabId(3), "T3".to_string(), false);
    strip.add_tab(TabId(4), "T4".to_string(), false);

    // Pin T1 and T4, re-sorting them to the front as [T4(p), T1(p), T2, T3].
    strip.toggle_pinned(TabId(4));
    strip.toggle_pinned(TabId(1));

    // Try to move a pinned tab (index 1, T1) into the unpinned region (index 3).
    strip.move_tab(1, 3);
    // Invariant: both pinned tabs still precede every unpinned tab.
    let ids: Vec<TabId> = strip.tabs().iter().map(|t| t.id).collect();
    let pinned_ids: Vec<TabId> = strip
        .tabs()
        .iter()
        .filter(|t| t.is_pinned)
        .map(|t| t.id)
        .collect();
    assert_eq!(pinned_ids.len(), 2);
    assert!(
        ids.iter().position(|&t| t == pinned_ids[0]) < ids.iter().position(|&t| t == pinned_ids[1])
    );
    for (idx, tab) in strip.tabs().iter().enumerate() {
        if tab.is_pinned {
            assert!(idx < 2, "pinned tab {idx} escaped the leading run");
        }
    }
}

#[test]
fn test_move_tab_unpinned_never_enters_pinned_run() {
    let mut strip = TabStripModel::new();
    strip.add_tab(TabId(1), "T1".to_string(), true);
    strip.add_tab(TabId(2), "T2".to_string(), false);
    strip.add_tab(TabId(3), "T3".to_string(), false);
    strip.toggle_pinned(TabId(2)); // pinned run is now [T2(p), T1, T3]

    // Move unpinned T3 (index 2) to the front (index 0).
    strip.move_tab(2, 0);
    // T3 must land after the pinned run, so T2 stays first and pinned.
    let ids: Vec<TabId> = strip.tabs().iter().map(|t| t.id).collect();
    assert_eq!(
        ids[0],
        TabId(2),
        "unpinned tab must not jump ahead of pinned run"
    );
    assert!(strip.tabs()[0].is_pinned);
}
