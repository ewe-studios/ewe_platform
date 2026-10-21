//! Libsql DocumentStore genericity check (F22b).
//!
//! Proves the generic stores **instantiate for libsql** at compile time — i.e.
//! `LibsqlStore` satisfies the `AsyncQueryStore` bound that `SqlDocumentStore<Q>`
//! requires, and that the sync view `Blocking<SqlDocumentStore<LibsqlStore>>`
//! is a `DocumentStore`. This is the real
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

use foundation_db::{Blocking, DocumentStore, LibsqlStore, SqlDocumentStore};

/// Compile-time proof that the SQL document store instantiates over
/// `LibsqlStore`, and its `Blocking` view is a sync `DocumentStore`. Never
/// executed (no I/O / runtime), so it can't hang on libsql's tokio runtime —
/// it only needs to type-check.
#[allow(dead_code)]
fn _generic_instantiates_for_libsql(
    sync_backend: LibsqlStore,
    async_backend: LibsqlStore,
) -> (
    Blocking<SqlDocumentStore<LibsqlStore>>,
    SqlDocumentStore<LibsqlStore>,
) {
    fn assert_document_store<D: DocumentStore>(_: &D) {}
    let sync_view = Blocking::new(SqlDocumentStore::new(sync_backend));
    assert_document_store(&sync_view);
    (sync_view, SqlDocumentStore::new(async_backend))
}

#[test]
fn libsql_generic_instantiation_compiles() {
    // The assertion is the successful compilation of the function above; this test
    // exists so the gated file has a runnable case under `--features libsql`.
    let _ = _generic_instantiates_for_libsql as fn(_, _) -> _;
}

/// Compile-time proof that the libsql-backed stores implement `Default`, so they
/// satisfy `S: Default` bounds such as `AgentSession::build()`'s.
#[allow(dead_code)]
fn _libsql_stores_implement_default() {
    fn assert_default<T: Default>() {}
    assert_default::<LibsqlStore>();
    assert_default::<SqlDocumentStore<LibsqlStore>>();
    assert_default::<Blocking<SqlDocumentStore<LibsqlStore>>>();
}
