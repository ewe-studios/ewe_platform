//! `InMemoryVectorStore` tests: the shared contract, plus the in-memory
//! store's own options (normalised vectors, L2 and dot metrics).

#[macro_use]
mod contract;

use foundation_core::valtron::valtron_test;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::sqrt::SqrtStrategy;
use foundation_vectors::store::{
    InMemoryVectorStore, VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig,
};
use foundation_vectors::vector::Vector;

const NS: &str = "test";

async fn make_store() -> ((), InMemoryVectorStore) {
    let store = InMemoryVectorStore::new(VectorStoreConfig::new(3, DistanceMetric::Cosine));
    ((), store)
}

vector_store_contract!(make_store);

fn entry(id: &str, data: Vec<f32>) -> VectorEntry {
    VectorEntry {
        id: id.to_string(),
        vector: Vector::new(data),
        metadata: VectorMetadata::default(),
    }
}

#[valtron_test]
async fn normalized_vectors_strategy() {
    let config = VectorStoreConfig {
        dimension: 2,
        metric: DistanceMetric::Cosine,
        sqrt_strategy: SqrtStrategy::NormalizedVectors,
    };
    let store = InMemoryVectorStore::new(config);
    store.insert(NS, entry("a", vec![3.0, 4.0])).await.unwrap();
    let got = store.get(NS, "a").await.unwrap().unwrap();
    let mag = got.vector.magnitude();
    assert!(
        (mag - 1.0).abs() < 1e-5,
        "stored vector should be normalized, mag={mag}"
    );
}

#[valtron_test]
async fn l2_search() {
    let store = InMemoryVectorStore::new(VectorStoreConfig::new(2, DistanceMetric::L2));
    store.insert(NS, entry("near", vec![0.1, 0.0])).await.unwrap();
    store.insert(NS, entry("far", vec![10.0, 10.0])).await.unwrap();
    let results = store.search(NS, &[0.0, 0.0], 1).await.unwrap();
    assert_eq!(results[0].id, "near");
}

#[valtron_test]
async fn dot_search() {
    let store = InMemoryVectorStore::new(VectorStoreConfig::new(2, DistanceMetric::Dot));
    store.insert(NS, entry("high", vec![10.0, 10.0])).await.unwrap();
    store.insert(NS, entry("low", vec![0.1, 0.1])).await.unwrap();
    let results = store.search(NS, &[1.0, 1.0], 1).await.unwrap();
    assert_eq!(results[0].id, "high");
}
