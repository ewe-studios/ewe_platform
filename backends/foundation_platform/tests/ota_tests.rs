//! OTA version and manifest tests moved out of `src/ota.rs` (spec 70
//! feature 59, rule 7).
//!
//! `chrono_now` was private; it is now public so the timestamp format stays
//! covered.

use foundation_platform::ota::chrono_now;
use foundation_platform::{LocalVersion, OtaManifest};

#[test]
fn local_version_write_and_read() {
    let dir = std::env::temp_dir().join("ewe_ota_test");
    let _ = std::fs::remove_dir_all(&dir);

    let lv = LocalVersion {
        app_id: "test-app".into(),
        bundle_version: "1.2.3".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    };
    lv.write(&dir).unwrap();

    let read = LocalVersion::read(&dir, "test-app").unwrap();
    assert_eq!(read.bundle_version, "1.2.3");
    assert_eq!(read.app_id, "test-app");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn local_version_missing_returns_none() {
    let dir = std::env::temp_dir().join("ewe_ota_nonexistent");
    assert!(LocalVersion::read(&dir, "nonexistent").is_none());
}

#[test]
fn ota_manifest_deserialize() {
    let json = r#"{
        "version": 1,
        "base_url": "https://cdn.example.com/bundles",
        "apps": [{
            "app_id": "app",
            "bundle_version": "2.0.0",
            "files": [{"path": "index.html", "sha256": "abc123", "size": 1024}]
        }]
    }"#;
    let manifest: OtaManifest = serde_json::from_str(json).unwrap();
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.apps.len(), 1);
    assert_eq!(manifest.apps[0].app_id, "app");
    assert_eq!(manifest.apps[0].bundle_version, "2.0.0");
}

#[test]
fn chrono_now_produces_iso8601() {
    let ctx = foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    ));
    let ts = chrono_now(&ctx);
    assert!(ts.contains('T'));
    assert!(ts.ends_with('Z'));
    // Basic format check: YYYY-MM-DDTHH:MM:SSZ
    assert_eq!(ts.len(), 20);
}
