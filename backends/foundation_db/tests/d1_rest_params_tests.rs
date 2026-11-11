//! D1 REST parameters: the API takes **strings only**, so `D1Store` sends each
//! value as a string and types it in SQL (`CAST(? AS INTEGER)`, `CAST(? AS
//! REAL)`, the `NULL` literal, `unhex(?)`). These tests check the SQLite storage
//! class each `DataValue` ends up in (`typeof(col)`) against wrangler's local
//! explorer API (`wrangler dev --local`, see [`common`]). Skipped unless
//! `CF_INTEGRATION_TEST=1`.
//!
//! The columns are declared without a type, so SQLite applies no column
//! affinity: the storage class is the one the SQL gave the value. The binding's
//! own type rules (a JS Number is always REAL there) are tested on the real
//! binding in the wasm testbed.

#![cfg(feature = "d1")]

mod common;

use common::make_d1_store;
use foundation_core::valtron::valtron_test;
use foundation_db::{D1Store, DataValue, QueryStore, SqlRow};
use futures_lite::StreamExt;

fn unique_table(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}_{nanos}")
}

async fn rows(store: &D1Store, sql: &str, params: &[DataValue]) -> Vec<SqlRow> {
    store
        .query(sql, params)
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await
}

/// `typeof()` of the four columns of the first row of `table`.
async fn types_of(store: &D1Store, table: &str, id: i64) -> [String; 4] {
    let found = rows(
        store,
        &format!(
            "SELECT typeof(i) AS ti, typeof(r) AS tr, typeof(n) AS tn, typeof(t) AS tt \
             FROM {table} WHERE id = ?1"
        ),
        &[DataValue::Integer(id)],
    )
    .await;
    assert_eq!(found.len(), 1, "row {id} of {table}");
    ["ti", "tr", "tn", "tt"].map(|c| found[0].get_by_name::<String>(c).unwrap())
}

async fn make_table(store: &D1Store, table: &str) {
    store
        .execute(&format!("CREATE TABLE {table} (id, i, r, n, t)"), &[])
        .await
        .unwrap();
}

#[valtron_test]
async fn d1_rest_params_are_strings_and_typed_by_cast() {
    let Some(store) = make_d1_store() else {
        println!("Skipping D1 param type test - wrangler explorer API not available (set CF_INTEGRATION_TEST=1)");
        return;
    };
    let table = unique_table("param_types");
    make_table(&store, &table).await;

    // [7, 1.5, null, "x"] -> integer, real, null, text.
    store
        .execute(
            &format!("INSERT INTO {table} (id, i, r, n, t) VALUES (?1, ?2, ?3, ?4, ?5)"),
            &[
                DataValue::Integer(1),
                DataValue::Integer(7),
                DataValue::Real(1.5),
                DataValue::Null,
                DataValue::Text("x".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        types_of(&store, &table, 1).await,
        ["integer", "real", "null", "text"]
    );

    // A whole-number REAL stays REAL: `2.0` is not an integer.
    store
        .execute(
            &format!("INSERT INTO {table} (id, i, r, n, t) VALUES (?1, ?2, ?3, ?4, ?5)"),
            &[
                DataValue::Integer(2),
                DataValue::Integer(-3),
                DataValue::Real(2.0),
                DataValue::Null,
                DataValue::Text(String::new()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        types_of(&store, &table, 2).await,
        ["integer", "real", "null", "text"]
    );

    // Bare `?` placeholders bind in order too.
    store
        .execute(
            &format!("INSERT INTO {table} (id, i, r, n, t) VALUES (?, ?, ?, ?, ?)"),
            &[
                DataValue::Integer(3),
                DataValue::Integer(0),
                DataValue::Real(-0.25),
                DataValue::Null,
                DataValue::Text("it's ? text".to_string()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        types_of(&store, &table, 3).await,
        ["integer", "real", "null", "text"]
    );

    store
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
}

#[valtron_test]
async fn d1_rest_batch_with_mixed_types() {
    let Some(store) = make_d1_store() else {
        println!("Skipping D1 param type test - wrangler explorer API not available (set CF_INTEGRATION_TEST=1)");
        return;
    };
    let table = unique_table("param_batch");
    make_table(&store, &table).await;

    let insert = format!("INSERT INTO {table} (id, i, r, n, t) VALUES (?1, ?2, ?3, ?4, ?5)");
    let statements = vec![
        (
            insert.clone(),
            vec![
                DataValue::Integer(1),
                DataValue::Integer(7),
                DataValue::Real(1.5),
                DataValue::Null,
                DataValue::Text("x".to_string()),
            ],
        ),
        (
            insert,
            vec![
                DataValue::Integer(2),
                DataValue::Null,
                DataValue::Integer(9),
                DataValue::Text("y".to_string()),
                DataValue::Real(0.5),
            ],
        ),
        (
            format!("UPDATE {table} SET n = ?2 WHERE id = ?1"),
            vec![DataValue::Integer(1), DataValue::Integer(42)],
        ),
    ];
    store.execute_batch_statements(&statements).await.unwrap();

    // Row 1: the update turned `n` from null into an integer.
    assert_eq!(
        types_of(&store, &table, 1).await,
        ["integer", "real", "integer", "text"]
    );
    // Row 2: each statement kept its own types (integer 9 in the REAL slot
    // stays an integer, 0.5 in the TEXT slot stays real).
    assert_eq!(
        types_of(&store, &table, 2).await,
        ["null", "integer", "text", "real"]
    );

    store
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
}

#[valtron_test]
async fn d1_rest_i64_extremes_round_trip() {
    let Some(store) = make_d1_store() else {
        println!("Skipping D1 param type test - wrangler explorer API not available (set CF_INTEGRATION_TEST=1)");
        return;
    };
    let table = unique_table("param_i64");
    store
        .execute(&format!("CREATE TABLE {table} (id, i)"), &[])
        .await
        .unwrap();

    for (id, value) in [(1_i64, i64::MAX), (2, i64::MIN), (3, 9_007_199_254_740_993)] {
        store
            .execute(
                &format!("INSERT INTO {table} (id, i) VALUES (?1, ?2)"),
                &[DataValue::Integer(id), DataValue::Integer(value)],
            )
            .await
            .unwrap();

        // Read back as text inside SQLite: the worker's JSON replies hold
        // JavaScript numbers (doubles), which cannot show the last digits.
        let found = rows(
            &store,
            &format!(
                "SELECT typeof(i) AS ti, CAST(i AS TEXT) AS txt, i = ?2 AS same \
                 FROM {table} WHERE id = ?1"
            ),
            &[DataValue::Integer(id), DataValue::Integer(value)],
        )
        .await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].get_by_name::<String>("ti").unwrap(), "integer");
        assert_eq!(
            found[0].get_by_name::<String>("txt").unwrap(),
            value.to_string()
        );
        assert_eq!(found[0].get_by_name::<i64>("same").unwrap(), 1);
    }

    store
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
}

#[valtron_test]
async fn d1_rest_blob_and_text_round_trip() {
    let Some(store) = make_d1_store() else {
        println!("Skipping D1 param type test - wrangler explorer API not available (set CF_INTEGRATION_TEST=1)");
        return;
    };
    let table = unique_table("param_blob");
    store
        .execute(&format!("CREATE TABLE {table} (id, b, t)"), &[])
        .await
        .unwrap();
    store
        .execute(
            &format!("INSERT INTO {table} (id, b, t) VALUES (?, ?, ?)"),
            &[
                DataValue::Integer(1),
                DataValue::Blob(vec![0x00, 0x01, 0xFF, 0x7F]),
                // A `?` inside the text and a quote are values, not placeholders.
                DataValue::Text("it's a ? and \"two\"".to_string()),
            ],
        )
        .await
        .unwrap();
    let found = rows(
        &store,
        &format!("SELECT typeof(b) AS tb, hex(b) AS hb, t FROM {table} WHERE id = ?"),
        &[DataValue::Integer(1)],
    )
    .await;
    assert_eq!(found[0].get_by_name::<String>("tb").unwrap(), "blob");
    assert_eq!(found[0].get_by_name::<String>("hb").unwrap(), "0001FF7F");
    assert_eq!(
        found[0].get_by_name::<String>("t").unwrap(),
        "it's a ? and \"two\""
    );
    store
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
}
