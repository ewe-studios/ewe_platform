//! Backend-state persistence race test — concurrent saves must keep every
//! update.
//!
//! WHY: `ProxyStateStore::save_backend_state` does a load-change-save of one
//! `proxy_state` document. Control-socket connections run on their own threads,
//! so two saves at once could each save over the other's change. The store now
//! serialises that read-modify-write behind an async mutex.
//!
//! WHAT: 16 threads each save a different backend 50 times. The final load must
//! hold every backend's last value.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_proxy::persistence::ProxyStateStore;

fn temp_state_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ewe_proxy_persistence_{}_{}",
        std::process::id(),
        nanos
    ))
}

#[test]
fn concurrent_backend_saves_keep_every_update() {
    let dir = temp_state_dir();
    let store = Arc::new(ProxyStateStore::new(&dir, "test.example"));

    let mut threads = Vec::new();
    for i in 0..16 {
        let store = Arc::clone(&store);
        threads.push(std::thread::spawn(move || {
            let backend = format!("http://127.0.0.1:{i}");
            for _ in 0..50 {
                futures_lite::future::block_on(store.save_backend_state(
                    "svc",
                    &backend,
                    &format!("state-{i}"),
                ))
                .expect("save backend state");
            }
        }));
    }
    for thread in threads {
        thread.join().expect("saving thread joined");
    }

    let persisted = futures_lite::future::block_on(store.load())
        .expect("load state")
        .expect("state present");
    for i in 0..16 {
        let key = format!("svc/http://127.0.0.1:{i}");
        assert_eq!(
            persisted.backend_states.get(&key).map(String::as_str),
            Some(format!("state-{i}").as_str()),
            "backend {i} must keep its last saved state"
        );
    }

    // Clean up the temp state files.
    let _ = std::fs::remove_dir_all(&dir);
}
