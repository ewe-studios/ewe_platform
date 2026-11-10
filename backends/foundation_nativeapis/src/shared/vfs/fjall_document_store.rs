#![cfg(feature = "vfs-fjall")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use foundation_core::valtron::{Ctx, Pipe};
use foundation_db::traits::{Document, DocumentStore, PromotableDocument};
use foundation_db::{AsyncStorageItemStream, StorageError, StorageResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::async_traits::{AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem};

const HWM_KEY_PREFIX: &[u8] = b"__hwm\x00";
const KEY_SEP: u8 = 0x00;

#[derive(Debug, Clone)]
pub struct DurabilityWriteConfig {
    pub batch_size_bytes: u64,
    pub flush_timeout: Duration,
}

impl Default for DurabilityWriteConfig {
    fn default() -> Self {
        Self {
            batch_size_bytes: 0,
            flush_timeout: Duration::from_secs(2),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct IndexValue {
    offset: u64,
    len: u32,
}

/// One decoded fjall index entry, sent from the scan producer thread to the
/// stream consumer through the bounded pipe.
enum ScanIndexItem {
    Entry(IndexValue),
    Error(StorageError),
}

/// Boxed fjall range iterator, erased without a `Send` bound (fjall's own
/// erasure). Owned by the scan producer thread.
type FjallRange = Box<dyn Iterator<Item = fjall::Result<(Vec<u8>, Vec<u8>)>>>;

#[derive(Serialize, Deserialize)]
struct NdjsonRow {
    id: String,
    content: String,
    metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_type: Option<String>,
}

impl NdjsonRow {
    fn to_document(&self) -> Document {
        Document::new(
            self.id.clone(),
            self.content.clone(),
            self.metadata.clone(),
            self.title.clone(),
            self.summary.clone(),
            self.record_type.clone(),
        )
    }
}

struct BatchEntry {
    line: Vec<u8>,
}

struct CollectionState {
    file_high_water_mark: u64,
    batch: Vec<BatchEntry>,
    accumulated_batch_bytes: u64,
    first_unflushed_at: Option<Instant>,
}

pub struct FjallDocumentStore<V: AsyncVfsFileSystem> {
    vfs: V,
    root: String,
    #[allow(dead_code)]
    keyspace: fjall::Keyspace,
    primary: fjall::PartitionHandle,
    by_type: fjall::PartitionHandle,
    collections: Mutex<HashMap<String, Arc<RwLock<CollectionState>>>>,
    durability: Arc<DurabilityWriteConfig>,
    ctx: Ctx,
}

impl<V: AsyncVfsFileSystem> FjallDocumentStore<V> {
    /// A store over `vfs` with its fjall index in a fresh directory under the
    /// system temp dir, so every store starts empty and does not share an index
    /// with another. The flush timer and the unique index name read the clock of `ctx`.
    ///
    /// # Panics
    ///
    /// If the index directory cannot be created or opened.
    #[must_use]
    pub fn new(vfs: V, ctx: Ctx) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = format!(
            "ewe-fjall-{}-{}-{}",
            std::process::id(),
            foundation_compact::time::unix_millis(ctx.clock().wall_now()),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        let index_path = std::env::temp_dir().join(unique);
        futures_lite::future::block_on(Self::open(
            vfs,
            "/documents".to_string(),
            &index_path,
            Arc::new(DurabilityWriteConfig::default()),
            ctx,
        ))
        .unwrap_or_else(|e| {
            panic!(
                "FjallDocumentStore::new: can't open an index at {}: {e}",
                index_path.display()
            )
        })
    }

    /// Open a store over `vfs` whose flush timer and unique index names read
    /// the clock of `ctx`.
    ///
    /// # Errors
    /// Returns an error if the fjall keyspace or VFS root cannot be opened.
    pub async fn open(
        vfs: V,
        root: String,
        index_path: impl AsRef<std::path::Path>,
        durability: Arc<DurabilityWriteConfig>,
        ctx: Ctx,
    ) -> StorageResult<Self> {
        vfs.mkdir_all_async(root.clone())
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let keyspace = fjall::Config::new(index_path)
            .open()
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let primary = keyspace
            .open_partition("primary", fjall::PartitionCreateOptions::default())
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let by_type = keyspace
            .open_partition("by_type", fjall::PartitionCreateOptions::default())
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let store = Self {
            vfs,
            root,
            keyspace,
            primary,
            by_type,
            collections: Mutex::new(HashMap::new()),
            durability,
            ctx,
        };

        store.rebuild_stale_indexes().await?;

        Ok(store)
    }

    fn collection_file_path(&self, key: &str) -> String {
        let hex_key = hex::encode(key);
        if self.root.ends_with('/') {
            format!("{}{}.jsonl", self.root, hex_key)
        } else {
            format!("{}/{}.jsonl", self.root, hex_key)
        }
    }

    fn composite_key(key: &str, doc_id: &str) -> Vec<u8> {
        let hex_key = hex::encode(key);
        let mut k = Vec::with_capacity(hex_key.len() + 1 + doc_id.len());
        k.extend_from_slice(hex_key.as_bytes());
        k.push(KEY_SEP);
        k.extend_from_slice(doc_id.as_bytes());
        k
    }

    fn type_composite_key(key: &str, record_type: &str, doc_id: &str) -> Vec<u8> {
        let hex_key = hex::encode(key);
        let mut k = Vec::with_capacity(hex_key.len() + 1 + record_type.len() + 1 + doc_id.len());
        k.extend_from_slice(hex_key.as_bytes());
        k.push(KEY_SEP);
        k.extend_from_slice(record_type.as_bytes());
        k.push(KEY_SEP);
        k.extend_from_slice(doc_id.as_bytes());
        k
    }

    fn collection_prefix(key: &str) -> Vec<u8> {
        let hex_key = hex::encode(key);
        let mut p = Vec::with_capacity(hex_key.len() + 1);
        p.extend_from_slice(hex_key.as_bytes());
        p.push(KEY_SEP);
        p
    }

    fn hwm_key(key: &str) -> Vec<u8> {
        let hex_key = hex::encode(key);
        let mut k = Vec::with_capacity(HWM_KEY_PREFIX.len() + hex_key.len());
        k.extend_from_slice(HWM_KEY_PREFIX);
        k.extend_from_slice(hex_key.as_bytes());
        k
    }

    async fn get_or_create_collection(&self, key: &str) -> Arc<RwLock<CollectionState>> {
        if let Some(col) = self.collections.lock().unwrap().get(key) {
            return col.clone();
        }

        let hwm = self.read_hwm(key).unwrap_or(0);
        let file_path = self.collection_file_path(key);
        let file_size = if self
            .vfs
            .exists_async(file_path.clone())
            .await
            .unwrap_or(false)
        {
            match self
                .vfs
                .open_async(file_path, super::types::OpenMode::Read)
                .await
            {
                Ok(f) => f.size_async().await.unwrap_or(0),
                Err(_) => 0,
            }
        } else {
            0
        };

        let col = Arc::new(RwLock::new(CollectionState {
            file_high_water_mark: file_size.max(hwm),
            batch: Vec::new(),
            accumulated_batch_bytes: 0,
            first_unflushed_at: None,
        }));

        // Racing insertions build the same state from the same VFS snapshot, so
        // keeping the first one is safe.
        self.collections
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_insert(col)
            .clone()
    }

    fn read_hwm(&self, key: &str) -> Option<u64> {
        let hwm_key = Self::hwm_key(key);
        self.primary.get(&hwm_key).ok().flatten().and_then(|v| {
            if v.len() == 8 {
                Some(u64::from_le_bytes(v[..8].try_into().unwrap()))
            } else {
                None
            }
        })
    }

    async fn ensure_file_exists(&self, key: &str) -> StorageResult<()> {
        let path = self.collection_file_path(key);
        if !self
            .vfs
            .exists_async(path.clone())
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
        {
            self.vfs
                .create_async(path, 0o644)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    async fn put(
        &self,
        key: &str,
        doc_id: String,
        content_json: String,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        self.ensure_file_exists(key).await?;

        let row = NdjsonRow {
            id: doc_id.clone(),
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type: record_type.clone(),
        };

        let mut line =
            serde_json::to_vec(&row).map_err(|e| StorageError::Serialization(e.to_string()))?;
        line.push(b'\n');

        let document = row.to_document();
        let line_len = line.len() as u64;

        let col = self.get_or_create_collection(key).await;

        // Mutate the batch state under the lock, then drop the guard before
        // any await so the append futures stay `Send` (a `std::sync` write
        // guard is not `Send`).
        let (offset, should_flush) = {
            let mut state = col.write().unwrap();
            let offset = state.file_high_water_mark + state.accumulated_batch_bytes;

            state.batch.push(BatchEntry { line });
            state.accumulated_batch_bytes += line_len;

            if state.first_unflushed_at.is_none() {
                state.first_unflushed_at = Some(self.ctx.clock().now());
            }

            let should_flush = if self.durability.batch_size_bytes == 0 {
                true
            } else if state.accumulated_batch_bytes >= self.durability.batch_size_bytes {
                true
            } else if let Some(first) = state.first_unflushed_at {
                first.elapsed() >= self.durability.flush_timeout
            } else {
                false
            };

            (offset, should_flush)
        };

        if should_flush {
            self.flush_locked(key, &col).await?;
        }

        // Insert into primary index
        let idx_key = Self::composite_key(key, &document.id);
        let idx_val = IndexValue {
            offset,
            len: line_len as u32,
        };
        let idx_bytes =
            serde_json::to_vec(&idx_val).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.primary
            .insert(&idx_key, &idx_bytes)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // Insert into by_type secondary index
        if let Some(ref rt) = document.record_type {
            let type_key = Self::type_composite_key(key, rt, &document.id);
            self.by_type
                .insert(&type_key, b"")
                .map_err(|e| StorageError::Backend(e.to_string()))?;
        }

        Ok(document)
    }

    async fn flush_locked(
        &self,
        key: &str,
        col: &Arc<RwLock<CollectionState>>,
    ) -> StorageResult<()> {
        // Extract the pending batch and advance the accounting under the lock,
        // so a concurrent `put` computes a fresh offset. The bytes are written
        // after the guard is dropped, so the future stays `Send`.
        let (batch, batch_bytes, write_offset) = {
            let mut state = col.write().unwrap();
            if state.batch.is_empty() {
                return Ok(());
            }

            let batch = std::mem::take(&mut state.batch);
            let batch_bytes = state.accumulated_batch_bytes;
            let write_offset = state.file_high_water_mark;

            state.file_high_water_mark += batch_bytes;
            state.accumulated_batch_bytes = 0;
            state.first_unflushed_at = None;

            (batch, batch_bytes, write_offset)
        };

        let mut combined = Vec::with_capacity(batch_bytes as usize);
        for entry in &batch {
            combined.extend_from_slice(&entry.line);
        }

        let file_path = self.collection_file_path(key);
        let file = self
            .vfs
            .open_async(file_path, super::types::OpenMode::ReadWrite)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        file.write_at_async(combined, write_offset)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        file.sync_data_async()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // Persist the advanced high water mark in fjall.
        let hwm = col.read().unwrap().file_high_water_mark;
        let hwm_key = Self::hwm_key(key);
        self.primary
            .insert(&hwm_key, &hwm.to_le_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Force flush all pending batches.
    ///
    /// # Errors
    /// Returns storage errors from VFS writes or fjall index updates.
    pub async fn flush(&self, key: &str) -> StorageResult<()> {
        let col = self.get_or_create_collection(key).await;
        self.flush_locked(key, &col).await
    }

    async fn read_document_at(&self, key: &str, offset: u64, len: u32) -> StorageResult<NdjsonRow> {
        let file_path = self.collection_file_path(key);

        let col = self.get_or_create_collection(key).await;

        // Check the unflushed batch first. The read guard is scoped so it is
        // dropped before any await below, keeping the future `Send`.
        {
            let state = col.read().unwrap();
            let batch_start = state.file_high_water_mark;
            if offset >= batch_start {
                let batch_offset = (offset - batch_start) as usize;
                let mut pos = 0usize;
                for entry in &state.batch {
                    if pos == batch_offset {
                        let line_str = std::str::from_utf8(&entry.line)
                            .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                        let row: NdjsonRow = serde_json::from_str(line_str.trim_end())
                            .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                        return Ok(row);
                    }
                    pos += entry.line.len();
                }
                return Err(StorageError::NotFound(format!(
                    "offset {offset} not found in batch"
                )));
            }
        }

        // Read from persisted file
        let file = self
            .vfs
            .open_async(file_path, super::types::OpenMode::Read)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let buf = file
            .read_at_async(len as usize, offset)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let line_str =
            std::str::from_utf8(&buf).map_err(|e| StorageError::Deserialization(e.to_string()))?;
        let row: NdjsonRow = serde_json::from_str(line_str.trim_end())
            .map_err(|e| StorageError::Deserialization(e.to_string()))?;
        Ok(row)
    }

    async fn collect_rows(
        &self,
        key: &str,
        newest_first: bool,
        from_id: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<NdjsonRow>> {
        let prefix = Self::collection_prefix(key);

        let range_start = if let Some(fid) = from_id {
            Self::composite_key(key, fid)
        } else {
            prefix.clone()
        };

        // Upper bound: prefix with last byte incremented
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        // Collect the matching index entries first. The fjall range iterator
        // is not `Send`, so it cannot be held across the document reads below.
        let range = self.primary.range(range_start..range_end);
        let mapped = range.map(|item| item.map(|(k, v)| (k.to_vec(), v.to_vec())));
        let iter: FjallRange = if newest_first {
            Box::new(mapped.rev())
        } else {
            Box::new(mapped)
        };

        let mut index_entries = Vec::new();
        for item in iter {
            let (k, v) = item.map_err(|e| StorageError::Backend(e.to_string()))?;

            if k.starts_with(HWM_KEY_PREFIX) {
                continue;
            }

            let idx: IndexValue = serde_json::from_slice(&v)
                .map_err(|e| StorageError::Deserialization(e.to_string()))?;

            index_entries.push(idx);

            if let Some(n) = limit {
                if index_entries.len() >= n {
                    break;
                }
            }
        }

        let mut rows = Vec::with_capacity(index_entries.len());
        for idx in index_entries {
            match self.read_document_at(key, idx.offset, idx.len).await {
                Ok(row) => rows.push(row),
                Err(_) => continue,
            }
        }

        Ok(rows)
    }

    /// A lazy scan: a producer thread walks the fjall index range one entry at
    /// a time and pushes decoded offsets through a bounded, waker-hooked pipe,
    /// and the stream reads one document per yielded item. Nothing is collected
    /// into a `Vec`, so `scan_all` is bounded only by how many items the caller
    /// drains.
    fn scan_rows_stream<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        newest_first: bool,
        from_id: Option<&str>,
        limit: Option<usize>,
    ) -> AsyncStorageItemStream<'_, C> {
        let prefix = Self::collection_prefix(key);
        let range_start =
            from_id.map_or_else(|| prefix.clone(), |fid| Self::composite_key(key, fid));
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        // The fjall range iterator is `!Send` (fjall erases the box without a
        // `Send` bound), but the stream must be `Send`. A producer thread owns
        // the iterator and streams decoded index entries through a bounded
        // pipe. The producer parks on a full pipe (backpressure) and stops when
        // the consumer drops its half.
        let primary = self.primary.clone();
        let (tx, rx) = Pipe::new();
        std::thread::spawn(move || {
            let range = primary.range(range_start..range_end);
            let mapped = range.map(|item| item.map(|(k, v)| (k.to_vec(), v.to_vec())));
            let iter: FjallRange = if newest_first {
                Box::new(mapped.rev())
            } else {
                Box::new(mapped)
            };
            for item in iter {
                let msg = match item {
                    Ok((k, v)) => {
                        if k.starts_with(HWM_KEY_PREFIX) {
                            continue;
                        }
                        match serde_json::from_slice::<IndexValue>(&v) {
                            Ok(idx) => ScanIndexItem::Entry(idx),
                            Err(e) => {
                                ScanIndexItem::Error(StorageError::Deserialization(e.to_string()))
                            }
                        }
                    }
                    Err(e) => ScanIndexItem::Error(StorageError::Backend(e.to_string())),
                };
                // Blocking send with backpressure. An error means the consumer
                // dropped the pipe, so the thread can stop.
                if futures_lite::future::block_on(tx.send(msg)).is_err() {
                    break;
                }
            }
            // Dropping `tx` closes the pipe; `rx.receive()` then returns None.
        });

        let key = key.to_string();
        Box::pin(futures_lite::stream::unfold(
            (rx, key, limit),
            move |(rx, key, mut remaining)| async move {
                if remaining == Some(0) {
                    return None;
                }
                loop {
                    let item = rx.receive().await?;
                    let idx = match item {
                        ScanIndexItem::Entry(idx) => idx,
                        ScanIndexItem::Error(e) => return Some((Err(e), (rx, key, remaining))),
                    };
                    let Ok(row) = self.read_document_at(&key, idx.offset, idx.len).await else {
                        continue;
                    };
                    let doc: C = match serde_json::from_str(&row.content) {
                        Ok(d) => d,
                        Err(e) => {
                            return Some((
                                Err(StorageError::Deserialization(e.to_string())),
                                (rx, key, remaining),
                            ))
                        }
                    };
                    if let Some(rem) = &mut remaining {
                        *rem -= 1;
                    }
                    return Some((Ok(doc), (rx, key, remaining)));
                }
            },
        ))
    }

    async fn rebuild_stale_indexes(&self) -> StorageResult<()> {
        // Scan all .jsonl files in root directory
        let dir = self
            .vfs
            .open_directory_async(self.root.clone())
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let entries = dir
            .list_async()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        for entry in entries {
            if !entry.name.ends_with(".jsonl") {
                continue;
            }

            let hex_name = entry.name.trim_end_matches(".jsonl");
            let collection_key = match hex::decode(hex_name) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let hwm = self.read_hwm(&collection_key).unwrap_or(0);
            let file_path = self.collection_file_path(&collection_key);
            let file = self
                .vfs
                .open_async(file_path, super::types::OpenMode::Read)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let file_size = file
                .size_async()
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;

            if file_size > hwm {
                self.rebuild_tail(&collection_key, hwm, file_size).await?;
            }
        }

        Ok(())
    }

    async fn rebuild_tail(&self, key: &str, from_offset: u64, file_size: u64) -> StorageResult<()> {
        let file_path = self.collection_file_path(key);
        let remaining = (file_size - from_offset) as usize;

        let file = self
            .vfs
            .open_async(file_path, super::types::OpenMode::Read)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let buf = file
            .read_at_async(remaining, from_offset)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let text =
            std::str::from_utf8(&buf).map_err(|e| StorageError::Deserialization(e.to_string()))?;

        let mut offset = from_offset;
        for line in text.lines() {
            if line.is_empty() {
                offset += 1; // newline
                continue;
            }

            let line_bytes = line.len() as u64 + 1; // +1 for newline

            if let Ok(row) = serde_json::from_str::<NdjsonRow>(line) {
                let idx_key = Self::composite_key(key, &row.id);
                let idx_val = IndexValue {
                    offset,
                    len: line_bytes as u32,
                };
                let idx_bytes = serde_json::to_vec(&idx_val)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;

                self.primary
                    .insert(&idx_key, &idx_bytes)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;

                if let Some(ref rt) = row.record_type {
                    let type_key = Self::type_composite_key(key, rt, &row.id);
                    self.by_type
                        .insert(&type_key, b"")
                        .map_err(|e| StorageError::Backend(e.to_string()))?;
                }
            }

            offset += line_bytes;
        }

        // Update HWM
        let hwm_key = Self::hwm_key(key);
        self.primary
            .insert(&hwm_key, &file_size.to_le_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(())
    }
}

fn limit_opt(limit: usize) -> Option<usize> {
    (limit > 0).then_some(limit)
}

fn backend(e: impl std::fmt::Display) -> StorageError {
    StorageError::Backend(e.to_string())
}

/// The fjall operations are local; the async impl awaits the VFS calls so an
/// I/O-backed store (`SyncFs`-wrapped) does not block the calling thread.
#[async_trait::async_trait]
impl<V: AsyncVfsFileSystem> DocumentStore for FjallDocumentStore<V> {
    async fn append<C: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: C,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id, content_json, None, None, None).await
    }

    async fn append_with_id<C: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: C,
    ) -> StorageResult<Document> {
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id.to_string(), content_json, None, None, None)
            .await
    }

    async fn append_promotable<C: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: C,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id, content_json, t, s, rt).await
    }

    async fn append_promotable_with_id<C: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: C,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id.to_string(), content_json, t, s, rt)
            .await
    }

    async fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let rows = self.collect_rows(key, true, None, Some(limit)).await?;
        Ok(rows.iter().map(NdjsonRow::to_document).collect())
    }

    async fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let rows = self
            .collect_rows(key, false, Some(from_id), limit_opt(limit))
            .await?;
        Ok(rows.iter().map(NdjsonRow::to_document).collect())
    }

    async fn scan<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, C>> {
        Ok(self.scan_rows_stream(key, true, None, Some(limit)))
    }

    async fn scan_all<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, C>> {
        Ok(self.scan_rows_stream(key, false, None, None))
    }

    async fn scan_from<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, C>> {
        Ok(self.scan_rows_stream(key, false, Some(from_id), limit_opt(limit)))
    }

    async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let idx_key = Self::composite_key(key, doc_id);

        // Read the index entry to get record_type for secondary cleanup.
        if let Some(val) = self.primary.get(&idx_key).map_err(backend)? {
            let idx = serde_json::from_slice::<IndexValue>(&val)
                .map_err(|e| StorageError::Deserialization(e.to_string()))?;
            let row = self.read_document_at(key, idx.offset, idx.len).await?;
            if let Some(ref rt) = row.record_type {
                let type_key = Self::type_composite_key(key, rt, doc_id);
                self.by_type.remove(&type_key).map_err(backend)?;
            }
        }

        self.primary.remove(&idx_key).map_err(backend)?;
        Ok(())
    }

    async fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let prefix = Self::collection_prefix(key);
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        let mut keys_to_remove = Vec::new();
        for item in self.primary.range(prefix..range_end) {
            let (k, _) = item.map_err(backend)?;
            keys_to_remove.push(k.to_vec());
        }

        let mut count = 0u64;
        for k in &keys_to_remove {
            if k.starts_with(HWM_KEY_PREFIX) {
                continue;
            }
            self.primary.remove(k).map_err(backend)?;
            count += 1;
        }

        // Clean up by_type secondary entries.
        let type_prefix = Self::collection_prefix(key);
        let mut type_end = type_prefix.clone();
        if let Some(last) = type_end.last_mut() {
            *last = last.saturating_add(1);
        }
        let mut type_keys = Vec::new();
        for item in self.by_type.range(type_prefix..type_end) {
            let (k, _) = item.map_err(backend)?;
            type_keys.push(k.to_vec());
        }
        for k in &type_keys {
            self.by_type.remove(k).map_err(backend)?;
        }

        // Remove the HWM entry too.
        self.primary.remove(Self::hwm_key(key)).map_err(backend)?;

        // Reset collection state.
        self.collections
            .lock()
            .map_err(|e| StorageError::Backend(format!("collections lock poisoned: {e}")))?
            .remove(key);

        Ok(count)
    }

    async fn count(&self, key: &str) -> StorageResult<u64> {
        let prefix = Self::collection_prefix(key);
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        let mut count = 0u64;
        for item in self.primary.range(prefix..range_end) {
            let (k, _) = item.map_err(backend)?;
            if !k.starts_with(HWM_KEY_PREFIX) {
                count += 1;
            }
        }
        Ok(count)
    }
}
