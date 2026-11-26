//! The Durable Object store contract (spec 83 feature 05, T20).
//!
//! The Durable Object SQL application programming interface (API) needs
//! `Miniflare` and a Worker. When that is absent, the same statements run on an
//! in-memory SQLite database through [`SqliteDoSql`]. Both engines follow the
//! contract that these tests pin: the document scan, the key-value store, the
//! blob store, the rate limiter and the SQL parameters.
//!
//! The `cf-agent-rpc` Miniflare case proves the same contract on the real
//! `worker::sql::SqlStorage`.

use std::sync::Arc;

use foundation_context::Ctx;
use foundation_db::{
    DoSqlStore, DocumentStore, KeyValueStore, RateLimiterStore, SqlDocumentStore, SqliteDoSql,
};
use futures_lite::future::block_on;
use serde::{Deserialize, Serialize};

fn ctx() -> Ctx {
    Ctx::host()
}

/// A fresh store with the schema that a Durable Object creates on first use.
fn store() -> DoSqlStore {
    block_on(async {
        let engine = SqliteDoSql::in_memory(ctx()).await.unwrap();
        let store = DoSqlStore::new(Arc::new(engine), "agent_rpc");
        store.init_schema().await.unwrap();
        store
    })
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Note {
    text: String,
}

#[test]
fn documents_keep_order_count_and_delete() {
    block_on(async {
        let store = store();
        let documents = SqlDocumentStore::new(store);

        let first = documents
            .append("session", Note { text: "one".into() })
            .await
            .unwrap();
        let second = documents
            .append("session", Note { text: "two".into() })
            .await
            .unwrap();
        assert!(first.id < second.id, "the ids are scru128 and ordered");

        assert_eq!(documents.count("session").await.unwrap(), 2);

        let all: Vec<Note> = collect(documents.scan_all("session").await.unwrap()).await;
        assert_eq!(
            all,
            vec![Note { text: "one".into() }, Note { text: "two".into() },]
        );

        let recent: Vec<Note> = collect(documents.scan("session", 1).await.unwrap()).await;
        assert_eq!(recent, vec![Note { text: "two".into() }]);

        documents.delete("session", &first.id).await.unwrap();
        assert_eq!(documents.count("session").await.unwrap(), 1);
        assert_eq!(documents.delete_all("session").await.unwrap(), 1);
        assert_eq!(documents.count("session").await.unwrap(), 0);
    });
}

#[test]
fn key_value_store_round_trips() {
    block_on(async {
        let store = store();
        store.set("alpha", Note { text: "a".into() }).await.unwrap();
        store.set("beta", Note { text: "b".into() }).await.unwrap();

        let alpha: Option<Note> = store.get("alpha").await.unwrap();
        assert_eq!(alpha.unwrap().text, "a");
        assert!(store.exists("alpha").await.unwrap());

        let taken: Option<Note> = store.take("alpha").await.unwrap();
        assert_eq!(taken.unwrap().text, "a");
        assert!(!store.exists("alpha").await.unwrap());

        let keys = store
            .list_keys(None)
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(keys, vec!["beta".to_string()]);

        store.delete("beta").await.unwrap();
        let missing: Option<Note> = store.get("beta").await.unwrap();
        assert!(missing.is_none());
    });
}

#[test]
fn blob_store_round_trips_binary() {
    block_on(async {
        use foundation_db::BlobStore;
        let store = store();
        let bytes = vec![0, 1, 2, 250, 255, 0];
        store.put_blob("archive/one", &bytes).await.unwrap();
        assert!(store.blob_exists("archive/one").await.unwrap());
        assert_eq!(store.get_blob("archive/one").await.unwrap(), Some(bytes));
        store.delete_blob("archive/one").await.unwrap();
        assert_eq!(store.get_blob("archive/one").await.unwrap(), None);
    });
}

#[test]
fn sql_parameters_keep_text_and_integer_types() {
    block_on(async {
        use foundation_db::{DataValue, QueryStore};
        let store = store();
        store
            .execute(
                "CREATE TABLE items (name TEXT, count INTEGER, body BLOB)",
                &[],
            )
            .await
            .unwrap();
        let written = store
            .execute(
                "INSERT INTO items (name, count, body) VALUES (?, ?, ?)",
                &[
                    DataValue::Text("widget".into()),
                    DataValue::Integer(42),
                    DataValue::Blob(vec![9, 8]),
                ],
            )
            .await
            .unwrap();
        assert_eq!(written, 1);

        let rows = store
            .query(
                "SELECT name, count, body FROM items WHERE count = ?",
                &[DataValue::Integer(42)],
            )
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get_by_name::<String>("name").unwrap(), "widget");
        assert_eq!(rows[0].get_by_name::<i64>("count").unwrap(), 42);
        assert_eq!(rows[0].get_by_name::<Vec<u8>>("body").unwrap(), vec![9, 8]);
    });
}

#[test]
fn rate_limiter_uses_the_window() {
    block_on(async {
        let store = store();
        let ctx = ctx();
        assert!(store.check_rate_limit(&ctx, "client", 2, 60).await.unwrap());
        assert_eq!(
            store.record_rate_limit(&ctx, "client", 60).await.unwrap(),
            1
        );
        assert_eq!(
            store.record_rate_limit(&ctx, "client", 60).await.unwrap(),
            2
        );
        assert!(!store.check_rate_limit(&ctx, "client", 2, 60).await.unwrap());
        store.reset_rate_limit("client").await.unwrap();
        assert!(store.check_rate_limit(&ctx, "client", 2, 60).await.unwrap());
    });
}

/// Collect an `AsyncStorageItemStream` into a `Vec`.
async fn collect<T: 'static>(mut stream: foundation_db::AsyncStorageItemStream<'_, T>) -> Vec<T> {
    use futures_lite::StreamExt;
    let mut items = Vec::new();
    while let Some(item) = stream.next().await {
        items.push(item.unwrap());
    }
    items
}
