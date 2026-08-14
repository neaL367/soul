//! Integration tests for sandboxed child process launching and Job Object confinement.

use sandbox::{ProcessLauncher, SandboxKind, SandboxProfile};
use std::path::Path;

#[test]
fn test_spawn_sandboxed_child_process() {
    let profile = SandboxProfile::for_kind(SandboxKind::Utility);
    let mut child =
        ProcessLauncher::spawn_sandboxed(Path::new("cmd.exe"), &["/C", "exit 0"], &profile)
            .expect("Spawn sandboxed cmd failed");

    let status = child.wait().expect("Wait for child failed");
    assert!(status.success());
}
