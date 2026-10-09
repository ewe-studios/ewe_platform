//! WireGuard mesh + HTTP over the overlay — proof that `Connection::Overlay`
//! carries real HTTP traffic through the encrypted tunnel (spec-55, F11).
//!
//! ```bash
//! cargo run -p foundation_connectrpc --example wireguard_echo
//! ```
//!
//! This example proves the bridge: two WgNodes form a mesh, B listens on its
//! overlay IP, A opens a `Connection::Overlay` via `WgHandle::overlay_connect()`,
//! and raw HTTP flows through the tunnel. The `Connection::Overlay` type IS the
//! integration point — the same `Read + Write` stream that H1Transport wraps.
//!
//! To use ConnectRPC over the mesh: call `WgHandle::overlay_connect(peer_ip, port)`
//! when constructing the H1Transport's HttpClient. The transport sees a
//! `Connection::Overlay` — identical to `Connection::Tcp` from its perspective.

use std::io::{Read, Write};
use std::time::Duration;

use foundation_wireguard::{SeedBits, WgConfig, WgSeed};
use foundation_wireguard::native::WgNode;

fn main() {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("random seed");
    let net = seed.derive_network_id();

    // ── Two nodes, one mesh ──────────────────────────────────────────
    let a = WgNode::from_config(WgConfig::seed(seed.clone(), net))
        .join().expect("A");
    let a_boot = a.bootstrap_addr();
    let b = WgNode::from_config(WgConfig::joiner(seed.clone(), net, vec![a_boot]))
        .join().expect("B");

    let dl = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < dl {
        if a.wait_for_peer(b.overlay_ip(), Duration::from_millis(100))
            && b.wait_for_peer(a.overlay_ip(), Duration::from_millis(100)) { break; }
    }
    assert!(a.wait_for_peer(b.overlay_ip(), Duration::from_millis(10)));
    println!("🟢 mesh formed — {} members via SWIM gossip", a.members().len());

    // ── B: listen on overlay IP ──────────────────────────────────────
    let b_ip = b.overlay_ip();
    let mut listener = b.tcp_listen(9000).expect("listen");
    let b_port = listener.local_addr().port();

    let t_b = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        if let Ok(mut c) = listener.accept() {
            let n = c.read(&mut buf).expect("B read");
            let req = String::from_utf8_lossy(&buf[..n]);
            println!("📨 B received:\n{req}");
            let body = format!("echo: {req}");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            c.write(resp.as_bytes()).expect("B write");
            println!("📤 B replied with HTTP 200");
        }
    });

    std::thread::sleep(Duration::from_millis(200));

    // ── A: connect over overlay → Connection::Overlay ────────────────
    // This is the F11 bridge. Connection::Overlay is a std::io::Read+Write.
    // ConnectRPC's H1Transport wraps this exact type through HttpClient.
    let mut conn = None;
    for _ in 0..40 {
        match a.overlay_connect(b_ip, b_port) {
            Ok(c) => { conn = Some(c); break; }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let mut conn = conn.expect("overlay_connect → Connection::Overlay");

    let http = "GET / HTTP/1.1\r\nHost: overlay\r\nConnection: close\r\n\r\n";
    conn.write(http.as_bytes()).expect("A write");

    let mut buf = [0u8; 4096];
    let n = conn.read(&mut buf).expect("A read");
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.contains("200 OK"), "got 200 OK");
    println!("📥 A received:\n{resp}");
    println!("✅ overlay HTTP confirmed — Connection::Overlay is a valid H1Transport stream");

    t_b.join().expect("B thread");
    a.shutdown();
    b.shutdown();
}
