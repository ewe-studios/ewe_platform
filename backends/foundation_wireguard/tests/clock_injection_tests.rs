//! Clock injection tests (spec 70 feature 134, phase D).
//!
//! WHY: The relay idle window is a `WireGuard` mesh timer. It must read the
//! injected [`Clock`], not the real clock, so a deterministic simulation can
//! replay it.
//!
//! HOW: Build a [`RelayServer`] on a [`SimulatedClock`], advance the clock past
//! the idle timeout, and assert the garbage collector removes the session. The
//! test never sleeps: only `SimulatedClock::advance` moves time.

#![cfg(not(target_family = "wasm"))]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{Clock, Ctx, SimulatedClock};
use foundation_wireguard::native::relay::RelayServer;
use foundation_wireguard::shared::membership::PeerId;

fn peer(byte: u8) -> PeerId {
    PeerId([byte; 32])
}

/// The relay drops a session only after the injected clock passes the idle
/// timeout.
#[test]
fn relay_idle_timeout_follows_the_injected_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(Arc::clone(&simulated) as Arc<dyn Clock>);
    let mut server = RelayServer::new(16, 1000, 1, ctx);

    assert!(server.attach(peer(7)));
    assert_eq!(server.session_count(), 1);

    // Half the idle timeout: the session is still live.
    simulated.advance(Duration::from_millis(500));
    server.gc();
    assert_eq!(
        server.session_count(),
        1,
        "a session younger than the idle timeout must survive"
    );

    // Past the idle timeout: only a clock-driven GC removes it.
    simulated.advance(Duration::from_millis(600));
    server.gc();
    assert_eq!(
        server.session_count(),
        0,
        "gc must compare against the injected clock, not the real one"
    );
}

/// Re-attaching a session restarts its idle window on the injected clock.
#[test]
fn relay_reattach_refreshes_the_idle_window_on_the_injected_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(Arc::clone(&simulated) as Arc<dyn Clock>);
    let mut server = RelayServer::new(16, 1000, 1, ctx);

    let id = peer(9);
    assert!(server.attach(id));

    // 700 ms pass, then the peer refreshes its session.
    simulated.advance(Duration::from_millis(700));
    assert!(server.attach(id));

    // 700 ms after the refresh: still inside the window.
    simulated.advance(Duration::from_millis(700));
    server.gc();
    assert_eq!(
        server.session_count(),
        1,
        "attach must restart the idle window on the injected clock"
    );

    // 1100 ms after the refresh: past the window.
    simulated.advance(Duration::from_millis(400));
    server.gc();
    assert_eq!(server.session_count(), 0);
}

/// A relay built from a `Ctx` reads the clock of that `Ctx`, so the change shows through the relay.
#[test]
fn relay_built_from_a_ctx_follows_the_ctx_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&simulated) as Arc<dyn Clock>, 7);
    let mut server = RelayServer::new(16, 1000, 1, ctx);

    assert!(server.attach(peer(3)));
    simulated.advance(Duration::from_millis(1500));
    server.gc();
    assert_eq!(
        server.session_count(),
        0,
        "the relay must read the clock of the Ctx it was built from"
    );
}
