//! Integration tests for sandboxed child process launching and Job Object confinement.

use sandbox::{JobObject, ProcessLauncher, RestrictedToken, SandboxKind, SandboxProfile};
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

#[test]
fn test_spawn_sandboxed_with_restricted_token_profile() {
    let profile = SandboxProfile::for_kind(SandboxKind::Renderer);
    assert!(profile.use_restricted_token);
    assert!(profile.low_integrity);

    let mut sandboxed_child =
        ProcessLauncher::spawn_sandboxed(Path::new("cmd.exe"), &["/C", "exit 0"], &profile)
            .expect("Spawn renderer sandboxed cmd failed");

    assert!(sandboxed_child.pid() > 0);
    let status = sandboxed_child.wait().expect("Wait for child failed");
    assert!(status.success());
}

#[test]
fn test_spawn_with_explicit_restricted_token() {
    let token = RestrictedToken::create_for_renderer().expect("Create restricted token failed");
    assert!(!token.raw_handle().is_invalid());

    let profile = SandboxProfile::for_kind(SandboxKind::Network);
    let mut sandboxed_child = ProcessLauncher::spawn_with_restricted_token(
        Path::new("cmd.exe"),
        &["/C", "exit 0"],
        &profile,
        &token,
    )
    .expect("Spawn with explicit restricted token failed");

    let status = sandboxed_child.wait().expect("Wait for child failed");
    assert!(status.success());
}

#[test]
fn test_job_object_limits_and_accounting() {
    let job = JobObject::create().expect("Create job object failed");
    job.set_memory_limit(256 * 1024 * 1024)
        .expect("Set memory limit failed");
    job.set_active_process_limit(2)
        .expect("Set active process limit failed");
    job.set_cpu_rate_limit(50)
        .expect("Set CPU rate limit failed");
    job.set_ui_restrictions()
        .expect("Set UI restrictions failed");

    let accounting = job
        .query_accounting()
        .expect("Query accounting on job failed");
    assert_eq!(accounting.active_processes, 0);
    assert_eq!(accounting.total_processes, 0);
}
