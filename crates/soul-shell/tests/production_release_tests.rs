//! Integration tests for auto-updater checks and crash reporting persistence.

use soul_shell::{CrashReport, UpdateChannel, UpdateManifest, check_for_update, is_newer_version};

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
    };

    let update = check_for_update("0.1.0", &manifest);
    assert_eq!(update, Some(manifest.clone()));

    let no_update = check_for_update("0.2.0", &manifest);
    assert_eq!(no_update, None);
}

#[test]
fn test_crash_reporter_serialization_and_disk_persistence() {
    let report = CrashReport::new(
        "GPU device lost during draw call",
        "gpu",
        "at GpuCompositor::composite_layers() [compositor/src/gpu_compositor.rs:52]",
    );

    let serialized = report.serialize_log();
    assert!(serialized.contains("subsystem: gpu"));
    assert!(serialized.contains("reason: GPU device lost"));

    let temp_dir = std::env::temp_dir().join(format!("soul_crash_tests_{}", std::process::id()));
    let path = report.persist_to_disk(&temp_dir).expect("persist crash report");

    assert!(path.exists());
    let content = std::fs::read_to_string(&path).expect("read persisted crash log");
    assert_eq!(content, serialized);

    let _ = std::fs::remove_dir_all(temp_dir);
}
