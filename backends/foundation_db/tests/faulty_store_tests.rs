//! The fault-injecting document store: a failure at one named step.
//!
//! Feature 04 task T18. The fake is behind the `testing` feature, so the
//! default test build leaves this file empty.
#![cfg(feature = "testing")]

use foundation_core::valtron::valtron_test;
use foundation_db::testing::{FaultStep, FaultyDocumentStore};
use foundation_db::{DocumentStore, MemoryDocumentStore};

#[valtron_test]
async fn faulty_document_store_fails_at_the_named_step() {
    let store = FaultyDocumentStore::new(MemoryDocumentStore::new(), FaultStep::Append);
    let error = store
        .append("history", "first message")
        .await
        .expect_err("the first append is injected to fail");
    assert!(
        error
            .to_string()
            .contains("injected storage failure at append call 1"),
        "{error}"
    );
    assert_eq!(store.calls(), 1);

    // A step that is not named still works, and nothing was written.
    assert_eq!(store.count("history").await.expect("count"), 0);
    assert_eq!(store.calls(), 1);
}

#[valtron_test]
async fn faulty_document_store_fails_only_the_chosen_call() {
    let store = FaultyDocumentStore::new(MemoryDocumentStore::new(), FaultStep::Append)
        .with_fail_on_call(Some(2));
    store
        .append("history", "first")
        .await
        .expect("first append");
    let error = store
        .append("history", "second")
        .await
        .expect_err("the second append is injected to fail");
    assert!(
        error
            .to_string()
            .contains("injected storage failure at append call 2"),
        "{error}"
    );
    store
        .append("history", "third")
        .await
        .expect("third append");
    assert_eq!(store.count("history").await.expect("count"), 2);
}
