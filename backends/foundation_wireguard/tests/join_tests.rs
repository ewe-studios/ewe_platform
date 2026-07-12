//! TLS-PSK bootstrap join tests (spec-55, feature 02; decision 06).
//!
//! WHY: Proves a joiner reaches a member over a real TCP + TLS-PSK channel keyed by the
//! seed-derived `tls_psk`, is admitted, and receives the membership — while a wrong seed
//! fails the handshake and a revoked seed is rejected.
#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_wireguard::native::{BootstrapClient, BootstrapHandler, BootstrapServer};
use foundation_wireguard::shared::bootstrap::Admission;
use foundation_wireguard::{Capabilities, PeerId, PeerRecord, WgSeed};
use tracing_test::traced_test;

fn peer_id(n: u8) -> PeerId {
    let mut bytes = [0u8; 32];
    bytes[0] = n;
    PeerId(bytes)
}

fn record(n: u8) -> PeerRecord {
    let ip: IpAddr = format!("10.4.0.{n}").parse().unwrap();
    let endpoint: SocketAddr = format!("127.0.0.1:{}", 51820 + n as u16).parse().unwrap();
    PeerRecord::new(peer_id(n), ip, vec![endpoint], Capabilities::default())
}

struct TestHandler {
    members: Mutex<Vec<PeerRecord>>,
    decision: Admission,
    announced: Mutex<Vec<PeerId>>,
}

impl TestHandler {
    fn new(initial: Vec<PeerRecord>, decision: Admission) -> Arc<Self> {
        Arc::new(Self {
            members: Mutex::new(initial),
            decision,
            announced: Mutex::new(Vec::new()),
        })
    }
}

impl BootstrapHandler for TestHandler {
    fn on_join(&self, record: PeerRecord) -> (Admission, Vec<PeerRecord>) {
        if self.decision == Admission::Admit {
            let mut members = self.members.lock().unwrap();
            if !members.iter().any(|r| r.id == record.id) {
                members.push(record);
            }
            (Admission::Admit, members.clone())
        } else {
            (self.decision, Vec::new())
        }
    }

    fn membership(&self) -> Vec<PeerRecord> {
        self.members.lock().unwrap().clone()
    }

    fn on_announce(&self, record: PeerRecord) {
        self.announced.lock().unwrap().push(record.id);
    }
}

fn tls_psk_for(seed_byte: u8) -> ([u8; 32], foundation_wireguard::NetworkId) {
    let seed = WgSeed::from_bytes(&[seed_byte; 32]).unwrap();
    let network = seed.derive_network_id();
    (seed.derive_bootstrap(&network).tls_psk, network)
}

#[test]
#[traced_test]
fn joiner_admitted_over_tls_psk_and_receives_membership() {
    let (tls_psk, network) = tls_psk_for(0x77);

    let seed_member = record(1);
    let handler = TestHandler::new(vec![seed_member.clone()], Admission::Admit);
    let server = BootstrapServer::bind("127.0.0.1:0".parse().unwrap(), tls_psk, handler.clone())
        .expect("bind server");
    let addr = server.local_addr().expect("addr");

    let server_thread = thread::spawn(move || server.serve_once());

    let client = BootstrapClient::new(tls_psk, network).expect("client ctx");
    let mut conn = client.connect(addr).expect("tls-psk connect");

    let joiner = record(2);
    let (decision, members) = conn.join(joiner.clone()).expect("join");
    assert_eq!(decision, Admission::Admit, "joiner admitted");
    assert!(
        members.iter().any(|r| r.id == seed_member.id),
        "membership includes the seed member"
    );
    assert!(
        members.iter().any(|r| r.id == joiner.id),
        "membership includes the joiner"
    );

    // A second call on the same connection pulls membership again.
    let pulled = conn.pull_membership().expect("pull membership");
    assert!(pulled.iter().any(|r| r.id == joiner.id));

    // Announce is observed by the handler.
    conn.announce(joiner.clone()).expect("announce");
    drop(conn); // close → server loop ends

    server_thread.join().expect("join thread").expect("serve ok");
    assert!(
        handler.announced.lock().unwrap().contains(&joiner.id),
        "server observed the announce"
    );
}

#[test]
#[traced_test]
fn wrong_seed_fails_handshake() {
    let (correct_psk, _network) = tls_psk_for(0x11);
    let (wrong_psk, wrong_network) = tls_psk_for(0x22);

    let handler = TestHandler::new(vec![record(1)], Admission::Admit);
    let server = BootstrapServer::bind("127.0.0.1:0".parse().unwrap(), correct_psk, handler)
        .expect("bind");
    let addr = server.local_addr().unwrap();

    // Server will attempt (and fail) the handshake; its thread returns an error.
    let server_thread = thread::spawn(move || server.serve_once());

    let client = BootstrapClient::new(wrong_psk, wrong_network).unwrap();
    let result = client.connect(addr);
    assert!(result.is_err(), "handshake with the wrong seed must fail");

    // Server side also fails; drain the thread so the test doesn't leak it.
    let _ = server_thread.join();
}

#[test]
#[traced_test]
fn revoked_seed_is_rejected() {
    let (tls_psk, network) = tls_psk_for(0x33);

    // Handler refuses admission (simulating an expired/revoked seed — decision 11).
    let handler = TestHandler::new(vec![record(1)], Admission::Reject);
    let server = BootstrapServer::bind("127.0.0.1:0".parse().unwrap(), tls_psk, handler)
        .expect("bind");
    let addr = server.local_addr().unwrap();
    let server_thread = thread::spawn(move || server.serve_once());

    let client = BootstrapClient::new(tls_psk, network).unwrap();
    let mut conn = client.connect(addr).expect("handshake still succeeds");
    let (decision, members) = conn.join(record(2)).expect("join reply");
    assert_eq!(decision, Admission::Reject, "revoked seed rejected");
    assert!(members.is_empty(), "no membership handed to a rejected joiner");

    drop(conn);
    let _ = server_thread.join();
}
