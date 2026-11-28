//! The object-store contract under the S3 and R2 durable stream virtual file
//! system (VFS).
//!
//! The contract is transport neutral. It has no Hypertext Transfer Protocol
//! (HTTP) type, header, socket or runtime. A host implements it with its own
//! transport: the S3/R2 HTTP client in this crate, an in-memory store in tests,
//! or a Cloudflare binding on wasm.
//!
//! An object is immutable when the caller follows two rules. A segment key
//! never changes its bytes, and a head manifest changes only through a
//! conditional PUT. The store reports every failed condition as
//! [`ObjectError::PreconditionFailed`]. A caller never treats that error as a
//! missing object.

use core::fmt;
use core::future::Future;
use core::pin::Pin;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// The default object key root. Feature 09 makes the root configuration.
pub const DEFAULT_OBJECT_ROOT: &str = "durable";

/// A boxed object-store operation.
pub type ObjectFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ObjectError>> + Send + 'a>>;

/// An entity tag (`ETag`) that pins one object version.
///
/// A conditional update sends the tag of the version it read. The store
/// refuses the update when the tag is stale.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub String);

impl Version {
    /// Build a version from its text form.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Return the text form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What one object store supports.
// The switches are independent capabilities, so one struct keeps the record in
// one place.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectCapabilities {
    /// True when the store honours `If-Match` and `If-None-Match`.
    ///
    /// A store that ignores the condition headers reports `false`. The VFS
    /// then uses a lease object and records the weaker guarantee.
    pub conditional_write: bool,
    /// True when the store assembles one object from several parts.
    pub multipart: bool,
    /// The largest object one single PUT can carry, in bytes.
    pub max_single_put_bytes: u64,
    /// True when the store checks a CRC32 checksum on a PUT.
    pub checksum_crc32: bool,
    /// True when a read after a write sees the write at once.
    pub strong_consistency: bool,
}

impl Default for ObjectCapabilities {
    fn default() -> Self {
        Self {
            conditional_write: true,
            multipart: true,
            // Amazon S3 accepts a single PUT up to 5 GiB. This default uses
            // 8 MiB, the smallest practical multipart part size, so a test
            // reaches the multipart path without a large allocation.
            max_single_put_bytes: 8 * 1024 * 1024,
            checksum_crc32: false,
            strong_consistency: true,
        }
    }
}

/// One object entry from a HEAD or a LIST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectMeta {
    /// The full object key.
    pub key: String,
    /// The object length in bytes.
    pub len: u64,
    /// The version tag.
    pub version: Version,
    /// The last write time in UNIX milliseconds.
    pub last_modified_ms: i64,
}

/// The result of one batch delete.
///
/// The report keeps every key that failed, so a caller never treats a partial
/// delete as a full delete.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteReport {
    /// The keys that are now absent.
    pub deleted: Vec<String>,
    /// The keys that failed, with the store's message.
    pub errors: Vec<(String, String)>,
}

impl DeleteReport {
    /// True when every key was deleted.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.errors.is_empty()
    }
}

/// An object-store refusal.
#[derive(Debug)]
pub enum ObjectError {
    /// The object is absent.
    NotFound,
    /// A conditional write lost its compare-and-swap.
    PreconditionFailed,
    /// The operation is not supported by this store.
    Unsupported(&'static str),
    /// The transport or the store failed.
    Backend(Box<dyn core::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("object not found"),
            Self::PreconditionFailed => f.write_str("object precondition failed"),
            Self::Unsupported(what) => write!(f, "object store does not support {what}"),
            Self::Backend(error) => write!(f, "object store error: {error}"),
        }
    }
}

impl core::error::Error for ObjectError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Backend(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// The object-store contract for the durable stream VFS.
///
/// Every method is async. A host supplies the transport. The VFS never reads a
/// whole stream: it asks for a bounded range.
pub trait ObjectStore: Send + Sync {
    /// Put a new object only when the key is absent (`If-None-Match: *`).
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::PreconditionFailed`] when the key exists.
    fn put_if_none_match(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version>;

    /// Put an object only when its version is still `version`.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::PreconditionFailed`] when the version is stale.
    fn put_if_match(
        &self,
        key: &str,
        bytes: Vec<u8>,
        version: &Version,
    ) -> ObjectFuture<'_, Version>;

    /// Put an object without a condition.
    ///
    /// The VFS uses this only behind a lease when the store has no conditional
    /// write.
    fn put(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version>;

    /// Put one object from several parts.
    ///
    /// The parts concatenate in order. The key is immutable, so a retry with
    /// the same parts writes the same bytes.
    fn put_parts(&self, key: &str, parts: Vec<Vec<u8>>) -> ObjectFuture<'_, Version>;

    /// Read `len` bytes at `start` from one object.
    ///
    /// A short result is an error at the caller. The store returns the bytes
    /// that remain when the range reaches the object end.
    fn get_range(&self, key: &str, start: u64, len: usize) -> ObjectFuture<'_, Vec<u8>>;

    /// Read one whole object.
    ///
    /// The VFS uses this only for a small head manifest.
    fn get(&self, key: &str) -> ObjectFuture<'_, Vec<u8>>;

    /// Return the object entry, or `None` when the key is absent.
    fn head(&self, key: &str) -> ObjectFuture<'_, Option<ObjectMeta>>;

    /// List every object whose key starts with `prefix`.
    fn list(&self, prefix: &str) -> ObjectFuture<'_, Vec<ObjectMeta>>;

    /// Delete one object. An absent key is not an error.
    fn delete(&self, key: &str) -> ObjectFuture<'_, ()>;

    /// Delete several objects and report each key result.
    fn delete_many(&self, keys: Vec<String>) -> ObjectFuture<'_, DeleteReport>;

    /// Return the capability record.
    fn capabilities(&self) -> ObjectCapabilities;
}

#[derive(Clone, Debug)]
struct StoredObject {
    bytes: Vec<u8>,
    version: Version,
    last_modified_ms: i64,
    parts: usize,
}

/// A single-object write record for the statistics.
#[derive(Clone, Debug, Default)]
pub struct ObjectStoreStats {
    /// The number of object writes (one per object, not per part).
    pub object_puts: u64,
    /// The number of part writes.
    pub part_puts: u64,
    /// The number of range GETs.
    pub range_gets: u64,
    /// The number of whole-object GETs.
    pub gets: u64,
    /// The number of HEAD requests.
    pub heads: u64,
    /// The number of LIST requests.
    pub lists: u64,
    /// The number of object deletes.
    pub deletes: u64,
    /// The largest number of parts in one object write.
    pub max_parts: usize,
    /// Every range GET, in order.
    pub range_reads: Vec<(String, u64, usize)>,
}

/// Fault switches for the in-memory store. A test uses them to model a real
/// store failure without a network.
// Each switch models one independent failure, so the test keeps them together.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
pub struct MemoryFaults {
    /// A PUT of this key returns a backend error.
    pub fail_put_key: Option<String>,
    /// The next conditional PUT of any kind fails once.
    pub fail_next_conditional_put: bool,
    /// The next `If-Match` head compare-and-swap loses the condition once.
    pub fail_next_put_if_match: bool,
    /// The next `If-Match` head compare-and-swap returns a backend error once.
    pub fail_next_head_cas: bool,
    /// The store ignores `If-Match`, so a stale update succeeds.
    pub ignore_if_match: bool,
    /// A DELETE of these keys reports an error and keeps the object.
    pub fail_delete_keys: BTreeSet<String>,
}

/// An in-memory object store.
///
/// It is the test double for a real service and a useful local store. It
/// implements the full contract: conditional writes, ranges, HEAD, LIST,
/// multipart and per-key delete results.
#[derive(Debug)]
pub struct MemoryObjectStore {
    objects: Mutex<BTreeMap<String, StoredObject>>,
    capabilities: Mutex<ObjectCapabilities>,
    faults: Mutex<MemoryFaults>,
    stats: Mutex<ObjectStoreStats>,
    next_version: Mutex<u64>,
}

impl Default for MemoryObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryObjectStore {
    /// Build an empty store with the default capabilities.
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: Mutex::new(BTreeMap::new()),
            capabilities: Mutex::new(ObjectCapabilities::default()),
            faults: Mutex::new(MemoryFaults::default()),
            stats: Mutex::new(ObjectStoreStats::default()),
            next_version: Mutex::new(0),
        }
    }

    /// Replace the capability record.
    pub fn set_capabilities(&self, capabilities: ObjectCapabilities) {
        *self.lock_capabilities() = capabilities;
    }

    /// Replace the fault switches.
    pub fn set_faults(&self, faults: MemoryFaults) {
        *self.lock_faults() = faults;
    }

    /// Change one fault switch.
    pub fn set_fault(&self, update: impl FnOnce(&mut MemoryFaults)) {
        update(&mut self.lock_faults());
    }

    /// Set the last-write time of one object, for a lifecycle test.
    pub fn set_last_modified(&self, key: &str, last_modified_ms: i64) {
        if let Some(object) = self.lock_objects().get_mut(key) {
            object.last_modified_ms = last_modified_ms;
        }
    }

    /// Return a copy of the statistics.
    #[must_use]
    pub fn stats(&self) -> ObjectStoreStats {
        self.lock_stats().clone()
    }

    /// Clear the statistics.
    pub fn reset_stats(&self) {
        *self.lock_stats() = ObjectStoreStats::default();
    }

    /// Return the stored bytes of one key.
    #[must_use]
    pub fn bytes(&self, key: &str) -> Option<Vec<u8>> {
        self.lock_objects()
            .get(key)
            .map(|object| object.bytes.clone())
    }

    /// Return every stored key.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.lock_objects().keys().cloned().collect()
    }

    /// True when the key is present.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.lock_objects().contains_key(key)
    }

    /// Return the number of part writes recorded for one key.
    #[must_use]
    pub fn parts_of(&self, key: &str) -> Option<usize> {
        self.lock_objects().get(key).map(|object| object.parts)
    }

    fn lock_objects(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, StoredObject>> {
        self.objects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_capabilities(&self) -> std::sync::MutexGuard<'_, ObjectCapabilities> {
        self.capabilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_faults(&self) -> std::sync::MutexGuard<'_, MemoryFaults> {
        self.faults
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_stats(&self) -> std::sync::MutexGuard<'_, ObjectStoreStats> {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn next_version(&self) -> Version {
        let mut next = self
            .next_version
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *next = next.saturating_add(1);
        Version(format!("v{next}"))
    }

    fn backend(message: impl Into<String>) -> ObjectError {
        ObjectError::Backend(Box::new(MemoryStoreError(message.into())))
    }

    fn put_object(&self, key: &str, bytes: Vec<u8>, parts: usize) -> Version {
        let version = self.next_version();
        self.lock_objects().insert(
            key.to_string(),
            StoredObject {
                bytes,
                version: version.clone(),
                last_modified_ms: 0,
                parts,
            },
        );
        version
    }

    fn put_sync(
        &self,
        key: &str,
        bytes: Vec<u8>,
        condition: &PutCondition,
        parts: usize,
    ) -> Result<Version, ObjectError> {
        if self
            .lock_faults()
            .fail_put_key
            .as_deref()
            .is_some_and(|failed| failed == key)
        {
            return Err(Self::backend(format!("injected PUT failure for {key}")));
        }
        if self.lock_faults().fail_next_conditional_put && *condition != PutCondition::None {
            self.lock_faults().fail_next_conditional_put = false;
            return Err(Self::backend("injected conditional PUT failure"));
        }
        if self.lock_faults().fail_next_head_cas && matches!(condition, PutCondition::IfMatch(_)) {
            self.lock_faults().fail_next_head_cas = false;
            return Err(Self::backend("injected head CAS failure"));
        }
        if self.lock_faults().fail_next_put_if_match
            && matches!(condition, PutCondition::IfMatch(_))
        {
            self.lock_faults().fail_next_put_if_match = false;
            return Err(ObjectError::PreconditionFailed);
        }
        let ignore_if_match = self.lock_faults().ignore_if_match;
        match condition {
            PutCondition::None => {}
            PutCondition::IfNoneMatch => {
                if self.lock_objects().contains_key(key) {
                    return Err(ObjectError::PreconditionFailed);
                }
            }
            PutCondition::IfMatch(expected) => {
                if !ignore_if_match {
                    let objects = self.lock_objects();
                    let Some(object) = objects.get(key) else {
                        return Err(ObjectError::PreconditionFailed);
                    };
                    if &object.version != expected {
                        return Err(ObjectError::PreconditionFailed);
                    }
                }
            }
        }
        {
            let mut stats = self.lock_stats();
            stats.object_puts = stats.object_puts.saturating_add(1);
            stats.max_parts = stats.max_parts.max(parts);
            if parts > 1 {
                stats.part_puts = stats.part_puts.saturating_add(parts as u64);
            }
        }
        Ok(self.put_object(key, bytes, parts))
    }

    fn get_range_sync(&self, key: &str, start: u64, len: usize) -> Result<Vec<u8>, ObjectError> {
        {
            let mut stats = self.lock_stats();
            stats.range_gets = stats.range_gets.saturating_add(1);
            stats.range_reads.push((key.to_string(), start, len));
        }
        let objects = self.lock_objects();
        let object = objects.get(key).ok_or(ObjectError::NotFound)?;
        let start = usize::try_from(start).map_err(|_| Self::backend("range start overflows"))?;
        if start > object.bytes.len() {
            return Err(Self::backend("range starts past the object end"));
        }
        let end = start.saturating_add(len).min(object.bytes.len());
        Ok(object.bytes[start..end].to_vec())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PutCondition {
    None,
    IfNoneMatch,
    IfMatch(Version),
}

#[derive(Debug)]
struct MemoryStoreError(String);

impl fmt::Display for MemoryStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for MemoryStoreError {}

impl ObjectStore for MemoryObjectStore {
    fn put_if_none_match(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version> {
        let key = key.to_string();
        Box::pin(async move { self.put_sync(&key, bytes, &PutCondition::IfNoneMatch, 1) })
    }

    fn put_if_match(
        &self,
        key: &str,
        bytes: Vec<u8>,
        version: &Version,
    ) -> ObjectFuture<'_, Version> {
        let version = version.clone();
        let key = key.to_string();
        Box::pin(async move { self.put_sync(&key, bytes, &PutCondition::IfMatch(version), 1) })
    }

    fn put(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version> {
        let key = key.to_string();
        Box::pin(async move { self.put_sync(&key, bytes, &PutCondition::None, 1) })
    }

    fn put_parts(&self, key: &str, parts: Vec<Vec<u8>>) -> ObjectFuture<'_, Version> {
        let count = parts.len().max(1);
        let bytes: Vec<u8> = parts.into_iter().flatten().collect();
        let key = key.to_string();
        Box::pin(async move { self.put_sync(&key, bytes, &PutCondition::None, count) })
    }

    fn get_range(&self, key: &str, start: u64, len: usize) -> ObjectFuture<'_, Vec<u8>> {
        let key = key.to_string();
        Box::pin(async move { self.get_range_sync(&key, start, len) })
    }

    fn get(&self, key: &str) -> ObjectFuture<'_, Vec<u8>> {
        let key = key.to_string();
        Box::pin(async move {
            {
                let mut stats = self.lock_stats();
                stats.gets = stats.gets.saturating_add(1);
            }
            self.lock_objects()
                .get(&key)
                .map(|object| object.bytes.clone())
                .ok_or(ObjectError::NotFound)
        })
    }

    fn head(&self, key: &str) -> ObjectFuture<'_, Option<ObjectMeta>> {
        let key = key.to_string();
        Box::pin(async move {
            {
                let mut stats = self.lock_stats();
                stats.heads = stats.heads.saturating_add(1);
            }
            Ok(self.lock_objects().get(&key).map(|object| ObjectMeta {
                key: key.clone(),
                len: object.bytes.len() as u64,
                version: object.version.clone(),
                last_modified_ms: object.last_modified_ms,
            }))
        })
    }

    fn list(&self, prefix: &str) -> ObjectFuture<'_, Vec<ObjectMeta>> {
        let prefix = prefix.to_string();
        Box::pin(async move {
            {
                let mut stats = self.lock_stats();
                stats.lists = stats.lists.saturating_add(1);
            }
            Ok(self
                .lock_objects()
                .iter()
                .filter(|(key, _)| key.starts_with(&prefix))
                .map(|(key, object)| ObjectMeta {
                    key: key.clone(),
                    len: object.bytes.len() as u64,
                    version: object.version.clone(),
                    last_modified_ms: object.last_modified_ms,
                })
                .collect())
        })
    }

    fn delete(&self, key: &str) -> ObjectFuture<'_, ()> {
        let key = key.to_string();
        Box::pin(async move {
            {
                let mut stats = self.lock_stats();
                stats.deletes = stats.deletes.saturating_add(1);
            }
            self.lock_objects().remove(&key);
            Ok(())
        })
    }

    fn delete_many(&self, keys: Vec<String>) -> ObjectFuture<'_, DeleteReport> {
        Box::pin(async move {
            let mut report = DeleteReport::default();
            for key in keys {
                let failed = self.lock_faults().fail_delete_keys.contains(&key);
                if failed {
                    report
                        .errors
                        .push((key.clone(), format!("injected delete failure for {key}")));
                    continue;
                }
                {
                    let mut stats = self.lock_stats();
                    stats.deletes = stats.deletes.saturating_add(1);
                }
                self.lock_objects().remove(&key);
                report.deleted.push(key);
            }
            Ok(report)
        })
    }

    fn capabilities(&self) -> ObjectCapabilities {
        self.lock_capabilities().clone()
    }
}
