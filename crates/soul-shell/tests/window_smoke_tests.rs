//! Real-window smoke tests for Soul native window and Win32 input delivery.
//!
//! Requires an interactive desktop session; skipped by default (`#[ignore]`).
//! Run with: `cargo test -p soul-shell --test window_smoke_tests -- --ignored --nocapture`

// Win32 window enumeration/input are well-known FFI boundaries (AGENTS.md §2).
#![allow(unsafe_code)]

use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static WINDOW_FOUND: AtomicBool = AtomicBool::new(false);
static EXPECTED_TITLE: OnceLock<String> = OnceLock::new();

/// Win32 `EnumWindows` callback matching visible window titles.
///
/// # Safety
///
/// Safe because callback only touches process-local statics and runs
/// synchronously on calling thread during `EnumWindows`.
#[allow(clippy::undocumented_unsafe_blocks, clippy::cast_sign_loss)]
unsafe extern "system" fn enum_proc(
    hwnd: windows::Win32::Foundation::HWND,
    _lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };

    // SAFETY: standard Win32 visibility query on valid window handle.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        // SAFETY: standard Win32 text query on valid visible window handle.
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            // SAFETY: buffer sized from GetWindowTextLengthW result.
            let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
            let title = String::from_utf16_lossy(&buf[..written as usize]);
            if title.contains(EXPECTED_TITLE.get().map_or("", String::as_str)) {
                WINDOW_FOUND.store(true, Ordering::SeqCst);
                return windows::Win32::Foundation::FALSE;
            }
        }
    }
    windows::Win32::Foundation::TRUE
}

fn find_window_by_title(expected: &str) -> bool {
    let _ = EXPECTED_TITLE.set(expected.to_string());
    WINDOW_FOUND.store(false, Ordering::SeqCst);
    // SAFETY: enum_proc only touches process-local statics; call is synchronous.
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(enum_proc),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
    WINDOW_FOUND.load(Ordering::SeqCst)
}

fn spawn_soul(stdout: Stdio, stderr: Stdio) -> Child {
    Command::new(env!("CARGO_BIN_EXE_soul"))
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .expect("failed to spawn soul")
}

fn wait_for_soul_window(child: &mut Child) {
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if find_window_by_title("Soul") {
            return;
        }
        assert!(
            child.try_wait().ok().flatten().is_none(),
            "soul exited before opening a window"
        );
    }
    panic!("no native window titled 'Soul' appeared within 10s");
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
#[ignore = "requires an interactive desktop session"]
fn test_soul_opens_native_window() {
    let mut child = spawn_soul(Stdio::null(), Stdio::null());
    wait_for_soul_window(&mut child);
    stop_child(&mut child);
    eprintln!("PASS: native window 'Soul' observed via EnumWindows");
}

#[test]
#[ignore = "requires an interactive desktop session"]
fn test_win32_input_reaches_soul_router() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_CHAR, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    };

    let mut child = spawn_soul(Stdio::piped(), Stdio::piped());
    wait_for_soul_window(&mut child);

    // Soul toolbar layout: buttons occupy left edge; omnibox starts near x=200,
    // y=20 in client coordinates.
    let hwnd = find_soul_hwnd().expect("Soul window handle disappeared");
    post_mouse(hwnd, WM_LBUTTONDOWN, 320, 20);
    post_mouse(hwnd, WM_LBUTTONUP, 320, 20);
    for character in "example.com".chars() {
        post_message(hwnd, WM_CHAR, WPARAM(character as usize), LPARAM(0));
    }
    post_message(
        hwnd,
        WM_KEYDOWN,
        WPARAM(usize::from(VK_RETURN.0)),
        LPARAM(0),
    );

    std::thread::sleep(Duration::from_secs(2));
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("failed to collect Soul output");
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        logs.contains("InputRouted"),
        "Soul process produced no routed-input log; output: {logs}"
    );
    eprintln!("PASS: Win32 mouse/keyboard input reached SoulEvent::InputRouted");
}

#[allow(clippy::undocumented_unsafe_blocks, clippy::cast_sign_loss)]
unsafe extern "system" fn find_hwnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
            let title = String::from_utf16_lossy(&buf[..written as usize]);
            if title.contains("Soul") {
                // SAFETY: caller passes pointer to a live HWND local for the
                // synchronous EnumWindows invocation.
                unsafe {
                    *(lparam.0 as *mut windows::Win32::Foundation::HWND) = hwnd;
                }
                return windows::Win32::Foundation::FALSE;
            }
        }
    }
    windows::Win32::Foundation::TRUE
}

#[allow(clippy::borrow_as_ptr)]
fn find_soul_hwnd() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    let mut found = HWND::default();
    // SAFETY: callback writes only to `found` during synchronous enumeration.
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(find_hwnd_proc),
            LPARAM((&raw mut found).cast::<core::ffi::c_void>() as isize),
        );
    }
    if found.0.is_null() { None } else { Some(found) }
}

#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn post_mouse(hwnd: windows::Win32::Foundation::HWND, message: u32, x: i32, y: i32) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    // SAFETY: posting standard mouse messages to a live Soul top-level window.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            hwnd,
            message,
            WPARAM(1),
            LPARAM(((y as u32) << 16 | x as u32) as isize),
        )
        .expect("failed to post mouse message");
    }
}

fn post_message(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) {
    // SAFETY: posting standard keyboard messages to a live Soul top-level window.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, message, wparam, lparam)
            .expect("failed to post input message");
    }
}
