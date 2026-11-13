//! Vault expiry reads the injected `Clock` (spec 70 feature 133).
//!
//! WHY: before this feature, `sends::create` and `sends::access` read the real
//! wall clock, so a deterministic test could not control a send's default
//! deletion date or its expiry check.
//!
//! WHAT: a send created with no explicit deletion date gets one seven days
//! after the simulated wall time. An explicit expiry 60 s after the simulated
//! wall time is live at that instant and gone once the simulated clock moves
//! past it.
//!
//! HOW: `SimulatedClock` starts at the UNIX epoch, so the test advances it to a
//! known wall time (2026-01-01T00:00:00Z). Before the fix the deletion date
//! followed real time, and the expiry check used real time, so both assertions
//! fail.

// The valtron stream chain (`execute(from_future(..))`) nests past the default
// trait-solver recursion limit (128); see `foundation_db/src/lib.rs`.
#![recursion_limit = "256"]

use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::DateTime;
use foundation_compact::time::unix_seconds;
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::{
    collect_one, execute, from_future, valtron_test, BlockingFutureExt, SimulatedClock,
};
use foundation_db::core::storage_provider::QueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::sends;
use foundation_keychain::core::models::send::{SendCreateRequest, SendType};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

/// 2026-01-01T00:00:00Z in unix seconds.
const SIMULATED_SECS: u64 = 1_767_225_600;
/// The explicit send expiry, in seconds after the simulated wall time.
const EXPIRY_SECS: i64 = 60;
/// The send's default deletion window, in seconds (seven days).
const DELETION_SECS: i64 = 7 * 24 * 60 * 60;

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

fn fresh_context(clock: Arc<SimulatedClock>) -> (KeychainContext, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("keychain_clock_test.db");
    let provider = BlockingFutureExt::block({
        let url = db_path.to_str().unwrap().to_string();
        async move {
            StorageProvider::new(StorageBackend::Turso {
                url,
                ctx: foundation_context::Ctx::host().into(),
            })
            .await
        }
    })
    .expect("init turso");
    let db: Arc<dyn QueryStore> = Arc::new(provider);
    let db_for_schema = Arc::clone(&db);
    drive(async move { apply_schema(db_for_schema.as_ref()).await }).expect("apply schema");
    (KeychainContext::new(db, Ctx::fixed(clock)), dir)
}

fn send_request(expiration_date: Option<String>) -> SendCreateRequest {
    SendCreateRequest {
        name: "Timed send".into(),
        notes: None,
        send_type: SendType::Text,
        data: Some("2.text|enc".into()),
        key: Some("2.key|enc".into()),
        password: None,
        max_access_count: None,
        deletion_date: None,
        expiration_date,
        disabled: Some(false),
        hide_email: Some(false),
    }
}

#[valtron_test]
fn send_dates_and_expiry_follow_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(StdDuration::ZERO));
    clock.advance_wall(StdDuration::from_secs(SIMULATED_SECS));
    let (ctx, _dir) = fresh_context(Arc::clone(&clock));

    let now_secs = unix_seconds(clock.wall_now());
    assert_eq!(now_secs, i64::try_from(SIMULATED_SECS).unwrap());
    let expires_at = DateTime::from_timestamp(now_secs + EXPIRY_SECS, 0).expect("valid timestamp");

    let c = ctx.clone();
    let created = drive(async move {
        sends::create(
            &c,
            "user-clock",
            send_request(Some(expires_at.to_rfc3339())),
        )
        .await
    })
    .expect("create send");

    // The default deletion date is seven days after the simulated wall time.
    let expected_deletion =
        DateTime::from_timestamp(now_secs + DELETION_SECS, 0).expect("valid timestamp");
    assert_eq!(
        created.deletion_date, expected_deletion,
        "the default deletion date must follow the injected clock"
    );
    assert_eq!(created.expiration_date, Some(expires_at));

    // At the simulated instant the send is still live.
    let c = ctx.clone();
    let id = created.id.clone();
    drive(async move { sends::access(&c, &id, None).await })
        .expect("the send is live before its expiry");

    // Past the simulated expiry the send is gone, even though real time is far
    // beyond the simulated instant.
    clock.advance_wall(StdDuration::from_secs(
        u64::try_from(EXPIRY_SECS + 1).unwrap(),
    ));
    let c = ctx.clone();
    let id = created.id.clone();
    let gone = drive(async move { sends::access(&c, &id, None).await });
    assert!(
        matches!(gone, Err(trace) if matches!(trace.current_context(), AppError::NotFound(_))),
        "the send must be gone after the injected clock passes its expiry"
    );
}

/// A keychain context reads its clock through the `Ctx` it was built from. An
/// advance of the simulated clock shows through the context's own `ctx()`.
#[valtron_test]
fn the_keychain_context_reads_its_clock_through_the_ctx_it_holds() {
    let clock = Arc::new(SimulatedClock::new(StdDuration::ZERO));
    clock.advance_wall(StdDuration::from_secs(SIMULATED_SECS));
    let (ctx, _dir) = fresh_context(Arc::clone(&clock));

    let before = unix_seconds(ctx.ctx().clock().wall_now());
    assert_eq!(before, i64::try_from(SIMULATED_SECS).unwrap());

    clock.advance_wall(StdDuration::from_secs(90));
    let after = unix_seconds(ctx.ctx().clock().wall_now());
    assert_eq!(after - before, 90, "the context must see the advance");
}
