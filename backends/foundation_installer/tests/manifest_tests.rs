//! Manifest parsing and validation against the spec 78 fixtures.
//!
//! The files in `tests/fixtures/manifest/` are byte-identical copies of the
//! canonical spec 78 fixtures. `scripts/ci/manifest_contract.py --check-copies`
//! holds the two copies together.

use std::path::PathBuf;

use foundation_installer::{InstallError, Manifest};
use serde_json::Value;

fn fixture(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/manifest")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

fn parse(relative: &str) -> Result<Manifest, InstallError> {
    Manifest::parse(&fixture(relative))
}

fn parse_faults(relative: &str) -> Vec<foundation_installer::FieldError> {
    match parse(relative) {
        Err(InstallError::Parse(faults)) => faults,
        other => panic!("expected a parse error for {relative}, got {other:?}"),
    }
}

#[test]
fn complete_fixture_parses_every_field() {
    let manifest = parse("valid/complete.json").expect("the complete fixture parses");
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.entries.len(), 3);
    assert_eq!(manifest.entries[0].name.as_str(), "agent-rpc");
    assert_eq!(manifest.entries[0].version, "0.4.0");
    assert_eq!(manifest.entries[1].name.as_str(), "ripgrep");
    assert_eq!(manifest.entries[2].name.as_str(), "vector-db");
    assert_eq!(manifest.entries[2].deploy.kind(), "docker");
}

#[test]
fn proxy_and_probes_pass_through_unread() {
    let text = fixture("valid/complete.json");
    let input: Value = serde_json::from_str(&text).expect("valid JSON");
    let manifest = Manifest::parse(&text).expect("the complete fixture parses");
    assert_eq!(
        manifest.entries[0].proxy.as_ref(),
        input["entries"][0].get("proxy")
    );
    assert_eq!(
        manifest.entries[0].probes.as_ref(),
        input["entries"][0].get("probes")
    );
}

#[test]
fn unknown_fields_are_ignored() {
    let manifest = parse("valid/unknown-field.json").expect("an unknown field is ignored");
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].name.as_str(), "ripgrep");
}

#[test]
fn empty_entries_are_valid() {
    let manifest = parse("valid/empty.json").expect("an empty list is valid");
    assert_eq!(manifest.entries.len(), 0);
}

#[test]
fn each_deploy_kind_parses() {
    assert_eq!(
        parse("valid/deploy-systemd.json")
            .expect("systemd parses")
            .entries[0]
            .deploy
            .kind(),
        "systemd"
    );
    assert_eq!(
        parse("valid/deploy-docker.json")
            .expect("docker parses")
            .entries[0]
            .deploy
            .kind(),
        "docker"
    );
    assert_eq!(
        parse("valid/deploy-none.json")
            .expect("none parses")
            .entries[0]
            .deploy
            .kind(),
        "none"
    );
}

#[test]
fn marks_fixture_names_the_entry_version() {
    let manifest = parse("valid/marks.json").expect("marks parses");
    assert_eq!(manifest.entries[0].name.as_str(), "agent-rpc");
    assert_eq!(manifest.entries[0].version, "0.4.0");
}

#[test]
fn version_two_is_unsupported() {
    match parse("invalid/v3-version-2.json") {
        Err(InstallError::UnsupportedManifestVersion { found, supported }) => {
            assert_eq!(found, 2);
            assert_eq!(supported, 1);
        }
        other => panic!("expected UnsupportedManifestVersion, got {other:?}"),
    }
}

#[test]
fn duplicate_name_fails_at_the_second_name() {
    let faults = parse_faults("invalid/v5-duplicate-name.json");
    assert_eq!(faults[0].path, "/entries/1/name");
    assert!(faults[0].message.contains("repeats"));
}

#[test]
fn unknown_source_kind_names_entry_and_field() {
    match parse("invalid/v6-unknown-kind.json") {
        Err(InstallError::UnknownKind { path, kind }) => {
            assert_eq!(path, "/entries/0/source/kind");
            assert_eq!(kind, "snap");
        }
        other => panic!("expected UnknownKind, got {other:?}"),
    }
}

#[test]
fn plain_http_url_is_rejected() {
    let faults = parse_faults("invalid/v7-http-url.json");
    assert_eq!(faults[0].path, "/entries/0/source/url");
}

#[test]
fn bad_sha256_is_rejected() {
    let faults = parse_faults("invalid/v7-bad-sha256.json");
    assert_eq!(faults[0].path, "/entries/0/source/sha256");
}

#[test]
fn a_bad_name_fails_with_its_field() {
    let text = r#"{
        "manifest_version": 1,
        "entries": [{
            "name": "Agent_RPC",
            "version": "1.0.0",
            "source": { "kind": "cargo", "crate": "x" },
            "deploy": { "kind": "none" }
        }]
    }"#;
    match Manifest::parse(text) {
        Err(InstallError::Parse(faults)) => {
            assert_eq!(faults[0].path, "/entries/0/name");
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn an_incompatible_target_fails_before_any_download() {
    let text = r#"{
        "manifest_version": 1,
        "entries": [{
            "name": "ripgrep",
            "version": "14.1.0",
            "source": { "kind": "cargo", "crate": "ripgrep" },
            "deploy": { "kind": "docker" }
        }]
    }"#;
    match Manifest::parse(text) {
        Err(InstallError::Incompatible {
            entry,
            deploy,
            artifact,
        }) => {
            assert_eq!(entry, "ripgrep");
            assert_eq!(deploy, "docker");
            assert_eq!(artifact, "tool");
        }
        other => panic!("expected Incompatible, got {other:?}"),
    }
}

#[test]
fn a_missing_target_field_is_a_field_error() {
    let text = r#"{
        "manifest_version": 1,
        "entries": [{
            "name": "agent-rpc",
            "version": "1.0.0",
            "source": { "kind": "url", "sha256": "0000000000000000000000000000000000000000000000000000000000000000" },
            "deploy": { "kind": "none" }
        }]
    }"#;
    let faults = match Manifest::parse(text) {
        Err(InstallError::Parse(faults)) => faults,
        other => panic!("expected a parse error, got {other:?}"),
    };
    assert_eq!(faults[0].path, "/entries/0/source/url");
}

#[test]
fn a_manifest_over_the_limit_is_refused() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("manifest.json");
    std::fs::write(&path, "x".repeat(4096)).expect("write the manifest");
    match Manifest::parse_file(&path, 1024) {
        Err(InstallError::ManifestTooLarge { size, max }) => {
            assert_eq!(size, 4096);
            assert_eq!(max, 1024);
        }
        other => panic!("expected ManifestTooLarge, got {other:?}"),
    }
}
