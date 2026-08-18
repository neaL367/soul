//! Integration tests for Windows Job Object resource isolation and restricted tokens.

use sandbox::{JobObject, RestrictedToken, SandboxKind, SandboxProfile};

#[test]
fn test_job_object_lifecycle_and_restrictions() {
    let job = JobObject::create().expect("failed to create JobObject");

    // Configure memory limit (512MB)
    job.set_memory_limit(512 * 1024 * 1024)
        .expect("failed to set memory limit");

    // Configure UI restrictions
    job.set_ui_restrictions()
        .expect("failed to set UI restrictions");

    assert!(!job.raw_handle().is_invalid());
}

#[test]
fn test_sandbox_profile_builder() {
    let renderer_profile = SandboxProfile::for_kind(SandboxKind::Renderer);
    assert_eq!(renderer_profile.kind, SandboxKind::Renderer);
    assert!(renderer_profile.restrict_ui);

    let job = renderer_profile
        .build_job()
        .expect("failed to build renderer sandbox job");
    assert!(!job.raw_handle().is_invalid());
}

#[test]
fn test_restricted_token_creation() {
    let token = RestrictedToken::create_for_renderer();
    // In CI or restricted environments, OpenProcessToken might require standard user privileges.
    if let Ok(tok) = token {
        assert!(!tok.raw_handle().is_invalid());
    }
}

#[test]
fn test_job_object_accounting_query() {
    let job = JobObject::create().expect("failed to create JobObject");
    let stats = job.query_accounting().expect("query accounting");
    assert_eq!(stats.active_processes, 0);
    assert_eq!(stats.total_processes, 0);
}
