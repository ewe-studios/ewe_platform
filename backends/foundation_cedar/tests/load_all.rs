#![cfg(feature = "db")]

//! `SqlPolicyStore::load_all` reads its row once, not once per field.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_cedar::{PolicyStore, SqlPolicyStore};
use foundation_core::valtron::valtron_test;
use foundation_db::{AsyncQueryStream, DataValue, QueryStore, SqlRow, StorageResult};
use futures_lite::stream;

/// A `QueryStore` that counts `query` calls and answers with one fixed row.
struct CountingStore {
    queries: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl QueryStore for CountingStore {
    async fn query(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        self.queries.fetch_add(1, Ordering::SeqCst);
        let row = SqlRow::new(vec![
            ("policy_text".into(), DataValue::Text("permit(...)".into())),
            ("schema_text".into(), DataValue::Text("entity User;".into())),
            ("entities_json".into(), DataValue::Null),
            ("version".into(), DataValue::Text("3".into())),
        ]);
        Ok(AsyncQueryStream::new(stream::once(Ok(row))))
    }

    async fn execute(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Ok(0)
    }

    async fn execute_batch(&self, _sql: &str) -> StorageResult<()> {
        Ok(())
    }
}

#[valtron_test]
async fn sql_policy_store_loads_in_one_query() {
    let queries = Arc::new(AtomicUsize::new(0));
    let counting = CountingStore {
        queries: Arc::clone(&queries),
    };
    let store = SqlPolicyStore::new(&counting, "authz");

    let bundle = store.load_all().await.expect("load_all");

    assert_eq!(bundle.policies, "permit(...)");
    assert_eq!(bundle.schema, "entity User;");
    assert_eq!(bundle.entities, None);
    assert_eq!(bundle.version, "3");
    assert_eq!(
        queries.load(Ordering::SeqCst),
        1,
        "load_all must read the row in a single query"
    );
}
