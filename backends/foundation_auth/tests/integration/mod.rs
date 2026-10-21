// Integration tests — organized by scenario.
mod e2e;
mod idp_server;
mod provider_admin;
mod provider_service;
mod upstream_client;

/// A migrated Turso-backed `StorageProvider` (its constructor is async), as the
/// sync `QueryStore` view the server services and handlers use.
pub(crate) fn turso_provider(url: String) -> foundation_db::Blocking<foundation_db::StorageProvider> {
    use foundation_core::valtron::BlockingFutureExt;
    let provider = async move {
        foundation_db::StorageProvider::new(foundation_db::StorageBackend::Turso { url }).await
    }
    .block()
    .expect("init turso");
    foundation_db::Blocking::new(provider)
}
