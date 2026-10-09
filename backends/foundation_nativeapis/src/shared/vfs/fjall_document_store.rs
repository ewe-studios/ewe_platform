#![cfg(feature = "vfs-fjall")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use foundation_core::valtron::Stream;
use foundation_db::traits::{Document, DocumentStore, PromotableDocument, StorageItemStream};
use foundation_db::{StorageError, StorageResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::traits::{VfsDirectory, VfsFile, VfsFileSystem};

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

pub struct FjallDocumentStore<V: VfsFileSystem> {
    vfs: V,
    root: String,
    #[allow(dead_code)]
    keyspace: fjall::Keyspace,
    primary: fjall::PartitionHandle,
    by_type: fjall::PartitionHandle,
    collections: Mutex<HashMap<String, Arc<RwLock<CollectionState>>>>,
    durability: Arc<DurabilityWriteConfig>,
}

impl<V: VfsFileSystem> FjallDocumentStore<V> {
    /// # Errors
    /// Returns an error if the fjall keyspace or VFS root cannot be opened.
    pub fn open(
        vfs: V,
        root: String,
        index_path: impl AsRef<std::path::Path>,
        durability: Arc<DurabilityWriteConfig>,
    ) -> StorageResult<Self> {
        vfs.mkdir_all(&root)
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
        };

        store.rebuild_stale_indexes()?;

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
        let mut k =
            Vec::with_capacity(hex_key.len() + 1 + record_type.len() + 1 + doc_id.len());
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

    fn get_or_create_collection(&self, key: &str) -> Arc<RwLock<CollectionState>> {
        let mut map = self.collections.lock().unwrap();
        map.entry(key.to_string())
            .or_insert_with(|| {
                let hwm = self.read_hwm(key).unwrap_or(0);
                let file_path = self.collection_file_path(key);
                let file_size = if self.vfs.exists(&file_path).unwrap_or(false) {
                    self.vfs
                        .open(&file_path, super::types::OpenMode::Read)
                        .and_then(|f| f.size())
                        .unwrap_or(0)
                } else {
                    0
                };
                Arc::new(RwLock::new(CollectionState {
                    file_high_water_mark: file_size.max(hwm),
                    batch: Vec::new(),
                    accumulated_batch_bytes: 0,
                    first_unflushed_at: None,
                }))
            })
            .clone()
    }

    fn read_hwm(&self, key: &str) -> Option<u64> {
        let hwm_key = Self::hwm_key(key);
        self.primary
            .get(&hwm_key)
            .ok()
            .flatten()
            .and_then(|v| {
                if v.len() == 8 {
                    Some(u64::from_le_bytes(v[..8].try_into().unwrap()))
                } else {
                    None
                }
            })
    }

    fn ensure_file_exists(&self, key: &str) -> StorageResult<()> {
        let path = self.collection_file_path(key);
        if !self.vfs.exists(&path).map_err(|e| StorageError::Backend(e.to_string()))? {
            self.vfs
                .create(&path, 0o644)
                .map_err(|e| StorageError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    fn put(
        &self,
        key: &str,
        doc_id: String,
        content_json: String,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        self.ensure_file_exists(key)?;

        let row = NdjsonRow {
            id: doc_id.clone(),
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type: record_type.clone(),
        };

        let mut line = serde_json::to_vec(&row)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        line.push(b'\n');

        let document = row.to_document();
        let line_len = line.len() as u64;

        let col = self.get_or_create_collection(key);
        let mut state = col.write().unwrap();

        let offset = state.file_high_water_mark + state.accumulated_batch_bytes;

        state.batch.push(BatchEntry { line });
        state.accumulated_batch_bytes += line_len;

        if state.first_unflushed_at.is_none() {
            state.first_unflushed_at = Some(Instant::now());
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

        if should_flush {
            self.flush_locked(key, &mut state)?;
        }

        drop(state);

        // Insert into primary index
        let idx_key = Self::composite_key(key, &document.id);
        let idx_val = IndexValue {
            offset,
            len: line_len as u32,
        };
        let idx_bytes = serde_json::to_vec(&idx_val)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
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

    fn flush_locked(
        &self,
        key: &str,
        state: &mut CollectionState,
    ) -> StorageResult<()> {
        if state.batch.is_empty() {
            return Ok(());
        }

        let batch = std::mem::take(&mut state.batch);
        let batch_bytes = state.accumulated_batch_bytes;
        let write_offset = state.file_high_water_mark;

        let mut combined = Vec::with_capacity(batch_bytes as usize);
        for entry in &batch {
            combined.extend_from_slice(&entry.line);
        }

        let file_path = self.collection_file_path(key);
        let file = self
            .vfs
            .open(&file_path, super::types::OpenMode::ReadWrite)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        file.write_at(&combined, write_offset)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        file.sync_data()
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        state.file_high_water_mark += batch_bytes;
        state.accumulated_batch_bytes = 0;
        state.first_unflushed_at = None;
        state.batch = Vec::new();

        // Update HWM in fjall
        let hwm_key = Self::hwm_key(key);
        self.primary
            .insert(&hwm_key, &state.file_high_water_mark.to_le_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Force flush all pending batches.
    ///
    /// # Errors
    /// Returns storage errors from VFS writes or fjall index updates.
    pub fn flush(&self, key: &str) -> StorageResult<()> {
        let col = self.get_or_create_collection(key);
        let mut state = col.write().unwrap();
        self.flush_locked(key, &mut state)
    }

    fn read_document_at(&self, key: &str, offset: u64, len: u32) -> StorageResult<NdjsonRow> {
        let file_path = self.collection_file_path(key);

        let col = self.get_or_create_collection(key);
        let state = col.read().unwrap();

        // Check if the offset falls within the unflushed batch
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
        drop(state);

        // Read from persisted file
        let file = self
            .vfs
            .open(&file_path, super::types::OpenMode::Read)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let mut buf = vec![0u8; len as usize];
        let n = file
            .read_at(&mut buf, offset)
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        buf.truncate(n);

        let line_str = std::str::from_utf8(&buf)
            .map_err(|e| StorageError::Deserialization(e.to_string()))?;
        let row: NdjsonRow = serde_json::from_str(line_str.trim_end())
            .map_err(|e| StorageError::Deserialization(e.to_string()))?;
        Ok(row)
    }

    fn collect_rows(
        &self,
        key: &str,
        newest_first: bool,
        from_id: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<NdjsonRow>> {
        let prefix = Self::collection_prefix(key);
        let mut rows = Vec::new();

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

        if newest_first {
            for item in self.primary.range(range_start..range_end).rev() {
                let (k, v) = item.map_err(|e| StorageError::Backend(e.to_string()))?;

                // Skip HWM keys
                if k.starts_with(HWM_KEY_PREFIX) {
                    continue;
                }

                let idx: IndexValue = serde_json::from_slice(&v)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;

                match self.read_document_at(key, idx.offset, idx.len) {
                    Ok(row) => rows.push(row),
                    Err(_) => continue,
                }

                if let Some(n) = limit {
                    if rows.len() >= n {
                        break;
                    }
                }
            }
        } else {
            for item in self.primary.range(range_start..range_end) {
                let (k, v) = item.map_err(|e| StorageError::Backend(e.to_string()))?;

                if k.starts_with(HWM_KEY_PREFIX) {
                    continue;
                }

                let idx: IndexValue = serde_json::from_slice(&v)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;

                match self.read_document_at(key, idx.offset, idx.len) {
                    Ok(row) => rows.push(row),
                    Err(_) => continue,
                }

                if let Some(n) = limit {
                    if rows.len() >= n {
                        break;
                    }
                }
            }
        }

        Ok(rows)
    }

    fn rebuild_stale_indexes(&self) -> StorageResult<()> {
        // Scan all .jsonl files in root directory
        let dir = self
            .vfs
            .open_directory(&self.root)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let entries = dir
            .list()
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
            let file_size = self
                .vfs
                .open(&file_path, super::types::OpenMode::Read)
                .and_then(|f| f.size())
                .map_err(|e| StorageError::Backend(e.to_string()))?;

            if file_size > hwm {
                self.rebuild_tail(&collection_key, hwm, file_size)?;
            }
        }

        Ok(())
    }

    fn rebuild_tail(
        &self,
        key: &str,
        from_offset: u64,
        file_size: u64,
    ) -> StorageResult<()> {
        let file_path = self.collection_file_path(key);
        let remaining = (file_size - from_offset) as usize;

        let file = self
            .vfs
            .open(&file_path, super::types::OpenMode::Read)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let mut buf = vec![0u8; remaining];
        let n = file
            .read_at(&mut buf, from_offset)
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        buf.truncate(n);

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

fn rows_to_stream<V: DeserializeOwned + Send + 'static>(
    rows: Vec<NdjsonRow>,
) -> StorageItemStream<'static, V> {
    Box::new(rows.into_iter().map(|r| {
        match serde_json::from_str::<V>(&r.content) {
            Ok(v) => Stream::Next(Ok(v)),
            Err(e) => Stream::Next(Err(StorageError::Deserialization(e.to_string()))),
        }
    }))
}

impl<V: VfsFileSystem + Send + Sync> DocumentStore for FjallDocumentStore<V>
where
    V::File: Send + Sync,
    V::SeekableFile: Send + Sync,
    V::Directory: Send + Sync,
{
    fn append<C: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: C,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id, content_json, None, None, None)
    }

    fn append_with_id<C: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: C,
    ) -> StorageResult<Document> {
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id.to_string(), content_json, None, None, None)
    }

    fn append_promotable<C: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: C,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id, content_json, t, s, rt)
    }

    fn append_promotable_with_id<C: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: C,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put(key, doc_id.to_string(), content_json, t, s, rt)
    }

    fn scan<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, C>> {
        let rows = self.collect_rows(key, true, None, Some(limit))?;
        Ok(rows_to_stream(rows))
    }

    fn scan_all<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, C>> {
        let rows = self.collect_rows(key, false, None, None)?;
        Ok(rows_to_stream(rows))
    }

    fn scan_from<C: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, C>> {
        let rows = self.collect_rows(key, false, Some(from_id), limit_opt(limit))?;
        Ok(rows_to_stream(rows))
    }

    fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let rows = self.collect_rows(key, true, None, Some(limit))?;
        Ok(rows.iter().map(NdjsonRow::to_document).collect())
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let rows = self.collect_rows(key, false, Some(from_id), limit_opt(limit))?;
        Ok(rows.iter().map(NdjsonRow::to_document).collect())
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let idx_key = Self::composite_key(key, doc_id);

        // Read the index entry to get record_type for secondary cleanup
        if let Ok(Some(val)) = self.primary.get(&idx_key) {
            if let Ok(idx) = serde_json::from_slice::<IndexValue>(&val) {
                if let Ok(row) = self.read_document_at(key, idx.offset, idx.len) {
                    if let Some(ref rt) = row.record_type {
                        let type_key = Self::type_composite_key(key, rt, doc_id);
                        let _ = self.by_type.remove(&type_key);
                    }
                }
            }
        }

        self.primary
            .remove(&idx_key)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let prefix = Self::collection_prefix(key);
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        let mut count = 0u64;
        let keys_to_remove: Vec<Vec<u8>> = self
            .primary
            .range(prefix..range_end)
            .filter_map(|item| item.ok().map(|(k, _)| k.to_vec()))
            .collect();

        for k in &keys_to_remove {
            if k.starts_with(HWM_KEY_PREFIX) {
                continue;
            }
            let _ = self.primary.remove(k);
            count += 1;
        }

        // Clean up by_type secondary entries
        let type_prefix = Self::collection_prefix(key);
        let mut type_end = type_prefix.clone();
        if let Some(last) = type_end.last_mut() {
            *last = last.saturating_add(1);
        }
        let type_keys: Vec<Vec<u8>> = self
            .by_type
            .range(type_prefix..type_end)
            .filter_map(|item| item.ok().map(|(k, _)| k.to_vec()))
            .collect();
        for k in &type_keys {
            let _ = self.by_type.remove(k);
        }

        // Remove the HWM entry too
        let hwm_key = Self::hwm_key(key);
        let _ = self.primary.remove(&hwm_key);

        // Reset collection state
        let mut map = self.collections.lock().unwrap();
        map.remove(key);

        Ok(count)
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let prefix = Self::collection_prefix(key);
        let mut range_end = prefix.clone();
        if let Some(last) = range_end.last_mut() {
            *last = last.saturating_add(1);
        }

        let count = self
            .primary
            .range(prefix..range_end)
            .filter(|item| {
                item.as_ref()
                    .map(|(k, _)| !k.starts_with(HWM_KEY_PREFIX))
                    .unwrap_or(false)
            })
            .count() as u64;

        Ok(count)
    }
}
