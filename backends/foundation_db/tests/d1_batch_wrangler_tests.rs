//! `D1Store::execute_batch_statements` against the local wrangler worker
//! (`worker-stub.js`), whose D1 database is a real SQLite behind the D1
//! binding. Skipped unless `CF_INTEGRATION_TEST=1` (see [`common`]).
//!
//! WHY: the mock in `d1_batch_tests.rs` only proves what the client sends.
//! These tests prove that a failed statement rolls the whole batch back in a
//! real D1 database, and that every statement binds its own parameters.

#![cfg(feature = "d1")]

mod common;

use common::make_d1_store;
use foundation_core::valtron::valtron_test;
use foundation_db::{D1Store, DataValue, QueryStore, StorageError};
use futures_lite::StreamExt;

fn unique_table(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}_{nanos}")
}

async fn row_count(store: &D1Store, table: &str) -> i64 {
    let rows: Vec<_> = store
        .query(&format!("SELECT COUNT(*) AS cnt FROM {table}"), &[])
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    rows[0].get_by_name("cnt").unwrap()
}

#[valtron_test]
async fn d1_batch_rolls_back_on_failure_wrangler() {
    let Some(store) = make_d1_store() else {
        println!(
            "Skipping D1 batch test - wrangler worker not available (set CF_INTEGRATION_TEST=1)"
        );
        return;
    };
    let table = unique_table("batch_rollback");
    store
        .execute(
            &format!("CREATE TABLE {table} (k TEXT PRIMARY KEY, n INTEGER)"),
            &[],
        )
        .await
        .unwrap();

    // Statements one and two are valid. Statement three repeats the primary key
    // of statement one, so it fails: none of the three may stay.
    let insert = format!("INSERT INTO {table} (k, n) VALUES (?1, ?2)");
    let failing = vec![
        (
            insert.clone(),
            vec![DataValue::Text("a".to_string()), DataValue::Integer(1)],
        ),
        (
            insert.clone(),
            vec![DataValue::Text("b".to_string()), DataValue::Null],
        ),
        (
            insert.clone(),
            vec![DataValue::Text("a".to_string()), DataValue::Integer(3)],
        ),
    ];
    let err = store.execute_batch_statements(&failing).await.unwrap_err();
    assert!(
        matches!(err, StorageError::Backend(_)),
        "a failed statement fails the batch: {err:?}"
    );
    assert_eq!(
        row_count(&store, &table).await,
        0,
        "the failed batch must leave no row behind"
    );

    // The same table takes a batch whose statements all succeed, and each
    // statement bound its own `?1` / `?2`.
    let ok = vec![
        (
            insert.clone(),
            vec![DataValue::Text("a".to_string()), DataValue::Integer(1)],
        ),
        (
            insert,
            vec![DataValue::Text("b".to_string()), DataValue::Integer(2)],
        ),
    ];
    store.execute_batch_statements(&ok).await.unwrap();
    assert_eq!(row_count(&store, &table).await, 2);

    store
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
}
