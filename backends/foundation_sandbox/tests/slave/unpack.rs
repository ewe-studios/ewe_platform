//! The unpack writer parks on pipe readiness (issue #977).
//!
//! The test fills every task worker of the pool with writers whose child does
//! not read for a second. A blocking write holds all the workers, so a queued
//! progress task cannot run. A write that parks on writability releases the
//! workers, so the progress task runs while the writers wait. One writer
//! streams a 100 MiB archive.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::env;
use foundation_core::valtron::timer::sleep_until;
use foundation_core::valtron::{from_future, send};
use foundation_sandbox::native::slave::unpack::{ChildOptions, UnpackSessions};
use foundation_sandbox::shared::error::SandboxResult;

use super::support::TempDir;

/// The writers that fill the task workers.
const WRITERS: usize = 8;
/// The largest chunk of one write.
const CHUNK: usize = 64 * 1024;
/// The size of the archive that the first writer streams.
const BIG_ARCHIVE: usize = 100 * 1024 * 1024;
/// The size of every other writer's archive.
const SMALL_ARCHIVE: usize = 1024 * 1024;

/// The child options that the probe ignores.
fn options() -> ChildOptions {
    ChildOptions {
        oci_whiteouts: false,
        keep_existing_dirs: false,
        skip: Vec::new(),
        max_entries: 1_000_000,
        max_bytes: 1024 * 1024 * 1024,
        max_path_bytes: 4096,
    }
}

/// Streams one archive through one session and commits it.
async fn write_archive(
    sessions: Arc<UnpackSessions>,
    root: PathBuf,
    bytes: usize,
) -> SandboxResult<(u64, u64)> {
    let upload = sessions.begin(&root, "/", &options()).await?;
    let chunk = vec![0x5a_u8; CHUNK];
    let mut offset = 0_u64;
    while (offset as usize) < bytes {
        let take = CHUNK.min(bytes - (offset as usize));
        sessions.chunk(&upload, offset, &chunk[..take]).await?;
        offset = offset.saturating_add(u64::try_from(take).unwrap_or(u64::MAX));
    }
    sessions.commit(&upload, offset).await
}

/// Waits until `condition` holds or `timeout` passes.
fn wait_for(timeout: Duration, condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    condition()
}

/// The writers park, so a queued task runs while the child reads nothing.
#[test]
fn unpack_writes_park_on_a_slow_reader() {
    crate::pool::pool();
    let ctx = Ctx::host();
    let environment = env::system(&ctx).expect("the host environment");
    ctx.store(environment);
    let directory = TempDir::new("unpack-parking");
    let root: PathBuf = directory.path().to_path_buf();
    let sessions = Arc::new(
        UnpackSessions::new(
            ctx.clone(),
            WRITERS + 2,
            Duration::from_secs(60),
            4 * 1024 * 1024,
        )
        .with_child_program(PathBuf::from(env!("CARGO_BIN_EXE_unpack_child_probe"))),
    );

    let completed = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let first_bytes = Arc::new(AtomicU64::new(0));
    let progress = Arc::new(AtomicBool::new(false));

    // The writers fill every task worker, then park on the full pipes.
    for index in 0..WRITERS {
        let sessions = Arc::clone(&sessions);
        let root = root.clone();
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);
        let first_bytes = Arc::clone(&first_bytes);
        let bytes = if index == 0 {
            BIG_ARCHIVE
        } else {
            SMALL_ARCHIVE
        };
        send(from_future(async move {
            match write_archive(sessions, root, bytes).await {
                Ok((_, read)) => {
                    if index == 0 {
                        first_bytes.store(read, Ordering::SeqCst);
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }
                Err(_) => {
                    failed.fetch_add(1, Ordering::SeqCst);
                }
            }
        }))
        .expect("submit the unpack writer");
    }

    // A short task queued behind the writers. It runs at once only when the
    // writers park.
    let task_ctx = ctx.clone();
    let flag = Arc::clone(&progress);
    send(from_future(async move {
        let deadline = task_ctx.clock().now() + Duration::from_millis(100);
        sleep_until(deadline, &task_ctx).await;
        flag.store(true, Ordering::SeqCst);
    }))
    .expect("submit the progress task");

    // The probe child reads nothing for one second, so a blocking writer holds
    // its worker for that whole second. The queued task must still run.
    assert!(
        wait_for(Duration::from_millis(800), || progress
            .load(Ordering::SeqCst)),
        "the progress task runs while every writer waits: the writes park"
    );

    // After the child drains, every writer completes.
    assert!(
        wait_for(Duration::from_secs(30), || {
            completed.load(Ordering::SeqCst) + failed.load(Ordering::SeqCst) == WRITERS
        }),
        "every writer ends"
    );
    assert_eq!(failed.load(Ordering::SeqCst), 0, "no writer fails");
    assert_eq!(
        first_bytes.load(Ordering::SeqCst),
        u64::try_from(BIG_ARCHIVE).expect("the archive size"),
        "the child read every byte of the 100 MiB archive"
    );
    sessions.remove_all();
}
