//! Tests for Windows UI Automation accessibility bridge.

use platform_windows::{UiaBridge, UiaControlType, UiaElement};

#[test]
fn test_uia_bridge_tree_and_hit_testing() {
    let mut root = UiaElement::new(1, UiaControlType::Document, "Document Root", (0.0, 0.0, 800.0, 600.0));
    let heading = UiaElement::new(2, UiaControlType::Heading, "Welcome Page", (20.0, 20.0, 760.0, 40.0));
    let button = UiaElement::new(3, UiaControlType::Button, "Submit Form", (20.0, 100.0, 120.0, 36.0));

    root.children.push(heading);
    root.children.push(button);

    let mut bridge = UiaBridge::new();
    bridge.set_root(root);

    // Hit-testing the button
    let hit_button = bridge.hit_test(30.0, 110.0).expect("button should be hit");
    assert_eq!(hit_button.id, 3);
    assert_eq!(hit_button.control_type, UiaControlType::Button);
    assert!(hit_button.is_interactive);

    // Find by ID
    let found_heading = bridge.find_element(2).expect("heading should be found");
    assert_eq!(found_heading.name, "Welcome Page");
    assert_eq!(found_heading.control_type, UiaControlType::Heading);
}
