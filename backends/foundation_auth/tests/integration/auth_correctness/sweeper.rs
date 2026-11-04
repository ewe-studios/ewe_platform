//! The `IdP` sweeps expired cache entries on an interval, as a valtron task,
//! and stops when the shutdown signal is set.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_auth::server::models::PendingLogin;
use foundation_auth::server::services::TosService;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::KeyValueStore;
use futures_lite::StreamExt;

use crate::integration::tos_login_gate::{config, start, Env};

/// Issue a `ToS` code that is already expired; return nothing (the entry stays
/// in the cache until a sweep removes it).
fn add_expired_code(env: &Env, user: &str) {
    let service = TosService::new(Arc::clone(&env.storage));
    let user = user.to_string();
    async move {
        service
            .gate(
                &user,
                PendingLogin {
                    auth_method: "password".into(),
                    client_id: None,
                    redirect_uri: None,
                    state: None,
                    nonce: None,
                },
                Duration::from_millis(1),
            )
            .await
            .unwrap()
            .expect("a code");
    }
    .block();
}

fn code_entries(env: &Env) -> usize {
    let storage = Arc::clone(&env.storage);
    async move {
        storage
            .cache
            .list_keys(Some("tos:code:"))
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
            .await
            .len()
    }
    .block()
}

#[valtron_test(threads = 8, timeout = 60000)]
fn expired_entries_are_swept_on_the_interval_and_sweeping_stops_on_shutdown() {
    let env = start(config().with_sweep_interval(Duration::from_millis(100)));
    env.seed_tos("v1", 1000);
    add_expired_code(&env, "u1");
    assert_eq!(code_entries(&env), 1, "the expired code is in the cache");

    let started = Instant::now();
    while code_entries(&env) > 0 {
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the expired entry was not swept within 1 s"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // After shutdown the task stops: a new expired entry stays.
    env.shutdown.turn_on();
    std::thread::sleep(Duration::from_millis(800));
    add_expired_code(&env, "u2");
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(code_entries(&env), 1, "no sweep after shutdown");
}
