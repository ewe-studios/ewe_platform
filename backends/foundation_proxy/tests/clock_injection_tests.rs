//! Proxy timeouts read the injected clock (spec 70 feature 134).
//!
//! WHY: the state-file lock wait compared two real `Instant::now` reads, so a
//! deterministic simulation test could not drive it. The wait now measures its
//! deadline on the clock the host passed to `ProxyServer::start`.
//!
//! WHAT: a second handle holds the state-file lock. A `SimulatedClock` starts at
//! zero; the test advances it past the lock timeout while the save waits, and
//! the save must fail at once instead of waiting for the real timeout.
//!
//! HOW: the real timeout is 2 seconds and the simulated advance is 30 seconds.
//! Before this feature the advance did nothing, so the save waited the real 2
//! seconds and the elapsed-time assertion failed.

#![cfg(unix)]

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_proxy::persistence::ProxyStateStore;

/// A counter that makes each test run's temporary directory unique, without a
/// wall-clock read.
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

/// A unique temporary state directory, without a wall-clock read.
fn temp_state_dir() -> PathBuf {
    let id = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("ewe_proxy_clock_{}_{}", std::process::id(), id))
}

/// The lock file `ProxyStateStore` takes for a `domain` (see `lock_state_file`).
fn lock_path(state_dir: &Path, domain: &str) -> PathBuf {
    state_dir
        .join(".deployment")
        .join("foundation_proxy")
        .join(domain.replace('/', ":"))
        .join("proxy_state.lock")
}

/// Hold an exclusive `flock` on `path` for the test's lifetime.
fn hold_lock(path: &Path) -> File {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create lock directory");
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .expect("open lock file");
    // SAFETY: `flock` reads no memory; `file` stays open while the lock is held.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(rc, 0, "the test must hold the state-file lock");
    file
}

#[test]
fn the_state_lock_timeout_follows_the_injected_clock() {
    let dir = temp_state_dir();
    let _held = hold_lock(&lock_path(&dir, "clock.example"));

    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(simulated.clone());
    let store = ProxyStateStore::new(&dir, "clock.example", ctx)
        .with_lock_timeout(Duration::from_secs(2))
        .with_lock_retry_interval(Duration::from_millis(1));

    // Move the simulated clock past the deadline while the save waits.
    let advancer = {
        let simulated = Arc::clone(&simulated);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            simulated.advance(Duration::from_secs(30));
        })
    };

    let started = Instant::now();
    let result = futures_lite::future::block_on(store.save_backend_state(
        "svc",
        "http://127.0.0.1:1",
        "active",
    ));
    let elapsed = started.elapsed();
    advancer.join().expect("advancer thread");

    assert!(result.is_err(), "a held state lock must time out");
    assert!(
        elapsed < Duration::from_secs(1),
        "the state lock timeout must follow the injected clock, but it waited {elapsed:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The store stamps the certificate time from the clock of the `Ctx` it is
/// given. A simulated wall-clock move shows in the next saved value.
#[test]
fn the_stored_cert_time_follows_the_ctx_clock() {
    let dir = temp_state_dir();
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let store = ProxyStateStore::new(&dir, "ctx.example", Ctx::fixed(simulated.clone()));

    futures_lite::future::block_on(store.save_tls_cert("cert-pem", "key-pem")).expect("first save");
    let first = futures_lite::future::block_on(store.load())
        .expect("load after first save")
        .and_then(|state| state.cert_renewed_at)
        .expect("first renewal time");

    simulated.advance_wall(Duration::from_secs(3600));
    futures_lite::future::block_on(store.save_tls_cert("cert-pem", "key-pem"))
        .expect("second save");
    let second = futures_lite::future::block_on(store.load())
        .expect("load after second save")
        .and_then(|state| state.cert_renewed_at)
        .expect("second renewal time");

    assert_eq!(
        first, 0,
        "the simulated wall clock starts at the UNIX epoch"
    );
    assert_eq!(
        second, 3600,
        "the stored time must follow the advanced clock"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
