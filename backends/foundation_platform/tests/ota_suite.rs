//! F40 — OTA manifest verification and the update pipeline.
//!
//! `process_manifest` is the gate every over-the-air update passes through.
//! These tests come at it the way an attacker would: a manifest signed by
//! the wrong key, one pointing at a different host, one replayed from
//! yesterday, one naming `../../` as a file path. Each must be refused, and
//! refused for the right reason.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_nativeapis::shared::vfs::dynfs::DynFs;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_platform::assets::{AssetLayout, PlatformAssetManager, MAX_OTA_FILES};
use foundation_platform::manifest::{self, KeyPair, MANIFEST_FILENAME};
use tracing_test::traced_test;

// ── Fixtures ────────────────────────────────────────────────────────────

const DOMAIN: &str = "cdn.test";

fn signing_keypair(name: &str) -> KeyPair {
    let dir = std::env::temp_dir().join(format!("ewe_f40_ota_keys_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let pair = manifest::ensure_keys(&dir).expect("mint keypair");
    assert!(pair.can_sign());
    pair
}

fn manager_with(key: Option<[u8; 32]>, domain: Option<&str>) -> PlatformAssetManager {
    PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(MemoryFs::new())),
        PathBuf::from("/base"),
        "0.1.0",
        AssetLayout::Versioned,
        domain.map(str::to_string),
        key,
    )
}

/// A file entry with a real hash, so only the field under test is wrong.
fn file_entry(path: &str, contents: &str) -> String {
    format!(
        r#"{{"path":"{path}","sha256":"{hash}","size":{size}}}"#,
        hash = manifest::sha256_hex(contents.as_bytes()),
        size = contents.len(),
    )
}

/// Build a manifest body with every field overridable, then sign it.
struct ManifestBuilder {
    source: String,
    domain: String,
    sequence: u64,
    base_url: String,
    rollback_to: String,
    delete_after: String,
    apps: String,
}

impl Default for ManifestBuilder {
    fn default() -> Self {
        Self {
            source: "ota".to_string(),
            domain: DOMAIN.to_string(),
            sequence: 1,
            base_url: format!("https://{DOMAIN}/bundles"),
            rollback_to: "null".to_string(),
            delete_after: "null".to_string(),
            apps: format!(
                r#"[{{"app_id":"app","bundle_version":"0.1.1","files":[{}]}}]"#,
                file_entry("index.html", "<html>ota</html>")
            ),
        }
    }
}

impl ManifestBuilder {
    fn sequence(mut self, n: u64) -> Self {
        self.sequence = n;
        self
    }
    fn base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }
    fn domain(mut self, domain: &str) -> Self {
        self.domain = domain.to_string();
        self
    }
    fn source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self
    }
    fn apps(mut self, apps: &str) -> Self {
        self.apps = apps.to_string();
        self
    }
    fn rollback_to(mut self, version: &str) -> Self {
        self.rollback_to = format!("\"{version}\"");
        self
    }
    fn delete_after(mut self, version: &str) -> Self {
        self.delete_after = format!("\"{version}\"");
        self
    }

    fn body(&self, signature: &str) -> String {
        format!(
            r#"{{"schema":1,"source":"{source}","manifest_domain":"{domain}",
                "created_at":"2026-07-23T10:00:00Z","sequence":{sequence},
                "base_url":"{base_url}","rollback_to":{rollback_to},
                "delete_after":{delete_after},"signature":{signature},
                "apps":{apps}}}"#,
            source = self.source,
            domain = self.domain,
            sequence = self.sequence,
            base_url = self.base_url,
            rollback_to = self.rollback_to,
            delete_after = self.delete_after,
            apps = self.apps,
        )
    }

    /// The signed document a real CDN would serve.
    fn signed(&self, pair: &KeyPair) -> String {
        let signature =
            manifest::sign_manifest(&self.body("null"), &pair.private.unwrap()).expect("sign");
        self.body(&format!("\"{signature}\""))
    }

    /// The same document with no signature at all.
    fn unsigned(&self) -> String {
        self.body("null")
    }
}

// ── Signature ───────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn a_correctly_signed_manifest_is_accepted() {
    let pair = signing_keypair("accept");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let plan = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect("a manifest we signed must be accepted");

    assert_eq!(plan.manifest.sequence, 1);
    assert_eq!(plan.downloads.len(), 1);
}

#[test]
#[traced_test]
fn a_manifest_signed_by_another_key_is_rejected() {
    let ours = signing_keypair("theirs_ours");
    let theirs = signing_keypair("theirs_theirs");
    let manager = manager_with(Some(ours.public), Some(DOMAIN));

    let err = manager
        .process_manifest(&ManifestBuilder::default().signed(&theirs))
        .expect_err("only the holder of our private key may declare an update");
    assert!(err.contains("rejected"), "got {err:?}");
}

#[test]
#[traced_test]
fn an_unsigned_manifest_is_rejected() {
    let pair = signing_keypair("unsigned");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    assert!(
        manager
            .process_manifest(&ManifestBuilder::default().unsigned())
            .is_err(),
        "a manifest with no signature authenticates nothing"
    );
}

#[test]
#[traced_test]
fn a_tampered_manifest_body_is_rejected() {
    let pair = signing_keypair("tampered");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    // A CDN attacker replaying our signature over a hash they control.
    let signed = ManifestBuilder::default().signed(&pair);
    let tampered = signed.replace(
        &manifest::sha256_hex(b"<html>ota</html>"),
        &"f".repeat(64),
    );

    assert!(
        manager.process_manifest(&tampered).is_err(),
        "the signature covers the declared hashes, so swapping one must break it"
    );
}

#[test]
#[traced_test]
fn a_build_without_a_baked_key_has_no_ota_at_all() {
    let pair = signing_keypair("nokey");
    // No public key baked in — key generation is automatic, so this is a
    // misconfigured build, not a mode we support.
    let manager = manager_with(None, Some(DOMAIN));

    let err = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect_err("without a key there is no way to know who wrote a manifest");
    assert!(err.contains("OTA is disabled"), "got {err:?}");
}

#[test]
#[traced_test]
fn an_apk_manifest_is_not_accepted_as_an_ota_update() {
    let pair = signing_keypair("apk_source");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let err = manager
        .process_manifest(&ManifestBuilder::default().source("apk").signed(&pair))
        .expect_err("a bundled manifest describes what already shipped, not an update");
    assert!(err.contains("source=apk"), "got {err:?}");
}

// ── Domain and URL derivation ───────────────────────────────────────────

#[test]
#[traced_test]
fn a_manifest_claiming_a_different_domain_is_rejected() {
    let pair = signing_keypair("domain_mismatch");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let err = manager
        .process_manifest(&ManifestBuilder::default().domain("evil.example.com").signed(&pair))
        .expect_err("the baked domain must match");
    assert!(err.contains("domain mismatch"), "got {err:?}");
}

#[test]
#[traced_test]
fn a_base_url_outside_the_baked_domain_is_rejected() {
    let pair = signing_keypair("base_url");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    for hostile in [
        // SSRF at cloud metadata.
        "http://169.254.169.254/latest/",
        // Loopback — a local service the app can reach but we cannot audit.
        "http://127.0.0.1:8080/bundles",
        // Plaintext to the right host is still not the right origin.
        "http://cdn.test/bundles",
        // Prefix confusion: a host that merely starts with ours.
        "https://cdn.test.evil.example.com/bundles",
    ] {
        let outcome =
            manager.process_manifest(&ManifestBuilder::default().base_url(hostile).signed(&pair));
        assert!(
            outcome.is_err(),
            "{hostile:?} must be rejected — the baked domain is a routing guard, \
             and a signed manifest may still name a host we will not talk to"
        );
    }

    // The legitimate origin, and a deeper path under it, must still pass.
    manager
        .process_manifest(
            &ManifestBuilder::default()
                .base_url(&format!("https://{DOMAIN}/bundles/eu"))
                .signed(&pair),
        )
        .expect("a sub-path of the baked domain is exactly what we expect");
}

#[test]
#[traced_test]
fn download_urls_are_derived_never_taken_from_the_manifest() {
    let pair = signing_keypair("derived_urls");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let plan = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect("accept");

    assert_eq!(
        plan.downloads[0].url,
        format!("https://{DOMAIN}/bundles/app/0.1.1/index.html"),
        "a per-file URL from the manifest would be a redirect vector"
    );
    assert_eq!(plan.downloads[0].vfs_path, "/app/v0.1.1/index.html");
}

#[test]
#[traced_test]
fn a_build_without_a_baked_domain_refuses_before_looking_at_content() {
    let pair = signing_keypair("nodomain");
    let manager = manager_with(Some(pair.public), None);

    let err = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect_err("no domain means no OTA");
    assert!(err.contains("OTA is disabled"), "got {err:?}");
}

// ── Replay ──────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn a_replayed_sequence_is_rejected_and_a_newer_one_accepted() {
    let pair = signing_keypair("replay");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    manager
        .process_manifest(&ManifestBuilder::default().sequence(5).signed(&pair))
        .expect("first acceptance");
    manager.commit_manifest_sequence(5).expect("commit");

    for stale in [1, 4, 5] {
        let err = manager
            .process_manifest(&ManifestBuilder::default().sequence(stale).signed(&pair))
            .expect_err("a manifest at or below the watermark is a replay");
        assert!(err.contains("not newer"), "sequence {stale} gave {err:?}");
    }

    manager
        .process_manifest(&ManifestBuilder::default().sequence(6).signed(&pair))
        .expect("a strictly greater sequence must be accepted");
}

#[test]
#[traced_test]
fn the_accepted_sequence_survives_a_restart() {
    let pair = signing_keypair("seq_persist");
    // One backing store, two managers — the second stands in for a relaunch.
    let fs = DynFs::new(Arc::new(MemoryFs::new()));

    let first = PlatformAssetManager::from_vfs(
        fs.clone(),
        PathBuf::from("/base"),
        "0.1.0",
        AssetLayout::Versioned,
        Some(DOMAIN.to_string()),
        Some(pair.public),
    );
    first.commit_manifest_sequence(9).expect("commit");

    let after_restart = PlatformAssetManager::from_vfs(
        fs,
        PathBuf::from("/base"),
        "0.1.0",
        AssetLayout::Versioned,
        Some(DOMAIN.to_string()),
        Some(pair.public),
    );

    assert_eq!(
        after_restart.last_manifest_sequence(),
        9,
        "replay protection that resets on restart is not replay protection"
    );
    assert!(
        after_restart
            .process_manifest(&ManifestBuilder::default().sequence(9).signed(&pair))
            .is_err(),
        "the watermark must still reject what we already accepted"
    );
}

#[test]
#[traced_test]
fn the_watermark_only_moves_after_a_successful_install() {
    let pair = signing_keypair("seq_uncommitted");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    manager
        .process_manifest(&ManifestBuilder::default().sequence(3).signed(&pair))
        .expect("accept");

    assert_eq!(
        manager.last_manifest_sequence(),
        0,
        "verification alone must not burn the sequence — a download that fails \
         halfway has to stay retryable"
    );
}

// ── Entry validation ────────────────────────────────────────────────────

#[test]
#[traced_test]
fn file_paths_that_escape_their_bundle_are_rejected() {
    let pair = signing_keypair("traversal");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    // `\\\\` in Rust is `\\\\` in the JSON source, which serde parses to a single
    // backslash. Writing `\\` here would emit `\\b`, a valid JSON escape for
    // backspace, and the validator would never see a separator at all.
    for hostile in [
        "../../../etc/passwd",
        "/etc/passwd",
        "a\\\\b.js",
        "nested/../../out.js",
    ] {
        let apps = format!(
            r#"[{{"app_id":"app","bundle_version":"0.1.1","files":[{{"path":"{hostile}","sha256":"{h}","size":3}}]}}]"#,
            h = "a".repeat(64),
        );
        assert!(
            manager
                .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
                .is_err(),
            "{hostile:?} must be rejected"
        );
    }
}

#[test]
#[traced_test]
fn app_ids_that_escape_their_directory_are_rejected() {
    let pair = signing_keypair("app_id_traversal");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    for hostile in ["../other", "a/b", "", "a\\\\b"] {
        let apps = format!(
            r#"[{{"app_id":"{hostile}","bundle_version":"0.1.1","files":[]}}]"#
        );
        assert!(
            manager
                .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
                .is_err(),
            "app_id {hostile:?} must be rejected"
        );
    }
}

#[test]
#[traced_test]
fn a_bundle_version_that_is_not_semver_is_rejected() {
    let pair = signing_keypair("bad_version");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    for hostile in ["latest", "../0.1.0", "0.1", "0.1.0/../.."] {
        let apps = format!(
            r#"[{{"app_id":"app","bundle_version":"{hostile}","files":[]}}]"#
        );
        assert!(
            manager
                .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
                .is_err(),
            "bundle_version {hostile:?} must be rejected"
        );
    }
}

#[test]
#[traced_test]
fn a_file_larger_than_the_limit_is_rejected() {
    let pair = signing_keypair("too_big");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let apps = format!(
        r#"[{{"app_id":"app","bundle_version":"0.1.1","files":[{{"path":"huge.bin","sha256":"{h}","size":52428801}}]}}]"#,
        h = "a".repeat(64),
    );
    let err = manager
        .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
        .expect_err("a declared size above the cap must be refused before any fetch");
    assert!(err.contains("limit is"), "got {err:?}");
}

#[test]
#[traced_test]
fn a_manifest_with_too_many_files_is_rejected() {
    let pair = signing_keypair("too_many");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let files: Vec<String> = (0..=MAX_OTA_FILES)
        .map(|i| format!(r#"{{"path":"f{i}.js","sha256":"{h}","size":1}}"#, h = "a".repeat(64)))
        .collect();
    let apps = format!(
        r#"[{{"app_id":"app","bundle_version":"0.1.1","files":[{}]}}]"#,
        files.join(",")
    );

    let err = manager
        .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
        .expect_err("an unbounded manifest is a denial-of-service vector");
    assert!(err.contains("limit is"), "got {err:?}");
}

#[test]
#[traced_test]
fn a_malformed_sha256_is_rejected() {
    let pair = signing_keypair("bad_hash");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    for hostile in ["", "abc", &"z".repeat(64), &"a".repeat(63)] {
        let apps = format!(
            r#"[{{"app_id":"app","bundle_version":"0.1.1","files":[{{"path":"a.js","sha256":"{hostile}","size":1}}]}}]"#
        );
        assert!(
            manager
                .process_manifest(&ManifestBuilder::default().apps(&apps).signed(&pair))
                .is_err(),
            "sha256 {hostile:?} must be rejected — a hash we cannot compare to is not a check"
        );
    }
}

#[test]
#[traced_test]
fn rollback_and_delete_directives_must_be_semver() {
    let pair = signing_keypair("directives");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    assert!(manager
        .process_manifest(&ManifestBuilder::default().rollback_to("../etc").signed(&pair))
        .is_err());
    assert!(manager
        .process_manifest(&ManifestBuilder::default().delete_after("latest").signed(&pair))
        .is_err());

    let plan = manager
        .process_manifest(
            &ManifestBuilder::default()
                .rollback_to("0.1.0")
                .delete_after("0.0.9")
                .signed(&pair),
        )
        .expect("valid directives must pass through");
    assert_eq!(plan.manifest.rollback_to.as_deref(), Some("0.1.0"));
    assert_eq!(plan.manifest.delete_after.as_deref(), Some("0.0.9"));
}

// ── Manifest provenance ─────────────────────────────────────────────────

#[test]
#[traced_test]
fn a_version_directory_keeps_the_manifest_that_produced_it() {
    let pair = signing_keypair("provenance");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let plan = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect("accept");
    manager
        .write_version_manifest("app", "0.1.1", &plan.manifest)
        .expect("write provenance");

    let stored = manager
        .read_version_manifest("app", "0.1.1")
        .expect("the version directory must describe itself");
    assert_eq!(stored.sequence, plan.manifest.sequence);
    assert!(
        stored.signature.is_some(),
        "the stored copy must keep its signature so it stays independently verifiable"
    );
}

#[test]
#[traced_test]
fn the_stored_manifest_still_verifies_against_the_baked_key() {
    let pair = signing_keypair("provenance_verify");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let plan = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect("accept");
    manager
        .write_version_manifest("app", "0.1.1", &plan.manifest)
        .expect("write");

    let bytes = manager
        .read(&format!("/app/v0.1.1/{MANIFEST_FILENAME}"))
        .expect("read stored manifest");
    let json = String::from_utf8(bytes).expect("utf-8");

    manifest::verify_manifest(&json, &pair.public)
        .expect("round-tripping through storage must not invalidate the signature");
}

#[test]
#[traced_test]
fn no_staging_file_survives_writing_a_manifest() {
    let pair = signing_keypair("staging");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));

    let plan = manager
        .process_manifest(&ManifestBuilder::default().signed(&pair))
        .expect("accept");
    manager
        .write_version_manifest("app", "0.1.1", &plan.manifest)
        .expect("write");

    assert!(
        !manager
            .exists(&format!("/app/v0.1.1/{MANIFEST_FILENAME}.tmp"))
            .expect("exists"),
        "the staged copy must be renamed, not left beside the real one"
    );
}

#[test]
#[traced_test]
fn reading_a_manifest_from_a_version_that_has_none_returns_nothing() {
    let pair = signing_keypair("no_manifest");
    let manager = manager_with(Some(pair.public), Some(DOMAIN));
    manager.write("/app/v0.1.0/index.html", b"<html/>").expect("seed");

    assert!(
        manager.read_version_manifest("app", "0.1.0").is_none(),
        "absence must be reported, not faked"
    );
}
