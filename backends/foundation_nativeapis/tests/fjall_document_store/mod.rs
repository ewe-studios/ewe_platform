#![cfg(feature = "vfs-fjall")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_db::traits::{DocumentStore, PromotableDocument};
use foundation_nativeapis::shared::vfs::async_traits::AsyncVfsFileSystem;
use foundation_nativeapis::shared::vfs::{OpenMode, VfsCapabilities, VfsMetadata};
use foundation_nativeapis::{
    DurabilityWriteConfig, FjallDocumentStore, MemoryFs, VfsFileSystem, VfsResult,
};

use foundation_core::valtron::valtron_test;
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};

fn make_store() -> FjallDocumentStore<MemoryFs> {
    let fs = MemoryFs::new();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig::default());
    futures_lite::future::block_on(FjallDocumentStore::open(
        fs,
        "/data".to_string(),
        tmpdir.path(),
        durability,
    ))
    .unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestRecord {
    name: String,
    value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TypedRecord {
    name: String,
    kind: String,
}

impl PromotableDocument for TypedRecord {
    fn record_type(&self) -> Option<String> {
        Some(self.kind.clone())
    }
    fn title(&self) -> Option<String> {
        Some(self.name.clone())
    }
}

/// Drain a scan, failing the test on any item error.
async fn collect_stream<T>(mut stream: foundation_db::AsyncStorageItemStream<'_, T>) -> Vec<T> {
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        out.push(item.expect("scan item"));
    }
    out
}

// --- Basic append and scan ---

#[valtron_test]
async fn append_and_scan_returns_documents() {
    let store = make_store();
    let doc1 = store
        .append("col", TestRecord { name: "first".into(), value: 1 }).await
        .unwrap();
    let doc2 = store
        .append("col", TestRecord { name: "second".into(), value: 2 }).await
        .unwrap();

    assert!(!doc1.id.is_empty());
    assert!(!doc2.id.is_empty());
    assert!(doc1.id < doc2.id, "scru128 ids should be chronologically ordered");

    let results: Vec<TestRecord> = collect_stream(store.scan("col", 10).await.unwrap()).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "second"); // newest first
    assert_eq!(results[1].name, "first");
}

#[valtron_test]
async fn scan_all_returns_oldest_first() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).await.unwrap();

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").await.unwrap()).await;
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].name, "a");
    assert_eq!(results[1].name, "b");
    assert_eq!(results[2].name, "c");
}

#[valtron_test]
async fn scan_from_returns_from_given_id() {
    let store = make_store();
    let _d1 = store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    let _d3 = store.append("col", TestRecord { name: "c".into(), value: 3 }).await.unwrap();

    let results: Vec<TestRecord> =
        collect_stream(store.scan_from("col", &d2.id, 0).await.unwrap()).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "b");
    assert_eq!(results[1].name, "c");
}

#[valtron_test]
async fn scan_from_with_limit() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).await.unwrap();
    store.append("col", TestRecord { name: "d".into(), value: 4 }).await.unwrap();

    let results: Vec<TestRecord> =
        collect_stream(store.scan_from("col", &d2.id, 2).await.unwrap()).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "b");
    assert_eq!(results[1].name, "c");
}

// --- append_with_id ---

#[valtron_test]
async fn append_with_id_uses_provided_id() {
    let store = make_store();
    let custom_id = foundation_compact::ids::new_scru128_string();
    let doc = store
        .append_with_id("col", &custom_id, TestRecord { name: "custom".into(), value: 42 }).await
        .unwrap();
    assert_eq!(doc.id, custom_id);

    let results: Vec<TestRecord> = collect_stream(store.scan("col", 10).await.unwrap()).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "custom");
}

// --- Promotable documents ---

#[valtron_test]
async fn append_promotable_populates_promoted_columns() {
    let store = make_store();
    let doc = store
        .append_promotable(
            "col",
            TypedRecord { name: "obs-1".into(), kind: "observation".into() },
        ).await
        .unwrap();

    assert_eq!(doc.title.as_deref(), Some("obs-1"));
    assert_eq!(doc.record_type.as_deref(), Some("observation"));
}

#[valtron_test]
async fn append_promotable_with_id_works() {
    let store = make_store();
    let custom_id = foundation_compact::ids::new_scru128_string();
    let doc = store
        .append_promotable_with_id(
            "col",
            &custom_id,
            TypedRecord { name: "refl-1".into(), kind: "reflection".into() },
        ).await
        .unwrap();

    assert_eq!(doc.id, custom_id);
    assert_eq!(doc.title.as_deref(), Some("refl-1"));
    assert_eq!(doc.record_type.as_deref(), Some("reflection"));
}

// --- scan_documents ---

#[valtron_test]
async fn scan_documents_returns_full_document_views() {
    let store = make_store();
    store
        .append_promotable(
            "col",
            TypedRecord { name: "item-1".into(), kind: "conversation".into() },
        ).await
        .unwrap();
    store
        .append_promotable(
            "col",
            TypedRecord { name: "item-2".into(), kind: "observation".into() },
        ).await
        .unwrap();

    let docs = store.scan_documents("col", 10).await.unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].title.as_deref(), Some("item-2")); // newest first
    assert_eq!(docs[1].title.as_deref(), Some("item-1"));
}

#[valtron_test]
async fn scan_documents_from_returns_from_id() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();

    let docs = store.scan_documents_from("col", &d2.id, 0).await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, d2.id);
}

// --- Delete ---

#[valtron_test]
async fn delete_removes_document_from_index() {
    let store = make_store();
    let d1 = store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();

    store.delete("col", &d1.id).await.unwrap();

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").await.unwrap()).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "b");
}

#[valtron_test]
async fn delete_all_removes_everything() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).await.unwrap();

    let count = store.delete_all("col").await.unwrap();
    assert_eq!(count, 3);

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").await.unwrap()).await;
    assert!(results.is_empty());
}

// --- Count ---

#[valtron_test]
async fn count_returns_number_of_documents() {
    let store = make_store();
    assert_eq!(store.count("col").await.unwrap(), 0);

    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    assert_eq!(store.count("col").await.unwrap(), 1);

    store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    assert_eq!(store.count("col").await.unwrap(), 2);
}

// --- Multiple collections ---

#[valtron_test]
async fn separate_collections_are_independent() {
    let store = make_store();
    store.append("alpha", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    store.append("beta", TestRecord { name: "b".into(), value: 2 }).await.unwrap();

    let alpha: Vec<TestRecord> = collect_stream(store.scan_all("alpha").await.unwrap()).await;
    let beta: Vec<TestRecord> = collect_stream(store.scan_all("beta").await.unwrap()).await;

    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].name, "a");
    assert_eq!(beta.len(), 1);
    assert_eq!(beta[0].name, "b");
}

// --- Empty collection ---

#[valtron_test]
async fn scan_empty_collection_returns_empty() {
    let store = make_store();
    let results: Vec<TestRecord> = collect_stream(store.scan("empty", 10).await.unwrap()).await;
    assert!(results.is_empty());
}

#[valtron_test]
async fn scan_all_empty_collection_returns_empty() {
    let store = make_store();
    let results: Vec<TestRecord> = collect_stream(store.scan_all("empty").await.unwrap()).await;
    assert!(results.is_empty());
}

// --- Batched durability ---

#[valtron_test]
async fn batched_durability_accumulates_then_flushes() {
    let fs = MemoryFs::new();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig {
        batch_size_bytes: 1024,
        flush_timeout: std::time::Duration::from_secs(60),
    });
    let store = FjallDocumentStore::open(fs, "/data".to_string(), tmpdir.path(), durability)
        .await
        .unwrap();

    store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();

    // Data should still be scannable from the batch
    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").await.unwrap()).await;
    assert_eq!(results.len(), 2);

    // Explicit flush
    store.flush("col").await.unwrap();

    // Still readable after flush
    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").await.unwrap()).await;
    assert_eq!(results.len(), 2);
}

// --- Crash recovery (tail rebuild) ---

#[valtron_test]
async fn tail_rebuild_recovers_unindexed_records() {
    let fs = MemoryFs::new();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig::default());

    // First: create store and add records
    {
        let store = FjallDocumentStore::open(
            fs.clone(),
            "/data".to_string(),
            tmpdir.path(),
            durability.clone(),
        )
        .await
        .unwrap();

        store.append("col", TestRecord { name: "a".into(), value: 1 }).await.unwrap();
        store.append("col", TestRecord { name: "b".into(), value: 2 }).await.unwrap();
    }

    // Simulate crash: write more data to the NDJSON file directly (bypassing index)
    let hex_key = hex::encode("col");
    let extra_path = format!("/data/{hex_key}.jsonl");
    let existing = fs.read_file(&extra_path).unwrap();
    let extra_line = serde_json::json!({
        "id": foundation_compact::ids::new_scru128_string(),
        "content": "\"orphan\"",
        "metadata": {}
    });
    let mut new_data = existing;
    new_data.extend_from_slice(serde_json::to_string(&extra_line).unwrap().as_bytes());
    new_data.push(b'\n');
    fs.write_file(&extra_path, &new_data).unwrap();

    // Re-open: tail rebuild should recover the orphan
    let store2 = FjallDocumentStore::open(
        fs,
        "/data".to_string(),
        tmpdir.path(),
        durability,
    )
    .await
    .unwrap();

    assert_eq!(store2.count("col").await.unwrap(), 3);
}

// --- Ordering matches other backends ---

#[valtron_test]
async fn ordering_is_chronological_by_scru128() {
    let store = make_store();
    let mut ids = Vec::new();
    for i in 0..5 {
        let doc = store
            .append("col", TestRecord { name: format!("item-{i}"), value: i }).await
            .unwrap();
        ids.push(doc.id);
    }

    // scan_all: oldest first
    let all_docs = store.scan_documents_from("col", &ids[0], 0).await.unwrap();
    for (i, doc) in all_docs.iter().enumerate() {
        assert_eq!(doc.id, ids[i]);
    }

    // scan: newest first
    let newest = store.scan_documents("col", 5).await.unwrap();
    for (i, doc) in newest.iter().enumerate() {
        assert_eq!(doc.id, ids[4 - i]);
    }
}

// --- Default ---

#[valtron_test]
async fn default_store_is_usable_and_starts_empty() {
    let store = FjallDocumentStore::<MemoryFs>::default();
    store
        .append("col", TestRecord { name: "first".into(), value: 1 }).await
        .unwrap();
    let docs = collect_stream(store.scan_all::<TestRecord>("col").await.unwrap()).await;
    assert_eq!(docs, vec![TestRecord { name: "first".into(), value: 1 }]);

    // A second default store gets its own index, so it doesn't see the first's data.
    let other = FjallDocumentStore::<MemoryFs>::default();
    let other_docs = collect_stream(other.scan_all::<TestRecord>("col").await.unwrap()).await;
    assert_eq!(other_docs, Vec::<TestRecord>::new());
}

// --- Lazy scan (E5) ---

/// A VFS wrapper that counts read opens. Scan laziness is proven by how many
/// document files the store opens while a stream is drained.
struct CountingFs<F> {
    inner: F,
    read_opens: Arc<AtomicUsize>,
}

impl<F: AsyncVfsFileSystem> CountingFs<F> {
    fn new(inner: F) -> (Self, Arc<AtomicUsize>) {
        let read_opens = Arc::new(AtomicUsize::new(0));
        (Self { inner, read_opens: read_opens.clone() }, read_opens)
    }
}

#[async_trait::async_trait]
impl<F: AsyncVfsFileSystem> AsyncVfsFileSystem for CountingFs<F> {
    type File = F::File;
    type SeekableFile = F::SeekableFile;
    type Directory = F::Directory;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.inner.stat_async(path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.inner.exists_async(path).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        self.inner.chmod_async(path, mode).await
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        self.inner.symlink_async(target, link).await
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        self.inner.readlink_async(path).await
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        self.inner.rename_async(from, to).await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        self.inner.remove_async(path).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        if matches!(mode, OpenMode::Read) {
            self.read_opens.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.open_async(path, mode).await
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        self.inner.open_seekable_async(path, mode).await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        self.inner.open_directory_async(path).await
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        self.inner.create_async(path, mode).await
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        self.inner.mkdir_async(path).await
    }
}

#[valtron_test]
async fn fjall_scan_yields_before_reading_all() {
    let (fs, opens) = CountingFs::new(MemoryFs::new());
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig::default());
    let store = FjallDocumentStore::open(fs, "/data".to_string(), tmpdir.path(), durability)
        .await
        .unwrap();

    for i in 0..50 {
        store
            .append("col", TestRecord { name: format!("item-{i}"), value: i }).await
            .unwrap();
    }
    store.flush("col").await.unwrap();

    // Creating the stream must not read any document.
    opens.store(0, Ordering::Relaxed);
    let mut stream = store.scan::<TestRecord>("col", 50).await.unwrap();
    assert_eq!(opens.load(Ordering::Relaxed), 0, "scan read eagerly while being created");

    // Draining one item must not read the whole collection.
    let first = stream.next().await.expect("first item").expect("scan item");
    assert_eq!(first.name, "item-49");
    let after_one = opens.load(Ordering::Relaxed);
    assert!(
        (1..50).contains(&after_one),
        "read {after_one} document files for a single scan item"
    );

    let rest = collect_stream(stream).await;
    assert_eq!(rest.len(), 49);
}
