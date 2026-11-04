//! `SqliteVecVectorStore` tests (F29): native sqlite-vec k-nearest neighbour
//! (KNN) search with the bundled `.so`.
//!
//! The shared contract covers the `VectorStore` behaviour. The tests here
//! cover the sqlite-vec limits: no dot product, safe namespace names, and the
//! L2 score.

// A contract-test body over `TursoStorage` forces a `Send` check through the
// `turso` crate's deeply nested async chain, which overflows the default
// recursion limit (128) and is a future-incompatible warning today. Raise it
// for this test crate (the depth is in the external `turso` dependency); same
// as `foundation_db/tests/turso_storage_tests.rs`.
#![recursion_limit = "256"]

#[macro_use]
mod contract;

use foundation_core::valtron::valtron_test;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::SqliteVecVectorStore;
use tempfile::TempDir;

async fn make_store() -> (TempDir, SqliteVecVectorStore) {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    let store = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .unwrap();
    (dir, store)
}

vector_store_contract!(make_store);

fn entry(id: &str, v: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

#[valtron_test]
async fn dot_metric_is_unsupported() {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::Dot);
    let err = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .err()
        .expect("dot is refused");
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn unsafe_namespace_is_refused() {
    let (_dir, store) = make_store().await;
    let err = store
        .insert("x; DROP TABLE t", entry("a", [1.0, 0.0, 0.0]))
        .await
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn l2_scores_are_negative_squared_distance() {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::L2);
    let store = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .unwrap();
    store
        .insert("ns", entry("near", [1.0, 0.0, 0.0]))
        .await
        .unwrap();
    store
        .insert("ns", entry("far", [3.0, 0.0, 0.0]))
        .await
        .unwrap();
    let m = store.search("ns", &[0.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(m[0].id, "near");
    assert!((m[0].score - -1.0).abs() < 1e-4, "score {}", m[0].score);
    assert!((m[1].score - -9.0).abs() < 1e-4, "score {}", m[1].score);
}

/// Seed a database with one old-shape `vec0` table (spec 36 F29) before the
/// store opens it. The old shape has no `metadata` column and no text primary
/// key.
async fn seed_old_table(path: &std::path::Path) {
    let db = libsql::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    conn.load_extension_enable().unwrap();
    conn.load_extension(env!("VEC0_SO"), None).unwrap();
    conn.load_extension_disable().unwrap();
    conn.execute(
        "CREATE VIRTUAL TABLE vec0_old USING vec0(embedding float[3], external_id text)",
        (),
    )
    .await
    .unwrap();
    let blob = |v: [f32; 3]| -> Vec<u8> { v.iter().flat_map(|f| f.to_le_bytes()).collect() };
    for (id, v) in [
        ("a", [1.0, 0.0, 0.0]),
        ("b", [0.0, 2.0, 0.0]),
        ("c", [0.0, 0.0, 3.0]),
    ] {
        conn.execute(
            "INSERT INTO vec0_old(embedding, external_id) VALUES (?, ?)",
            libsql::params![blob(v), id],
        )
        .await
        .unwrap();
    }
}

#[valtron_test]
async fn old_vec0_table_is_migrated() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.db");
    seed_old_table(&path).await;

    let cfg = VectorStoreConfig::new(3, DistanceMetric::L2);
    let store = SqliteVecVectorStore::open(cfg, &path).await.unwrap();

    // The migrated rows are searchable, best first by L2 distance.
    let m = store.search("old", &[1.0, 0.0, 0.0], 10).await.unwrap();
    let ids: Vec<&str> = m.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "c"]);

    // The migrated table has the current shape, so `get` reads the row and a
    // default metadata (the old shape had none).
    let got = store.get("old", "a").await.unwrap().expect("row migrated");
    assert_eq!(got.id, "a");
    assert!(got.metadata.tags.is_empty());
}

#[valtron_test]
async fn old_vec0_table_with_wrong_dimension_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v.db");
    {
        let db = libsql::Builder::new_local(path.to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        conn.load_extension_enable().unwrap();
        conn.load_extension(env!("VEC0_SO"), None).unwrap();
        conn.load_extension_disable().unwrap();
        conn.execute(
            "CREATE VIRTUAL TABLE vec0_bad USING vec0(embedding float[7], external_id text)",
            (),
        )
        .await
        .unwrap();
    }

    let cfg = VectorStoreConfig::new(3, DistanceMetric::L2);
    let err = SqliteVecVectorStore::open(cfg, &path)
        .await
        .err()
        .expect("wrong dimension is refused");
    assert!(matches!(err, VectorStoreError::SchemaMismatch(_)), "{err}");
}
