//! Standalone browser storage consumer for the cross-tab tests.
use foundation_db::core::storage_provider::KeyValueStore;
use foundation_db::wasm::browser::{BrowserBackend, BrowserStore};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn record(
    indexed: bool,
    namespace: String,
    key: String,
    value: Option<String>,
) -> Result<(), JsValue> {
    let store = store(indexed, &namespace);
    match value {
        Some(title) => store.set(&key, title).await,
        None => store.delete(&key).await,
    }
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub async fn entries(indexed: bool, namespace: String) -> Result<String, JsValue> {
    async fn read(store: BrowserStore) -> foundation_db::core::errors::StorageResult<String> {
        let keys = store
            .list_keys(Some("pending/"))
            .await?
            .collect_all()
            .await?;
        let mut rows = std::collections::BTreeMap::new();
        for key in keys {
            if let Some(value) = store.get::<String>(&key).await? {
                rows.insert(key, value);
            }
        }
        Ok(serde_json::to_string(&rows)?)
    }
    read(store(indexed, &namespace))
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn store(indexed: bool, namespace: &str) -> BrowserStore {
    let backend = if indexed {
        BrowserBackend::IndexedDb
    } else {
        BrowserBackend::LocalStorage
    };
    BrowserStore::new(backend, namespace).with_page_size(1.try_into().unwrap())
}
