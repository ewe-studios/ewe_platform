//! Manifest view tests (spec 82 feature 01, T2).
//!
//! The canonical fixtures are the spec 78 copies in `tests/fixtures/manifest`;
//! `scripts/ci/manifest_contract.py --check-copies` keeps them byte-identical.

mod support;

use foundation_registrar::manifest::ManifestView;
use foundation_registrar::RegistrarError;

fn fixture(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/manifest")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn the_complete_fixture_parses_the_proxy_and_probes() {
    let manifest = ManifestView::parse(&fixture("valid/complete.json")).expect("valid manifest");
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.entries.len(), 3);

    let agent = manifest.entry("agent-rpc").expect("agent-rpc");
    let proxy = agent.proxy.as_ref().expect("the proxy block");
    assert_eq!(proxy.service, "agents");
    assert_eq!(proxy.port, 7443);
    assert_eq!(proxy.protocol, "grpc");
    assert_eq!(proxy.route.host, "agents.example.com");
    assert_eq!(proxy.path_prefix(), "/");
    let probes = agent.probes.as_ref().expect("the probes block");
    assert_eq!(
        probes.startup.as_ref().expect("startup").period_secs,
        Some(2)
    );
    assert_eq!(
        probes.liveness.as_ref().expect("liveness").period_secs,
        Some(10)
    );
    assert_eq!(
        probes.readiness.as_ref().expect("readiness").check.kind(),
        "http"
    );

    // An entry without a proxy block is not registered.
    let ripgrep = manifest.entry("ripgrep").expect("ripgrep");
    assert!(ripgrep.proxy.is_none());
    assert_eq!(ripgrep.deploy.kind, "none");

    let vector = manifest.entry("vector-db").expect("vector-db");
    assert_eq!(vector.deploy.kind, "docker");
}

#[test]
fn the_marks_fixture_maps_a_unit_to_an_entry() {
    let manifest = ManifestView::parse(&fixture("valid/marks.json")).expect("marks");
    let agent = manifest.entry("agent-rpc").expect("agent-rpc");
    assert_eq!(agent.version, "0.4.0");
    assert_eq!(agent.proxy.as_ref().expect("proxy").port, 7443);
}

#[test]
fn an_unknown_field_is_ignored() {
    let manifest = ManifestView::parse(&fixture("valid/unknown-field.json"))
        .expect("an unknown field is ignored");
    assert_eq!(manifest.entries.len(), 1);
}

#[test]
fn an_empty_manifest_registers_nothing() {
    let manifest = ManifestView::parse(&fixture("valid/empty.json")).expect("empty");
    assert_eq!(manifest.entries.len(), 0);
    assert_eq!(manifest.watched(&[]).count(), 0);
}

#[test]
fn an_unsupported_version_is_refused() {
    let error = ManifestView::parse(&fixture("invalid/v3-version-2.json"))
        .expect_err("version 2 is refused");
    assert_eq!(
        error,
        RegistrarError::UnsupportedManifestVersion {
            found: 2,
            supported: 1
        }
    );
}

#[test]
fn a_duplicate_name_is_refused_at_its_path() {
    let error =
        ManifestView::parse(&fixture("invalid/v5-duplicate-name.json")).expect_err("duplicate");
    let text = error.to_string();
    assert!(
        text.contains("/entries/1/name"),
        "the error names the second name: {text}"
    );
}

#[test]
fn an_unknown_source_kind_is_refused_at_its_path() {
    let error =
        ManifestView::parse(&fixture("invalid/v6-unknown-kind.json")).expect_err("unknown kind");
    let text = error.to_string();
    assert!(
        text.contains("/entries/0/source/kind"),
        "the error names the source kind path: {text}"
    );
    assert!(
        text.contains("unknown kind"),
        "the error says unknown: {text}"
    );
}

#[test]
fn a_bad_probe_kind_fails_with_the_entry_named() {
    let text = serde_json::json!({
        "manifest_version": 1,
        "entries": [{
            "name": "agent-rpc",
            "version": "1",
            "source": { "kind": "url" },
            "deploy": { "kind": "systemd" },
            "probes": { "readiness": { "kind": "carrier-pigeon", "port": 1 } }
        }]
    })
    .to_string();
    let error = ManifestView::parse(&text).expect_err("unknown probe kind");
    let message = error.to_string();
    assert!(
        message.contains("entry agent-rpc"),
        "the entry name is in the error: {message}"
    );
    assert!(message.contains("carrier-pigeon"), "{message}");
}

#[test]
fn an_unknown_protocol_is_refused() {
    let text = serde_json::json!({
        "manifest_version": 1,
        "entries": [{
            "name": "agent-rpc",
            "version": "1",
            "source": { "kind": "url" },
            "deploy": { "kind": "systemd" },
            "proxy": {
                "service": "agents",
                "route": { "host": "agents.example.com" },
                "port": 7443,
                "protocol": "quic-ish"
            }
        }]
    })
    .to_string();
    let error = ManifestView::parse(&text).expect_err("unknown protocol");
    assert!(error.to_string().contains("/proxy/protocol"));
}
