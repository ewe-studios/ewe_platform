//! Two stores on one state directory must not lose updates (spec 70 feature 45,
//! E2).
//!
//! WHY: `ProxyStateStore` serialises its load-change-save with a lock. An
//! in-process lock does not stop a second proxy process (or a second store
//! object) on the same directory from saving over the first one's change. The
//! state store behind it has no conditional write, so the store takes an
//! advisory lock on a file in the state directory for each load-change-save;
//! the lock works across stores and across processes.
//!
//! WHAT: two `ProxyStateStore` objects on one directory each save 100 different
//! backends from their own thread, interleaved. The final state holds all 200.

use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_proxy::persistence::ProxyStateStore;

fn temp_state_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ewe_proxy_two_stores_{}_{}",
        std::process::id(),
        nanos
    ))
}

#[test]
fn two_stores_on_one_backend_do_not_lose_updates() {
    let dir = temp_state_dir();
    // Two separate store objects: separate in-process locks, one directory.
    let stores = [
        Arc::new(ProxyStateStore::new(
            &dir,
            "two.example",
            foundation_core::valtron::Ctx::host(),
        )),
        Arc::new(ProxyStateStore::new(
            &dir,
            "two.example",
            foundation_core::valtron::Ctx::host(),
        )),
    ];
    let start = Arc::new(Barrier::new(2));

    let threads: Vec<_> = stores
        .iter()
        .enumerate()
        .map(|(which, store)| {
            let store = Arc::clone(store);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for i in 0..100 {
                    let backend = format!("http://127.0.0.1:{which}{i:03}");
                    futures_lite::future::block_on(store.save_backend_state(
                        "svc",
                        &backend,
                        &format!("state-{which}-{i}"),
                    ))
                    .expect("save backend state");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("saving thread joined");
    }

    let persisted = futures_lite::future::block_on(stores[0].load())
        .expect("load state")
        .expect("state present");
    assert_eq!(
        persisted.backend_states.len(),
        200,
        "every update of both stores must survive"
    );
    for which in 0..2 {
        for i in 0..100 {
            let key = format!("svc/http://127.0.0.1:{which}{i:03}");
            assert_eq!(
                persisted.backend_states.get(&key).map(String::as_str),
                Some(format!("state-{which}-{i}").as_str()),
                "backend {key} must keep its saved state"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}
