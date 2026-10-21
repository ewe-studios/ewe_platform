// Integration tests — organized by scenario.
mod e2e;
mod idp_server;
mod provider_admin;
mod provider_service;
mod upstream_client;

use foundation_core::valtron::BlockingFutureExt;
use foundation_db::{Blocking, StorageBackend, StorageProvider};

/// A migrated Turso-backed `StorageProvider` (its constructor is async), as the
/// sync `QueryStore` view the server services and handlers use.
pub(crate) fn turso_provider(url: String) -> Blocking<StorageProvider> {
    let provider = async move {
        StorageProvider::new(StorageBackend::Turso { url }).await
    }
    .block()
    .expect("init turso");
    Blocking::new(provider)
}
