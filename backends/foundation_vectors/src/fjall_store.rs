//! Persistent native `VectorStore` over fjall + a persisted IVF index (F29).
//!
//! WHY: the in-memory store (F28) loses data on restart; native sessions need
//! durable vectors with fast ANN. fjall (an on-disk LSM key-value store) holds
//! the serialized vectors; the F25 [`IvfIndex`] provides the ANN search and is
//! itself persisted (`to_bytes`/`load_index`) so a reopened store queries
//! without a full rebuild.
//!
//! WHAT: [`FjallVectorStore`] implements the async [`VectorStore`] (insert /
//! search / delete / get / len, dimension + namespace enforcement). Namespaces are
//! isolated structurally: each lives under its own key prefix in the vector
//! partition and its own key in the index partition.
//!
//! HOW: two fjall partitions —
//! - **`vectors`**: key `{ns}\0{id}` → serialized [`VectorEntry`]; the source of
//!   truth, used to rebuild an index from scratch.
//! - **`indexes`**: key `{ns}` → the IVF index `to_bytes()` blob.
//!
//! A per-namespace [`IvfIndex`] is kept live in memory (the IVF tracks its own
//! staleness and rebuilds centroids internally). Writes update both fjall and
//! the live index and mark the namespace dirty; [`FjallVectorStore::flush`]
//! persists dirty indexes and `fsync`s the keyspace. On open, a namespace's
//! index is loaded from the `indexes` partition if present, else rebuilt by
//! scanning its vectors. `nprobe == nlist` by default, so search is exact (the
//! IVF visits every cluster) while still exercising the persisted-index path;
//! lower `nprobe` trades recall for speed.
//!
//! Sync boundary: fjall has only a sync application programming interface
//! (API). Its reads and writes go to local disk through the operating system
//! (OS) page cache, and a write appends to a journal without an `fsync`. So
//! small reads and writes run inline: `insert`, `delete`, `search`, `get` and
//! `len` never wait for the disk, and every future is ready on its first poll.
//! Two operations do real disk work and run on valtron's blocking-work seam
//! (the same offload pool as `NativeFs` in `foundation_nativeapis`, spec 50
//! feature 04): a namespace index rebuild (a full scan of the vectors
//! partition) and [`FjallVectorStore::flush`] (persist dirty index blobs and
//! `fsync` the keyspace). Both are async and await a result on a [`Pipe`];
//! nothing blocks a valtron worker.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle, PersistMode};
use foundation_core::valtron::{BackgroundJobConfig, BackgroundJobRegistry, Pipe};

use crate::index::{load_index, VectorIndex};
use crate::ivf::IvfIndex;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

/// Default number of IVF clusters per namespace index.
const DEFAULT_NLIST: usize = 16;

/// Default for [`FjallVectorStore::open`]'s offload worker count: 2 threads.
/// Two workers match `NativeFs`'s default (spec 50 feature 04): a rebuild and
/// a `flush` are serialized per keyspace, so two workers cover the read-heavy
/// mix without spare threads.
const DEFAULT_OFFLOAD_THREADS: usize = 2;

fn backend_err<E: core::fmt::Display>(ctx: &str) -> impl Fn(E) -> VectorStoreError + '_ {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

/// The error for a lock that a panicking thread poisoned.
fn poisoned<T>(_: std::sync::PoisonError<T>) -> VectorStoreError {
    VectorStoreError::Backend("fjall vector store lock poisoned".into())
}

/// Live, in-memory state for one namespace.
struct NamespaceState {
    index: Box<dyn VectorIndex>,
    /// The live index has changed since it was last persisted.
    dirty: bool,
}

/// A bounded pool that runs fjall's blocking disk work off the valtron workers.
///
/// Backed by valtron's [`BackgroundJobRegistry`]: a fixed worker count and a
/// bounded first-in-first-out (FIFO) queue, with panic protection per job. Each
/// offloaded job sends its result back through a [`Pipe`], which the async
/// caller awaits.
#[derive(Clone)]
struct OffloadPool {
    registry: Arc<BackgroundJobRegistry>,
}

impl OffloadPool {
    fn new(threads: usize) -> Result<Self, VectorStoreError> {
        if threads == 0 {
            return Err(VectorStoreError::Backend(
                "fjall vector store offload pool needs at least one worker thread".into(),
            ));
        }
        let background = BackgroundJobConfig::new(threads);
        let registry = BackgroundJobRegistry::with_config(background).map_err(|err| {
            VectorStoreError::Backend(format!("start fjall vector store offload pool: {err:?}"))
        })?;
        Ok(Self { registry })
    }

    /// Run `job` on an offload worker and await its result.
    ///
    /// The submit returns an error when the bounded queue is full or closed;
    /// the caller then gets that error instead of unbounded buffering.
    async fn offload<T, F>(&self, job: F) -> Result<T, VectorStoreError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, VectorStoreError> + Send + 'static,
    {
        let (tx, rx) = Pipe::new();
        self.registry
            .submit(move || {
                let _ = tx.try_send(job());
            })
            .map_err(|err| {
                VectorStoreError::Backend(format!(
                    "fjall vector store offload queue full or closed: {err}"
                ))
            })?;
        match rx.receive().await {
            Some(result) => result,
            None => Err(VectorStoreError::Backend(
                "fjall vector store offload worker dropped the result channel".into(),
            )),
        }
    }
}

pub struct FjallVectorStore {
    keyspace: Keyspace,
    vectors: PartitionHandle,
    indexes: PartitionHandle,
    config: VectorStoreConfig,
    nlist: usize,
    nprobe: usize,
    namespaces: RwLock<HashMap<String, NamespaceState>>,
    offload: OffloadPool,
}

impl FjallVectorStore {
    /// Open (or create) a persistent vector store rooted at `path`.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] if the fjall keyspace or its
    /// partitions cannot be opened.
    pub fn open<P: AsRef<Path>>(
        path: P,
        config: VectorStoreConfig,
    ) -> Result<Self, VectorStoreError> {
        Self::open_with_offload_threads(path, config, DEFAULT_OFFLOAD_THREADS)
    }

    /// Open the store and run its offload pool with `offload_threads` workers.
    ///
    /// Use this when the default of 2 workers (see [`DEFAULT_OFFLOAD_THREADS`])
    /// does not fit. Zero workers is an error.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] if the fjall keyspace, its
    /// partitions, or the offload pool cannot be opened.
    pub fn open_with_offload_threads<P: AsRef<Path>>(
        path: P,
        config: VectorStoreConfig,
        offload_threads: usize,
    ) -> Result<Self, VectorStoreError> {
        let keyspace = Config::new(path)
            .open()
            .map_err(backend_err("open keyspace"))?;
        let vectors = keyspace
            .open_partition("vectors", PartitionCreateOptions::default())
            .map_err(backend_err("open vectors partition"))?;
        let indexes = keyspace
            .open_partition("indexes", PartitionCreateOptions::default())
            .map_err(backend_err("open indexes partition"))?;
        let nlist = DEFAULT_NLIST;
        Ok(Self {
            keyspace,
            vectors,
            indexes,
            config,
            nlist,
            // nprobe == nlist → the IVF visits every cluster → exact top-k.
            nprobe: nlist,
            namespaces: RwLock::new(HashMap::new()),
            offload: OffloadPool::new(offload_threads)?,
        })
    }

    /// Replace the offload pool with one of `threads` workers.
    ///
    /// Use it before the first search or flush; the pool that it replaces is
    /// shut down.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] when the pool cannot start (for
    /// example, zero workers).
    pub fn with_offload_threads(mut self, threads: usize) -> Result<Self, VectorStoreError> {
        self.offload = OffloadPool::new(threads)?;
        Ok(self)
    }

    /// Override the IVF clustering parameters (clusters / clusters-probed).
    /// Lower `nprobe` trades recall for speed; the default is exact search.
    #[must_use]
    pub fn with_ivf_params(mut self, nlist: usize, nprobe: usize) -> Self {
        self.nlist = nlist.max(1);
        self.nprobe = nprobe.clamp(1, self.nlist);
        self
    }

    fn vec_key(namespace: &str, id: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(namespace.len() + id.len() + 1);
        k.extend_from_slice(namespace.as_bytes());
        k.push(0);
        k.extend_from_slice(id.as_bytes());
        k
    }

    fn ns_prefix(namespace: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(namespace.len() + 1);
        k.extend_from_slice(namespace.as_bytes());
        k.push(0);
        k
    }

    fn validate(&self, dim: usize, zero: bool) -> Result<(), VectorStoreError> {
        if dim != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: dim,
            });
        }
        if zero {
            return Err(VectorStoreError::ZeroVector);
        }
        Ok(())
    }

    /// Build a fresh, live IVF index for `namespace` from its stored vectors.
    ///
    /// The scan reads every vector of the namespace from the `vectors`
    /// partition, so for a large namespace it runs on the offload pool.
    async fn rebuild_index(
        &self,
        namespace: &str,
    ) -> Result<Box<dyn VectorIndex>, VectorStoreError> {
        let vectors = self.vectors.clone();
        let config = self.config.clone();
        let nlist = self.nlist;
        let nprobe = self.nprobe;
        let ns = namespace.to_string();
        self.offload
            .offload(move || {
                let index: Box<dyn VectorIndex> = Box::new(IvfIndex::new(
                    config.metric,
                    config.dimension,
                    nlist,
                    nprobe,
                ));
                for kv in vectors.prefix(Self::ns_prefix(&ns)) {
                    let (key, val) = kv.map_err(backend_err("scan vectors"))?;
                    let id = id_from_key(&key, &ns);
                    let entry = decode_entry(&val)?;
                    index
                        .insert(&id, &entry.vector.data)
                        .map_err(backend_err("index insert"))?;
                }
                Ok(index)
            })
            .await
    }

    /// Ensure `namespace` has a live index (loaded from disk or rebuilt), then
    /// run `f` against it.
    async fn with_index<T>(
        &self,
        namespace: &str,
        f: impl FnOnce(&dyn VectorIndex) -> Result<T, VectorStoreError>,
    ) -> Result<T, VectorStoreError> {
        {
            let guard = self.namespaces.read().map_err(poisoned)?;
            if let Some(state) = guard.get(namespace) {
                return f(state.index.as_ref());
            }
        }
        let index = self.load_or_rebuild(namespace).await?;
        let mut guard = self.namespaces.write().map_err(poisoned)?;
        let state = guard
            .entry(namespace.to_string())
            .or_insert(NamespaceState {
                index,
                dirty: false,
            });
        f(state.index.as_ref())
    }

    async fn load_or_rebuild(
        &self,
        namespace: &str,
    ) -> Result<Box<dyn VectorIndex>, VectorStoreError> {
        if let Some(bytes) = self
            .indexes
            .get(namespace.as_bytes())
            .map_err(backend_err("load index"))?
        {
            if let Ok(idx) = load_index(&bytes) {
                return Ok(idx);
            }
            // Corrupt/incompatible blob → rebuild from the source-of-truth vectors.
        }
        self.rebuild_index(namespace).await
    }

    /// Mutate the live index for `namespace`, marking it dirty.
    async fn mutate_index(
        &self,
        namespace: &str,
        f: impl FnOnce(&dyn VectorIndex) -> Result<(), VectorStoreError>,
    ) -> Result<(), VectorStoreError> {
        {
            let mut guard = self.namespaces.write().map_err(poisoned)?;
            if let Some(state) = guard.get_mut(namespace) {
                f(state.index.as_ref())?;
                state.dirty = true;
                return Ok(());
            }
        }
        // The namespace has no live index yet: load or rebuild it off the lock,
        // then insert it. `or_insert` keeps a state another task added first.
        let index = self.load_or_rebuild(namespace).await?;
        let mut guard = self.namespaces.write().map_err(poisoned)?;
        let state = guard
            .entry(namespace.to_string())
            .or_insert(NamespaceState {
                index,
                dirty: false,
            });
        f(state.index.as_ref())?;
        state.dirty = true;
        Ok(())
    }

    /// Persist every dirty namespace index and `fsync` the keyspace.
    ///
    /// The `fsync` waits for the disk, so this runs on the offload pool.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] on a fjall write/persist failure.
    pub async fn flush(&self) -> Result<(), VectorStoreError> {
        let mut dirty: Vec<(String, Vec<u8>)> = Vec::new();
        {
            let mut guard = self.namespaces.write().map_err(poisoned)?;
            for (ns, state) in guard.iter_mut() {
                if state.dirty {
                    dirty.push((ns.clone(), state.index.to_bytes()));
                    state.dirty = false;
                }
            }
        }
        let indexes = self.indexes.clone();
        let keyspace = self.keyspace.clone();
        self.offload
            .offload(move || {
                for (ns, bytes) in dirty {
                    indexes
                        .insert(ns.as_bytes(), &bytes)
                        .map_err(backend_err("persist index"))?;
                }
                keyspace
                    .persist(PersistMode::SyncAll)
                    .map_err(backend_err("persist keyspace"))?;
                Ok(())
            })
            .await
    }
}

#[async_trait::async_trait]
impl VectorStore for FjallVectorStore {
    async fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let key = Self::vec_key(namespace, &entry.id);
        self.vectors
            .insert(&key, encode_entry(&entry)?)
            .map_err(backend_err("insert vector"))?;
        let id = entry.id.clone();
        let data = entry.vector.data.clone();
        self.mutate_index(namespace, |idx| {
            idx.insert(&id, &data).map_err(backend_err("index insert"))
        })
        .await
    }

    async fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let key = Self::vec_key(namespace, id);
        if !self
            .vectors
            .contains_key(&key)
            .map_err(backend_err("look up vector"))?
        {
            return Err(VectorStoreError::NotFound { id: id.to_string() });
        }
        self.vectors
            .remove(&key)
            .map_err(backend_err("delete vector"))?;
        self.mutate_index(namespace, |idx| {
            idx.remove(id).map_err(backend_err("index remove"))
        })
        .await
    }

    async fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        self.validate(query.len(), false)?;
        self.with_index(namespace, |idx| {
            idx.search(query, k).map_err(backend_err("search"))
        })
        .await
    }

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
        let key = Self::vec_key(namespace, id);
        match self.vectors.get(&key).map_err(backend_err("get vector"))? {
            Some(val) => {
                let mut entry = decode_entry(&val)?;
                entry.id = id.to_string();
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError> {
        let mut count = 0;
        for kv in self.vectors.prefix(Self::ns_prefix(namespace)) {
            kv.map_err(backend_err("count vectors"))?;
            count += 1;
        }
        Ok(count)
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

/// `[vec_len u32][vec f32 LE…][meta_json…]`. The id lives in the fjall key.
fn encode_entry(entry: &VectorEntry) -> Result<Vec<u8>, VectorStoreError> {
    let dim = entry.vector.data.len();
    let dim_u32 = u32::try_from(dim).map_err(backend_err("vector dimension"))?;
    let meta = serde_json::to_string(&entry.metadata).map_err(backend_err("encode metadata"))?;
    let mut buf = Vec::with_capacity(4 + dim * 4 + meta.len());
    buf.extend_from_slice(&dim_u32.to_le_bytes());
    for f in &entry.vector.data {
        buf.extend_from_slice(&f.to_le_bytes());
    }
    buf.extend_from_slice(meta.as_bytes());
    Ok(buf)
}

fn decode_entry(bytes: &[u8]) -> Result<VectorEntry, VectorStoreError> {
    if bytes.len() < 4 {
        return Err(VectorStoreError::Backend("entry blob too short".into()));
    }
    let dim = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let vec_end = 4 + dim * 4;
    if bytes.len() < vec_end {
        return Err(VectorStoreError::Backend("entry blob truncated".into()));
    }
    let mut data = Vec::with_capacity(dim);
    for chunk in bytes[4..vec_end].chunks_exact(4) {
        data.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let metadata: VectorMetadata =
        serde_json::from_slice(&bytes[vec_end..]).map_err(backend_err("decode metadata"))?;
    Ok(VectorEntry {
        id: String::new(),
        vector: Vector::new(data),
        metadata,
    })
}

/// Recover the entry id from a `{ns}\0{id}` fjall key.
fn id_from_key(key: &[u8], namespace: &str) -> String {
    let start = namespace.len() + 1;
    if key.len() > start {
        String::from_utf8_lossy(&key[start..]).into_owned()
    } else {
        String::new()
    }
}

/// Convenience: a thread-safe handle to a [`FjallVectorStore`].
pub type SharedFjallVectorStore = Arc<FjallVectorStore>;
