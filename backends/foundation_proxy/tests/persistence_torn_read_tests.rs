//! A plain `load` never reads a half-written state file (spec 70 feature 65, E6).
//!
//! WHY: the state store writes its file in place, so a reader that does not take
//! the state-file lock can read a truncated or partial document and fail to
//! parse it. `ProxyStateStore::load` now takes the shared state-file lock.
//!
//! WHAT: one store saves large states in a loop; another store object on the
//! same directory (a stand-in for a second process) loads 1000 times. Every
//! load must parse.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_proxy::persistence::ProxyStateStore;

#[test]
fn load_never_sees_a_torn_file() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ewe_proxy_torn_{}_{nanos}", std::process::id()));
    let writer = Arc::new(ProxyStateStore::new(
        &dir,
        "torn.example",
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    ));
    let reader = ProxyStateStore::new(
        &dir,
        "torn.example",
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    );

    // The first state exists before the reader starts, so every load has a file.
    futures_lite::future::block_on(writer.save_backend_state("svc", "http://seed", "active"))
        .expect("seed state");

    let stop = Arc::new(AtomicBool::new(false));
    let (w, s) = (Arc::clone(&writer), Arc::clone(&stop));
    let saver = std::thread::spawn(move || {
        // 500 distinct 200-byte keys make a document of about 100 KB, so the
        // in-place write is long enough to overlap a read.
        let mut i = 0usize;
        while !s.load(Ordering::SeqCst) {
            let url = format!(
                "http://127.0.0.1/{}",
                "x".repeat(200) + &(i % 500).to_string()
            );
            futures_lite::future::block_on(w.save_backend_state("svc", &url, "active"))
                .expect("save");
            i += 1;
        }
    });

    for n in 0..1000 {
        let loaded = futures_lite::future::block_on(reader.load());
        assert!(loaded.is_ok(), "load {n} failed: {:?}", loaded.err());
    }
    stop.store(true, Ordering::SeqCst);
    saver.join().expect("saver joined");
    let _ = std::fs::remove_dir_all(&dir);
}
