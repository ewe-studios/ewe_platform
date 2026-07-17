//! Data-plane selection + kernel-TUN path tests (spec-55, feature 11).
//!
//! WHY: The smoltcp netstack plane is exercised end-to-end by `tunnel_tests`/`mesh_tests`,
//! but the kernel-TUN plane — `DataPlaneMode::Tun`, `MeshDataPlane::is_tun`, and the node's
//! kernel-UDP gossip branch — had no coverage at all. These assert plane selection and, under
//! privilege, that a TUN-mode node actually stands up over a real `/dev/net/tun` device.
//!
//! NOTE: A full *two-node* SWIM-over-TUN exchange needs one network namespace per node — a
//! single host has one routing table, so both overlay IPs cannot coexist without the kernel
//! delivering the peer's overlay address locally and short-circuiting the WireGuard path
//! (which would be test theater). That netns harness is tracked as a follow-up; these cover
//! the TUN plane up to a live single node.
#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, Ipv4Addr};

use foundation_wireguard::native::dataplane::MeshDataPlane;
use foundation_wireguard::native::{WgConfig, WgNode};
use foundation_wireguard::{DataPlaneMode, WgSeed};
use tracing_test::traced_test;

/// Overlay prefix length (matches `node::OVERLAY_PREFIX` — the 10.0.0.0/8 overlay).
const OVERLAY_PREFIX: u8 = 8;

fn overlay_ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
}

/// Non-privileged: the default smoltcp plane reports NOT-tun and exposes a netstack handle.
///
/// This is the branch predicate the node uses to choose overlay gossip vs. kernel gossip, so
/// pinning it also guards the selection logic without needing a TUN device.
#[test]
#[traced_test]
fn netstack_plane_is_not_tun_and_exposes_netstack() {
    let dp = MeshDataPlane::from_config(&DataPlaneMode::Netstack, overlay_ip(1), OVERLAY_PREFIX)
        .expect("netstack plane builds without privileges");

    assert!(!dp.is_tun(), "smoltcp plane must not report TUN mode");
    assert!(
        dp.netstack().is_some(),
        "smoltcp plane must expose a NetStack handle for overlay sockets"
    );
}

/// Privileged: opening a real kernel TUN device yields a TUN plane with no netstack handle.
#[cfg(target_os = "linux")]
#[test]
#[traced_test]
#[ignore = "requires CAP_NET_ADMIN / root to open /dev/net/tun"]
fn tun_plane_opens_and_reports_tun() {
    let mode = DataPlaneMode::Tun {
        name: "ewe-wg-dp0".into(),
        mtu: 1380,
    };
    let dp = MeshDataPlane::from_config(&mode, overlay_ip(1), OVERLAY_PREFIX)
        .expect("TUN plane opens (needs CAP_NET_ADMIN)");

    assert!(dp.is_tun(), "kernel plane must report TUN mode");
    assert!(
        dp.netstack().is_none(),
        "TUN plane has no userspace netstack — apps use OS sockets"
    );
}

/// Privileged: a seed node in TUN mode stands up over the kernel-UDP gossip branch.
///
/// Exercises `MeshDataPlane::from_config(Tun)` + `add_tun_route` + the `else` (kernel gossip)
/// arm of the node data-plane branch, which the netstack tests never reach. A seed node needs
/// no peer, so this proves the full TUN startup path with a single privileged node.
#[cfg(target_os = "linux")]
#[test]
#[traced_test]
#[ignore = "requires CAP_NET_ADMIN / root to open /dev/net/tun"]
fn tun_mode_seed_node_starts_over_kernel_gossip() {
    let seed = WgSeed::from_bytes(&[0x5Au8; 32]).expect("seed");
    let network_id = seed.derive_network_id();

    let config = WgConfig::builder()
        .seed(seed)
        .network_id(network_id)
        .dataplane_tun("ewe-wg-node0", 1380)
        .build()
        .expect("build TUN seed config");

    let node = WgNode::from_config(config)
        .join()
        .expect("TUN-mode seed node stands up (needs CAP_NET_ADMIN)");

    // A bound bootstrap listener means the whole TUN startup path — device open, overlay
    // route, and the kernel-UDP gossip bind — completed.
    let addr = node.bootstrap_addr();
    assert_ne!(addr.port(), 0, "bootstrap listener bound to a real port in TUN mode");

    node.shutdown();
}
