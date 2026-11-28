//! The head manifest, the object keys and the metadata record format.
//!
//! # Metadata record format
//!
//! The head manifest is a text record. Its first lines are exactly the record
//! that the file virtual file system (VFS) writes in its `meta` file:
//! `content_type`, `len`, `closed`, `expires`, `last_access_ms`, `trim_from`,
//! `version`, `ref_count` and `soft_deleted`, then `fork`, `producer` and
//! `stream_seq`. A tab separates the fields. The manifest adds `published_own`,
//! `format` and one `segment` line for each published object. A reader of the
//! file format ignores the extra lines. This keeps one metadata format for both
//! VFS implementations.
//!
//! A manifest is the compare-and-swap (CAS) target. Its `version` is the
//! metadata version. The store entity tag (`ETag`) is a separate transport
//! value.

use super::object_store::DEFAULT_OBJECT_ROOT;
use super::ObjectLayout;
use foundation_core::durable::{
    ContentType, Expiry, ForkInfo, ProducerEpochState, ProducerId, StreamName, StreamSeq, VfsError,
    VfsMeta, WriterState,
};
use std::collections::BTreeMap;
use std::fmt;

/// The manifest format label.
pub const HEAD_FORMAT: &str = "s3-vfs-1";

/// One published segment object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentEntry {
    /// The first stream byte offset in the segment.
    pub at: u64,
    /// The segment length in bytes.
    pub len: u64,
    /// The object key.
    pub key: String,
    /// The batch identity: a content hash of the batch bytes.
    pub batch: String,
}

impl SegmentEntry {
    /// The byte offset after the segment.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.at.saturating_add(self.len)
    }

    /// True when the segment covers `at`.
    #[must_use]
    pub fn covers(&self, at: u64) -> bool {
        self.at <= at && at < self.end()
    }
}

/// The head manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeadManifest {
    /// The stream content type.
    pub content_type: Option<ContentType>,
    /// The published stream length in bytes, including a fork prefix.
    pub len: u64,
    /// True when the stream is closed.
    pub closed: bool,
    /// The stream expiry.
    pub expires: Option<Expiry>,
    /// The last read or write time in UNIX milliseconds.
    pub last_access_ms: i64,
    /// The first readable byte after retention.
    pub trim_from: u64,
    /// The metadata version.
    pub version: u64,
    /// The number of forks that reference this stream.
    pub ref_count: u32,
    /// True when the stream is soft-deleted.
    pub soft_deleted: bool,
    /// The fork source and inherited prefix length.
    pub fork: Option<ForkInfo>,
    /// The producer state.
    pub producers: BTreeMap<ProducerId, ProducerEpochState>,
    /// The last accepted `Stream-Seq`.
    pub stream_seq: Option<StreamSeq>,
    /// The published segments, in stream order.
    pub segments: Vec<SegmentEntry>,
    /// The inherited prefix length plus the published own bytes.
    pub published_own: u64,
}

impl HeadManifest {
    /// Build a manifest from the metadata of a new stream.
    #[must_use]
    pub fn from_meta(meta: &VfsMeta) -> Self {
        let base = meta.fork.as_ref().map_or(0, |fork| fork.at);
        Self {
            content_type: Some(meta.content_type.clone()),
            len: meta.len,
            closed: meta.closed,
            expires: meta.expires.clone(),
            last_access_ms: meta.last_access_ms,
            trim_from: meta.trim_from,
            version: 0,
            ref_count: 0,
            soft_deleted: false,
            fork: meta.fork.clone(),
            producers: meta.writer.producers.clone(),
            stream_seq: meta.writer.stream_seq.clone(),
            segments: Vec::new(),
            published_own: base,
        }
    }

    /// Reduce the manifest to the metadata record.
    #[must_use]
    pub fn to_meta(&self) -> VfsMeta {
        VfsMeta {
            content_type: self.content_type.clone().unwrap_or_default(),
            len: self.len,
            closed: self.closed,
            expires: self.expires.clone(),
            writer: WriterState {
                stream_seq: self.stream_seq.clone(),
                producers: self.producers.clone(),
            },
            fork: self.fork.clone(),
            last_access_ms: self.last_access_ms,
            trim_from: self.trim_from,
            version: self.version,
        }
    }

    /// Apply the metadata fields that an append replaces.
    ///
    /// The length, the version, the fork, the retention point and the
    /// reference count stay under the publisher's control.
    pub fn apply_meta(&mut self, meta: &VfsMeta) {
        self.content_type = Some(meta.content_type.clone());
        self.closed = meta.closed;
        self.expires.clone_from(&meta.expires);
        self.last_access_ms = meta.last_access_ms;
        self.producers = meta.writer.producers.clone();
        self.stream_seq.clone_from(&meta.writer.stream_seq);
    }

    /// The inherited prefix length.
    #[must_use]
    pub fn base(&self) -> u64 {
        self.fork.as_ref().map_or(0, |fork| fork.at)
    }

    /// The published own bytes, measured from the fork base.
    #[must_use]
    pub fn own_len(&self) -> u64 {
        self.len.saturating_sub(self.base())
    }

    /// Render the manifest in the shared metadata record format.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        let expires = render_expiry(self.expires.as_ref());
        let _ = write!(
            out,
            "content_type\t{}\nlen\t{}\nclosed\t{}\nexpires\t{}\nlast_access_ms\t{}\ntrim_from\t{}\nversion\t{}\nref_count\t{}\nsoft_deleted\t{}\n",
            self.content_type.as_ref().map_or("", ContentType::as_str),
            self.len,
            u8::from(self.closed),
            expires,
            self.last_access_ms,
            self.trim_from,
            self.version,
            self.ref_count,
            u8::from(self.soft_deleted),
        );
        if let Some(fork) = &self.fork {
            let _ = writeln!(
                out,
                "fork\t{}\t{}",
                hex(fork.source.as_str().as_bytes()),
                fork.at
            );
        }
        for (producer, state) in &self.producers {
            let _ = writeln!(
                out,
                "producer\t{}\t{}\t{}",
                hex(producer.as_str().as_bytes()),
                state.epoch,
                state.last_seq
            );
        }
        if let Some(seq) = &self.stream_seq {
            let _ = writeln!(out, "stream_seq\t{}", hex(seq.as_bytes()));
        }
        let _ = writeln!(out, "published_own\t{}", self.published_own);
        let _ = writeln!(out, "format\t{HEAD_FORMAT}");
        for segment in &self.segments {
            let _ = writeln!(
                out,
                "segment\t{}\t{}\t{}\t{}",
                segment.at, segment.len, segment.key, segment.batch
            );
        }
        out
    }

    /// Parse a rendered manifest.
    ///
    /// # Errors
    ///
    /// Returns [`HeadError`] when a field is malformed.
    pub fn parse(text: &str) -> Result<Self, HeadError> {
        let mut manifest = Self::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let Some(key) = parts.next() else { continue };
            match key {
                "content_type" => {
                    let value = parts.next().ok_or_else(|| HeadError::new("content type"))?;
                    if !value.is_empty() {
                        manifest.content_type =
                            Some(ContentType::parse(value).map_err(|_| HeadError::new(value))?);
                    }
                }
                "len" => manifest.len = parse_number(parts.next(), "len")?,
                "closed" => manifest.closed = parts.next() == Some("1"),
                "expires" => {
                    manifest.expires = parse_expiry(parts.next().unwrap_or("none"))?;
                }
                "last_access_ms" => {
                    manifest.last_access_ms = parse_signed(parts.next(), "last_access_ms")?;
                }
                "trim_from" => manifest.trim_from = parse_number(parts.next(), "trim_from")?,
                "version" => manifest.version = parse_number(parts.next(), "version")?,
                "ref_count" => {
                    manifest.ref_count =
                        u32::try_from(parse_number::<u64>(parts.next(), "ref_count")?)
                            .map_err(|_| HeadError::new("ref_count"))?;
                }
                "soft_deleted" => manifest.soft_deleted = parts.next() == Some("1"),
                "published_own" => {
                    manifest.published_own = parse_number(parts.next(), "published_own")?;
                }
                "fork" => {
                    let source = parts.next().ok_or_else(|| HeadError::new("fork"))?;
                    let at = parse_number(parts.next(), "fork at")?;
                    let bytes = unhex(source).ok_or_else(|| HeadError::new("fork hex"))?;
                    let text = String::from_utf8(bytes).map_err(|_| HeadError::new("fork name"))?;
                    manifest.fork = Some(ForkInfo {
                        source: StreamName::parse(&text)
                            .map_err(|_| HeadError::new("fork name"))?,
                        at,
                    });
                }
                "producer" => {
                    let id = parts.next().ok_or_else(|| HeadError::new("producer"))?;
                    let epoch = parse_number(parts.next(), "producer epoch")?;
                    let last_seq = parse_number(parts.next(), "producer seq")?;
                    let bytes = unhex(id).ok_or_else(|| HeadError::new("producer hex"))?;
                    let text =
                        String::from_utf8(bytes).map_err(|_| HeadError::new("producer name"))?;
                    manifest.producers.insert(
                        ProducerId::parse(&text).map_err(|_| HeadError::new("producer name"))?,
                        ProducerEpochState { epoch, last_seq },
                    );
                }
                "stream_seq" => {
                    let value = parts.next().ok_or_else(|| HeadError::new("stream seq"))?;
                    let bytes = unhex(value).ok_or_else(|| HeadError::new("stream seq hex"))?;
                    manifest.stream_seq = Some(StreamSeq::from_bytes(bytes));
                }
                "segment" => {
                    let at = parse_number(parts.next(), "segment at")?;
                    let len = parse_number(parts.next(), "segment len")?;
                    let key = parts.next().ok_or_else(|| HeadError::new("segment key"))?;
                    let batch = parts
                        .next()
                        .ok_or_else(|| HeadError::new("segment batch"))?;
                    manifest.segments.push(SegmentEntry {
                        at,
                        len,
                        key: key.to_string(),
                        batch: batch.to_string(),
                    });
                }
                _ => {}
            }
        }
        manifest.segments.sort_by_key(|segment| segment.at);
        Ok(manifest)
    }
}

fn render_expiry(expires: Option<&Expiry>) -> String {
    match expires {
        None => "none".to_string(),
        Some(Expiry::Ttl(ttl)) => format!("ttl:{}", ttl.as_millis()),
        Some(Expiry::Absolute(at)) => format!("abs:{at}"),
    }
}

fn parse_expiry(value: &str) -> Result<Option<Expiry>, HeadError> {
    if value == "none" {
        return Ok(None);
    }
    if let Some(ms) = value.strip_prefix("ttl:") {
        return Ok(Some(Expiry::Ttl(core::time::Duration::from_millis(
            ms.parse().map_err(|_| HeadError::new("ttl"))?,
        ))));
    }
    if let Some(at) = value.strip_prefix("abs:") {
        return Ok(Some(Expiry::Absolute(
            at.parse().map_err(|_| HeadError::new("absolute expiry"))?,
        )));
    }
    Err(HeadError::new("expiry"))
}

fn parse_number<T: core::str::FromStr>(value: Option<&str>, field: &str) -> Result<T, HeadError> {
    value
        .ok_or_else(|| HeadError::new(field))?
        .parse()
        .map_err(|_| HeadError::new(field))
}

fn parse_signed<T: core::str::FromStr>(value: Option<&str>, field: &str) -> Result<T, HeadError> {
    parse_number(value, field)
}

/// A malformed metadata record.
#[derive(Debug)]
pub struct HeadError(String);

impl HeadError {
    fn new(field: &str) -> Self {
        Self(format!("bad head manifest field: {field}"))
    }
}

impl fmt::Display for HeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for HeadError {}

impl From<HeadError> for VfsError {
    fn from(error: HeadError) -> Self {
        Self::Io(Box::new(error))
    }
}

/// The key of a stream head.
#[must_use]
pub fn head_key(root: &str, name: &StreamName) -> String {
    format!(
        "{}/{}/HEAD",
        root.trim_end_matches('/'),
        hex(name.as_str().as_bytes())
    )
}

/// The key prefix of one stream.
#[must_use]
pub fn stream_prefix(root: &str, name: &StreamName) -> String {
    format!(
        "{}/{}/",
        root.trim_end_matches('/'),
        hex(name.as_str().as_bytes())
    )
}

/// The key of one segment object.
///
/// A segmented object carries the generation and both sequence bounds. An
/// append-only object carries the generation and the first sequence only. The
/// batch identity is a content hash, so a retry writes the same key.
#[must_use]
pub fn segment_key(
    root: &str,
    name: &StreamName,
    layout: ObjectLayout,
    first: u64,
    last: u64,
    batch: &str,
) -> String {
    let prefix = stream_prefix(root, name);
    match layout {
        ObjectLayout::Segmented => {
            format!("{prefix}seg/{first:020}-{last:020}-{batch}.log")
        }
        ObjectLayout::AppendOnly => format!("{prefix}log/{first:020}-{batch}.log"),
    }
}

/// The key of the lease object for a stream.
#[must_use]
pub fn lease_key(root: &str, name: &StreamName) -> String {
    format!("{}/LEASE", stream_prefix(root, name).trim_end_matches('/'))
}

/// The key prefix of the probe objects.
#[must_use]
pub fn probe_prefix(root: &str) -> String {
    format!("{}/.probe/", root.trim_end_matches('/'))
}

/// The default probe prefix.
#[must_use]
pub fn default_probe_prefix() -> String {
    probe_prefix(DEFAULT_OBJECT_ROOT)
}

/// Parse the stream name from a head key.
#[must_use]
pub fn parse_head_key(root: &str, key: &str) -> Option<StreamName> {
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let rest = key.strip_prefix(&prefix)?;
    let name_hex = rest.strip_suffix("/HEAD")?;
    let bytes = unhex(name_hex)?;
    let text = String::from_utf8(bytes).ok()?;
    StreamName::parse(&text).ok()
}

/// Parse the stream name from any object key under the root.
///
/// The key may be a head, a segment, a log chunk or a lease.
#[must_use]
pub fn parse_stream_key(root: &str, key: &str) -> Option<StreamName> {
    if let Some(name) = parse_head_key(root, key) {
        return Some(name);
    }
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let rest = key.strip_prefix(&prefix)?;
    let (name_hex, _) = rest.split_once('/')?;
    let bytes = unhex(name_hex)?;
    let text = String::from_utf8(bytes).ok()?;
    StreamName::parse(&text).ok()
}

/// True when a key names a segment or a log object.
// The VFS generates every key in lower case, so a case-insensitive extension
// comparison is not needed.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
#[must_use]
pub fn is_segment_key(key: &str) -> bool {
    key.ends_with(".log") && (key.contains("/seg/") || key.contains("/log/"))
}

/// A stable content hash for a batch identity.
///
/// The hash is 64-bit Fowler-Noll-Vo (FNV-1a). It is deterministic, so a retry
/// of the same bytes gets the same key. It is not a cryptographic hash.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.as_chunks::<2>().0 {
        let high = (chunk[0] as char).to_digit(16)?;
        let low = (chunk[1] as char).to_digit(16)?;
        out.push(u8::try_from((high << 4) | low).ok()?);
    }
    Some(out)
}
