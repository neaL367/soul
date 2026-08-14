//! Integration tests for sandboxed child process launching and Job Object confinement.

use sandbox::{ProcessLauncher, SandboxKind, SandboxProfile};
use std::path::Path;

#[test]
fn test_spawn_sandboxed_child_process() {
    let profile = SandboxProfile::for_kind(SandboxKind::Utility);
    let mut sandboxed_child =
        ProcessLauncher::spawn_sandboxed(Path::new("cmd.exe"), &["/C", "exit 0"], &profile)
            .expect("Spawn sandboxed cmd failed");

    let status = sandboxed_child.wait().expect("Wait for child failed");
    assert!(status.success());
}

#[test]
fn test_spawn_sandboxed_kill_job() {
    let profile = SandboxProfile::for_kind(SandboxKind::Utility);
    let mut sandboxed_child = ProcessLauncher::spawn_sandboxed(
        Path::new("cmd.exe"),
        &["/C", "ping 127.0.0.1 -n 5 > nul"],
        &profile,
    )
    .expect("Spawn sandboxed long-running process failed");

    // Force job termination
    sandboxed_child.kill_job(99).expect("Kill job failed");

    let status = sandboxed_child.wait().expect("Wait failed");
    // Terminated by job object
    assert!(!status.success());
}
