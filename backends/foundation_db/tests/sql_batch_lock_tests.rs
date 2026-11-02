//! The libsql and Turso stores keep one connection behind an async read/write
//! lock: reads run together; a write or a batch (`BEGIN` .. `COMMIT`/`ROLLBACK`)
//! has the connection to itself. These tests run the same cases on both
//! backends.
//!
//! Without the lock, a statement run while a batch is open joins the batch's
//! transaction: a rollback erases it, and a second batch fails with "cannot
//! start a transaction within a transaction".

#![recursion_limit = "256"]
#![cfg(unix)]

use foundation_db::{DataValue, KeyValueStore, QueryStore};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Statements in each long batch: long enough that the batch stays open while
/// another thread writes (the stores have no hook between statements).
const BATCH_LEN: usize = 1500;

fn insert_statements(prefix: &str) -> Vec<(String, Vec<DataValue>)> {
    (0..BATCH_LEN)
        .map(|i| {
            (
                "INSERT INTO lock_test (key, value) VALUES (?, ?)".to_string(),
                vec![
                    DataValue::Text(format!("{prefix}{i}")),
                    DataValue::Text("batch".to_string()),
                ],
            )
        })
        .collect()
}

async fn count_rows<S: QueryStore>(store: &S, prefix: &str) -> i64 {
    let rows = store
        .query(
            "SELECT COUNT(*) AS cnt FROM lock_test WHERE key LIKE ?",
            &[DataValue::Text(format!("{prefix}%"))],
        )
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    rows[0].get::<i64>(0).unwrap()
}

/// CPU time used so far by the calling thread, in nanoseconds.
fn thread_cpu_nanos() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out pointer; CLOCK_THREAD_CPUTIME_ID is supported
    // on every unix target this crate tests on.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[cfg(feature = "turso")]
async fn open_turso_file(path: &str) -> foundation_db::StorageResult<foundation_db::TursoStorage> {
    foundation_db::TursoStorage::new(path).await
}

#[cfg(feature = "libsql")]
async fn open_libsql_file(path: &str) -> foundation_db::StorageResult<foundation_db::LibsqlStore> {
    foundation_db::LibsqlStore::new_kv(path, None).await
}

macro_rules! lock_tests {
    ($module:ident, $store:ty, $feature:literal, $memory:expr, $file:path) => {
        #[cfg(feature = $feature)]
        mod $module {
            use super::*;
            use foundation_core::valtron::valtron_test;

            const CREATE: &str =
                "CREATE TABLE lock_test (key TEXT PRIMARY KEY, value TEXT NOT NULL)";

            async fn memory_store() -> $store {
                let store: $store = $memory.await.unwrap();
                store.execute_batch(CREATE).await.unwrap();
                store
            }

            /// A write made while a batch is open is not part of the batch: it
            /// survives when the batch fails and rolls back.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn batch_rollback_does_not_erase_other_writes() {
                let store = memory_store().await;
                let started = Arc::new(AtomicBool::new(false));
                let done = Arc::new(AtomicBool::new(false));

                // The batch inserts many rows, then fails on a missing table.
                let mut stmts = insert_statements("b");
                stmts.push((
                    "INSERT INTO missing_table (key) VALUES ('x')".to_string(),
                    vec![],
                ));
                let batch_store = store.clone();
                let (started_t, done_t) = (Arc::clone(&started), Arc::clone(&done));
                let batch = thread::spawn(move || {
                    started_t.store(true, Ordering::SeqCst);
                    let result = futures_lite::future::block_on(
                        batch_store.execute_batch_statements(&stmts),
                    );
                    done_t.store(true, Ordering::SeqCst);
                    result
                });

                let deadline = Instant::now() + Duration::from_secs(30);
                while !started.load(Ordering::SeqCst) {
                    assert!(Instant::now() < deadline, "batch thread did not start");
                    thread::yield_now();
                }
                // Write single rows while the batch runs. A write that waits
                // for the batch, or runs before it, is fine; it must succeed
                // and it must stay.
                let mut written = 0_i64;
                while !done.load(Ordering::SeqCst) {
                    assert!(Instant::now() < deadline, "batch did not finish");
                    store
                        .execute(
                            "INSERT INTO lock_test (key, value) VALUES (?, ?)",
                            &[
                                DataValue::Text(format!("w{written}")),
                                DataValue::Text("single".to_string()),
                            ],
                        )
                        .await
                        .unwrap();
                    written += 1;
                }

                assert!(batch.join().unwrap().is_err(), "the batch must fail");
                assert!(written > 0, "no concurrent write was attempted");
                assert_eq!(
                    count_rows(&store, "b").await,
                    0,
                    "batch rows must roll back"
                );
                assert_eq!(
                    count_rows(&store, "w").await,
                    written,
                    "a concurrent write was rolled back with the batch"
                );
            }

            /// Two batches that start together both commit.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn concurrent_batches_both_commit() {
                let store = memory_store().await;
                let spawn_batch = |prefix: &'static str| {
                    let store = store.clone();
                    thread::spawn(move || {
                        futures_lite::future::block_on(
                            store.execute_batch_statements(&insert_statements(prefix)),
                        )
                    })
                };
                let first = spawn_batch("p");
                let second = spawn_batch("q");
                first.join().unwrap().expect("first batch must commit");
                second.join().unwrap().expect("second batch must commit");

                let expected = i64::try_from(BATCH_LEN).unwrap();
                assert_eq!(count_rows(&store, "p").await, expected);
                assert_eq!(count_rows(&store, "q").await, expected);
            }

            /// Two open row streams (each holds a read guard) coexist.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn reads_run_concurrently() {
                let store = memory_store().await;
                let (tx, rx) = mpsc::channel();
                let reader_store = store.clone();
                let reader = thread::spawn(move || {
                    futures_lite::future::block_on(async {
                        let first = reader_store.list_keys(None).await.unwrap();
                        // The second read starts while the first is still open.
                        let second = reader_store.list_keys(None).await.unwrap();
                        tx.send(()).unwrap();
                        drop((first, second));
                    });
                });
                rx.recv_timeout(Duration::from_secs(10))
                    .expect("a second read waited for the first read");
                reader.join().unwrap();
            }

            /// A write waits for an open read; a read that starts while the
            /// write waits runs after the write and sees its data.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn write_waits_for_reads_and_blocks_new_reads() {
                let store = memory_store().await;
                let open_read = store.list_keys(None).await.unwrap();
                let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));

                let writer_store = store.clone();
                let writer_events = Arc::clone(&events);
                let writer = thread::spawn(move || {
                    futures_lite::future::block_on(writer_store.set("k", 1_i64)).unwrap();
                    writer_events.lock().unwrap().push("write");
                });
                thread::sleep(Duration::from_millis(200));
                assert!(events.lock().unwrap().is_empty(), "write ran during a read");

                let reader_store = store.clone();
                let reader_events = Arc::clone(&events);
                let reader = thread::spawn(move || {
                    let value =
                        futures_lite::future::block_on(reader_store.get::<i64>("k")).unwrap();
                    reader_events.lock().unwrap().push("read");
                    value
                });
                thread::sleep(Duration::from_millis(200));
                assert!(
                    events.lock().unwrap().is_empty(),
                    "a new read ran while a write was waiting"
                );

                drop(open_read);
                writer.join().unwrap();
                let value = reader.join().unwrap();
                assert_eq!(*events.lock().unwrap(), vec!["write", "read"]);
                assert_eq!(value, Some(1), "the read must see the write");
            }

            /// A batch waits, parked, until an open row stream is dropped.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn batch_waits_for_open_row_stream() {
                let store = memory_store().await;
                let open_read = store.list_keys(None).await.unwrap();
                let (tx, rx) = mpsc::channel();
                let batch_store = store.clone();
                let batch = thread::spawn(move || {
                    let cpu_start = thread_cpu_nanos();
                    let result = futures_lite::future::block_on(
                        batch_store.execute_batch_statements(&insert_statements("s")[..2]),
                    );
                    let cpu = thread_cpu_nanos() - cpu_start;
                    tx.send(()).unwrap();
                    (result, cpu)
                });

                assert!(
                    rx.recv_timeout(Duration::from_millis(300)).is_err(),
                    "the batch finished while a row stream was open"
                );
                drop(open_read);
                rx.recv_timeout(Duration::from_secs(10))
                    .expect("the batch did not finish after the stream was dropped");
                let (result, cpu) = batch.join().unwrap();
                result.unwrap();
                assert!(
                    cpu < 20_000_000,
                    "the waiting batch used {cpu} ns of CPU in 300 ms: it spins"
                );
            }

            /// A failed rollback leaves the transaction open. The next
            /// operation notices, rolls back, and its write commits.
            #[valtron_test(threads = 3, timeout = 60_000)]
            async fn failed_rollback_leaves_store_usable() {
                let dir = tempfile::TempDir::new().unwrap();
                let path = dir.path().join("rollback.db");
                let path = path.to_str().unwrap();
                let store: $store = ($file)(path).await.unwrap();
                store.execute_batch(CREATE).await.unwrap();

                store.inject_rollback_failure();
                let stmts = vec![
                    (
                        "INSERT INTO lock_test (key, value) VALUES ('a', '1')".to_string(),
                        vec![],
                    ),
                    (
                        "INSERT INTO missing_table (key) VALUES ('x')".to_string(),
                        vec![],
                    ),
                ];
                let error = store
                    .execute_batch_statements(&stmts)
                    .await
                    .expect_err("the batch must fail")
                    .to_string();
                assert!(
                    error.contains("rollback also failed"),
                    "the rollback error must be attached: {error}"
                );

                store
                    .execute("INSERT INTO lock_test (key, value) VALUES ('b', '2')", &[])
                    .await
                    .unwrap();

                // A new store instance on the same file sees the commit, and
                // not the rolled-back row.
                let other: $store = ($file)(path).await.unwrap();
                assert_eq!(
                    count_rows(&other, "b").await,
                    1,
                    "the write was not committed"
                );
                assert_eq!(count_rows(&other, "a").await, 0, "the batch row leaked");
            }
        }
    };
}

lock_tests!(
    turso,
    foundation_db::TursoStorage,
    "turso",
    foundation_db::TursoStorage::in_memory(),
    open_turso_file
);
lock_tests!(
    libsql,
    foundation_db::LibsqlStore,
    "libsql",
    foundation_db::LibsqlStore::in_memory(),
    open_libsql_file
);
