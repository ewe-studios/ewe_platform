//! The Durable Streams protocol core.
//!
//! This module is runtime-neutral and transport-neutral. It has no valtron, HTTP, socket or
//! global-runtime dependency. The HTTP, ConnectRPC, WebSocket and embedded surfaces wrap it.
//!
//! The types follow `specifications/71-durable-streams-in-valtron`. Do not add a transport
//! status, header or ETag here. The adapters own the wire rules.

#![allow(async_fn_in_trait)] // Native async trait. The trait is used generically, never as `dyn`.

mod memory;

pub use memory::MemoryVfs;

use core::fmt;
use core::future::Future;
use core::pin::Pin;
use std::sync::Arc;

/// An opaque, byte-lexicographically ordered position in a stream.
///
/// Only a store builds offsets. A reader never parses or constructs one. It compares offsets
/// from one stream by byte order.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamOffset(Box<[u8]>);

impl StreamOffset {
    /// Build an offset from its bytes.
    ///
    /// Backends use this. A reader must not call it.
    #[must_use]
    pub fn from_bytes(bytes: impl Into<Box<[u8]>>) -> Self {
        Self(bytes.into())
    }

    /// Return the offset bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for StreamOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StreamOffset").field(&self.0).finish()
    }
}

/// An opaque, byte-lexicographically ordered writer sequence.
///
/// The writer supplies it. The store compares it with the last accepted value for the stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamSeq(Box<[u8]>);

impl StreamSeq {
    /// Build a sequence value from its bytes.
    #[must_use]
    pub fn from_bytes(bytes: impl Into<Box<[u8]>>) -> Self {
        Self(bytes.into())
    }

    /// Return the sequence bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for StreamSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StreamSeq").field(&self.0).finish()
    }
}

/// A parsed media type.
///
/// The type and subtype compare case-insensitively. Parameters are ignored. The default is
/// `application/octet-stream`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ContentType(Box<str>);

impl ContentType {
    /// Parse a media type. Parameters are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Header`] when the value has no
    /// type, no subtype or a control character.
    pub fn parse(raw: &str) -> Result<Self, StoreError> {
        let media = raw.split(';').next().unwrap_or_default().trim();
        let Some((ty, sub)) = media.split_once('/') else {
            return Err(StoreError::BadRequest(BadRequestKind::Header));
        };
        if ty.is_empty() || sub.is_empty() {
            return Err(StoreError::BadRequest(BadRequestKind::Header));
        }
        if media.chars().any(char::is_control) {
            return Err(StoreError::BadRequest(BadRequestKind::Header));
        }
        Ok(Self(media.to_ascii_lowercase().into_boxed_str()))
    }

    /// Return the normalized media type.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the stream uses JSON message boundaries.
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.0.as_ref() == "application/json"
    }
}

impl Default for ContentType {
    fn default() -> Self {
        Self("application/octet-stream".into())
    }
}

impl fmt::Debug for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentType").field(&self.0).finish()
    }
}

/// A validated stream path.
///
/// It has no leading or trailing slash, no empty segment and no `.` or `..` segment. It rejects
/// control characters, whitespace, `?`, `#` and `\`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamName(Box<str>);

impl StreamName {
    /// Validate and build a stream name.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Header`] for an invalid path.
    pub fn parse(raw: &str) -> Result<Self, StoreError> {
        let name = raw.trim();
        if name.is_empty() || name.starts_with('/') || name.ends_with('/') {
            return Err(StoreError::BadRequest(BadRequestKind::Header));
        }
        for segment in name.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(StoreError::BadRequest(BadRequestKind::Header));
            }
            if segment
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '?' | '#' | '\\'))
            {
                return Err(StoreError::BadRequest(BadRequestKind::Header));
            }
        }
        Ok(Self(name.into()))
    }

    /// Return the stream path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StreamName").field(&self.0).finish()
    }
}

/// A non-empty producer identifier.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProducerId(Box<str>);

impl ProducerId {
    /// Validate and build a producer identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Producer`] when the value is
    /// empty or has a control character.
    pub fn parse(raw: &str) -> Result<Self, StoreError> {
        if raw.is_empty() || raw.chars().any(char::is_control) {
            return Err(StoreError::BadRequest(BadRequestKind::Producer));
        }
        Ok(Self(raw.into()))
    }

    /// Return the producer identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProducerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProducerId").field(&self.0).finish()
    }
}

/// A delivery unit from a stream.
///
/// For a JSON stream, one chunk is one message. For any other stream, concatenation of chunks is
/// byte-exact.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Chunk(Box<[u8]>);

impl Chunk {
    /// Build a chunk from bytes.
    #[must_use]
    pub fn new(bytes: impl Into<Box<[u8]>>) -> Self {
        Self(bytes.into())
    }

    /// Return the chunk bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return the chunk length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when the chunk has no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Chunk").field(&self.0).finish()
    }
}

/// Where a read starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFrom {
    /// The beginning of the stream. This is the protocol's `-1`.
    Start,
    /// A stored resume cursor.
    Offset(StreamOffset),
    /// The current tail. This is the protocol's `now`.
    Tail,
}

/// A bounded read request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadRequest {
    /// Where the read starts.
    pub from: ReadFrom,
    /// The page limit.
    pub limit: ReadLimit,
}

/// The page limit for one read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadLimit {
    /// Maximum bytes in one page.
    pub max_bytes: Option<usize>,
    /// Maximum chunks in one page.
    pub max_chunks: Option<usize>,
}

impl ReadLimit {
    /// Build a page limit. `None` means the store default.
    #[must_use]
    pub fn new(max_bytes: Option<usize>, max_chunks: Option<usize>) -> Self {
        Self {
            max_bytes,
            max_chunks,
        }
    }
}

/// The resume state a read returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeCursor {
    /// The offset to pass on the next read.
    pub next: StreamOffset,
    /// True when the page has all data available at response time.
    pub up_to_date: bool,
    /// True when the page reaches the final offset of a closed stream.
    pub closed: bool,
}

/// One bounded page from a read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadPage {
    /// The page chunks.
    pub chunks: Vec<Chunk>,
    /// The resume state after this page.
    pub cursor: ResumeCursor,
}

/// A stream expiry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expiry {
    /// A sliding time window. Each read and write resets it.
    Ttl(core::time::Duration),
    /// An absolute RFC 3339 deadline.
    Absolute(Box<str>),
}

impl Expiry {
    /// Build an expiry from the two request headers.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Header`] when both values are
    /// present, or when the expiry string is empty.
    pub fn from_headers(
        ttl: Option<core::time::Duration>,
        expires_at: Option<&str>,
    ) -> Result<Option<Self>, StoreError> {
        match (ttl, expires_at) {
            (Some(_), Some(_)) | (None, Some("")) => {
                Err(StoreError::BadRequest(BadRequestKind::Header))
            }
            (Some(ttl), None) => Ok(Some(Self::Ttl(ttl))),
            (None, Some(at)) => Ok(Some(Self::Absolute(at.into()))),
            (None, None) => Ok(None),
        }
    }
}

/// The metadata a `HEAD` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamHead {
    /// The stream content type.
    pub content_type: ContentType,
    /// The tail offset.
    pub tail: StreamOffset,
    /// True when the stream is closed.
    pub closed: bool,
    /// The stream expiry.
    pub expires: Option<Expiry>,
}

/// The specification for stream creation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreateSpec {
    /// The content type. `None` inherits the fork source or uses the default.
    pub content_type: Option<ContentType>,
    /// The expiry.
    pub expires: Option<Expiry>,
    /// True when the stream starts closed.
    pub closed: bool,
    /// Optional initial content.
    pub initial: Option<Chunk>,
    /// Optional fork source.
    pub fork: Option<ForkSpec>,
}

/// A fork source and divergence point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSpec {
    /// The source stream.
    pub source: StreamName,
    /// The divergence offset. `None` means the source tail.
    pub at: Option<StreamOffset>,
    /// The sub-position past the divergence offset. `0` means absent.
    pub sub_offset: u32,
}

/// A producer identity and position in one epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Producer {
    /// The producer identifier.
    pub id: ProducerId,
    /// The producer epoch.
    pub epoch: u64,
    /// The sequence within the epoch.
    pub seq: u64,
}

/// One append request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendRequest {
    /// The appended chunk.
    pub chunk: Chunk,
    /// The request content type. `None` is valid only for a close-only request.
    pub content_type: Option<ContentType>,
    /// The optional application writer sequence.
    pub stream_seq: Option<StreamSeq>,
    /// The optional producer triple.
    pub producer: Option<Producer>,
    /// True to close the stream after this append.
    pub close: bool,
}

impl AppendRequest {
    /// Build a plain append for one chunk.
    #[must_use]
    pub fn new(chunk: Chunk) -> Self {
        Self {
            chunk,
            content_type: None,
            stream_seq: None,
            producer: None,
            close: false,
        }
    }
}

/// Whether an append created data or was a duplicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppendKind {
    /// The append created data.
    Created,
    /// The append was a duplicate. The data already existed.
    Duplicate,
}

/// The result of an append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendOutcome {
    /// Created or duplicate.
    pub kind: AppendKind,
    /// The resume state after the append.
    pub cursor: ResumeCursor,
    /// The highest accepted producer sequence.
    pub producer_seq: Option<u64>,
    /// True when the stream is closed.
    pub closed: bool,
}

/// The result of a create.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOutcome {
    /// True for `201`, false for an idempotent `200`.
    pub created: bool,
    /// The stream head after creation.
    pub head: StreamHead,
}

/// A close request with an optional final chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseRequest {
    /// The final chunk. `None` is a close-only request.
    pub final_chunk: Option<Chunk>,
    /// The optional producer triple.
    pub producer: Option<Producer>,
}

/// The result of a delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeleteOutcome {
    /// True when the stream was soft-deleted because forks still reference it.
    pub soft_deleted: bool,
}

/// The internal durability level for an append.
///
/// This is configuration. It never changes a wire response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckLevel {
    /// Durable at the tail.
    Accepted,
    /// Durable in a sealed segment.
    Sealed,
}

/// A protocol refusal.
#[derive(Debug)]
pub enum StoreError {
    /// The stream does not exist.
    NotFound,
    /// The stream is soft-deleted.
    Gone,
    /// Retention dropped the requested offset.
    Trimmed,
    /// A stale producer epoch wrote.
    Fenced {
        /// The current server epoch.
        current_epoch: u64,
    },
    /// A new epoch did not start at `seq = 0`.
    InvalidEpochSeq {
        /// The rejected sequence.
        seq: u64,
    },
    /// A producer sequence gap.
    SequenceGap {
        /// The expected sequence.
        expected: u64,
        /// The received sequence.
        received: u64,
    },
    /// A `Stream-Seq` value at or below the last accepted value.
    StreamSeqConflict {
        /// The last accepted value, when one exists.
        last: Option<StreamSeq>,
    },
    /// The request content type differs from the stream content type.
    ContentTypeMismatch {
        /// The stream content type.
        expected: ContentType,
        /// The request content type.
        got: ContentType,
    },
    /// A create request differs from the existing stream configuration.
    ConfigConflict,
    /// An append to a closed stream.
    Closed {
        /// The final offset of the closed stream.
        tail: StreamOffset,
    },
    /// A malformed header, body, offset or producer request.
    BadRequest(BadRequestKind),
    /// The caller exceeded a rate limit.
    RateLimited {
        /// The server request delay.
        retry_after: Option<core::time::Duration>,
    },
    /// The operation is not supported for this stream.
    Unsupported,
    /// The backend failed.
    Storage(Box<dyn core::error::Error + Send + Sync + 'static>),
}

/// The kind of malformed request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadRequestKind {
    /// A malformed header.
    Header,
    /// A malformed body.
    Body,
    /// A malformed offset.
    Offset,
    /// Invalid JSON.
    Json,
    /// A malformed producer request.
    Producer,
    /// A malformed fork request.
    Fork,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("stream not found"),
            Self::Gone => f.write_str("stream is gone"),
            Self::Trimmed => f.write_str("offset was trimmed"),
            Self::Fenced { current_epoch } => {
                write!(f, "stale producer epoch, current epoch {current_epoch}")
            }
            Self::InvalidEpochSeq { seq } => {
                write!(f, "new epoch must start at sequence 0, got {seq}")
            }
            Self::SequenceGap { expected, received } => {
                write!(
                    f,
                    "producer sequence gap, expected {expected}, got {received}"
                )
            }
            Self::StreamSeqConflict { .. } => f.write_str("stream sequence conflict"),
            Self::ContentTypeMismatch { expected, got } => {
                write!(
                    f,
                    "content type mismatch, expected {expected:?}, got {got:?}"
                )
            }
            Self::ConfigConflict => f.write_str("stream configuration mismatch"),
            Self::Closed { .. } => f.write_str("stream is closed"),
            Self::BadRequest(kind) => write!(f, "bad request: {kind:?}"),
            Self::RateLimited { .. } => f.write_str("rate limited"),
            Self::Unsupported => f.write_str("operation not supported"),
            Self::Storage(error) => write!(f, "storage error: {error}"),
        }
    }
}

impl core::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// A readiness signal for a stream append.
pub trait Notifier: Send + Sync {
    /// Register a one-shot callback. The producer fires it on the next append or close.
    fn register(&self, cb: Box<dyn Fn() + Send + 'static>);

    /// Signal that new data is available.
    fn notify(&self);
}

/// A shared notifier.
pub type NotifierRef = Arc<dyn Notifier>;

/// A boxed VFS operation.
pub type VfsFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, VfsError>> + Send + 'a>>;

/// The metadata the VFS stores for one stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsMeta {
    /// The stream content type.
    pub content_type: ContentType,
    /// The stream length in bytes.
    pub len: u64,
    /// True when the stream is closed.
    pub closed: bool,
    /// The stream expiry.
    pub expires: Option<Expiry>,
}

/// A byte position in the VFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VfsOffset(pub u64);

impl VfsOffset {
    /// Build an offset.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the offset value.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// One VFS entry from a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsEntry {
    /// The stream name.
    pub name: StreamName,
    /// The stream metadata.
    pub meta: VfsMeta,
}

/// A VFS refusal.
#[derive(Debug)]
pub enum VfsError {
    /// The entry does not exist.
    NotFound,
    /// The entry exists with a different configuration.
    Conflict,
    /// The VFS does not support the operation.
    Unsupported,
    /// The storage layer failed.
    Io(Box<dyn core::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("vfs entry not found"),
            Self::Conflict => f.write_str("vfs entry conflict"),
            Self::Unsupported => f.write_str("vfs operation not supported"),
            Self::Io(error) => write!(f, "vfs io error: {error}"),
        }
    }
}

impl core::error::Error for VfsError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<VfsError> for StoreError {
    fn from(error: VfsError) -> Self {
        match error {
            VfsError::NotFound => Self::NotFound,
            VfsError::Conflict => Self::ConfigConflict,
            VfsError::Unsupported => Self::Unsupported,
            VfsError::Io(error) => Self::Storage(error),
        }
    }
}

impl From<VfsMeta> for StreamHead {
    fn from(meta: VfsMeta) -> Self {
        Self {
            content_type: meta.content_type,
            tail: StreamOffset::from(VfsOffset(meta.len)),
            closed: meta.closed,
            expires: meta.expires,
        }
    }
}

impl From<VfsOffset> for StreamOffset {
    fn from(offset: VfsOffset) -> Self {
        Self::from_bytes(offset.0.to_be_bytes())
    }
}

impl From<&StreamOffset> for VfsOffset {
    fn from(offset: &StreamOffset) -> Self {
        let bytes = offset.as_bytes();
        let take = bytes.len().min(8);
        let mut buf = [0_u8; 8];
        let start = 8_usize.saturating_sub(take);
        buf[start..].copy_from_slice(&bytes[bytes.len() - take..]);
        Self(u64::from_be_bytes(buf))
    }
}

/// The swappable storage layer under [`DurableStream`].
///
/// The VFS owns bytes, metadata and the append notifier. It has no transport, runtime or
/// protocol state. Its methods return boxed futures, so the trait is object-safe.
pub trait DurableStreamVfs: Send + Sync {
    /// Create an entry. Returns `true` when it is new, `false` when it already matches.
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool>;

    /// Append bytes and return the new length.
    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset>;

    /// Read up to `limit` bytes at an offset.
    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>>;

    /// Return the entry metadata.
    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta>;

    /// Mark the entry closed.
    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()>;

    /// Remove the entry.
    fn remove(&self, name: &StreamName) -> VfsFuture<'_, ()>;

    /// List entries under a prefix.
    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>>;

    /// Return the append notifier, when the VFS supports one.
    fn notifier(&self, name: &StreamName) -> Option<NotifierRef>;
}

/// The execution-context driver for the server core.
///
/// A driver supplies the store and the append notifier. Embedded, HTTP, WebSocket, DO and
/// Session contexts implement this trait. The business API takes a driver.
pub trait DurableDriver: Send + Sync {
    /// The store for this context.
    type Store: DurableStreamStore;

    /// Return the store.
    fn store(&self) -> &Self::Store;

    /// Return the append notifier for a stream, when the context supports one.
    fn notifier(&self, name: &StreamName) -> Option<NotifierRef>;
}

/// The storage contract for the Durable Streams protocol core.
///
/// The store owns offsets, producer state, `Stream-Seq` state, TTL access time, fork state and
/// the append notifier. It owns no transport, status code or header.
pub trait DurableStreamStore {
    /// Create a stream. Idempotent for a matching configuration.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the stream exists with a different configuration, the fork
    /// source is missing, or the backend fails.
    async fn create(&self, name: StreamName, spec: CreateSpec)
        -> Result<CreateOutcome, StoreError>;

    /// Return stream metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotFound`] or [`StoreError::Gone`].
    async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError>;

    /// Append a chunk.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] for a closed stream, a producer refusal, a `Stream-Seq`
    /// regression, a content-type mismatch or a backend failure.
    async fn append(
        &self,
        name: &StreamName,
        req: AppendRequest,
    ) -> Result<AppendOutcome, StoreError>;

    /// Read a bounded page.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Trimmed`] for a dropped offset or [`StoreError::NotFound`].
    async fn read(&self, name: &StreamName, req: ReadRequest) -> Result<ReadPage, StoreError>;

    /// Close a stream, optionally with a final chunk.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] for a conflicting close or [`StoreError::NotFound`].
    async fn close(
        &self,
        name: &StreamName,
        req: CloseRequest,
    ) -> Result<AppendOutcome, StoreError>;

    /// Delete a stream. A stream with forks is soft-deleted.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotFound`] or [`StoreError::Gone`].
    async fn delete(&self, name: &StreamName) -> Result<DeleteOutcome, StoreError>;

    /// Create a fork.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a missing source, an invalid offset or a config conflict.
    async fn fork(&self, name: StreamName, spec: ForkSpec) -> Result<CreateOutcome, StoreError>;

    /// Return the append notifier for a stream, when the backend supports one.
    fn notifier(&self, name: &StreamName) -> Option<NotifierRef>;
}

/// The default protocol store. It wraps a boxed VFS.
///
/// Swap the VFS to change storage. The protocol core and `Ctx` do not change.
#[derive(Clone)]
pub struct DurableStream {
    vfs: Arc<dyn DurableStreamVfs>,
}

impl DurableStream {
    /// Build a stream store over a VFS.
    #[must_use]
    pub fn new(vfs: Arc<dyn DurableStreamVfs>) -> Self {
        Self { vfs }
    }

    /// Return the VFS.
    #[must_use]
    pub fn vfs(&self) -> &Arc<dyn DurableStreamVfs> {
        &self.vfs
    }
}

impl DurableDriver for DurableStream {
    type Store = Self;

    fn store(&self) -> &Self::Store {
        self
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.vfs.notifier(name)
    }
}

impl DurableStreamStore for DurableStream {
    async fn create(
        &self,
        name: StreamName,
        spec: CreateSpec,
    ) -> Result<CreateOutcome, StoreError> {
        let CreateSpec {
            content_type,
            expires,
            closed,
            initial,
            fork,
        } = spec;
        if fork.is_some() {
            return Err(StoreError::Unsupported);
        }

        let meta = VfsMeta {
            content_type: content_type.unwrap_or_default(),
            len: 0,
            closed,
            expires,
        };
        let created = self.vfs.create(&name, meta).await?;
        if let Some(initial) = initial {
            self.vfs.append(&name, initial.as_bytes()).await?;
            if closed {
                self.vfs.close(&name).await?;
            }
        }
        let head = self.vfs.meta(&name).await?;
        Ok(CreateOutcome {
            created,
            head: head.into(),
        })
    }

    async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError> {
        self.vfs
            .meta(name)
            .await
            .map(StreamHead::from)
            .map_err(StoreError::from)
    }

    async fn append(
        &self,
        name: &StreamName,
        req: AppendRequest,
    ) -> Result<AppendOutcome, StoreError> {
        let AppendRequest {
            chunk,
            content_type,
            producer,
            close,
            ..
        } = req;

        let before = self.vfs.meta(name).await?;
        if before.closed {
            return Err(StoreError::Closed {
                tail: StreamOffset::from(VfsOffset(before.len)),
            });
        }
        if let Some(content_type) = &content_type {
            if content_type != &before.content_type {
                return Err(StoreError::ContentTypeMismatch {
                    expected: before.content_type,
                    got: content_type.clone(),
                });
            }
        }

        self.vfs.append(name, chunk.as_bytes()).await?;
        if close {
            self.vfs.close(name).await?;
        }
        let after = self.vfs.meta(name).await?;
        Ok(AppendOutcome {
            kind: AppendKind::Created,
            cursor: ResumeCursor {
                next: StreamOffset::from(VfsOffset(after.len)),
                up_to_date: true,
                closed: after.closed,
            },
            producer_seq: producer.as_ref().map(|producer| producer.seq),
            closed: after.closed,
        })
    }

    async fn read(&self, name: &StreamName, req: ReadRequest) -> Result<ReadPage, StoreError> {
        let meta = self.vfs.meta(name).await?;
        let start = match req.from {
            ReadFrom::Start => VfsOffset(0),
            ReadFrom::Offset(ref offset) => VfsOffset::from(offset),
            ReadFrom::Tail => VfsOffset(meta.len),
        };
        let limit = req.limit.max_bytes.unwrap_or(usize::MAX);
        let bytes = self.vfs.read_at(name, &start, limit).await?;
        let end = start.0.saturating_add(bytes.len() as u64).min(meta.len);
        let chunks = if bytes.is_empty() {
            Vec::new()
        } else {
            vec![Chunk::new(bytes)]
        };
        Ok(ReadPage {
            chunks,
            cursor: ResumeCursor {
                next: StreamOffset::from(VfsOffset(end)),
                up_to_date: end == meta.len,
                closed: meta.closed && end == meta.len,
            },
        })
    }

    async fn close(
        &self,
        name: &StreamName,
        req: CloseRequest,
    ) -> Result<AppendOutcome, StoreError> {
        let before = self.vfs.meta(name).await?;
        if let Some(chunk) = req.final_chunk {
            self.vfs.append(name, chunk.as_bytes()).await?;
        }
        self.vfs.close(name).await?;
        let after = self.vfs.meta(name).await?;
        Ok(AppendOutcome {
            kind: if before.closed {
                AppendKind::Duplicate
            } else {
                AppendKind::Created
            },
            cursor: ResumeCursor {
                next: StreamOffset::from(VfsOffset(after.len)),
                up_to_date: true,
                closed: true,
            },
            producer_seq: req.producer.as_ref().map(|producer| producer.seq),
            closed: true,
        })
    }

    async fn delete(&self, name: &StreamName) -> Result<DeleteOutcome, StoreError> {
        self.vfs.remove(name).await?;
        Ok(DeleteOutcome {
            soft_deleted: false,
        })
    }

    #[allow(clippy::unused_async)] // Matches the trait; the VFS fork lands in feature 04.
    async fn fork(&self, _name: StreamName, _spec: ForkSpec) -> Result<CreateOutcome, StoreError> {
        Err(StoreError::Unsupported)
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.vfs.notifier(name)
    }
}

/// The business API for the protocol core.
///
/// A caller names a stream and runs the use case. The driver decides the runtime and storage.
pub struct DurableStreams<D: DurableDriver> {
    driver: D,
}

impl<D: DurableDriver> DurableStreams<D> {
    /// Build the business API on a driver.
    #[must_use]
    pub fn new(driver: D) -> Self {
        Self { driver }
    }

    /// Return the driver.
    #[must_use]
    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Open a stream handle.
    #[must_use]
    pub fn stream(&self, name: StreamName) -> StreamRef<'_, D> {
        StreamRef {
            streams: self,
            name,
        }
    }
}

/// A stream handle from the business API.
pub struct StreamRef<'a, D: DurableDriver> {
    streams: &'a DurableStreams<D>,
    name: StreamName,
}

impl<D: DurableDriver> StreamRef<'_, D> {
    /// Return the stream name.
    #[must_use]
    pub fn name(&self) -> &StreamName {
        &self.name
    }

    /// Create the stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn create(&self, spec: CreateSpec) -> Result<CreateOutcome, StoreError> {
        self.streams
            .driver
            .store()
            .create(self.name.clone(), spec)
            .await
    }

    /// Return the stream head.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn head(&self) -> Result<StreamHead, StoreError> {
        self.streams.driver.store().head(&self.name).await
    }

    /// Append a chunk.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn append(&self, req: AppendRequest) -> Result<AppendOutcome, StoreError> {
        self.streams.driver.store().append(&self.name, req).await
    }

    /// Read a page.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn read(&self, req: ReadRequest) -> Result<ReadPage, StoreError> {
        self.streams.driver.store().read(&self.name, req).await
    }

    /// Close the stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn close(&self, req: CloseRequest) -> Result<AppendOutcome, StoreError> {
        self.streams.driver.store().close(&self.name, req).await
    }

    /// Delete the stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn delete(&self) -> Result<DeleteOutcome, StoreError> {
        self.streams.driver.store().delete(&self.name).await
    }

    /// Create a fork from this stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    pub async fn fork(&self, spec: ForkSpec) -> Result<CreateOutcome, StoreError> {
        self.streams
            .driver
            .store()
            .fork(self.name.clone(), spec)
            .await
    }
}
