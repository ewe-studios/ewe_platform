//! Persistent native `VectorStore` over fjall + a persisted IVF index (F29).
//!
//! WHY: the in-memory store (F28) loses data on restart; native sessions need
//! durable vectors with fast ANN. fjall (an on-disk LSM key-value store) holds
//! the serialized vectors; the F25 [`IvfIndex`] provides the ANN search and is
//! itself persisted (`to_bytes`/`load_index`) so a reopened store queries
//! without a full rebuild.
//!
//! WHAT: [`FjallVectorStore`] implements [`VectorStore`] (insert / search /
//! delete / get / len, dimension + namespace enforcement). Namespaces are
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

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle, PersistMode};

use crate::index::{load_index, VectorIndex};
use crate::ivf::IvfIndex;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

/// Default number of IVF clusters per namespace index.
const DEFAULT_NLIST: usize = 16;

fn backend_err<E: core::fmt::Display>(ctx: &str) -> impl Fn(E) -> VectorStoreError + '_ {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

/// Live, in-memory state for one namespace.
struct NamespaceState {
    index: Box<dyn VectorIndex>,
    /// The live index has changed since it was last persisted.
    dirty: bool,
}

pub struct FjallVectorStore {
    keyspace: Keyspace,
    vectors: PartitionHandle,
    indexes: PartitionHandle,
    config: VectorStoreConfig,
    nlist: usize,
    nprobe: usize,
    namespaces: RwLock<HashMap<String, NamespaceState>>,
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
        })
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
    fn rebuild_index(&self, namespace: &str) -> Result<Box<dyn VectorIndex>, VectorStoreError> {
        let index: Box<dyn VectorIndex> = Box::new(IvfIndex::new(
            self.config.metric,
            self.config.dimension,
            self.nlist,
            self.nprobe,
        ));
        for kv in self.vectors.prefix(Self::ns_prefix(namespace)) {
            let (key, val) = kv.map_err(backend_err("scan vectors"))?;
            let id = id_from_key(&key, namespace);
            let entry = decode_entry(&val)?;
            index
                .insert(&id, &entry.vector.data)
                .map_err(backend_err("index insert"))?;
        }
        Ok(index)
    }

    /// Ensure `namespace` has a live index (loaded from disk or rebuilt), then
    /// run `f` against it.
    fn with_index<T>(
        &self,
        namespace: &str,
        f: impl FnOnce(&dyn VectorIndex) -> Result<T, VectorStoreError>,
    ) -> Result<T, VectorStoreError> {
        {
            let guard = self.namespaces.read().unwrap();
            if let Some(state) = guard.get(namespace) {
                return f(state.index.as_ref());
            }
        }
        let index = self.load_or_rebuild(namespace)?;
        let mut guard = self.namespaces.write().unwrap();
        let state = guard
            .entry(namespace.to_string())
            .or_insert(NamespaceState { index, dirty: false });
        f(state.index.as_ref())
    }

    fn load_or_rebuild(&self, namespace: &str) -> Result<Box<dyn VectorIndex>, VectorStoreError> {
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
        self.rebuild_index(namespace)
    }

    /// Mutate the live index for `namespace`, marking it dirty.
    fn mutate_index(
        &self,
        namespace: &str,
        f: impl FnOnce(&dyn VectorIndex) -> Result<(), VectorStoreError>,
    ) -> Result<(), VectorStoreError> {
        let mut guard = self.namespaces.write().unwrap();
        if !guard.contains_key(namespace) {
            drop(guard);
            let index = self.load_or_rebuild(namespace)?;
            guard = self.namespaces.write().unwrap();
            guard
                .entry(namespace.to_string())
                .or_insert(NamespaceState { index, dirty: false });
        }
        let state = guard.get_mut(namespace).expect("just inserted");
        f(state.index.as_ref())?;
        state.dirty = true;
        Ok(())
    }

    /// Persist every dirty namespace index and `fsync` the keyspace.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] on a fjall write/persist failure.
    pub fn flush(&self) -> Result<(), VectorStoreError> {
        let mut guard = self.namespaces.write().unwrap();
        for (ns, state) in guard.iter_mut() {
            if state.dirty {
                self.indexes
                    .insert(ns.as_bytes(), state.index.to_bytes())
                    .map_err(backend_err("persist index"))?;
                state.dirty = false;
            }
        }
        self.keyspace
            .persist(PersistMode::SyncAll)
            .map_err(backend_err("persist keyspace"))?;
        Ok(())
    }
}

impl VectorStore for FjallVectorStore {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let key = Self::vec_key(namespace, &entry.id);
        self.vectors
            .insert(&key, encode_entry(&entry))
            .map_err(backend_err("insert vector"))?;
        let id = entry.id.clone();
        let data = entry.vector.data.clone();
        self.mutate_index(namespace, |idx| {
            idx.insert(&id, &data).map_err(backend_err("index insert"))
        })
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let key = Self::vec_key(namespace, id);
        self.vectors
            .remove(&key)
            .map_err(backend_err("delete vector"))?;
        self.mutate_index(namespace, |idx| {
            idx.remove(id).map_err(backend_err("index remove"))
        })
    }

    fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        self.validate(query.len(), false)?;
        self.with_index(namespace, |idx| {
            idx.search(query, k).map_err(backend_err("search"))
        })
    }

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
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

    fn len(&self, namespace: &str) -> usize {
        self.vectors
            .prefix(Self::ns_prefix(namespace))
            .filter(|kv| kv.is_ok())
            .count()
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

/// `[vec_len u32][vec f32 LE…][meta_json…]`. The id lives in the fjall key.
fn encode_entry(entry: &VectorEntry) -> Vec<u8> {
    let dim = entry.vector.data.len();
    let meta = serde_json::to_string(&entry.metadata).unwrap_or_else(|_| "{}".to_string());
    let mut buf = Vec::with_capacity(4 + dim * 4 + meta.len());
    buf.extend_from_slice(&(dim as u32).to_le_bytes());
    for f in &entry.vector.data {
        buf.extend_from_slice(&f.to_le_bytes());
    }
    buf.extend_from_slice(meta.as_bytes());
    buf
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
    let metadata: VectorMetadata = serde_json::from_slice(&bytes[vec_end..]).unwrap_or_default();
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

// Allow `Arc<FjallVectorStore>` to be shared; fjall handles are internally synced.
unsafe impl Send for FjallVectorStore {}
unsafe impl Sync for FjallVectorStore {}

/// Convenience: a thread-safe handle to a [`FjallVectorStore`].
pub type SharedFjallVectorStore = Arc<FjallVectorStore>;
