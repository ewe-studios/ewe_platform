//! `LibsqlStore::execute_batch_statements` all-or-nothing transaction tests
//! against a local libsql file.

#![cfg(feature = "libsql-local")]

use foundation_core::valtron::valtron_test;
use foundation_db::{LibsqlStore, QueryStore};
use tempfile::TempDir;

#[valtron_test]
async fn batch_statements_apply_atomically() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let storage = LibsqlStore::new_kv(
        db_path.to_str().unwrap(),
        None,
        foundation_context::Ctx::host(),
    )
    .await
    .unwrap();
    storage
        .execute_batch("CREATE TABLE batch_test (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .await
        .unwrap();

    let stmts = vec![
        (
            "INSERT INTO batch_test (key, value) VALUES ('a', '1')".to_string(),
            vec![],
        ),
        (
            "INSERT INTO batch_test (key, value) VALUES ('b', '2')".to_string(),
            vec![],
        ),
        (
            "INSERT INTO missing_table (key, value) VALUES ('c', '3')".to_string(),
            vec![],
        ),
    ];
    assert!(storage.execute_batch_statements(&stmts).await.is_err());

    let rows = storage
        .query("SELECT COUNT(*) AS cnt FROM batch_test", &[])
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(rows[0].get::<i64>(0).unwrap(), 0);
}

#[valtron_test]
async fn imports_keep_receipts_and_conditional_updates_preserve_deletions() {
    use foundation_db::KeyValueStore;
    use std::collections::BTreeMap;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Records ü.sqlite3");
    let open = async || {
        let store = LibsqlStore::new_kv(
            path.to_str().unwrap(),
            None,
            foundation_context::Ctx::host(),
        )
        .await
        .unwrap();
        store.init_kv().await.unwrap();
        store
    };
    let first = open().await;
    let second = open().await;
    let legacy = BTreeMap::from([("a".into(), "original"), ("b".into(), "another")]);
    first
        .change_records(
            "rows/",
            &BTreeMap::from([("b", &"existing")]),
            &BTreeMap::new(),
            &[],
        )
        .await
        .unwrap();
    first
        .import_once("rows/", "imported/", &legacy)
        .await
        .unwrap();
    assert_eq!(
        second.get::<String>("rows/b").await.unwrap().as_deref(),
        Some("existing")
    );
    second
        .change_records(
            "rows/",
            &BTreeMap::<&str, &String>::new(),
            &BTreeMap::new(),
            &["a"],
        )
        .await
        .unwrap();
    first
        .change_records(
            "rows/",
            &BTreeMap::new(),
            &BTreeMap::from([("a", &"stale")]),
            &[],
        )
        .await
        .unwrap();
    drop(first);
    drop(second);
    let restarted = open().await;
    restarted
        .import_once("rows/", "imported/", &legacy)
        .await
        .unwrap();
    assert!(!restarted.exists("rows/a").await.unwrap());
    assert!(restarted.exists("imported/a").await.unwrap());
    assert_eq!(
        restarted.get::<String>("rows/b").await.unwrap().as_deref(),
        Some("existing")
    );
}

#[valtron_test]
async fn record_writes_keep_the_creation_time_and_use_the_injected_update_time() {
    use foundation_core::valtron::SimulatedClock;
    use foundation_db::KeyValueStore;
    use std::{collections::BTreeMap, sync::Arc, time::Duration};

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_millis(700));
    let db = LibsqlStore::new_kv(
        ":memory:",
        None,
        foundation_context::Ctx::fixed(clock.clone()),
    )
    .await
    .unwrap();
    db.init_kv().await.unwrap();
    db.set("direct", 7).await.unwrap();
    db.change_records(
        "row/",
        &BTreeMap::from([("one", &1)]),
        &BTreeMap::new(),
        &[],
    )
    .await
    .unwrap();
    clock.advance_wall(Duration::from_millis(100));
    db.set("direct", 8).await.unwrap();
    db.change_records(
        "row/",
        &BTreeMap::new(),
        &BTreeMap::from([("one", &2)]),
        &[],
    )
    .await
    .unwrap();
    db.import_once("row/", "receipt/", &BTreeMap::from([("two".into(), 2)]))
        .await
        .unwrap();
    let rows = db
        .query(
            "SELECT key,created_at,updated_at FROM kv_store ORDER BY key",
            &[],
        )
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    let times: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row.get::<String>(0).unwrap(),
                row.get::<i64>(1).unwrap(),
                row.get::<i64>(2).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        times,
        [
            ("direct".into(), 700, 800),
            ("receipt/two".into(), 800, 800),
            ("row/one".into(), 700, 800),
            ("row/two".into(), 800, 800)
        ]
    );
}

#[test]
fn separate_connections_preserve_two_simultaneous_transactions() {
    use foundation_db::KeyValueStore;
    use std::{collections::BTreeMap, sync::Barrier, time::Duration};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("concurrent.sqlite3");
    let open = || {
        futures_lite::future::block_on(async {
            let db = LibsqlStore::new_kv(
                path.to_str().unwrap(),
                None,
                foundation_context::Ctx::host(),
            )
            .await
            .unwrap();
            db.set_busy_timeout(Duration::from_secs(5)).await.unwrap();
            db.init_kv().await.unwrap();
            db
        })
    };
    let reader = open();
    let left = open();
    let right = open();
    let start = Barrier::new(2);
    std::thread::scope(|scope| {
        for (db, id) in [(left, "left"), (right, "right")] {
            let barrier = &start;
            scope.spawn(move || {
                barrier.wait();
                futures_lite::future::block_on(db.change_records(
                    "rows/",
                    &BTreeMap::from([(id, &id)]),
                    &BTreeMap::new(),
                    &[],
                ))
                .unwrap();
            });
        }
    });
    futures_lite::future::block_on(async {
        for id in ["left", "right"] {
            assert_eq!(
                reader
                    .get::<String>(&format!("rows/{id}"))
                    .await
                    .unwrap()
                    .as_deref(),
                Some(id)
            );
        }
    });
}
