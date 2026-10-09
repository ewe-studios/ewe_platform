//! Libsql DocumentStore genericity check (F22b).
//!
//! Proves the generic stores **instantiate for libsql** at compile time — i.e.
//! `LibsqlStore` satisfies the `QueryStore` / `AsyncQueryStore` bounds that
//! `SqlDocumentStore<Q>` and `AsyncSqlDocumentStore<Q>` require. This is the real
//! F22b claim for libsql ("the generic works for every SQL backend").
//!
//! Behavioural conformance (scru128 ordering, inclusive `scan_from`, promoted
//! columns) is covered by the Turso suite (`async_sql_document_store_tests.rs`):
//! all SQL backends share the same `sql` builder module, so the statements are
//! identical. Driving libsql at runtime additionally needs a **tokio** runtime
//! (libsql is tokio-based, unlike pure-Rust Turso), so a behavioural libsql run
//! belongs in a tokio-harnessed follow-up, not here.
//!
//! Gated behind the `libsql` feature:
//! `cargo test -p foundation_db --no-default-features --features libsql`.
#![cfg(feature = "libsql")]

use foundation_db::{AsyncSqlDocumentStore, LibsqlStore, SqlDocumentStore};

/// Compile-time proof that both the sync and async generic SQL document stores
/// instantiate over `LibsqlStore`. Never executed (no I/O / runtime), so it can't
/// hang on libsql's tokio runtime — it only needs to type-check.
#[allow(dead_code)]
fn _generic_instantiates_for_libsql(
    sync_backend: LibsqlStore,
    async_backend: LibsqlStore,
) -> (
    SqlDocumentStore<LibsqlStore>,
    AsyncSqlDocumentStore<LibsqlStore>,
) {
    (
        SqlDocumentStore::new(sync_backend),
        AsyncSqlDocumentStore::new(async_backend),
    )
}

#[test]
fn libsql_generic_instantiation_compiles() {
    // The assertion is the successful compilation of the function above; this test
    // exists so the gated file has a runnable case under `--features libsql`.
    let _ = _generic_instantiates_for_libsql as fn(_, _) -> _;
}
