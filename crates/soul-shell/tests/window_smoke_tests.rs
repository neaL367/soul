//! Real-window smoke test: launches `soul_shell`, verifies a native window
//! with the expected title exists (via Win32 `EnumWindows`), then terminates it.
//!
//! Requires an interactive desktop session; skipped by default (`#[ignore]`).
//! Run with: `cargo test -p soul_shell --test window_smoke_tests -- --ignored --nocapture`

// Win32 window enumeration is a well-known FFI boundary (AGENTS.md §2): the
// unsafe surface here is a single callback + one EnumWindows call.
#![allow(unsafe_code)]

use std::process::{Child, Command};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static WINDOW_FOUND: AtomicBool = AtomicBool::new(false);
static EXPECTED_TITLE: OnceLock<String> = OnceLock::new();

/// Win32 `EnumWindows` callback matching visible window titles.
///
/// # Safety
///
/// Safe because the callback only touches process-local statics and is invoked
/// synchronously on the calling thread during `EnumWindows`.
#[allow(clippy::undocumented_unsafe_blocks, clippy::cast_sign_loss)]
unsafe extern "system" fn enum_proc(
    hwnd: windows::Win32::Foundation::HWND,
    _lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };

    // SAFETY: standard Win32 visibility query on a valid window handle.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        // SAFETY: standard Win32 text queries on a valid visible window handle.
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            // SAFETY: buffer sized from GetWindowTextLengthW result.
            let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
            let title = String::from_utf16_lossy(&buf[..written as usize]);
            if title.contains(EXPECTED_TITLE.get().map_or("", String::as_str)) {
                WINDOW_FOUND.store(true, Ordering::SeqCst);
                return windows::Win32::Foundation::FALSE; // stop enumeration
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

fn spawn_shell() -> Child {
    Command::new(env!("CARGO_BIN_EXE_soul"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn soul")
}

#[test]
#[ignore = "requires an interactive desktop session"]
fn test_shell_opens_native_window() {
    let mut child = spawn_shell();

    // Give the engine + GPUI time to start and present the first frame.
    let mut window_found = false;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if find_window_by_title("Soul") {
            window_found = true;
            break;
        }
        assert!(
            child.try_wait().ok().flatten().is_none(),
            "soul exited before opening a window"
        );
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        window_found,
        "no native window titled 'Soul' appeared within 10s"
    );
    eprintln!("PASS: native window 'Soul' observed via EnumWindows");
}
