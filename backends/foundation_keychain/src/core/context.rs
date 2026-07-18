//! Request-scoped context passed to every portable handler (spec-57, F008 Stage 1).
//!
//! WHY: The `core/api` handlers must be cross-platform and testable — they can't
//! depend on `worker::RouteContext` (wasm) or a `foundation_http` request (native).
//! `KeychainContext` is the dependency-injection seam: it carries the
//! `foundation_db` storage handle (and, in later stages, the JWT signing key,
//! blob store, and notifier). Both transports (native `server/native.rs`, workers
//! `server/cloudflare.rs`) build one and hand it to the same handlers.
//!
//! WHAT: `KeychainContext { db }` for Stage 1 (vault CRUD). It is `Clone` (all
//! fields are `Arc`), so it is cheap to move into async tasks.
//!
//! HOW: `db` is an `Arc<dyn AsyncQueryStore>` — `foundation_db::StorageProvider`
//! resolves it to Turso (native) or D1 (wasm/Workers) by target, per decision 03.

use std::sync::Arc;

use foundation_db::core::storage_provider::AsyncQueryStore;

/// Dependencies the portable vault handlers operate over.
#[derive(Clone)]
pub struct KeychainContext {
    /// Relational store (Turso on native, D1 on wasm) — the vault's system of record.
    db: Arc<dyn AsyncQueryStore>,
}

impl KeychainContext {
    /// Build a context over the given async SQL store.
    #[must_use]
    pub fn new(db: Arc<dyn AsyncQueryStore>) -> Self {
        Self { db }
    }

    /// The relational store handlers query.
    #[must_use]
    pub fn db(&self) -> &dyn AsyncQueryStore {
        self.db.as_ref()
    }
}
