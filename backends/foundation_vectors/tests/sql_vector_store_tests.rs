#[macro_use]
mod contract;

use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_db::TursoStorage;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::{SqlVectorStore, SqlVectorStoreConfig};
use tempfile::TempDir;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

async fn make_store() -> (TempDir, SqlVectorStore<TursoStorage>) {
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("vectors.db").to_string_lossy().into_owned();
    let storage = Arc::new(TursoStorage::new(&url).await.unwrap());
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    let store = SqlVectorStore::new(storage, config).await.unwrap();
    (dir, store)
}

vector_store_contract!(make_store);

fn entry(id: &str, data: Vec<f32>) -> VectorEntry {
    VectorEntry {
        id: id.to_string(),
        vector: Vector::new(data),
        metadata: VectorMetadata::default(),
    }
}

const NS: &str = "test";

#[valtron_test]
async fn insert_and_get() {
    let (_dir, store) = make_store().await;
    store
        .insert(NS, entry("a", vec![1.0, 0.0, 0.0]))
        .await
        .unwrap();
    let got = store.get(NS, "a").await.unwrap().unwrap();
    assert_eq!(got.id, "a");
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
}

#[valtron_test]
async fn dimension_mismatch_rejected() {
    let (_dir, store) = make_store().await;
    let err = store
        .insert(NS, entry("bad", vec![1.0, 0.0]))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        VectorStoreError::DimensionMismatch {
            expected: 3,
            got: 2
        }
    ));
}

#[valtron_test]
async fn zero_vector_rejected() {
    let (_dir, store) = make_store().await;
    let err = store
        .insert(NS, entry("zero", vec![0.0, 0.0, 0.0]))
        .await
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::ZeroVector));
}

#[valtron_test]
async fn search_top_k() {
    let (_dir, store) = make_store().await;
    store
        .insert(NS, entry("x", vec![1.0, 0.0, 0.0]))
        .await
        .unwrap();
    store
        .insert(NS, entry("y", vec![0.0, 1.0, 0.0]))
        .await
        .unwrap();
    store
        .insert(NS, entry("z", vec![0.9, 0.1, 0.0]))
        .await
        .unwrap();

    let results = store.search(NS, &[1.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "x");
    assert_eq!(results[1].id, "z");
}

#[valtron_test]
async fn namespace_isolation() {
    let (_dir, store) = make_store().await;
    store
        .insert("ns_a", entry("v1", vec![1.0, 0.0, 0.0]))
        .await
        .unwrap();
    store
        .insert("ns_b", entry("v2", vec![0.0, 1.0, 0.0]))
        .await
        .unwrap();

    assert_eq!(store.len("ns_a").await.unwrap(), 1);
    assert_eq!(store.len("ns_b").await.unwrap(), 1);

    let results = store.search("ns_a", &[1.0, 0.0, 0.0], 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "v1");

    assert!(store.get("ns_a", "v2").await.unwrap().is_none());
    assert!(store.get("ns_b", "v1").await.unwrap().is_none());
}

#[valtron_test]
async fn delete_entry() {
    let (_dir, store) = make_store().await;
    store
        .insert(NS, entry("a", vec![1.0, 0.0, 0.0]))
        .await
        .unwrap();
    assert_eq!(store.len(NS).await.unwrap(), 1);
    store.delete(NS, "a").await.unwrap();
    assert_eq!(store.len(NS).await.unwrap(), 0);
    assert!(store.get(NS, "a").await.unwrap().is_none());
}

#[valtron_test]
async fn delete_not_found() {
    let (_dir, store) = make_store().await;
    let err = store.delete(NS, "nope").await.unwrap_err();
    assert!(matches!(err, VectorStoreError::NotFound { .. }));
}

#[valtron_test]
async fn overwrite_on_insert() {
    let (_dir, store) = make_store().await;
    store
        .insert(NS, entry("a", vec![1.0, 0.0, 0.0]))
        .await
        .unwrap();
    store
        .insert(NS, entry("a", vec![0.0, 1.0, 0.0]))
        .await
        .unwrap();
    let got = store.get(NS, "a").await.unwrap().unwrap();
    assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
    assert_eq!(store.len(NS).await.unwrap(), 1);
}

#[valtron_test]
async fn empty_store() {
    let (_dir, store) = make_store().await;
    assert_eq!(store.len(NS).await.unwrap(), 0);
    let results = store.search(NS, &[1.0, 0.0, 0.0], 5).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[valtron_test]
async fn metadata_round_trip() {
    let (_dir, store) = make_store().await;
    let mut meta = VectorMetadata::default();
    meta.tags.insert("source".into(), "test".into());
    meta.tags.insert("type".into(), "embedding".into());

    let e = VectorEntry {
        id: "m1".into(),
        vector: Vector::new(vec![1.0, 0.0, 0.0]),
        metadata: meta.clone(),
    };
    store.insert(NS, e).await.unwrap();

    let got = store.get(NS, "m1").await.unwrap().unwrap();
    assert_eq!(got.metadata.tags.get("source").unwrap(), "test");
    assert_eq!(got.metadata.tags.get("type").unwrap(), "embedding");
}

#[valtron_test]
async fn persistence_across_reopen() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("persist.db");
    let url = db_path.to_str().unwrap();
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);

    {
        let storage = Arc::new(TursoStorage::new(url).await.unwrap());
        let store = SqlVectorStore::new(storage, config.clone()).await.unwrap();
        store
            .insert(NS, entry("p1", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(NS, entry("p2", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
    }

    {
        let storage = Arc::new(TursoStorage::new(url).await.unwrap());
        let store = SqlVectorStore::new(storage, config).await.unwrap();
        assert_eq!(store.len(NS).await.unwrap(), 2);
        let got = store.get(NS, "p1").await.unwrap().unwrap();
        assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);

        let results = store.search(NS, &[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(results[0].id, "p1");
    }
}

/// Records the target of every WARN event it sees. Used to prove a scan over
/// the warning threshold emits exactly one warning per namespace.
struct WarnCapture {
    targets: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for WarnCapture
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().level() == &Level::WARN {
            self.targets
                .lock()
                .unwrap()
                .push(event.metadata().target().to_string());
        }
    }
}

/// A scan over the threshold writes one warning per namespace, then stays
/// quiet on later scans of that namespace. The store is driven with
/// `futures_lite::future::block_on` so the capture subscriber sees the event
/// on this thread.
#[test]
fn sql_store_warns_above_scan_threshold() {
    let targets: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(WarnCapture {
            targets: targets.clone(),
        }),
        || {
            futures_lite::future::block_on(async {
                let dir = TempDir::new().unwrap();
                let url = dir.path().join("vectors.db").to_string_lossy().into_owned();
                let storage = Arc::new(TursoStorage::new(&url).await.unwrap());
                let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);
                let scan = SqlVectorStoreConfig::default().with_scan_warning_threshold(10);
                let store = SqlVectorStore::with_scan_config(storage, config, scan)
                    .await
                    .unwrap();
                for i in 0..11 {
                    store
                        .insert(NS, entry(&format!("v{i}"), vec![1.0, 0.0, 0.0]))
                        .await
                        .unwrap();
                }
                store.search(NS, &[1.0, 0.0, 0.0], 3).await.unwrap();
                store.search(NS, &[1.0, 0.0, 0.0], 3).await.unwrap();
            });
        },
    );

    let seen = targets.lock().unwrap();
    let count = seen
        .iter()
        .filter(|t| t.starts_with("foundation_vectors"))
        .count();
    assert_eq!(count, 1, "one warning, once per namespace: {seen:?}");
}
