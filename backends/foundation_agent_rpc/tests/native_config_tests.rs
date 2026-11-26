//! Native host configuration: the insecure guard and a missing operator key.
//!
//! Invariants I1 and I2 of feature 04.

#![cfg(unix)]
#![recursion_limit = "256"]

use std::sync::Arc;

use foundation_agent_rpc::host::native::{
    is_loopback_bind, NativeHost, NativeHostConfig, NativeSecurity,
};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;

/// A context with the real effect bundle installed.
fn ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

#[test]
fn native_loopback_bind_detection_accepts_only_a_loopback_address() {
    assert!(is_loopback_bind("127.0.0.1:0"));
    assert!(is_loopback_bind("127.0.0.1:7443"));
    assert!(is_loopback_bind("localhost:7443"));
    assert!(is_loopback_bind("[::1]:7443"));
    assert!(!is_loopback_bind("0.0.0.0:7443"));
    assert!(!is_loopback_bind("[::]:7443"));
    assert!(!is_loopback_bind("192.0.2.10:7443"));
    assert!(!is_loopback_bind("agents.example.com:7443"));
}

#[test]
fn native_client_csr_is_seeded() {
    let make = |seed: u64| {
        let ctx = Ctx::with_seed(Arc::new(MonotonicClock::new()), seed);
        foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
        foundation_agent_rpc::client::generate_csr_seeded("laptop", &ctx).expect("the seeded CSR")
    };

    let (key_a, csr_a) = make(42);
    let (key_b, csr_b) = make(42);
    assert_eq!(csr_a, csr_b, "seed 42 replays the CSR exactly");
    assert_eq!(key_a.as_str(), key_b.as_str(), "the key is seeded too");

    let (_, csr_c) = make(43);
    assert_ne!(csr_a, csr_c, "a different seed gives a different CSR");
}

#[valtron_test]
async fn native_insecure_plaintext_refuses_a_non_loopback_bind_before_it_binds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = NativeHostConfig::default()
        .with_state_dir(dir.path())
        .with_bind("0.0.0.0:0")
        .with_security(NativeSecurity::InsecurePlaintext);
    let error = NativeHost::start(config, ctx())
        .await
        .expect_err("a non-loopback insecure bind must be refused");
    let message = error.current_context().to_string();
    assert!(
        message.contains("non-loopback") && message.contains("InsecurePlaintext"),
        "{message}"
    );
    // The guard runs before any key or listener work.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .collect::<Result<_, _>>()
        .expect("entries");
    assert!(entries.is_empty(), "the refused start wrote {entries:?}");
}

#[valtron_test]
async fn native_missing_operator_key_names_the_command_that_makes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = NativeHostConfig::default()
        .with_state_dir(dir.path())
        .with_bind("127.0.0.1:0");
    let error = NativeHost::start(config, ctx())
        .await
        .expect_err("a missing signing key must stop the start");
    let message = error.current_context().to_string();
    assert!(
        message.contains("openssl genpkey -algorithm ED25519"),
        "the error must name the command: {message}"
    );
    // Invariant I1: the host never makes a key.
    assert!(
        !dir.path().join("bootstrap-signing-key.pem").exists(),
        "the host must not create the signing key"
    );
}
