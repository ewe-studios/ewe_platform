//! Docker self-assembly example — containers form a private mesh via env vars.
//!
//! Set these environment variables (injected by `foundation_deployment_platform`
//! or a Docker Compose file):
//!
//! ```text
//! WG_SECRET=<base64url 32-byte seed>        (mandatory)
//! WG_NETWORK=<hex network id>               (optional — derived from seed if unset)
//! WG_SEED_ENDPOINTS=<host:port,host:port>   (joiner only — empty for the first/seed)
//! WG_RELAY=true                             (optional — advertise relay capability)
//! ```
//!
//! The first container (no `WG_SEED_ENDPOINTS`) becomes the seed — it generates
//! an identity and keeps its bootstrap door open. Subsequent containers get
//! `WG_SEED_ENDPOINTS` pointing at the seed and join through it. SWIM gossip
//! propagates membership so later containers can join through any existing member.
//!
//! ```bash
//! # Terminal 1 — seed (WG port 51820 published)
//! WG_SECRET=$(wg-seed generate) cargo run --example docker_self_assemble
//!
//! # Terminal 2 — joiner
//! WG_SECRET=<same-seed> WG_SEED_ENDPOINTS=127.0.0.1:51820 cargo run --example docker_self_assemble
//! ```

use std::net::SocketAddr;
use std::time::Duration;

use foundation_wireguard::{WgConfig, WgSeed};
use foundation_wireguard::native::WgNode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Load config from environment (WG_SECRET / WG_NETWORK / WG_SEED_ENDPOINTS).
    // This is the canonical container self-assembly path.
    let config = WgConfig::from_env()?;
    let is_seed = config.seed_endpoints().is_empty();

    let identity_hint = config.wg_seed().derive_bootstrap(&config.network_id());
    tracing::info!(
        network = %config.network_id(),
        seed = is_seed,
        relay = config.relay.advertise,
        "🚀 booting mesh node"
    );

    let node = WgNode::from_config(config);
    let handle = node.join()?;

    tracing::info!(
        identity = %handle.identity(),
        overlay_ip = %handle.overlay_ip(),
        udp = %handle.udp_addr(),
        bootstrap = %handle.bootstrap_addr(),
        "✅ joined mesh"
    );

    // Wait for membership to converge (at least one other peer discovered).
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let members = handle.members();
        if members.len() > 1 {
            tracing::info!("👥 {} peers in mesh:", members.len() - 1);
            for m in &members {
                if m.identity != handle.identity() {
                    tracing::info!("  peer {} at {} (relay={})", m.identity, m.tunnel_ip, m.caps.relay);
                }
            }
            break;
        }
        if is_seed {
            tracing::info!("⏳ seed waiting for joiners...");
        } else {
            tracing::info!("⏳ waiting for gossip convergence...");
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    // ── Overlay service demo ──────────────────────────────────────────
    // If another peer exists, open an overlay TCP listener and print the
    // address so other containers can reach us.
    let listener = handle.tcp_listen(9000)?;
    let addr: SocketAddr = listener.local_addr();
    tracing::info!("📡 overlay service listening on {addr}");

    // Keep running until Ctrl-C.
    tracing::info!("running (Ctrl-C to stop)");
    loop {
        std::thread::sleep(Duration::from_secs(5));
        tracing::info!("mesh: {} members", handle.members().len());
    }
}
