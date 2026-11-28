//! The immutable block format and the block store contract.
//!
//! The batcher writes one block per batch. A block is an index of chunks
//! followed by their bytes. The index lets a reader fetch only the byte range
//! it needs. A block never changes after its PUT.
//!
//! Block layout, little endian:
//!
//! | Part | Bytes |
//! |---|---|
//! | magic `DSBLK001` | 8 |
//! | chunk count (`u32`) | 4 |
//! | one entry per chunk: `seq` (`u64`), payload offset (`u64`), length (`u64`) | 24 each |
//! | payload bytes, in chunk order | remainder |
//!
//! The `seq` of a chunk is its start offset in the stream's own bytes. A fork
//! counts its own bytes from zero; the inherited prefix lives in the source.

use super::super::StreamName;
use core::fmt;
use core::future::Future;
use core::pin::Pin;

/// The magic bytes at the start of every block.
pub const BLOCK_MAGIC: [u8; 8] = *b"DSBLK001";

/// The fixed header length: magic and chunk count.
pub const BLOCK_HEADER_BYTES: u64 = 12;

/// The length of one index entry.
pub const BLOCK_ENTRY_BYTES: u64 = 24;

/// The default key prefix. Feature 09 makes the object root configuration.
pub const DEFAULT_BLOCK_PREFIX: &str = "durable";

/// A boxed block store operation.
pub type BlockFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BlockError>> + Send + 'a>>;

/// A block store refusal.
#[derive(Debug)]
pub enum BlockError {
    /// The block is absent. A live block must never be missing.
    NotFound,
    /// The block bytes do not follow this module's format.
    Invalid(&'static str),
    /// The store failed.
    Io(Box<dyn core::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("block not found"),
            Self::Invalid(reason) => write!(f, "invalid block: {reason}"),
            Self::Io(error) => write!(f, "block store error: {error}"),
        }
    }
}

impl core::error::Error for BlockError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// An immutable object store for blocks.
///
/// The key is deterministic, so a retried PUT writes the same bytes to the same
/// key. A range GET returns exactly `len` bytes. A short or missing block is an
/// error, never zero bytes.
pub trait BlockStore: Send + Sync {
    /// Write one immutable block. A retry with the same key and bytes is safe.
    fn put(&self, key: &str, bytes: Vec<u8>) -> BlockFuture<'_, ()>;

    /// Read `len` bytes at `start` from one block.
    fn get_range(&self, key: &str, start: u64, len: usize) -> BlockFuture<'_, Vec<u8>>;
}

/// One chunk position inside a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockEntry {
    /// The chunk start offset in the stream's own bytes.
    pub seq: u64,
    /// The chunk start inside the block payload.
    pub payload_offset: u64,
    /// The chunk length.
    pub len: u64,
}

impl BlockEntry {
    /// The end offset of the chunk in the stream's own bytes.
    #[must_use]
    pub fn end_seq(self) -> u64 {
        self.seq.saturating_add(self.len)
    }
}

/// A delivered block as the metadata record lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockIndexEntry {
    /// The first stream byte offset in the block.
    pub first_seq: u64,
    /// The byte offset after the last byte in the block.
    pub last_seq: u64,
    /// The deterministic object key.
    pub key: String,
}

/// Build the deterministic key for one block.
#[must_use]
pub fn block_key(prefix: &str, stream: &StreamName, first_seq: u64, last_seq: u64) -> String {
    format!(
        "{prefix}/{}/{first_seq}-{last_seq}",
        hex(stream.as_str().as_bytes())
    )
}

/// Encode one block from chunks in stream order.
///
/// # Errors
///
/// Returns [`BlockError::Invalid`] when the chunk count or a chunk length does
/// not fit the block format.
pub fn encode_block(chunks: &[(u64, &[u8])]) -> Result<Vec<u8>, BlockError> {
    let count = u32::try_from(chunks.len())
        .map_err(|_| BlockError::Invalid("too many chunks for one block"))?;
    let payload_len: usize = chunks.iter().map(|(_, bytes)| bytes.len()).sum();
    let index_len = chunks
        .len()
        .checked_mul(24)
        .ok_or(BlockError::Invalid("block index overflows"))?;
    let capacity = 12_usize
        .checked_add(index_len)
        .and_then(|value| value.checked_add(payload_len))
        .ok_or(BlockError::Invalid("block overflows"))?;

    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(&BLOCK_MAGIC);
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 0_u64;
    for (seq, bytes) in chunks {
        let len = u64::try_from(bytes.len())
            .map_err(|_| BlockError::Invalid("chunk length overflows"))?;
        out.extend_from_slice(&seq.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        offset = offset.saturating_add(len);
    }
    for (_, bytes) in chunks {
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

/// Read the chunk count from a block header.
///
/// # Errors
///
/// Returns [`BlockError::Invalid`] when the header is short or the magic is
/// wrong.
pub fn read_header(header: &[u8]) -> Result<u32, BlockError> {
    if header.len() < 12 {
        return Err(BlockError::Invalid("short block header"));
    }
    if header.get(..8) != Some(BLOCK_MAGIC.as_slice()) {
        return Err(BlockError::Invalid("bad block magic"));
    }
    let mut count = [0_u8; 4];
    count.copy_from_slice(&header[8..12]);
    Ok(u32::from_le_bytes(count))
}

/// Decode the chunk index of a block.
///
/// # Errors
///
/// Returns [`BlockError::Invalid`] when the index is truncated.
pub fn decode_index(index: &[u8], count: u32) -> Result<Vec<BlockEntry>, BlockError> {
    let count = usize::try_from(count).map_err(|_| BlockError::Invalid("chunk count overflows"))?;
    let expected = count
        .checked_mul(24)
        .ok_or(BlockError::Invalid("block index overflows"))?;
    if index.len() != expected {
        return Err(BlockError::Invalid("short block index"));
    }
    let mut entries = Vec::with_capacity(count);
    let (chunks, remainder) = index.as_chunks::<24>();
    if !remainder.is_empty() {
        return Err(BlockError::Invalid("short block index"));
    }
    for chunk in chunks {
        let mut seq = [0_u8; 8];
        let mut offset = [0_u8; 8];
        let mut len = [0_u8; 8];
        seq.copy_from_slice(&chunk[0..8]);
        offset.copy_from_slice(&chunk[8..16]);
        len.copy_from_slice(&chunk[16..24]);
        entries.push(BlockEntry {
            seq: u64::from_le_bytes(seq),
            payload_offset: u64::from_le_bytes(offset),
            len: u64::from_le_bytes(len),
        });
    }
    Ok(entries)
}

/// The payload range one read needs from a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadRange {
    /// The start offset inside the block payload.
    pub start: u64,
    /// The length to fetch.
    pub len: u64,
}

/// Compute the payload range that covers `[start, end)` in stream bytes.
///
/// Returns `None` when the block holds no byte in that range.
#[must_use]
pub fn payload_range(entries: &[BlockEntry], start: u64, end: u64) -> Option<PayloadRange> {
    let mut first: Option<u64> = None;
    let mut last = 0_u64;
    for entry in entries {
        if entry.end_seq() <= start || entry.seq >= end {
            continue;
        }
        let chunk_start = entry
            .payload_offset
            .saturating_add(start.saturating_sub(entry.seq));
        let chunk_end = entry
            .payload_offset
            .saturating_add(end.min(entry.end_seq()).saturating_sub(entry.seq));
        first = Some(first.map_or(chunk_start, |value| value.min(chunk_start)));
        last = last.max(chunk_end);
    }
    first.map(|start| PayloadRange {
        start,
        len: last.saturating_sub(start),
    })
}

/// Copy the requested stream bytes out of one block payload.
///
/// `payload_start` is the payload offset of the first byte in `payload`.
#[must_use]
pub fn extract_range(
    entries: &[BlockEntry],
    payload: &[u8],
    payload_start: u64,
    start: u64,
    end: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let overlap_start = start.max(entry.seq);
        let overlap_end = end.min(entry.end_seq());
        if overlap_start >= overlap_end {
            continue;
        }
        let from = entry
            .payload_offset
            .saturating_add(overlap_start.saturating_sub(entry.seq))
            .saturating_sub(payload_start);
        let to = entry
            .payload_offset
            .saturating_add(overlap_end.saturating_sub(entry.seq))
            .saturating_sub(payload_start);
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            continue;
        };
        if to <= payload.len() && from < to {
            out.extend_from_slice(&payload[from..to]);
        }
    }
    out
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
