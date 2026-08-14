//! Integration tests for the `InputRouter` and input event dispatching.

use soul_ui::{
    InputEvent, InputRouter, KeyModifiers, KeyPhase, LogicalPosition, MouseButton, MousePhase,
    PhysicalPosition, WheelDeltaMode, WindowId,
};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_mouse_move_and_dpi_scaling() {
    let mut router = InputRouter::new(2.0);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    router.subscribe(move |_window_id, event| {
        events_clone.lock().unwrap().push(event.clone());
    });

    let window_id = WindowId(1);
    router.handle_mouse_move(window_id, PhysicalPosition::new(200.0, 400.0));

    assert_eq!(router.cursor_position(), LogicalPosition::new(100.0, 200.0));

    let recorded = events.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    if let InputEvent::Mouse(mouse_event) = &recorded[0] {
        assert_eq!(mouse_event.position, LogicalPosition::new(100.0, 200.0));
        assert_eq!(mouse_event.phase, MousePhase::Move);
    } else {
        panic!("Expected Mouse event");
    }
}

#[test]
fn test_mouse_button_press_and_multi_click_detection() {
    let mut router = InputRouter::new(1.0);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    router.subscribe(move |_window_id, event| {
        events_clone.lock().unwrap().push(event.clone());
    });

    let window_id = WindowId(1);
    let pos = PhysicalPosition::new(50.0, 50.0);

    // First click (down + up)
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Down, pos);
    assert!(router.is_button_pressed(&MouseButton::Left));
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Up, pos);
    assert!(!router.is_button_pressed(&MouseButton::Left));

    // Second click within interval (double click)
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Down, pos);
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Up, pos);

    // Third click within interval (triple click)
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Down, pos);
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Up, pos);

    let recorded = events.lock().unwrap().clone();
    assert_eq!(recorded.len(), 6);

    if let InputEvent::Mouse(ref m) = recorded[0] {
        assert_eq!(m.click_count, 1);
    }
    if let InputEvent::Mouse(ref m) = recorded[2] {
        assert_eq!(m.click_count, 2);
    }
    if let InputEvent::Mouse(ref m) = recorded[4] {
        assert_eq!(m.click_count, 3);
    }
}

#[test]
fn test_multi_click_timeout_reset() {
    let mut router = InputRouter::new(1.0);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    router.subscribe(move |_window_id, event| {
        events_clone.lock().unwrap().push(event.clone());
    });

    let window_id = WindowId(1);
    let pos = PhysicalPosition::new(10.0, 10.0);

    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Down, pos);
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Up, pos);

    // Wait for multi-click duration to expire
    sleep(Duration::from_millis(550));

    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Down, pos);
    router.handle_mouse_button(window_id, MouseButton::Left, MousePhase::Up, pos);

    let recorded = events.lock().unwrap().clone();
    if let InputEvent::Mouse(ref m) = recorded[2] {
        assert_eq!(m.click_count, 1);
    }
}

#[test]
fn test_wheel_and_keyboard_event_routing() {
    let mut router = InputRouter::new(1.0);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    router.subscribe(move |_window_id, event| {
        events_clone.lock().unwrap().push(event.clone());
    });

    let window_id = WindowId(1);

    // Wheel event
    router.handle_wheel(window_id, 0.0, -120.0, WheelDeltaMode::Line);

    // Keyboard event with modifiers
    let modifiers = KeyModifiers {
        shift: false,
        ctrl: true,
        alt: false,
        meta: false,
    };
    router.handle_key(
        window_id,
        "t".to_string(),
        "KeyT".to_string(),
        KeyPhase::Down,
        modifiers,
        Some("t".to_string()),
    );

    assert_eq!(router.active_modifiers(), modifiers);

    let recorded = events.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2);

    if let InputEvent::Wheel(ref w) = recorded[0] {
        assert!((w.delta_y - (-120.0)).abs() < f64::EPSILON);
        assert_eq!(w.delta_mode, WheelDeltaMode::Line);
    } else {
        panic!("Expected Wheel event");
    }

    if let InputEvent::Keyboard(ref k) = recorded[1] {
        assert_eq!(k.key, "t");
        assert!(k.modifiers.ctrl);
    } else {
        panic!("Expected Keyboard event");
    }
}
