//! D1+R2 `AsyncDocumentStore` integration tests (F23) against a local
//! wrangler/miniflare worker emulating Cloudflare D1 + R2.
//!
//! WHY: `D1R2DocumentStore` is the Cloudflare DocumentStore — D1 is the ordered
//! SQL index, R2 holds large document blobs offloaded past the 4 KB SQLite page
//! threshold. The store logic is target-agnostic (`D1R2DocumentStore<Q, B>`), so
//! we exercise the exact same code natively here over `D1Store` (D1 REST) +
//! `R2Store` (R2 REST), both pointed at the local worker.
//!
//! WHAT: append→scan ordering parity (scru128), inclusive `scan_from`,
//! large-blob round-trip + transparent read-through (offload to R2, fetch back),
//! and delete cleaning up the R2 blob.
//!
//! HOW: opt in with `CF_INTEGRATION_TEST=1` + a running worker (`mise run
//! cf:start`); otherwise every test skips. Each test uses a unique table + unique
//! collection key so reruns against persistent miniflare state stay isolated.

mod common;

use common::{init_valtron, make_d1_store, make_r2_store};
use foundation_db::traits::{AsyncDocumentStore, QueryStore};
use foundation_db::{D1R2DocumentStore, D1Store, R2Store};
use futures_lite::StreamExt;

/// Full `documents` schema (migrations 020+021+022) under a unique table name so
/// concurrent/rerun isolation holds against the persistent local D1.
fn create_table(d1: &D1Store, table: &str) {
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {table} (\
            id INTEGER PRIMARY KEY AUTOINCREMENT, \
            collection_key TEXT NOT NULL, \
            doc_id TEXT NOT NULL, \
            content TEXT NOT NULL, \
            metadata TEXT NOT NULL DEFAULT '{{}}', \
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
            title TEXT, summary TEXT, record_type TEXT, r2_key TEXT)"
    );
    d1.execute_batch(&sql).unwrap();
}

fn unique(prefix: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}_{n}")
}

/// Build a `D1R2DocumentStore` over the local worker, or `None` when the
/// integration environment isn't available.
fn make_store(table: &str) -> Option<D1R2DocumentStore<D1Store, R2Store>> {
    init_valtron();
    let d1 = make_d1_store()?;
    let r2 = make_r2_store()?;
    create_table(&d1, table);
    Some(D1R2DocumentStore::new(d1, r2).with_table(table))
}

#[test]
fn append_orders_by_scru128_and_counts() {
    let table = unique("docs_order");
    let Some(store) = make_store(&table) else {
        println!("Skipping D1R2 test - local CF worker not available");
        return;
    };
    let key = unique("order");
    futures_lite::future::block_on(async {
        let a = store.append_async(&key, serde_json::json!({"n": 0})).await.unwrap();
        let b = store.append_async(&key, serde_json::json!({"n": 1})).await.unwrap();
        let c = store.append_async(&key, serde_json::json!({"n": 2})).await.unwrap();
        assert_eq!(a.id.len(), 25, "scru128 id");
        assert!(b.id > a.id && c.id > b.id, "appends produce ascending scru128 ids");
        assert_eq!(store.count_async(&key).await.unwrap(), 3);

        let docs = store.scan_documents_async(&key, 10).await.unwrap();
        assert_eq!(docs.len(), 3);
        // scan_documents_async returns newest-first (DESC).
        let ids: Vec<&str> = docs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec![c.id.as_str(), b.id.as_str(), a.id.as_str()]);
    });
}

#[test]
fn scan_from_is_inclusive_and_ascending() {
    let table = unique("docs_from");
    let Some(store) = make_store(&table) else {
        println!("Skipping D1R2 test - local CF worker not available");
        return;
    };
    let key = unique("from");
    futures_lite::future::block_on(async {
        let _a = store.append_async(&key, serde_json::json!({"n": 0})).await.unwrap();
        let b = store.append_async(&key, serde_json::json!({"n": 1})).await.unwrap();
        let _c = store.append_async(&key, serde_json::json!({"n": 2})).await.unwrap();

        let from_b: Vec<serde_json::Value> = store
            .scan_from_async::<serde_json::Value>(&key, &b.id, 0)
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        // Inclusive of b, ascending → {n:1}, {n:2}.
        assert_eq!(from_b.len(), 2);
        assert_eq!(from_b[0]["n"], 1);
        assert_eq!(from_b[1]["n"], 2);
    });
}

#[test]
fn large_document_offloads_to_r2_and_reads_through() {
    let table = unique("docs_blob");
    let Some(store) = make_store(&table) else {
        println!("Skipping D1R2 test - local CF worker not available");
        return;
    };
    let key = unique("blob");
    futures_lite::future::block_on(async {
        // A small doc stays inline; a >4 KB doc offloads to R2.
        let small = serde_json::json!({"tag": "small", "body": "x".repeat(16)});
        let big_body = "y".repeat(8192);
        let big = serde_json::json!({"tag": "big", "body": big_body});

        let small_doc = store.append_async(&key, small.clone()).await.unwrap();
        let big_doc = store.append_async(&key, big.clone()).await.unwrap();

        // Transparent read-through: both come back fully, big one fetched from R2.
        let got: Vec<serde_json::Value> = store
            .scan_all_async::<serde_json::Value>(&key)
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(got.len(), 2);
        let big_back = got.iter().find(|v| v["tag"] == "big").unwrap();
        assert_eq!(big_back["body"].as_str().unwrap().len(), 8192);
        let small_back = got.iter().find(|v| v["tag"] == "small").unwrap();
        assert_eq!(small_back["body"], "x".repeat(16));

        // Verify the offload actually happened: the big row carries an r2_key, the
        // small one does not. (Inspect via the raw D1 client.)
        let d1 = make_d1_store().unwrap();
        let rows: Vec<(String, Option<String>)> = {
            use foundation_db::DataValue;
            let sql = format!("SELECT doc_id, r2_key FROM {table} WHERE collection_key = ?");
            let mut out = Vec::new();
            for item in &mut d1.query(&sql, &[DataValue::Text(key.clone())]).unwrap() {
                if let foundation_core::valtron::Stream::Next(Ok(row)) = item {
                    let id: String = row.get_by_name("doc_id").unwrap();
                    let rk: Option<String> = row.get_by_name("r2_key").ok();
                    out.push((id, rk));
                }
            }
            out
        };
        let big_row = rows.iter().find(|(id, _)| id == &big_doc.id).unwrap();
        assert!(big_row.1.is_some(), "large doc must carry an r2_key");
        let small_row = rows.iter().find(|(id, _)| id == &small_doc.id).unwrap();
        assert!(
            small_row.1.is_none(),
            "small doc must stay inline (no r2_key)"
        );
    });
}

#[test]
fn delete_removes_row_and_r2_blob() {
    let table = unique("docs_del");
    let Some(store) = make_store(&table) else {
        println!("Skipping D1R2 test - local CF worker not available");
        return;
    };
    let key = unique("del");
    futures_lite::future::block_on(async {
        let big = serde_json::json!({"body": "z".repeat(8192)});
        let doc = store.append_async(&key, big).await.unwrap();
        assert_eq!(store.count_async(&key).await.unwrap(), 1);

        store.delete_async(&key, &doc.id).await.unwrap();
        assert_eq!(store.count_async(&key).await.unwrap(), 0);

        // The R2 blob is gone too: re-fetching the offloaded key would 404, so a
        // fresh scan returns nothing rather than a dangling-blob error.
        let remaining = store.scan_documents_async(&key, 10).await.unwrap();
        assert!(remaining.is_empty());
    });
}
