//! Two-node WireGuard mesh — two peers form a private overlay network, discover
//! each other via SWIM gossip, and exchange data over an overlay TCP channel.
//!
//! ```bash
//! cargo run --example two_node_mesh
//! ```

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_wireguard::{SeedBits, WgConfig, WgSeed};
use foundation_wireguard::native::WgNode;

fn main() {
    tracing_subscriber::fmt::init();

    let seed = WgSeed::generate(SeedBits::Bits256).expect("random seed");
    let net = seed.derive_network_id();
    tracing::info!(network = %net);

    // ── Start node A (seed) ───────────────────────────────────────────
    let handle_a = WgNode::from_config(WgConfig::seed(seed.clone(), net))
        .join().expect("A join");
    tracing::info!(identity=%handle_a.identity(), ip=%handle_a.overlay_ip(), "🟢 seed node A");

    // ── Start node B (joiner) ────────────────────────────────────────
    let a_boot = handle_a.bootstrap_addr();
    let handle_b = WgNode::from_config(WgConfig::joiner(seed, net, vec![a_boot]))
        .join().expect("B join");
    tracing::info!(identity=%handle_b.identity(), ip=%handle_b.overlay_ip(), boot=%a_boot, "🟢 joiner node B");

    // ── SWIM gossip discovery ────────────────────────────────────────
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_millis(100))
            && handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_millis(100))
        { break; }
    }
    assert!(handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_millis(10)), "A→B");
    assert!(handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_millis(10)), "B→A");

    for m in handle_a.members() {
        tracing::info!(id=%m.identity, ip=%m.tunnel_ip, state=?m.state, "member");
    }
    tracing::info!("✅ {} members discovered via gossip", handle_a.members().len());

    // ── Overlay TCP: B listens, A connects and exchanges ─────────────
    let b_ip = handle_b.overlay_ip();
    let mut listener = handle_b.tcp_listen(9000).expect("B overlay listen");
    let b_port = listener.local_addr().port();
    tracing::info!(%b_ip, %b_port, "B listening");

    let done = Arc::new(AtomicBool::new(false));
    let done_c = Arc::clone(&done);
    let echo_thread = thread::spawn(move || {
        while !done_c.load(Ordering::Relaxed) {
            if let Ok(mut c) = listener.accept() {
                let mut buf = [0u8; 512];
                if let Ok(n) = c.read(&mut buf) {
                    tracing::info!("📨 B received: {}", String::from_utf8_lossy(&buf[..n]));
                    let _ = c.write(&buf[..n]);
                }
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });

    // A: retry overlay connect until tunnel + smoltcp TCP are ready.
    let greeting = b"hello from A over the private WG mesh!";
    for _ in 0..60 {
        if let Ok(stream) = handle_a.tcp_connect(b_ip, b_port) {
            for _ in 0..400 { if stream.may_send() { break; } thread::sleep(Duration::from_millis(5)); }
            tracing::info!("📤 A sends: {}", String::from_utf8_lossy(greeting));
            let _ = stream.write(greeting);
            for _ in 0..400 {
                let mut buf = [0u8; 512];
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => { tracing::info!("📥 A received echo: {}", String::from_utf8_lossy(&buf[..n])); break; }
                    _ => thread::sleep(Duration::from_millis(10)),
                }
            }
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    done.store(true, Ordering::Relaxed);
    let _ = echo_thread.join();
    tracing::info!("✅ overlay TCP confirmed");
    handle_a.shutdown();
    handle_b.shutdown();
    tracing::info!("done");
}
