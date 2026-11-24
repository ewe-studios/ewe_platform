//! Browser storage selected when an application boots.
extern crate std;
use alloc::string::String;
use alloc::{boxed::Box, collections::BTreeMap, format};
use core::cell::RefCell;
use foundation_db::wasm::browser::{BrowserBackend, BrowserStore};
pub use foundation_db::KeyValueStore;
use foundation_db::{AsyncListStream, MemoryStorage, StorageResult};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

/// The selected provider. Each application instance owns its memory store.
pub struct HostStorage(Provider);

enum Provider {
    Memory(MemoryStorage),
    Browser(BrowserStore),
}

std::thread_local! { static BOOT: RefCell<Option<HostStorage>> = const { RefCell::new(None) }; }

/// Set the provider for the next application initializer.
///
/// # Errors
/// Return an error for an unknown provider or a missing namespace.
#[wasm_bindgen(js_name = configureHostStorage)]
pub fn configure_storage(provider: &str, namespace: String) -> Result<(), JsValue> {
    let storage = match provider {
        "memory" => Provider::Memory(MemoryStorage::new()),
        "indexedDB" | "localStorage" if namespace.is_empty() => {
            return Err(JsValue::from_str("Missing storage namespace"));
        }
        "indexedDB" => Provider::Browser(BrowserStore::new(BrowserBackend::IndexedDb, namespace)),
        "localStorage" => {
            Provider::Browser(BrowserStore::new(BrowserBackend::LocalStorage, namespace))
        }
        _ => return Err(JsValue::from_str("Unknown storage provider")),
    };
    BOOT.with_borrow_mut(|boot| *boot = Some(HostStorage(storage)));
    Ok(())
}

pub(crate) fn install(ctx: &foundation_core::valtron::Ctx, task: Option<Startup>) {
    STARTUP.set(task);
    ctx.store(
        BOOT.take()
            .unwrap_or_else(|| HostStorage(Provider::Memory(MemoryStorage::new()))),
    );
}

pub(crate) type Startup =
    core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = Result<(), String>>>>;
std::thread_local! { static STARTUP: RefCell<Option<Startup>> = const { RefCell::new(None) }; }

/// Complete application setup before the browser accepts actions.
///
/// # Errors
/// Return the application setup error.
#[wasm_bindgen(js_name = prepareBrowserHost)]
pub async fn prepare_browser_host() -> Result<(), JsValue> {
    if let Some(task) = STARTUP.take() {
        task.await.map_err(|message| JsValue::from_str(&message))?;
    }
    Ok(())
}

#[async_trait::async_trait]
impl KeyValueStore for HostStorage {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        match &self.0 {
            Provider::Memory(store) => store.get(key).await,
            Provider::Browser(store) => store.get(key).await,
        }
    }
    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        match &self.0 {
            Provider::Memory(store) => store.set(key, value).await,
            Provider::Browser(store) => store.set(key, value).await,
        }
    }
    async fn delete(&self, key: &str) -> StorageResult<()> {
        match &self.0 {
            Provider::Memory(store) => store.delete(key).await,
            Provider::Browser(store) => store.delete(key).await,
        }
    }
    async fn exists(&self, key: &str) -> StorageResult<bool> {
        match &self.0 {
            Provider::Memory(store) => store.exists(key).await,
            Provider::Browser(store) => store.exists(key).await,
        }
    }
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        match &self.0 {
            Provider::Memory(store) => store.list_keys(prefix).await,
            Provider::Browser(store) => store.list_keys(prefix).await,
        }
    }
}

impl HostStorage {
    /// Read a record from `localStorage` for migration into a durable session.
    ///
    /// # Errors
    /// Return an access or decoding error.
    pub async fn legacy_record<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        match &self.0 {
            Provider::Memory(_) => Ok(None),
            Provider::Browser(store) => store.legacy_record(key).await,
        }
    }

    /// Save new records and their migration receipts in one `IndexedDB` transaction.
    ///
    /// # Errors
    /// Return a configuration, encoding, or transaction error.
    pub async fn import_once<V: Serialize>(
        &self,
        destination: &str,
        receipts: &str,
        entries: &BTreeMap<String, V>,
    ) -> StorageResult<()> {
        match &self.0 {
            Provider::Memory(_) => Err(foundation_db::StorageError::Migration(
                "Import requires IndexedDB".into(),
            )),
            Provider::Browser(store) => store.import_once(destination, receipts, entries).await,
        }
    }

    /// Save independent record changes. `IndexedDB` commits all changes in one transaction.
    ///
    /// # Errors
    /// Return an encoding or storage error.
    pub async fn change_records<V: Serialize + Send + Clone + 'static>(
        &self,
        prefix: &str,
        inserts: &BTreeMap<&str, &V>,
        updates: &BTreeMap<&str, &V>,
        removals: &[&str],
    ) -> StorageResult<()> {
        match &self.0 {
            Provider::Browser(store) => {
                store
                    .change_records(prefix, inserts, updates, removals)
                    .await
            }
            Provider::Memory(store) => {
                for (&key, &value) in inserts {
                    store.set(&format!("{prefix}{key}"), value.clone()).await?;
                }
                for (&key, &value) in updates {
                    let key = format!("{prefix}{key}");
                    if store.exists(&key).await? {
                        store.set(&key, value.clone()).await?;
                    }
                }
                for key in removals {
                    store.delete(&format!("{prefix}{key}")).await?;
                }
                Ok(())
            }
        }
    }
}
