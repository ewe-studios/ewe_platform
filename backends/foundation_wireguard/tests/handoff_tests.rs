//! Identity handoff and seed revocation tests (spec-55, feature 08).
//!
//! WHY: Proves success criterion 4 — post-handoff peers use per-peer identity keys (not the
//! shared bootstrap key), and a revoked seed blocks a fresh join.
#![cfg(not(target_family = "wasm"))]

use std::thread;
use std::time::Duration;

use foundation_wireguard::native::{WgConfig, WgNode};
use foundation_wireguard::{IdentityKeypair, WgSeed};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn identity_persists_across_restart() {
    let identity = IdentityKeypair::generate().expect("gen");
    let secret_bytes = identity.to_secret_bytes();

    // Simulate a restart: reconstruct from the persisted secret.
    let restored = IdentityKeypair::from_secret_bytes(secret_bytes);
    assert_eq!(
        restored.public().as_bytes(),
        identity.public().as_bytes(),
        "restored identity has the same public key"
    );
}

#[test]
#[traced_test]
fn mesh_uses_identity_keys_and_revoked_seed_blocks_new_join() {
    let seed = WgSeed::from_bytes(&[0x2Bu8; 32]).unwrap();
    let network_id = seed.derive_network_id();
    let bootstrap = seed.derive_bootstrap(&network_id);

    let node_a = WgNode::from_config(WgConfig::seed(seed.clone(), network_id))
        .join()
        .expect("A joins");
    let a_boot = node_a.bootstrap_addr();

    // B joins while the seed is valid.
    let node_b = WgNode::from_config(WgConfig::joiner(seed.clone(), network_id, vec![a_boot]))
        .join()
        .expect("B joins");

    // Success criterion 4: peers use their own random identity keys, NOT the shared
    // bootstrap key.
    assert_ne!(
        node_b.identity().as_bytes(),
        bootstrap.public.as_bytes(),
        "B's mesh identity is a per-peer key, not the shared bootstrap key"
    );
    assert_ne!(node_a.identity().as_bytes(), node_b.identity().as_bytes());

    // Revoke the seed on A; a later joiner presenting the same seed is refused.
    node_a.revoke_seed();
    // Give the revocation a moment (atomic store is immediate, but be tolerant).
    thread::sleep(Duration::from_millis(50));

    let node_c = WgNode::from_config(WgConfig::joiner(seed.clone(), network_id, vec![a_boot])).join();
    assert!(
        node_c.is_err(),
        "a fresh join with the revoked seed must be refused"
    );

    node_a.shutdown();
    node_b.shutdown();
}
