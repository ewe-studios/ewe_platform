use foundation_db::core::storage_provider::KeyValueStore;
use foundation_db::wasm::browser::{BrowserBackend, BrowserStore};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
async fn both_stores_keep_independent_keys_and_namespaces() {
    for backend in [BrowserBackend::IndexedDb, BrowserBackend::LocalStorage] {
        let store =
            BrowserStore::new(backend, "browser:test").with_page_size(1.try_into().unwrap());
        let other = BrowserStore::new(backend, "browser:test:other");
        other.set("todo:b", 9).await.unwrap();
        store.set("other", true).await.unwrap();
        store.set("todo:b", vec![1, 2]).await.unwrap();
        store.set("todo:a", vec![3, 4]).await.unwrap();
        assert_eq!(
            store
                .list_keys(Some("todo:"))
                .await
                .unwrap()
                .collect_all()
                .await
                .unwrap(),
            ["todo:a", "todo:b"]
        );
        assert_eq!(
            store.get::<Vec<i32>>("todo:b").await.unwrap(),
            Some(vec![1, 2])
        );
        assert!(store.exists("todo:b").await.unwrap());
        store.delete("todo:b").await.unwrap();
        assert!(!store.exists("todo:b").await.unwrap());
        assert_eq!(other.get::<i32>("todo:b").await.unwrap(), Some(9));
        store.delete("todo:a").await.unwrap();
        store.delete("other").await.unwrap();
        other.delete("todo:b").await.unwrap();
    }
}

#[wasm_bindgen_test]
async fn a_new_memory_store_starts_empty() {
    use foundation_db::MemoryStorage;

    let first = MemoryStorage::new();
    first.set("pending/add", 42).await.unwrap();
    assert_eq!(first.get::<u32>("pending/add").await.unwrap(), Some(42));
    assert_eq!(
        MemoryStorage::new()
            .get::<u32>("pending/add")
            .await
            .unwrap(),
        None
    );
}

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/tests/tabs.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = verifyTabs)]
    async fn verify_tabs() -> Result<(), JsValue>;
}

#[wasm_bindgen_test]
async fn concurrent_tabs_recover_pending_changes_and_report_failures() {
    // Link the standalone consumer into the browser test module.
    foundation_db_browser_tests::record(false, "tabs".into(), "probe".into(), None)
        .await
        .unwrap();
    verify_tabs().await.unwrap();
}

#[wasm_bindgen_test]
async fn import_preserves_destination_and_confirmed_records() {
    use std::collections::BTreeMap;
    let store = BrowserStore::new(BrowserBackend::IndexedDb, "import-receipts");
    let old = BTreeMap::from([("add:a".to_owned(), 1), ("add:b".to_owned(), u64::MAX)]);
    store.set("pending/add:a", 7).await.unwrap();
    store
        .import_once("pending/", "receipts/", &old)
        .await
        .unwrap();
    assert_eq!(store.get::<i32>("pending/add:a").await.unwrap(), Some(7));
    assert_eq!(
        store.get::<u64>("pending/add:b").await.unwrap(),
        Some(u64::MAX)
    );
    store.delete("pending/add:b").await.unwrap();
    store
        .import_once("pending/", "receipts/", &old)
        .await
        .unwrap();
    assert!(!store.exists("pending/add:b").await.unwrap());
}
