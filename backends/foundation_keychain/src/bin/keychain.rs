//! `foundation_keychain` native server binary (spec-57, F008 Stage 4).
//!
//! Boots the valtron pool, opens the Turso/SQLite database, applies the schema,
//! and serves the Bitwarden API. Config via env: `LISTEN_ADDR`
//! (default `0.0.0.0:8080`), `KEYCHAIN_DB` (default `/data/keychain.db`).

// The valtron stream chain behind `block_on_future` nests past the default
// trait-solver recursion limit (128) when the compiler checks
// `DrivenStreamIterator<FutureTask<..>>: IntoIterator`
// (`recursion_depth_exceeding_limit`, a future-incompatible lint). Raise the
// limit, as `foundation_db/src/lib.rs` does for its `turso` async chain: the
// depth is in the generic stream types, not in this binary.
#![recursion_limit = "256"]

#[cfg(not(target_family = "wasm"))]
fn main() {
    use std::sync::Arc;

    use foundation_core::valtron::{
        block_on_future, initialize_pool, BlockingFutureExt, MonotonicClock,
    };
    use foundation_db::core::storage_provider::QueryStore;
    use foundation_db::{StorageBackend, StorageProvider};
    use foundation_http::native::server::{HttpServer, ServerConfig};
    use foundation_http::ShutdownSignal;
    use foundation_keychain::core::store::apply_schema;
    use foundation_keychain::server::native::KeychainServer;
    use foundation_keychain::KeychainContext;

    // Bring up the valtron multi-worker pool for the duration of the process.
    let _pool = initialize_pool(0x5eed, None);

    let db_url = std::env::var("KEYCHAIN_DB").unwrap_or_else(|_| "/data/keychain.db".to_string());
    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let provider = BlockingFutureExt::block({
        let url = db_url.clone();
        async move { StorageProvider::new(StorageBackend::Turso { url }).await }
    })
    .expect("open keychain database");
    let db: Arc<dyn QueryStore> = Arc::new(provider);

    let schema_db = Arc::clone(&db);
    block_on_future(async move { apply_schema(schema_db.as_ref()).await })
        .expect("apply keychain schema");

    let clock = Arc::new(MonotonicClock::new());
    let app = KeychainServer::new(KeychainContext::new(db, clock)).http_app();
    let listener = std::net::TcpListener::bind(&addr).expect("bind listen address");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server = HttpServer::with_config(app, &addr, ServerConfig::defaults());

    println!("foundation_keychain serving the Bitwarden API on {addr} (db: {db_url})");
    server.serve_with_listener(&listener, &shutdown);
}

#[cfg(target_family = "wasm")]
fn main() {}
