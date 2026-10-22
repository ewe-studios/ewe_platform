//! The `VectorStore` contract: the checks that every vector store must pass.
//!
//! Each store's test file declares `#[macro_use] mod contract;` and calls
//! `vector_store_contract!(make_store)`. `make_store` is an `async fn` that
//! returns `(guard, store)`: the guard keeps resources alive (a temp dir, a
//! mock server), and the store is 3-dimensional with the cosine metric.
//!
//! The macro makes one `#[valtron_test]` per check, so a failure names the
//! store and the check.

#![allow(dead_code)]

use std::collections::HashMap;

use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreError,
};
use foundation_vectors::vector::Vector;

pub const NS: &str = "contract";

pub fn entry(id: &str, data: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.to_string(),
        vector: Vector::new(data.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

fn ids(store_matches: &[foundation_vectors::store::VectorMatch]) -> Vec<&str> {
    store_matches.iter().map(|m| m.id.as_str()).collect()
}

/// An entry comes back with its vector and its metadata.
pub async fn insert_then_get_round_trips(store: &dyn VectorStore) {
    let mut tags = HashMap::new();
    tags.insert("kind".to_string(), "tool".to_string());
    let e = VectorEntry {
        id: "a".into(),
        vector: Vector::new(vec![1.0, 2.0, 3.0]),
        metadata: VectorMetadata { tags },
    };
    store.insert_async(NS, e.clone()).await.unwrap();
    let got = store.get_async(NS, "a").await.unwrap().expect("entry present");
    assert_eq!(got.id, "a");
    assert_eq!(got.vector.data, e.vector.data);
    assert_eq!(got.metadata, e.metadata);
}

/// An insert with an existing id replaces the entry.
pub async fn insert_replaces_existing_id(store: &dyn VectorStore) {
    store.insert_async(NS, entry("a", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("a", [0.0, 1.0, 0.0])).await.unwrap();
    assert_eq!(store.len_async(NS).await.unwrap(), 1);
    let got = store.get_async(NS, "a").await.unwrap().unwrap();
    assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
    let m = store.search_async(NS, &[0.0, 1.0, 0.0], 5).await.unwrap();
    assert_eq!(ids(&m), vec!["a"]);
}

/// Wrong dimensions and zero vectors are refused, and nothing is stored.
pub async fn invalid_vectors_rejected(store: &dyn VectorStore) {
    let bad = VectorEntry {
        id: "d".into(),
        vector: Vector::new(vec![1.0, 2.0]),
        metadata: VectorMetadata::default(),
    };
    assert!(matches!(
        store.insert_async(NS, bad).await.unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 2 }
    ));
    assert!(matches!(
        store.insert_async(NS, entry("z", [0.0, 0.0, 0.0])).await.unwrap_err(),
        VectorStoreError::ZeroVector
    ));
    assert!(matches!(
        store.search_async(NS, &[1.0], 1).await.unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 1 }
    ));
    assert_eq!(store.len_async(NS).await.unwrap(), 0);
}

/// Search returns at most `k` matches, best first.
pub async fn search_ranks_best_first(store: &dyn VectorStore) {
    store.insert_async(NS, entry("x", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("y", [0.0, 1.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("z", [0.9, 0.1, 0.0])).await.unwrap();

    let m = store.search_async(NS, &[1.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(ids(&m), vec!["x", "z"]);
    assert!(m[0].score >= m[1].score);
    assert!((m[0].score - 1.0).abs() < 1e-4, "cosine score of an equal vector is 1");

    let all = store.search_async(NS, &[1.0, 0.0, 0.0], 10).await.unwrap();
    assert_eq!(ids(&all), vec!["x", "z", "y"]);
    assert_eq!(store.search_async(NS, &[1.0, 0.0, 0.0], 0).await.unwrap(), Vec::new());
}

/// A namespace with no entries reads as empty, not as an error.
pub async fn unknown_namespace_is_empty(store: &dyn VectorStore) {
    let ns = "never_written";
    assert_eq!(store.search_async(ns, &[1.0, 0.0, 0.0], 5).await.unwrap(), Vec::new());
    assert!(store.get_async(ns, "a").await.unwrap().is_none());
    assert_eq!(store.len_async(ns).await.unwrap(), 0);
    assert!(store.is_empty_async(ns).await.unwrap());
}

/// Delete removes the entry from get, search and len; deleting an unknown
/// id is `NotFound`.
pub async fn delete_removes_and_reports_missing(store: &dyn VectorStore) {
    store.insert_async(NS, entry("a", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("b", [0.0, 1.0, 0.0])).await.unwrap();
    store.delete_async(NS, "a").await.unwrap();

    assert!(store.get_async(NS, "a").await.unwrap().is_none());
    assert_eq!(store.len_async(NS).await.unwrap(), 1);
    let m = store.search_async(NS, &[1.0, 0.0, 0.0], 5).await.unwrap();
    assert_eq!(ids(&m), vec!["b"]);

    assert!(matches!(
        store.delete_async(NS, "a").await.unwrap_err(),
        VectorStoreError::NotFound { .. }
    ));
    assert!(matches!(
        store.delete_async("never_written", "a").await.unwrap_err(),
        VectorStoreError::NotFound { .. }
    ));
}

/// One namespace never sees another's entries.
pub async fn namespaces_are_isolated(store: &dyn VectorStore) {
    store.insert_async("ns_a", entry("a1", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("ns_b", entry("b1", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("ns_b", entry("b2", [0.0, 1.0, 0.0])).await.unwrap();

    assert_eq!(store.len_async("ns_a").await.unwrap(), 1);
    assert_eq!(store.len_async("ns_b").await.unwrap(), 2);
    let ma = store.search_async("ns_a", &[1.0, 0.0, 0.0], 10).await.unwrap();
    assert_eq!(ids(&ma), vec!["a1"]);
    assert!(store.get_async("ns_a", "b1").await.unwrap().is_none());
    assert!(store.get_async("ns_b", "a1").await.unwrap().is_none());
    assert!(matches!(
        store.delete_async("ns_a", "b1").await.unwrap_err(),
        VectorStoreError::NotFound { .. }
    ));
}

/// Make one `#[valtron_test]` per contract check for the store that
/// `$make` (an `async fn() -> (guard, store)`) builds.
macro_rules! vector_store_contract {
    ($make:ident) => {
        mod vector_store_contract {
            use foundation_core::valtron::valtron_test;

            #[valtron_test]
            async fn insert_then_get_round_trips() {
                let (_guard, store) = super::$make().await;
                crate::contract::insert_then_get_round_trips(&store).await;
            }

            #[valtron_test]
            async fn insert_replaces_existing_id() {
                let (_guard, store) = super::$make().await;
                crate::contract::insert_replaces_existing_id(&store).await;
            }

            #[valtron_test]
            async fn invalid_vectors_rejected() {
                let (_guard, store) = super::$make().await;
                crate::contract::invalid_vectors_rejected(&store).await;
            }

            #[valtron_test]
            async fn search_ranks_best_first() {
                let (_guard, store) = super::$make().await;
                crate::contract::search_ranks_best_first(&store).await;
            }

            #[valtron_test]
            async fn unknown_namespace_is_empty() {
                let (_guard, store) = super::$make().await;
                crate::contract::unknown_namespace_is_empty(&store).await;
            }

            #[valtron_test]
            async fn delete_removes_and_reports_missing() {
                let (_guard, store) = super::$make().await;
                crate::contract::delete_removes_and_reports_missing(&store).await;
            }

            #[valtron_test]
            async fn namespaces_are_isolated() {
                let (_guard, store) = super::$make().await;
                crate::contract::namespaces_are_isolated(&store).await;
            }
        }
    };
}
