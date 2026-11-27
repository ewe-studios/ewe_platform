//! The S3 and R2 durable stream virtual file system (VFS).
//!
//! The design follows feature 09 of specification 71. A write lands in the
//! local spool first. A batcher seals the spool into immutable segment objects.
//! The head manifest is the authority, and a conditional PUT is its
//! compare-and-swap (CAS). A read loads the head, then asks for byte ranges. No
//! whole stream is ever read or copied.
//!
//! # Structure
//!
//! | Module | Holds |
//! |---|---|
//! | [`object_store`] | the transport-neutral object-store contract and an in-memory store |
//! | [`head`] | the head manifest, the object keys and the content hash |
//! | [`spool`] | the local spool, replay and the in-flight batch record |
//! | [`vfs`] | the VFS core over one object store |
//!
//! # Limits
//!
//! Every size, age, retry and concurrency bound is a field on
//! [`ObjectVfsConfig`]. The defaults are documented on the fields.
//!
//! # Acknowledgement
//!
//! An append returns `Accepted` after the spool fsync. A batch is `Sealed` when
//! the segment PUT and the head CAS both succeed. [`PublishPolicy::Immediate`]
//! makes an append wait for the seal. The default
//! [`PublishPolicy::Batched`] does not block the caller on a network round
//! trip.

pub mod head;
pub mod object_store;
pub mod spool;
pub mod vfs;

use core::time::Duration;
use std::path::PathBuf;

pub use head::{HeadManifest, SegmentEntry};
pub use object_store::{
    DeleteReport, MemoryFaults, MemoryObjectStore, ObjectCapabilities as StoreCapabilities,
    ObjectError, ObjectFuture, ObjectMeta, ObjectStore, ObjectStoreStats, Version,
};
pub use spool::{InflightBatch, Spool, SpoolState};
pub use vfs::{
    CompactionReport, FlushReport, LifecycleReport, ObjectNotifier, ObjectVfs, ProbeReport,
    ProviderCapabilities,
};

/// The object layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectLayout {
    /// One immutable object per batch under `<stream>/seg/`. The default.
    #[default]
    Segmented,
    /// One immutable object per batch under `<stream>/log/`, never compacted.
    AppendOnly,
}

/// When an append waits for the object store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PublishPolicy {
    /// The append returns after the local fsync. A host flushes on a timer.
    #[default]
    Batched,
    /// The append flushes its stream before it returns.
    Immediate,
}

/// How the head compare-and-swap works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CasMode {
    /// The store honours `If-Match` and `If-None-Match`.
    ConditionalPut,
    /// The store ignores `If-Match`; a lease object serialises the writers.
    ///
    /// The guarantee is weaker. A writer that stops while it holds the lease
    /// blocks other writers until [`ObjectVfsConfig::lease_timeout`].
    Lease,
}

/// The lifecycle state of one object.
///
/// A live object is referenced by a head. A retired object is not referenced
/// and waits for the grace window. A delete-issued object is being deleted. A
/// deleted object is absent. The VFS never deletes a live or a retired object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectLifecycle {
    /// A head references the object.
    Live,
    /// No head references the object; its grace window has not ended.
    Retired,
    /// The grace window ended and the delete is in progress.
    DeleteIssued,
    /// The object is absent.
    Deleted,
}

/// The provider that owns the object store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Amazon Simple Storage Service (S3).
    S3,
    /// Cloudflare R2.
    R2,
}

impl Provider {
    /// Return the provider name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::S3 => "s3",
            Self::R2 => "r2",
        }
    }
}

/// The explicit provider settings that the VFS records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderSettings {
    /// The provider.
    pub provider: Provider,
    /// The signing region. R2 uses `auto`.
    pub region: String,
    /// True for path-style addressing.
    pub path_style: bool,
    /// True when the provider checks a CRC32 checksum.
    pub crc32: bool,
}

impl ProviderSettings {
    /// The S3 defaults: region `us-east-1`, virtual-host style, no CRC32.
    #[must_use]
    pub fn s3() -> Self {
        Self {
            provider: Provider::S3,
            region: "us-east-1".to_string(),
            path_style: false,
            crc32: false,
        }
    }

    /// The R2 defaults: region `auto`, path-style and CRC32.
    #[must_use]
    pub fn r2() -> Self {
        Self {
            provider: Provider::R2,
            region: "auto".to_string(),
            path_style: true,
            crc32: true,
        }
    }
}

/// The default segment target: 256 KiB, the Rivet shard object size.
pub const DEFAULT_SEGMENT_TARGET_BYTES: usize = 256 * 1024;
/// The default batch cap: 1 MiB.
pub const DEFAULT_MAX_BLOCK_BYTES: usize = 1024 * 1024;
/// The default batch age: 5 s.
///
/// No established server defines a default object-batch age. Rivet seals a
/// shard when it reaches its size. The 5 s linger is a policy choice. It bounds
/// the time between `Accepted` and `Sealed` when the size is not reached.
pub const DEFAULT_BATCH_AGE: Duration = Duration::from_secs(5);
/// The default number of parallel stream PUTs.
///
/// The Amazon S3 transfer manager uploads five parts at a time by default. This
/// value applies the same bound to whole streams.
pub const DEFAULT_PARALLEL_PUTS: usize = 4;
/// The default retry bound for one PUT or one head CAS.
pub const DEFAULT_RETRY_BOUND: u32 = 3;
/// The default single-PUT limit: 8 MiB.
///
/// The Amazon S3 multipart part minimum is 5 MiB. This value is the typical
/// first part size.
pub const DEFAULT_MAX_SINGLE_PUT_BYTES: u64 = 8 * 1024 * 1024;
/// The default multipart part size: 8 MiB.
pub const DEFAULT_MULTIPART_PART_BYTES: usize = 8 * 1024 * 1024;
/// The default fork-chain depth limit. The memory and file VFS use 64.
pub const DEFAULT_MAX_FORK_DEPTH: u32 = 64;
/// The default head cache time: 1 s.
pub const DEFAULT_HEAD_CACHE_MS: i64 = 1000;
/// The default range cache size: 16 MiB.
pub const DEFAULT_RANGE_CACHE_BYTES: usize = 16 * 1024 * 1024;
/// The default lifecycle grace window: 24 h.
///
/// The window separates an object retire from its delete. A shorter window
/// risks deleting an object that a slow reader still references.
pub const DEFAULT_LIFECYCLE_GRACE: Duration = Duration::from_hours(24);
/// The default lease timeout: 30 s.
pub const DEFAULT_LEASE_TIMEOUT: Duration = Duration::from_secs(30);
/// The default number of read revalidations: 2.
pub const DEFAULT_REVALIDATION_RETRIES: u32 = 2;

/// The configuration of an object-store VFS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectVfsConfig {
    /// The object layout.
    pub layout: ObjectLayout,
    /// The object key root.
    pub root: String,
    /// The target segment size. The batcher splits a large batch at this size.
    pub segment_target_bytes: usize,
    /// The largest batch that one flush seals.
    pub max_block_bytes: usize,
    /// The age after which a pending batch is eligible.
    pub batch_age: Duration,
    /// The number of stream PUTs that run at the same time.
    pub parallel_puts: usize,
    /// The retry bound for one PUT or one head CAS.
    pub retry_bound: u32,
    /// When an append waits for the object store.
    pub publish_policy: PublishPolicy,
    /// The largest object one single PUT may carry.
    pub max_single_put_bytes: u64,
    /// The multipart part size.
    pub multipart_part_bytes: usize,
    /// The deepest fork chain that a read follows.
    pub max_fork_depth: u32,
    /// The time a cached head stays fresh, in milliseconds.
    pub head_cache_ms: i64,
    /// The byte cap of the range cache.
    pub range_cache_bytes: usize,
    /// The grace window between an object retire and its delete.
    pub lifecycle_grace: Duration,
    /// The time a lease object survives its writer.
    pub lease_timeout: Duration,
    /// The number of times a read revalidates a changed head.
    pub revalidation_retries: u32,
    /// The local spool root. A host must set it.
    pub spool_root: PathBuf,
}

impl Default for ObjectVfsConfig {
    fn default() -> Self {
        Self {
            layout: ObjectLayout::Segmented,
            root: object_store::DEFAULT_OBJECT_ROOT.to_string(),
            segment_target_bytes: DEFAULT_SEGMENT_TARGET_BYTES,
            max_block_bytes: DEFAULT_MAX_BLOCK_BYTES,
            batch_age: DEFAULT_BATCH_AGE,
            parallel_puts: DEFAULT_PARALLEL_PUTS,
            retry_bound: DEFAULT_RETRY_BOUND,
            publish_policy: PublishPolicy::Batched,
            max_single_put_bytes: DEFAULT_MAX_SINGLE_PUT_BYTES,
            multipart_part_bytes: DEFAULT_MULTIPART_PART_BYTES,
            max_fork_depth: DEFAULT_MAX_FORK_DEPTH,
            head_cache_ms: DEFAULT_HEAD_CACHE_MS,
            range_cache_bytes: DEFAULT_RANGE_CACHE_BYTES,
            lifecycle_grace: DEFAULT_LIFECYCLE_GRACE,
            lease_timeout: DEFAULT_LEASE_TIMEOUT,
            revalidation_retries: DEFAULT_REVALIDATION_RETRIES,
            spool_root: PathBuf::new(),
        }
    }
}

impl ObjectVfsConfig {
    /// Set the object layout.
    #[must_use]
    pub fn with_layout(mut self, layout: ObjectLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set the object key root.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<String>) -> Self {
        self.root = root.into();
        self
    }

    /// Set the target segment size.
    #[must_use]
    pub fn with_segment_target_bytes(mut self, bytes: usize) -> Self {
        self.segment_target_bytes = bytes.max(1);
        self
    }

    /// Set the largest batch that one flush seals.
    #[must_use]
    pub fn with_max_block_bytes(mut self, bytes: usize) -> Self {
        self.max_block_bytes = bytes.max(1);
        self
    }

    /// Set the batch age.
    #[must_use]
    pub fn with_batch_age(mut self, age: Duration) -> Self {
        self.batch_age = age;
        self
    }

    /// Set the number of parallel stream PUTs.
    #[must_use]
    pub fn with_parallel_puts(mut self, puts: usize) -> Self {
        self.parallel_puts = puts.max(1);
        self
    }

    /// Set the retry bound.
    #[must_use]
    pub fn with_retry_bound(mut self, retries: u32) -> Self {
        self.retry_bound = retries;
        self
    }

    /// Set when an append waits for the object store.
    #[must_use]
    pub fn with_publish_policy(mut self, policy: PublishPolicy) -> Self {
        self.publish_policy = policy;
        self
    }

    /// Set the single-PUT limit.
    #[must_use]
    pub fn with_max_single_put_bytes(mut self, bytes: u64) -> Self {
        self.max_single_put_bytes = bytes.max(1);
        self
    }

    /// Set the multipart part size.
    #[must_use]
    pub fn with_multipart_part_bytes(mut self, bytes: usize) -> Self {
        self.multipart_part_bytes = bytes.max(1);
        self
    }

    /// Set the fork-chain depth limit.
    #[must_use]
    pub fn with_max_fork_depth(mut self, depth: u32) -> Self {
        self.max_fork_depth = depth;
        self
    }

    /// Set the head cache time.
    #[must_use]
    pub fn with_head_cache_ms(mut self, ms: i64) -> Self {
        self.head_cache_ms = ms;
        self
    }

    /// Set the range cache size.
    #[must_use]
    pub fn with_range_cache_bytes(mut self, bytes: usize) -> Self {
        self.range_cache_bytes = bytes;
        self
    }

    /// Set the lifecycle grace window.
    #[must_use]
    pub fn with_lifecycle_grace(mut self, grace: Duration) -> Self {
        self.lifecycle_grace = grace;
        self
    }

    /// Set the lease timeout.
    #[must_use]
    pub fn with_lease_timeout(mut self, timeout: Duration) -> Self {
        self.lease_timeout = timeout;
        self
    }

    /// Set the number of read revalidations.
    #[must_use]
    pub fn with_revalidation_retries(mut self, retries: u32) -> Self {
        self.revalidation_retries = retries;
        self
    }

    /// Set the local spool root.
    #[must_use]
    pub fn with_spool_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.spool_root = root.into();
        self
    }
}
