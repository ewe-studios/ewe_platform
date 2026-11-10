//! A `Ctx` built over a simulated clock sets the timestamps that the namespaced
//! store writes. Moving the clock moves the next stored timestamp.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{valtron_test, Ctx, SimulatedClock};
use foundation_db::state::namespaced::NamespacedStore;
use foundation_db::state::{FileStateStore, StateStore};

#[valtron_test]
async fn advancing_the_ctx_clock_moves_the_stored_timestamp() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = FileStateStore::with_root(dir.path().to_path_buf());
    store.init().await.expect("init store");
    let ns = NamespacedStore::new(Arc::new(store), "ctx-test");

    // The test keeps one handle to the clock, so it can move the time. The
    // `Ctx` holds a second handle to the same clock.
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(1_000));
    let ctx = Ctx::fixed(clock.clone());

    ns.store_typed("resource", &1, &ctx)
        .await
        .expect("store the first value");
    let first = ns
        .get("resource")
        .await
        .expect("read the first row")
        .expect("the first row exists");
    assert_eq!(first.updated_at.timestamp(), 1_000);

    clock.advance_wall(Duration::from_secs(60));
    ns.store_typed("resource", &2, &ctx)
        .await
        .expect("store the second value");
    let second = ns
        .get("resource")
        .await
        .expect("read the second row")
        .expect("the second row exists");
    assert_eq!(second.updated_at.timestamp(), 1_060);
}
