//! `#[docker_container(ctx = <expr>)]` (spec 72, feature 04 — issue #220).
//!
//! WHY: the macro could not pass a clock at all, so a caller outside a valtron
//! pool had no way to start a container. The new `ctx = <expr>` argument
//! supplies the injected clock directly.
//!
//! WHAT: the sync and async arms of the macro both compile with `ctx = ...`
//! and use it. It points `DOCKER_HOST` at a socket that does not exist, so both
//! skip without starting anything.
//!
//! HOW: its own test binary, because `DOCKER_HOST` is process-wide. The clock is
//! a `SimulatedClock` from the crate's `test-clock` dev-dependency.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::{SimulatedClock, block_on_future, initialize_pool};
use foundation_deployment_platform::docker::ContainerGroup;
use foundation_deployment_platform::docker_container;

static SYNC_BODY_RAN: AtomicBool = AtomicBool::new(false);
static ASYNC_BODY_RAN: AtomicBool = AtomicBool::new(false);

fn simulated_ctx() -> Ctx {
    Ctx::fixed(Arc::new(SimulatedClock::new(Duration::ZERO)))
}

#[docker_container(
    image = "redis:7-alpine",
    as = "cache",
    port = 6379,
    ctx = simulated_ctx()
)]
fn sync_body(_containers: ContainerGroup) {
    SYNC_BODY_RAN.store(true, Ordering::SeqCst);
}

#[docker_container(
    image = "redis:7-alpine",
    as = "cache",
    port = 6379,
    ctx = simulated_ctx()
)]
async fn async_body(_containers: ContainerGroup) {
    ASYNC_BODY_RAN.store(true, Ordering::SeqCst);
}

#[test]
fn the_macro_accepts_a_ctx_argument() {
    let socket = std::env::temp_dir().join(format!("ewe-no-docker-{}.sock", std::process::id()));
    assert!(!socket.exists(), "{socket:?} must not exist for this test");

    // SAFETY: this is the only test in this binary and it sets the variable
    // before starting any thread of its own (the valtron pool comes up below),
    // so nothing reads the environment concurrently.
    unsafe {
        std::env::set_var("DOCKER_HOST", format!("unix://{}", socket.display()));
    }

    let guard = initialize_pool(0xc10c, None);

    sync_body();
    assert!(
        !SYNC_BODY_RAN.load(Ordering::SeqCst),
        "the sync arm must skip its body when Docker is unreachable"
    );

    block_on_future(async_body());
    assert!(
        !ASYNC_BODY_RAN.load(Ordering::SeqCst),
        "the async arm must skip its body when Docker is unreachable"
    );

    drop(guard);
}
