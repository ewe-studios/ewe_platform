// Integration tests — organized by scenario.
mod e2e;
mod idp_server;
mod provider_admin;
mod provider_service;
mod upstream_client;

use foundation_core::valtron::BlockingFutureExt;
use foundation_db::{StorageBackend, StorageProvider};

/// A migrated Turso-backed `StorageProvider`. Its constructor is async; test
/// setup is a sync boundary, so this waits for it with `.block()`.
pub(crate) fn turso_provider(url: String) -> StorageProvider {
    async move { StorageProvider::new(StorageBackend::Turso { url }).await }
        .block()
        .expect("init turso")
}
