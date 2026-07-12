//! Identity handoff, seed revocation, and optional mTLS tests (spec-55, feature 08).
//!
//! WHY: Proves success criterion 4 — post-handoff peers use per-peer identity keys (not
//! the shared bootstrap key), a revoked seed blocks a fresh join, and optional app-layer
//! mTLS authenticates peers by their WG identity.
#![cfg(not(target_family = "wasm"))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use foundation_wireguard::native::{mtls, WgConfig, WgNode};
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
fn shared_psk_is_symmetric_and_identity_and_context_bound() {
    let a = IdentityKeypair::generate().unwrap();
    let b = IdentityKeypair::generate().unwrap();
    let c = IdentityKeypair::generate().unwrap();
    let ctx = b"session-1";

    let ab = a.derive_shared_psk(&b.public(), ctx);
    let ba = b.derive_shared_psk(&a.public(), ctx);
    assert_eq!(ab, ba, "x25519 ECDH PSK is symmetric between the two identities");

    let ac = a.derive_shared_psk(&c.public(), ctx);
    assert_ne!(ab, ac, "a different peer yields a different PSK");

    let ab_other_ctx = a.derive_shared_psk(&b.public(), b"session-2");
    assert_ne!(ab, ab_other_ctx, "a different context yields a different PSK");
}

#[test]
#[traced_test]
fn mtls_authenticates_peers_by_identity() {
    let a = IdentityKeypair::generate().unwrap();
    let b = IdentityKeypair::generate().unwrap();
    let ctx = b"overlay-mtls";

    let psk_client = a.derive_shared_psk(&b.public(), ctx); // A → B
    let psk_server = b.derive_shared_psk(&a.public(), ctx); // B → A (identical)

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut tls = mtls::accept(tcp, psk_server).expect("server mtls handshake");
        let mut buf = [0u8; 64];
        let n = tls.read(&mut buf).unwrap();
        tls.write_all(&buf[..n]).unwrap();
        tls.flush().unwrap();
    });

    let tcp = TcpStream::connect(addr).unwrap();
    let mut tls = mtls::connect(tcp, psk_client).expect("client mtls handshake");
    tls.write_all(b"identity-authenticated").unwrap();
    tls.flush().unwrap();
    let mut buf = [0u8; 64];
    let n = tls.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"identity-authenticated", "mTLS carries app bytes");

    server.join().unwrap();
}

#[test]
#[traced_test]
fn mtls_rejects_wrong_identity() {
    let a = IdentityKeypair::generate().unwrap();
    let b = IdentityKeypair::generate().unwrap();
    let impostor = IdentityKeypair::generate().unwrap();
    let ctx = b"overlay-mtls";

    // Server B expects to talk to A.
    let psk_server = b.derive_shared_psk(&a.public(), ctx);
    // Impostor C tries to reach B (derives a PSK C↔B, which B will not match to A).
    let psk_impostor = impostor.derive_shared_psk(&b.public(), ctx);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        mtls::accept(tcp, psk_server).is_ok()
    });

    let tcp = TcpStream::connect(addr).unwrap();
    let result = mtls::connect(tcp, psk_impostor);
    assert!(result.is_err(), "impostor's PSK must fail the handshake");
    assert!(!server.join().unwrap(), "server also rejects the impostor");
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
