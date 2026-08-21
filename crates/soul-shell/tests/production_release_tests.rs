//! Integration tests for auto-updater verification, staging, crash reporting, and diagnostics.

use soul_shell::{
    BreadcrumbTracker, CrashReport, SystemDiagnostics, UpdateChannel, UpdateError, UpdateManifest,
    apply_staged_update, check_for_update, compute_sha256, is_newer_version, prune_old_reports,
    stage_update_payload, verify_manifest_signature, verify_payload_checksum,
};

#[test]
fn test_updater_version_comparison_and_notification() {
    assert!(is_newer_version("0.1.0", "0.2.0"));
    assert!(is_newer_version("0.1.0", "0.1.1"));
    assert!(is_newer_version("0.1.0", "1.0.0"));
    assert!(!is_newer_version("0.2.0", "0.1.0"));
    assert!(!is_newer_version("0.1.0", "0.1.0"));

    let manifest = UpdateManifest {
        version: "0.2.0".to_string(),
        channel: UpdateChannel::Stable,
        release_notes: "Production release with full hardware acceleration.".to_string(),
        download_url: "https://updates.soulbrowser.com/releases/soul-0.2.0.msi".to_string(),
        sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        signature: "valid_sig".to_string(),
        min_os_version: Some("10.0.22000".to_string()),
    };

    let update = check_for_update("0.1.0", &manifest);
    assert_eq!(update, Some(manifest.clone()));

    let no_update = check_for_update("0.2.0", &manifest);
    assert_eq!(no_update, None);
}

#[test]
fn test_sha256_checksum_and_manifest_signature_verification() {
    let payload = b"SoulBrowserSetup-0.2.0-x86_64";
    let sha = compute_sha256(payload);
    assert_eq!(sha.len(), 64);
    assert!(verify_payload_checksum(payload, &sha));
    assert!(!verify_payload_checksum(b"tampered payload", &sha));

    let public_token = "soul_prod_key_token_99";
    let sign_payload =
        format!("0.2.0:{sha}:https://updates.soulbrowser.com/release.exe:{public_token}");
    let valid_signature = compute_sha256(sign_payload.as_bytes());

    let mut manifest = UpdateManifest {
        version: "0.2.0".to_string(),
        channel: UpdateChannel::Stable,
        release_notes: "Secure update".to_string(),
        download_url: "https://updates.soulbrowser.com/release.exe".to_string(),
        sha256: sha,
        signature: valid_signature,
        min_os_version: None,
    };

    assert!(verify_manifest_signature(&manifest, public_token).unwrap());

    // Tampered signature must fail
    manifest.signature = "deadbeefdeadbeef".to_string();
    assert!(matches!(
        verify_manifest_signature(&manifest, public_token),
        Err(UpdateError::InvalidSignature { .. })
    ));
}

#[test]
fn test_staging_and_atomic_update_replacement() {
    let temp_dir = std::env::temp_dir().join(format!("soul_stage_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let payload = b"MZ_STUB_SOUL_BROWSER_NEW_VERSION";
    let sha = compute_sha256(payload);

    // Staging with valid checksum
    let staged_file = stage_update_payload(&temp_dir, payload, "soul_new.exe", &sha)
        .expect("stage payload with valid sha256");
    assert!(staged_file.exists());

    // Staging with corrupted checksum fails
    let corrupt_res = stage_update_payload(&temp_dir, payload, "soul_corrupt.exe", "0000000000");
    assert!(matches!(
        corrupt_res,
        Err(UpdateError::ChecksumMismatch { .. })
    ));

    // Target executable to replace
    let target_bin = temp_dir.join("bin").join("soul.exe");
    std::fs::create_dir_all(target_bin.parent().unwrap()).unwrap();
    std::fs::write(&target_bin, b"OLD_VERSION").unwrap();

    // Atomic swap
    apply_staged_update(&staged_file, &target_bin).expect("apply staged update");
    assert!(!staged_file.exists());
    assert!(target_bin.exists());

    let installed_bytes = std::fs::read(&target_bin).unwrap();
    assert_eq!(installed_bytes, payload);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_crash_reporter_with_breadcrumbs_and_pruning() {
    let tracker = BreadcrumbTracker::global();
    tracker.clear();
    tracker.record("navigated: https://example.com");
    tracker.record("click: button#submit");
    tracker.record("xhr: /api/v1/user");

    let report = CrashReport::new(
        "GPU device lost during draw call",
        "gpu",
        "at GpuCompositor::composite_layers()",
    );

    assert_eq!(report.breadcrumbs.len(), 3);
    assert!(report.breadcrumbs[0].contains("https://example.com"));

    let serialized = report.serialize_log();
    assert!(serialized.contains("subsystem: gpu"));
    assert!(serialized.contains("reason: GPU device lost"));
    assert!(serialized.contains("navigated: https://example.com"));

    let temp_dir = std::env::temp_dir().join(format!("soul_crash_prune_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Create 5 dummy old log files
    std::fs::create_dir_all(&temp_dir).unwrap();
    for i in 1..=5 {
        let dummy = temp_dir.join(format!("crash_old_{i}.log"));
        std::fs::write(dummy, b"dummy").unwrap();
    }

    // Persist real crash report
    let path = report
        .persist_to_disk(&temp_dir)
        .expect("persist crash report");
    assert!(path.exists());

    // Prune keeping at most 3
    let pruned = prune_old_reports(&temp_dir, 3).expect("prune old reports");
    assert_eq!(pruned, 3);

    let remaining: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(remaining.len(), 3);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_system_diagnostics_and_about_pages() {
    let diag = SystemDiagnostics::current();
    assert!(!diag.browser_version.is_empty());
    assert!(diag.logical_cores >= 1);

    let version_html = diag.render_about_version_html();
    assert!(version_html.contains("Soul Browser"));
    assert!(version_html.contains(&diag.browser_version));

    let gpu_html = SystemDiagnostics::render_about_gpu_html("wgpu D3D12 Adapter: NVIDIA RTX 4080");
    assert!(gpu_html.contains("Graphics Diagnostics"));
    assert!(gpu_html.contains("NVIDIA RTX 4080"));

    let crash_logs = vec!["Crash #1: OOM in renderer".to_string()];
    let crashes_html = SystemDiagnostics::render_about_crashes_html(&crash_logs);
    assert!(crashes_html.contains("Crash Reports"));
    assert!(crashes_html.contains("OOM in renderer"));
}
