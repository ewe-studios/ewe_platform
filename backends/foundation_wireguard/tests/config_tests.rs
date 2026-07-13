//! Integration tests for WgConfig — macro, builder, and TOML convergence (spec-55, F09).
//!
//! Tests that the same network expressed via macro, builder, and TOML all produce
//! equivalent configurations and working nodes.

#![cfg(not(target_family = "wasm"))]

use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use foundation_wireguard::{NetworkId, SeedBits, WgConfig, WgSeed};
use std::sync::Mutex;
use tracing_test::traced_test;

/// Helper: generate a fresh 256-bit seed + its network id.
fn fresh_seed() -> (WgSeed, NetworkId) {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("generate seed");
    let net = seed.derive_network_id();
    (seed, net)
}

// ---------------------------------------------------------------------------
// Builder convergence
// ---------------------------------------------------------------------------

#[traced_test]
#[test]
fn builder_minimal_produces_valid_config() {
    let (seed, net) = fresh_seed();
    let cfg = WgConfig::builder()
        .seed(seed)
        .network_id(net)
        .build()
        .expect("build");
    assert_eq!(cfg.network_id(), net);
    assert_eq!(cfg.udp_listen().port(), 0); // ephemeral default
    assert!(!cfg.relay.advertise);
}

#[traced_test]
#[test]
fn builder_full_config_round_trips() {
    let (seed, net) = fresh_seed();
    let cfg = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .udp_listen("127.0.0.1:9999".parse::<SocketAddr>().expect("addr"))
        .bootstrap_listen("127.0.0.1:8443".parse::<SocketAddr>().expect("addr"))
        .mtu(1400)
        .keepalive_secs(30)
        .relay_advertise(true)
        .relay_max_sessions(512)
        .mtls(true)
        .identity_path("/ewe/wg/identity.key")
        .build()
        .expect("build");

    assert_eq!(cfg.wg_seed().as_bytes(), seed.as_bytes());
    assert_eq!(cfg.udp_listen().port(), 9999);
    assert_eq!(cfg.bootstrap_listen().port(), 8443);
    assert_eq!(cfg.dataplane.mtu, 1400);
    assert_eq!(cfg.dataplane.keepalive_secs, 30);
    assert!(cfg.relay.advertise);
    assert_eq!(cfg.relay.max_sessions, 512);
    assert!(cfg.security.mtls);
    assert_eq!(cfg.node.identity_path.as_deref(), Some("/ewe/wg/identity.key"));
}

#[traced_test]
#[test]
fn builder_requires_seed_no_silent_default() {
    let err = WgConfig::builder().build().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("seed"), "expected seed-related error, got: {msg}");
}

#[traced_test]
#[test]
fn seed_and_joiner_constructors_preserved() {
    let (seed, net) = fresh_seed();

    // Seed constructor: no join targets.
    let cfg = WgConfig::seed(seed.clone(), net);
    assert!(cfg.seed_endpoints().is_empty());

    // Joiner constructor: has join targets.
    let ep = "10.0.0.1:51820".parse().expect("addr");
    let cfg = WgConfig::joiner(seed, net, vec![ep]);
    assert_eq!(cfg.seed_endpoints().len(), 1);
    assert_eq!(cfg.seed_endpoints()[0], ep);
}

// ---------------------------------------------------------------------------
// TOML loading
// ---------------------------------------------------------------------------

#[traced_test]
#[test]
fn toml_round_trip_preserves_config() {
    let (seed, net) = fresh_seed();
    let orig = WgConfig::seed(seed.clone(), net);

    let toml_str = toml::to_string_pretty(&orig).expect("serialize");
    let round: WgConfig = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(orig.wg_seed().as_bytes(), round.wg_seed().as_bytes());
    assert_eq!(orig.network_id(), round.network_id());
    assert_eq!(orig.udp_listen(), round.udp_listen());
}

#[traced_test]
#[test]
fn load_file_succeeds_on_valid_toml() {
    let (seed, net) = fresh_seed();
    let cfg = WgConfig::seed(seed, net);
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(toml_str.as_bytes()).expect("write");
    tmp.flush().expect("flush");

    let loaded = WgConfig::load_file(tmp.path()).expect("load_file");
    assert_eq!(cfg.wg_seed().as_bytes(), loaded.wg_seed().as_bytes());
}

#[traced_test]
#[test]
fn load_file_fails_on_missing_file() {
    let err = WgConfig::load_file("/nonexistent/wireguard.toml").unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("cannot read"), "expected file error, got: {msg}");
}

#[traced_test]
#[test]
fn load_file_fails_on_malformed_toml() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(b"this is not toml {{{").expect("write");
    tmp.flush().expect("flush");

    let err = WgConfig::load_file(tmp.path()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("invalid TOML"), "expected TOML error, got: {msg}");
}

// ---------------------------------------------------------------------------
// from_env — serialized because env vars are process-global.
// An EnvGuard saves/restores the original env so panics don't leak state
// and concurrent test binaries don't see each other's values.
// ---------------------------------------------------------------------------

/// Saves the WG_* env before a test and restores it on drop.
struct EnvGuard {
    secret: Option<String>,
    network: Option<String>,
    endpoints: Option<String>,
}

impl EnvGuard {
    fn save() -> Self {
        Self {
            secret: std::env::var("WG_SECRET").ok(),
            network: std::env::var("WG_NETWORK").ok(),
            endpoints: std::env::var("WG_SEED_ENDPOINTS").ok(),
        }
    }

    fn restore(&self) {
        let set_or_remove = |var: &str, saved: &Option<String>| match saved {
            Some(val) => std::env::set_var(var, val),
            None => std::env::remove_var(var),
        };
        set_or_remove("WG_SECRET", &self.secret);
        set_or_remove("WG_NETWORK", &self.network);
        set_or_remove("WG_SEED_ENDPOINTS", &self.endpoints);
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

fn setup_env(seed_b64: &str) -> EnvGuard {
    let guard = EnvGuard::save();
    std::env::set_var("WG_SECRET", seed_b64);
    std::env::remove_var("WG_NETWORK");
    std::env::remove_var("WG_SEED_ENDPOINTS");
    guard
}

/// WHY: Process-global env var writes must be serialised across all tests
/// (including tests in other binaries). A cross-process file lock would be
/// overkill; a static Mutex at least serialises within *this* binary, and
/// the `EnvGuard` (Drop restore) prevents leaking state to other binaries.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[traced_test]
#[test]
fn from_env_reads_wg_secret() {
    let _lock = ENV_MUTEX.lock().expect("env lock");
    let (seed, _net) = fresh_seed();
    let seed_b64 = seed.to_base64url();
    let _guard = setup_env(&seed_b64);

    let cfg = WgConfig::from_env().expect("from_env");
    assert_eq!(cfg.wg_seed().as_bytes(), seed.as_bytes());
}

#[traced_test]
#[test]
fn from_env_missing_secret_is_error() {
    let _lock = ENV_MUTEX.lock().expect("env lock");
    let _guard = EnvGuard::save();
    std::env::remove_var("WG_SECRET");
    assert!(WgConfig::from_env().is_err());
}

#[traced_test]
#[test]
fn from_env_with_network_and_endpoints() {
    let _lock = ENV_MUTEX.lock().expect("env lock");
    let (seed, _net) = fresh_seed();
    let seed_b64 = seed.to_base64url();
    let explicit_net = seed.derive_network_id();
    let net_hex = explicit_net.to_hex();
    let _guard = EnvGuard::save();

    std::env::set_var("WG_SECRET", &seed_b64);
    std::env::set_var("WG_NETWORK", &net_hex);
    std::env::set_var("WG_SEED_ENDPOINTS", "10.0.0.1:51820,10.0.0.2:51820");

    let cfg = WgConfig::from_env().expect("from_env");
    assert_eq!(cfg.wg_seed().as_bytes(), seed.as_bytes());
    assert_eq!(cfg.network_id(), explicit_net);
    assert_eq!(cfg.seed_endpoints().len(), 2);
    assert_eq!(cfg.seed_endpoints()[0].to_string(), "10.0.0.1:51820");
    assert_eq!(cfg.seed_endpoints()[1].to_string(), "10.0.0.2:51820");
}

// ---------------------------------------------------------------------------
// Two-node mesh from config (end-to-end F09 verification)
// ---------------------------------------------------------------------------

#[traced_test]
#[test]
fn two_nodes_join_from_config() {
    use foundation_wireguard::native::WgNode;

    let (seed, net) = fresh_seed();

    // Node A: seed — constructed programmatically via builder.
    let cfg_a = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .build()
        .expect("build A");

    let handle_a = WgNode::from_config(cfg_a).join().expect("join A");
    let a_boot = handle_a.bootstrap_addr();

    // Same config via TOML round-trip (verify equivalence).
    let toml_str = toml::to_string_pretty(&WgConfig::seed(seed.clone(), net)).expect("serialize");
    let cfg_b_toml: WgConfig = toml::from_str(&toml_str).expect("deserialize");
    let cfg_b = WgConfig::builder()
        .seed(cfg_b_toml.wg_seed().clone())
        .network_id(cfg_b_toml.network_id())
        .seed_endpoint(a_boot)
        .build()
        .expect("build B");

    // Node B: joiner from A.
    let handle_b = WgNode::from_config(cfg_b).join().expect("join B");

    // Verify mutual gossip discovery (this is what F09 is about: nodes formed from config join).
    assert!(
        handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_secs(5)),
        "node A discovered B"
    );
    assert!(
        handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_secs(5)),
        "node B discovered A"
    );

    // Verify identities are accessible.
    assert_ne!(handle_a.identity(), handle_b.identity());

    handle_a.shutdown();
    handle_b.shutdown();
}
