//! The history archive (spec 83 feature 05, T26).
//!
//! The tests prove three rules:
//! 1. The archive holds every session document, one JSON object per line,
//!    compressed with gzip.
//! 2. The upload finishes before the history is deleted.
//! 3. A failed upload deletes nothing (invariant I5).

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_agent_rpc::core::archive::{ArchiveConfig, HistoryArchive};
use foundation_agent_rpc::core::registry::Scope;
use foundation_context::Ctx;
use foundation_db::traits::BlobStream;
use foundation_db::{
    BlobStore, DocumentStore, MemoryDocumentStore, MemoryStorage, StorageError, StorageResult,
};
use futures_lite::future::block_on;

const SESSION: &str = "session-0001";
const COLLECTION: &str = "session-0001";

fn ctx() -> Ctx {
    Ctx::host()
}

/// Three records with a known content and a stable order.
fn documents() -> Arc<MemoryDocumentStore> {
    let store = Arc::new(MemoryDocumentStore::new());
    block_on(async {
        for index in 0..3_u32 {
            store
                .append(COLLECTION, serde_json::json!({ "n": index }))
                .await
                .unwrap();
        }
    });
    store
}

/// The original JSON lines, in the order that the archive writes them.
fn lines(store: &MemoryDocumentStore) -> Vec<String> {
    block_on(async {
        store
            .scan_documents_from(COLLECTION, "", 0)
            .await
            .unwrap()
            .into_iter()
            .map(|document| document.content)
            .collect()
    })
}

/// Decompress an archive object into its JSON lines.
fn gunzip(bytes: &[u8]) -> Vec<String> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut text = String::new();
    decoder.read_to_string(&mut text).unwrap();
    text.lines().map(str::to_string).collect()
}

#[test]
fn archive_uploads_before_the_history_is_deleted() {
    let documents = documents();
    let blob = Arc::new(MemoryStorage::new());
    let archive = HistoryArchive::new(blob.clone(), ArchiveConfig::default(), &ctx());

    let report = block_on(archive.archive(
        Arc::clone(&documents),
        COLLECTION,
        &Scope::Shared,
        "instance-one",
        SESSION,
        "model-a",
    ))
    .unwrap();

    assert!(report.key.starts_with("archives/shared/instance-one/"));
    assert!(report.key.ends_with(".jsonl.gz"));
    assert_eq!(report.metadata.record_count, 3);
    assert_eq!(report.metadata.session_id, SESSION);
    assert_eq!(report.metadata.sha256.len(), 64);

    let original = lines(&documents);
    let stored = block_on(blob.get_blob(&report.key)).unwrap().unwrap();
    assert_eq!(gunzip(&stored), original);

    // Only now does the destroy delete the history. The object stays.
    block_on(documents.delete_all(COLLECTION)).unwrap();
    assert_eq!(block_on(documents.count(COLLECTION)).unwrap(), 0);
    let after = block_on(blob.get_blob(&report.key)).unwrap().unwrap();
    assert_eq!(gunzip(&after), original);
}

#[test]
fn a_failed_upload_deletes_nothing() {
    let documents = documents();
    let archive = HistoryArchive::new(Arc::new(FailingBlobStore), ArchiveConfig::default(), &ctx());

    let result = block_on(archive.archive(
        Arc::clone(&documents),
        COLLECTION,
        &Scope::Shared,
        "instance-one",
        SESSION,
        "model-a",
    ));
    assert!(result.is_err(), "the upload must fail");

    // I5: a failed upload deletes nothing.
    assert_eq!(block_on(documents.count(COLLECTION)).unwrap(), 3);
}

#[test]
fn the_archive_streams_in_parts() {
    let documents = Arc::new(MemoryDocumentStore::new());
    block_on(async {
        for index in 0..20_u32 {
            documents
                .append(
                    COLLECTION,
                    serde_json::json!({ "n": index, "text": "x".repeat(200) }),
                )
                .await
                .unwrap();
        }
    });

    let recorder = Arc::new(RecordingBlobStore::default());
    let archive = HistoryArchive::new(
        recorder.clone(),
        ArchiveConfig::default()
            .with_part_size(64)
            .with_page_size(1),
        &ctx(),
    );
    let report = block_on(archive.archive(
        Arc::clone(&documents),
        COLLECTION,
        &Scope::Shared,
        "instance-one",
        SESSION,
        "model-a",
    ))
    .unwrap();

    assert_eq!(report.metadata.record_count, 20);
    assert!(
        recorder.parts() > 1,
        "a small part size yields several stream parts"
    );
    let stored = recorder.stored.lock().unwrap().clone().unwrap();
    assert_eq!(gunzip(&stored).len(), 20);
}

/// A blob store that refuses every write.
struct FailingBlobStore;

#[async_trait::async_trait]
impl BlobStore for FailingBlobStore {
    async fn put_blob(&self, _key: &str, _data: &[u8]) -> StorageResult<()> {
        Err(StorageError::Backend("the object store is down".into()))
    }

    async fn get_blob(&self, _key: &str) -> StorageResult<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn delete_blob(&self, _key: &str) -> StorageResult<()> {
        Ok(())
    }

    async fn blob_exists(&self, _key: &str) -> StorageResult<bool> {
        Ok(false)
    }

    async fn put_blob_stream(
        &self,
        _key: &str,
        _length: Option<u64>,
        _chunks: BlobStream,
    ) -> StorageResult<()> {
        Err(StorageError::Backend("the object store is down".into()))
    }
}

/// A blob store that counts the stream parts and keeps the whole object.
#[derive(Default)]
struct RecordingBlobStore {
    parts: AtomicUsize,
    stored: Mutex<Option<Vec<u8>>>,
}

impl RecordingBlobStore {
    fn parts(&self) -> usize {
        self.parts.load(Ordering::Acquire)
    }
}

#[async_trait::async_trait]
impl BlobStore for RecordingBlobStore {
    async fn put_blob(&self, _key: &str, data: &[u8]) -> StorageResult<()> {
        *self.stored.lock().unwrap() = Some(data.to_vec());
        Ok(())
    }

    async fn get_blob(&self, _key: &str) -> StorageResult<Option<Vec<u8>>> {
        Ok(self.stored.lock().unwrap().clone())
    }

    async fn delete_blob(&self, _key: &str) -> StorageResult<()> {
        *self.stored.lock().unwrap() = None;
        Ok(())
    }

    async fn blob_exists(&self, _key: &str) -> StorageResult<bool> {
        Ok(self.stored.lock().unwrap().is_some())
    }

    async fn put_blob_stream(
        &self,
        key: &str,
        _length: Option<u64>,
        mut chunks: BlobStream,
    ) -> StorageResult<()> {
        use futures_lite::StreamExt;
        let mut all = Vec::new();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk?;
            self.parts.fetch_add(1, Ordering::AcqRel);
            all.extend_from_slice(&chunk);
        }
        self.put_blob(key, &all).await
    }
}
