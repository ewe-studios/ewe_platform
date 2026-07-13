//! Integration tests for relay framing, server, and hole-punch (spec-55, F05).
//!
//! Tests: RelayFrame encode/decode, RelayServer attach/forward/GC/rate-limit,
//! RelaySelector selection + failover, RelaySession connectivity ladder,
//! and HolePuncher SYN/ACK flow.

#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_wireguard::shared::membership::{Capabilities, MemberState, PeerId, PeerRecord};
use foundation_wireguard::shared::relay::{
    ConnectivityPath, RelayFrame, RelaySelector, RelaySession, RelayStrategy, RELAY_MAX_PAYLOAD,
};
use foundation_wireguard::native::relay::{HolePuncher, RelayClient, RelayServer};
use tracing_test::traced_test;

fn test_peer(id: u8) -> PeerId {
    PeerId([id; 32])
}

fn test_record(id: u8, relay: bool, ip_octet: u8) -> PeerRecord {
    let mut caps = Capabilities::default();
    caps.relay = relay;
    PeerRecord {
        id: test_peer(id),
        tunnel_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, ip_octet)),
        endpoints: vec![SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            51000 + id as u16,
        )],
        caps,
        incarnation: 1,
        state: MemberState::Alive,
        heartbeat: 1,
    }
}

// ── RelayFrame ──

#[traced_test]
#[test]
fn frame_round_trip() {
    let frame = RelayFrame::new(test_peer(42), vec![10, 20, 30]);
    let encoded = frame.encode();
    let decoded = RelayFrame::decode(&encoded).expect("decode");
    assert_eq!(decoded, frame);
    assert_eq!(decoded.ciphertext.len(), 3);
}

#[traced_test]
#[test]
fn frame_max_payload() {
    let frame = RelayFrame::new(test_peer(1), vec![0u8; RELAY_MAX_PAYLOAD]);
    let encoded = frame.encode();
    let decoded = RelayFrame::decode(&encoded).expect("decode");
    assert_eq!(decoded.ciphertext.len(), RELAY_MAX_PAYLOAD);
}

#[traced_test]
#[test]
fn frame_rejects_short_buffer() {
    assert!(RelayFrame::decode(&[0u8; 10]).is_none());
}

#[traced_test]
#[test]
fn frame_rejects_wrong_version() {
    let frame = RelayFrame::new(test_peer(1), vec![1, 2]);
    let mut encoded = frame.encode();
    encoded[0] = 99;
    assert!(RelayFrame::decode(&encoded).is_none());
}

// ── RelayServer ──

#[traced_test]
#[test]
fn server_attach_and_forward() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut server = RelayServer::new(16, 1000, 60, stop);
    let src = test_peer(10);
    let dst = test_peer(20);

    assert!(server.attach(src));
    assert!(server.attach(dst));
    assert_eq!(server.session_count(), 2);

    let frame = RelayFrame::new(dst, b"hello relay".to_vec());
    let fwd = server.forward(src, &frame.encode()).expect("forward");
    assert_eq!(fwd.ciphertext, b"hello relay");
    assert_eq!(fwd.src, src);
    assert_eq!(fwd.dst, dst);
}

#[traced_test]
#[test]
fn server_rejects_unattached_sender() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut server = RelayServer::new(16, 1000, 60, stop);
    server.attach(test_peer(20)); // only dst
    let frame = RelayFrame::new(test_peer(20), vec![1, 2, 3]);
    assert!(server.forward(test_peer(99), &frame.encode()).is_none());
}

#[traced_test]
#[test]
fn server_rejects_unknown_dst() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut server = RelayServer::new(16, 1000, 60, stop);
    server.attach(test_peer(10)); // only src
    let frame = RelayFrame::new(test_peer(99), vec![1, 2]); // dst not attached
    assert!(server.forward(test_peer(10), &frame.encode()).is_none());
}

#[traced_test]
#[test]
fn server_gc_removes_stale() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut server = RelayServer::new(16, 1000, 0, stop); // 0s timeout
    server.attach(test_peer(1));
    server.attach(test_peer(2));
    assert_eq!(server.session_count(), 2);
    server.gc();
    assert_eq!(server.session_count(), 0);
}

#[traced_test]
#[test]
fn server_detach_removes_peer() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut server = RelayServer::new(16, 1000, 60, stop);
    server.attach(test_peer(1));
    server.detach(&test_peer(1));
    assert!(!server.is_attached(&test_peer(1)));
}

// ── RelaySelector ──

#[traced_test]
#[test]
fn selector_filters_non_relay_peers() {
    let records = vec![
        test_record(1, true, 1),
        test_record(2, false, 2),
        test_record(3, true, 3),
    ];
    let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
    sel.update(&records);
    assert_eq!(sel.len(), 2);
}

#[traced_test]
#[test]
fn selector_filters_dead_peers() {
    let mut dead = test_record(1, true, 1);
    dead.state = MemberState::Dead;
    let records = vec![dead, test_record(2, true, 2)];
    let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
    sel.update(&records);
    assert_eq!(sel.len(), 1);
}

#[traced_test]
#[test]
fn selector_selects_first_candidate() {
    let records = vec![test_record(1, true, 1), test_record(2, true, 2)];
    let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
    sel.update(&records);
    let first = sel.select().expect("select").peer_id;
    assert_eq!(first, test_peer(1)); // lowest id sorts first for load strat
}

#[traced_test]
#[test]
fn selector_failover_advances() {
    let records = vec![test_record(1, true, 1), test_record(2, true, 2)];
    let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
    sel.update(&records);
    let first = sel.select().expect("first").peer_id;
    let second = sel.failover().expect("second").peer_id;
    assert_ne!(first, second);
    assert_eq!(sel.len(), 1); // one removed
}

#[traced_test]
#[test]
fn selector_rtt_degradation_triggers_failover() {
    let records = vec![test_record(1, true, 1), test_record(2, true, 2)];
    let mut sel = RelaySelector::new(RelayStrategy::LowestRtt);
    sel.update(&records);
    let first_id = sel.select().expect("first").peer_id;
    // Record bad RTT on first; should fail over on next select.
    sel.record_rtt(first_id, 100_000); // 100ms
    // Second relay should now be better — failover picks it.
    sel.failover();
    let next_id = sel.select().expect("next").peer_id;
    assert_ne!(next_id, first_id);
}

// ── RelaySession (connectivity ladder) ──

#[traced_test]
#[test]
fn session_ladder_from_direct_to_relay() {
    let mut s = RelaySession::new();
    assert!(!s.is_relayed());

    s.on_direct_success();
    assert_eq!(s.path, ConnectivityPath::Direct);

    let relay = test_peer(99);
    s.on_direct_failure(Some(relay));
    assert_eq!(s.path, ConnectivityPath::HolePunching);

    s.on_direct_failure(Some(relay));
    assert!(s.is_relayed());
}

#[traced_test]
#[test]
fn session_upgrades_from_relay_to_direct() {
    let mut s = RelaySession::new();
    let relay = test_peer(99);
    // From Disconnected, go straight to relayed.
    s.on_direct_failure(Some(relay));
    assert!(s.is_relayed());

    s.on_direct_success();
    assert!(!s.is_relayed());
    assert!(s.relay.is_none());
}

#[traced_test]
#[test]
fn session_hole_punch_success() {
    let mut s = RelaySession::new();
    // Start from Direct to get to HolePunching ladder rung.
    s.on_direct_success();
    s.on_direct_failure(Some(test_peer(99)));
    assert_eq!(s.path, ConnectivityPath::HolePunching);

    s.on_hole_punch_success();
    assert_eq!(s.path, ConnectivityPath::Direct);
}

// ── RelayClient ──

#[traced_test]
#[test]
fn client_tracks_peer_paths() {
    let sel = RelaySelector::new(RelayStrategy::LowestLoad);
    let mut client = RelayClient::new(sel);

    client.on_direct_probe_success(test_peer(5));
    assert_eq!(client.peer_path(test_peer(5)), ConnectivityPath::Direct);

    // Direct failure with no relay → HolePunching.
    client.on_direct_probe_failure(test_peer(5));
    assert_eq!(client.peer_path(test_peer(5)), ConnectivityPath::HolePunching);
}

// ── HolePuncher ──

#[traced_test]
#[test]
fn hole_punch_flow_syn_ack() {
    let mut puncher = HolePuncher::new(30);
    let peer = test_peer(77);
    let ep: SocketAddr = "10.0.0.77:51820".parse().expect("addr");

    puncher.initiate(peer, &[ep]);
    assert!(puncher.has_pending(&peer));

    let syns = puncher.drain_syns(Instant::now());
    assert_eq!(syns.len(), 1);
    assert_eq!(syns[0].1, ep);

    // Receive ACK from peer at that endpoint.
    let ack = HolePuncher::build_ack(peer);
    let result = puncher.on_punch_packet(peer, ep, &ack);
    assert_eq!(result, Some(ep));

    let confirmed = puncher.tick(Instant::now());
    assert_eq!(confirmed, vec![peer]);
}

#[traced_test]
#[test]
fn hole_punch_timeout_cleans_up() {
    let mut puncher = HolePuncher::new(0); // 0s timeout
    let peer = test_peer(88);
    puncher.initiate(peer, &["127.0.0.1:1".parse().expect("addr")]);

    // Drain syn ensures the attempt moves from Gathering to SynSent.
    puncher.drain_syns(Instant::now());

    // Tick should remove timed-out SynSent.
    let confirmed = puncher.tick(Instant::now());
    assert!(confirmed.is_empty());
    assert!(!puncher.has_pending(&peer));
}

#[traced_test]
#[test]
fn hole_punch_syn_contains_magic() {
    let syn = HolePuncher::build_syn(test_peer(1));
    assert_eq!(&syn[..4], b"HPv1");
    assert_eq!(syn[4], 0x01);
}

#[traced_test]
#[test]
fn hole_punch_ack_contains_magic() {
    let ack = HolePuncher::build_ack(test_peer(2));
    assert_eq!(&ack[..4], b"HPv1");
    assert_eq!(ack[4], 0x02);
}

// ── Integration: 3-node mesh with one relay-capable member ──

#[traced_test]
#[test]
fn three_node_mesh_with_relay() {
    use foundation_wireguard::{SeedBits, WgConfig, WgSeed};
    use foundation_wireguard::native::{WgNode};
    use foundation_wireguard::shared::membership::MemberState;

    let seed = WgSeed::generate(SeedBits::Bits256).expect("seed");
    let net = seed.derive_network_id();

    // Node A: seed + relay.
    let cfg_a = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .relay_advertise(true)
        .relay_max_sessions(128)
        .build()
        .expect("A");
    let handle_a = WgNode::from_config(cfg_a).join().expect("join A");
    let a_boot = handle_a.bootstrap_addr();

    // Node B: joiner (not relay).
    let cfg_b = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .seed_endpoint(a_boot)
        .build()
        .expect("B");
    let handle_b = WgNode::from_config(cfg_b).join().expect("join B");

    // Node C: joiner (not relay).
    let cfg_c = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .seed_endpoint(a_boot)
        .build()
        .expect("C");
    let handle_c = WgNode::from_config(cfg_c).join().expect("join C");

    // All three should discover each other.
    assert!(handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_secs(5)));
    assert!(handle_a.wait_for_peer(handle_c.overlay_ip(), Duration::from_secs(5)));
    assert!(handle_b.wait_for_peer(handle_c.overlay_ip(), Duration::from_secs(5)));

    // Verify A has relay capability in its membership view.
    let a_members = handle_a.members();
    let a_self = a_members.iter().find(|p| p.identity == handle_a.identity()).expect("A in own members");
    assert!(a_self.caps.relay, "Node A should advertise relay");

    handle_a.shutdown();
    handle_b.shutdown();
    handle_c.shutdown();
}
