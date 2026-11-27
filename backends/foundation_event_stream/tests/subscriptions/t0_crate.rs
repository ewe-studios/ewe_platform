//! T0: the crate surface.

use foundation_event_stream::shared::SubscriptionConfig;
use foundation_event_stream::{durable, Jwks};

/// Compile-time proof that a type is `Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn durable_types_match_the_core() {
    let core = foundation_core::durable::StreamName::parse("events/a").expect("valid");
    let reexported: durable::StreamName = core;
    assert_eq!(reexported.as_str(), "events/a");

    let jwks = Jwks {
        keys: Vec::new(),
        issuer: None,
    };
    let auth_jwks: foundation_auth::shared::jwks::Jwks = jwks;
    assert!(auth_jwks.keys.is_empty());

    assert_send_sync::<foundation_event_stream::SubscriptionEngine>();
    assert_send_sync::<SubscriptionConfig>();
}

#[test]
fn crate_builds_wasm() {
    if !wasm_target_installed() {
        eprintln!("wasm32-unknown-unknown is not installed; the CI wasm gate covers this build");
        return;
    }
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "check",
            "--quiet",
            "-p",
            "foundation_event_stream",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("cargo check starts");
    assert!(
        status.success(),
        "the crate must build for wasm32-unknown-unknown"
    );
}

fn wasm_target_installed() -> bool {
    std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .is_ok_and(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim() == "wasm32-unknown-unknown")
        })
}
