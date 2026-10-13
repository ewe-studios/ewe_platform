//! `#[docker_container]` when no Docker daemon can be reached: the sync and
//! the `async fn` arm must behave the same — log a skip and return without
//! running the body.
//!
//! WHY a separate test binary: it points `DOCKER_HOST` at a socket that does
//! not exist, which is process-wide. Its own binary, with a single test that
//! sets the variable before anything else runs, keeps that from leaking into
//! the tests that need a real daemon (`docker_macro_tests.rs`).
//!
//! The `required = true` case is the opposite contract (a missing daemon is a
//! hard failure) and is exercised against a real daemon in
//! `docker_macro_tests.rs`.

use std::sync::atomic::{AtomicBool, Ordering};

use foundation_core::valtron::{block_on_future, initialize_pool};
use foundation_deployment_platform::docker::ContainerGroup;
use foundation_deployment_platform::docker_container;

static SYNC_BODY_RAN: AtomicBool = AtomicBool::new(false);
static ASYNC_BODY_RAN: AtomicBool = AtomicBool::new(false);

#[docker_container(image = "redis:7-alpine", as = "cache", port = 6379)]
fn sync_body(_containers: ContainerGroup) {
    SYNC_BODY_RAN.store(true, Ordering::SeqCst);
}

#[docker_container(image = "redis:7-alpine", as = "cache", port = 6379)]
async fn async_body(_containers: ContainerGroup) {
    ASYNC_BODY_RAN.store(true, Ordering::SeqCst);
}

#[test]
fn sync_and_async_arms_both_skip_when_docker_is_unreachable() {
    let socket = std::env::temp_dir().join(format!(
        "ewe-no-docker-{}.sock",
        std::process::id()
    ));
    assert!(!socket.exists(), "{socket:?} must not exist for this test");

    // SAFETY: this is the only test in this binary and it sets the variable
    // before starting any thread of its own (the valtron pool comes up below),
    // so nothing reads the environment concurrently.
    unsafe {
        std::env::set_var("DOCKER_HOST", format!("unix://{}", socket.display()));
    }

    let guard = initialize_pool(0x5eed, None);

    sync_body();
    assert!(
        !SYNC_BODY_RAN.load(Ordering::SeqCst),
        "the sync arm must skip its body when Docker is unreachable"
    );

    block_on_future(async_body());
    assert!(
        !ASYNC_BODY_RAN.load(Ordering::SeqCst),
        "the async arm must skip its body when Docker is unreachable, like the sync arm"
    );

    drop(guard);
}
