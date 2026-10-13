//! Shared setup for the real-daemon integration tests.
//!
//! WHY: These tests create containers from `alpine:latest`. On a daemon with a
//! cold image cache (a fresh CI runner) that image is not there, and every
//! `create` fails with `404 No such image` — the tests depended on something
//! else having pulled it first.
//!
//! WHAT: [`ensure_image`] pulls an image unless the daemon already has it;
//! [`client_with_alpine`] is a client for the local daemon with `alpine:latest`
//! in place.
//!
//! HOW: Through the client under test: `image_inspect` for the cache check,
//! then `image_pull`, whose progress stream is read to its end (the daemon
//! pulls while it writes it, and reports a failed pull inside it).

// Every test binary that declares `mod common;` compiles all of it, and each
// uses only some of these helpers.
#![allow(dead_code)]

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use foundation_core::valtron::sleep_async;
use foundation_deployment_docker::{DockerClient, DockerError};

/// Whether some test in this binary is checking or pulling an image right now.
///
/// A binary's tests run in parallel, and a burst of identical pulls is what
/// gets an anonymous client rate-limited by Docker Hub (`429 Too Many
/// Requests`). One caller at a time checks and pulls; the rest wait their turn
/// and then find the image cached.
static PULLING: AtomicU8 = AtomicU8::new(IDLE);
const IDLE: u8 = 0;
const BUSY: u8 = 1;

/// Releases [`PULLING`] when dropped — also when the pull panics.
struct Turn;

impl Drop for Turn {
    fn drop(&mut self) {
        PULLING.store(IDLE, Ordering::Release);
    }
}

/// Wait until no other test is checking or pulling, then take the turn.
async fn take_turn() -> Turn {
    while PULLING
        .compare_exchange(IDLE, BUSY, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        sleep_async(Duration::from_millis(100)).await;
    }
    Turn
}

/// The local daemon's socket.
pub const DOCKER_SOCK: &str = "/var/run/docker.sock";

/// Pull `repo:tag` unless the daemon already has it.
///
/// # Panics
///
/// If the daemon cannot be asked, or the pull fails.
pub async fn ensure_image(client: &DockerClient, repo: &str, tag: &str) {
    let _turn = take_turn().await;
    let reference = format!("{repo}:{tag}");
    match client.image_inspect(&reference).await {
        Ok(_) => return,
        Err(DockerError::Api { status: 404, .. }) => {}
        Err(e) => panic!("inspect {reference}: {e:?}"),
    }
    client
        .image_pull(Some(repo), None, None, Some(tag), None, None)
        .await
        .unwrap_or_else(|e| panic!("start pulling {reference}: {e:?}"))
        .finish()
        .await
        .unwrap_or_else(|e| panic!("pull {reference}: {e:?}"));
}

/// A client for the local daemon, with `alpine:latest` pulled.
///
/// # Panics
///
/// As [`ensure_image`].
pub async fn client_with_alpine() -> DockerClient {
    let client = DockerClient::connect_unix(DOCKER_SOCK);
    ensure_image(&client, "alpine", "latest").await;
    client
}
