//! Concurrent reads on the one Turso connection.
//!
//! Turso 0.8.3 lets only one statement use a connection at a time. Every
//! statement clones the connection's `ConcurrentGuard`, and `execute`/`step`
//! call `try_use`. A second statement that runs while the first is live fails
//! with `TursoError::Misuse("concurrent use forbidden")`. These tests prove
//! that the store serializes read-only statements and that all reads succeed.

#![cfg(all(unix, feature = "turso"))]
#![recursion_limit = "256"]

use foundation_core::valtron::valtron_test;
use foundation_db::{DataValue, QueryStore, TursoStorage};
use std::thread;

/// Threads that read at the same time.
const READERS: usize = 8;
/// Queries each reader runs.
const ROUNDS: usize = 100;
/// Rows in the table, so each query steps many times.
const ROWS: usize = 200;

/// A store with a table of `ROWS` rows. Each row holds 2 KiB, so one step does
/// real work and the window for a concurrent step is wide.
async fn store_with_rows() -> TursoStorage {
    let store = TursoStorage::in_memory(foundation_context::Ctx::host())
        .await
        .unwrap();
    store
        .execute_batch("CREATE TABLE reads_test (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .await
        .unwrap();
    let filler = "x".repeat(2048);
    let statements: Vec<(String, Vec<DataValue>)> = (0..ROWS)
        .map(|i| {
            (
                "INSERT INTO reads_test (key, value) VALUES (?, ?)".to_string(),
                vec![
                    DataValue::Text(format!("k{i}")),
                    DataValue::Text(filler.clone()),
                ],
            )
        })
        .collect();
    store.execute_batch_statements(&statements).await.unwrap();
    store
}

/// Eight threads read the table at the same time, many times. Every query must
/// succeed: a "concurrent use forbidden" error fails the test.
#[valtron_test(threads = 8, timeout = 300_000)]
async fn concurrent_reads_all_succeed() {
    let store = store_with_rows().await;
    let mut readers = Vec::new();
    for reader in 0..READERS {
        let store = store.clone();
        readers.push(thread::spawn(move || {
            futures_lite::future::block_on(async {
                for round in 0..ROUNDS {
                    let rows = store
                        .query("SELECT key, value FROM reads_test", &[])
                        .await
                        .unwrap_or_else(|e| panic!("reader {reader} round {round}: {e}"))
                        .collect_all()
                        .await
                        .unwrap_or_else(|e| panic!("reader {reader} round {round}: {e}"));
                    assert_eq!(rows.len(), ROWS);
                }
            });
        }));
    }
    for reader in readers {
        reader.join().unwrap();
    }
}

/// Two reads from one task are serialized: the second `query` waits until the
/// first row stream ends. A short limit turns the wait into `LockTimeout`, so
/// the test proves the wait without a long sleep.
#[valtron_test(threads = 4, timeout = 60_000)]
async fn second_read_waits_for_the_first() {
    let store = store_with_rows()
        .await
        .with_lock_timeout(std::time::Duration::from_millis(200));
    let first = store
        .query("SELECT key FROM reads_test", &[])
        .await
        .unwrap();
    let Err(error) = store.query("SELECT key FROM reads_test", &[]).await else {
        panic!("the second read must wait for the first");
    };
    assert!(
        matches!(error, foundation_db::StorageError::LockTimeout { .. }),
        "unexpected error: {error}"
    );
    drop(first);
    // The lock is free again.
    store
        .query("SELECT key FROM reads_test", &[])
        .await
        .unwrap();
}
