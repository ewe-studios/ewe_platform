//! A Structured Query Language (SQL) backed Durable Streams virtual file system (VFS).
//!
//! One engine, two executors. [`SqlVfs`] implements [`DurableStreamVfs`] over any
//! [`SqlExecutor`]: Cloudflare D1 (`foundation_db::D1Executor`) and a Durable
//! Object (DO) SQLite store (`foundation_db::DoSqlExecutor`). The engine holds the
//! tables, the read path and the code that maintains the metadata record. The
//! executor only runs statements.
//!
//! # Tables
//!
//! | Table | Holds |
//! |---|---|
//! | `stream_meta` | one row per stream: the metadata record, the delivered sequence, the reference count, the soft-delete flag, the version and the delivered-block index |
//! | `pending_data` | appended chunks: stream, sequence, bytes, state (`pending` or `delivered`) and append time |
//!
//! # Hot path and cold tier
//!
//! An append writes the chunk row and the metadata row in one transaction. The
//! server acknowledges after the commit when [`Durability::Hot`] is set (the
//! default). A batcher groups pending chunks per stream into one immutable block
//! of at most [`SqlVfsConfig::max_block_bytes`] bytes and puts it in a
//! [`BlockStore`]. Then it marks the rows delivered and advances the delivered
//! sequence. A vacuum removes delivered rows after the sequence is durable.
//! A read serves the SQL rows while they exist; it fetches a vacuumed block by
//! byte range.
//!
//! [`DurableStreamVfs`]: super::DurableStreamVfs
//! [`SqlVfs`]: vfs::SqlVfs

pub mod block;
mod tier;
mod vfs;

pub use block::{
    block_key, decode_index, encode_block, extract_range, payload_range, read_header, BlockEntry,
    BlockError, BlockFuture, BlockIndexEntry, BlockStore, PayloadRange, BLOCK_ENTRY_BYTES,
    BLOCK_HEADER_BYTES, BLOCK_MAGIC, DEFAULT_BLOCK_PREFIX,
};
pub use tier::{FlushReport, VacuumReport};
pub use vfs::SqlVfs;

use super::{Expiry, ProducerId, StreamSeq, VfsError, WriterState};
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::time::Duration;
use std::collections::BTreeMap;

/// A boxed executor operation.
pub type SqlFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, SqlError>> + Send + 'a>>;

/// One bound value.
///
/// The engine never puts a value into the statement text. Every variable is a
/// `?` placeholder with its value in this list.
#[derive(Clone, Debug, PartialEq)]
pub enum SqlValue {
    /// A missing value.
    Null,
    /// A signed 64-bit integer. Stream lengths and versions use it.
    Integer(i64),
    /// A floating point number. The engine does not use it today; it is here so
    /// an executor maps every SQLite type.
    Real(f64),
    /// Text.
    Text(String),
    /// Bytes.
    Blob(Vec<u8>),
}

/// One row returned by a query.
///
/// Columns keep their query order. A caller reads them by index.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlRow {
    values: Vec<SqlValue>,
}

impl SqlRow {
    /// Build a row from its columns in query order.
    #[must_use]
    pub fn new(values: Vec<SqlValue>) -> Self {
        Self { values }
    }

    /// Return the columns.
    #[must_use]
    pub fn values(&self) -> &[SqlValue] {
        &self.values
    }

    /// Read a text column.
    ///
    /// # Errors
    ///
    /// Returns [`SqlError::Decode`] when the column is missing or not text.
    pub fn text(&self, index: usize) -> Result<&str, SqlError> {
        match self.values.get(index) {
            Some(SqlValue::Text(value)) => Ok(value),
            _ => Err(SqlError::Decode {
                column: index,
                expected: "text",
            }),
        }
    }

    /// Read an optional text column. `Null` returns `None`.
    ///
    /// # Errors
    ///
    /// Returns [`SqlError::Decode`] when the column is missing or not text.
    pub fn opt_text(&self, index: usize) -> Result<Option<&str>, SqlError> {
        match self.values.get(index) {
            Some(SqlValue::Null) => Ok(None),
            Some(SqlValue::Text(value)) => Ok(Some(value)),
            _ => Err(SqlError::Decode {
                column: index,
                expected: "optional text",
            }),
        }
    }

    /// Read an integer column.
    ///
    /// # Errors
    ///
    /// Returns [`SqlError::Decode`] when the column is missing or not an integer.
    pub fn integer(&self, index: usize) -> Result<i64, SqlError> {
        match self.values.get(index) {
            Some(SqlValue::Integer(value)) => Ok(*value),
            _ => Err(SqlError::Decode {
                column: index,
                expected: "integer",
            }),
        }
    }

    /// Read a byte column.
    ///
    /// # Errors
    ///
    /// Returns [`SqlError::Decode`] when the column is missing or not bytes.
    pub fn blob(&self, index: usize) -> Result<&[u8], SqlError> {
        match self.values.get(index) {
            Some(SqlValue::Blob(value)) => Ok(value),
            _ => Err(SqlError::Decode {
                column: index,
                expected: "blob",
            }),
        }
    }
}

/// One statement with its bound values.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlStatement {
    /// The statement text, with `?` placeholders.
    pub sql: String,
    /// The values, in placeholder order.
    pub params: Vec<SqlValue>,
}

impl SqlStatement {
    /// Build a statement.
    #[must_use]
    pub fn new(sql: impl Into<String>, params: Vec<SqlValue>) -> Self {
        Self {
            sql: sql.into(),
            params,
        }
    }

    /// Build a statement with no values.
    #[must_use]
    pub fn plain(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            params: Vec::new(),
        }
    }
}

/// An executor refusal.
#[derive(Debug)]
pub enum SqlError {
    /// A conditional write matched no row. The transaction rolled back.
    Conflict,
    /// A returned column has an unexpected type.
    Decode {
        /// The zero-based column index.
        column: usize,
        /// The expected type.
        expected: &'static str,
    },
    /// The engine failed.
    Io(Box<dyn core::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => f.write_str("sql write conflict"),
            Self::Decode { column, expected } => {
                write!(f, "sql column {column} is not {expected}")
            }
            Self::Io(error) => write!(f, "sql error: {error}"),
        }
    }
}

impl core::error::Error for SqlError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<SqlError> for VfsError {
    fn from(error: SqlError) -> Self {
        match error {
            SqlError::Conflict => Self::Conflict,
            SqlError::Decode { column, expected } => {
                Self::Io(Box::new(SqlError::Decode { column, expected }))
            }
            SqlError::Io(error) => Self::Io(error),
        }
    }
}

impl From<BlockError> for VfsError {
    fn from(error: BlockError) -> Self {
        Self::Io(Box::new(error))
    }
}

/// The SQL engine under the VFS.
///
/// The trait is small on purpose. An executor maps the three calls to its
/// engine: Cloudflare D1 uses `prepare`, `bind`, `run`, `raw` and `batch`. A DO
/// SQLite executor uses its synchronous storage calls behind a future.
pub trait SqlExecutor: Send + Sync {
    /// Run one statement and return the number of changed rows.
    fn execute(&self, statement: SqlStatement) -> SqlFuture<'_, u64>;

    /// Run one query and return its rows in query order.
    fn query(&self, statement: SqlStatement) -> SqlFuture<'_, Vec<SqlRow>>;

    /// Run the statements as one atomic transaction.
    ///
    /// Returns the changed-row count of each statement, in order. When any
    /// statement fails, the whole set rolls back. The engine must reject the
    /// call when it cannot promise atomicity.
    fn transaction(&self, statements: Vec<SqlStatement>) -> SqlFuture<'_, Vec<u64>>;
}

/// When an append is acknowledged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Durability {
    /// Acknowledge after the SQL commit. The hot copy is durable. This is the
    /// default for a Durable Object.
    #[default]
    Hot,
    /// Acknowledge after the block PUT. The append waits for the cold tier.
    Object,
}

/// The engine limits.
///
/// Limits are configuration, not constants. The defaults follow the sources
/// named on each field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlVfsConfig {
    /// The most bytes in one block. The default is 256 KiB, the segment target
    /// of the reference design (feature 09 requirement 20, from Rivet).
    pub max_block_bytes: usize,
    /// The longest time a chunk waits for a batch. The default is one second.
    /// A flush after this age bounds the acknowledgement latency of the cold
    /// tier.
    pub batch_age: Duration,
    /// The delivered row count that triggers a vacuum. The default is 64 rows.
    pub vacuum_threshold: u64,
    /// The number of retries after a failed block PUT. The default is 3.
    pub retry_bound: u32,
    /// The acknowledge point. The default is [`Durability::Hot`].
    pub durability: Durability,
    /// The deepest fork chain a read follows. The default is 64, the same as
    /// the memory and file VFS.
    pub max_fork_depth: u32,
    /// The object key prefix for blocks. The default is
    /// [`DEFAULT_BLOCK_PREFIX`]; feature 09 makes the root configuration.
    pub block_prefix: String,
}

impl Default for SqlVfsConfig {
    fn default() -> Self {
        Self {
            max_block_bytes: 256 * 1024,
            batch_age: Duration::from_millis(1000),
            vacuum_threshold: 64,
            retry_bound: 3,
            durability: Durability::default(),
            max_fork_depth: 64,
            block_prefix: DEFAULT_BLOCK_PREFIX.to_string(),
        }
    }
}

impl SqlVfsConfig {
    /// Set the block byte cap.
    #[must_use]
    pub fn with_max_block_bytes(mut self, bytes: usize) -> Self {
        self.max_block_bytes = bytes;
        self
    }

    /// Set the batch age.
    #[must_use]
    pub fn with_batch_age(mut self, age: Duration) -> Self {
        self.batch_age = age;
        self
    }

    /// Set the delivered row count that triggers a vacuum.
    #[must_use]
    pub fn with_vacuum_threshold(mut self, rows: u64) -> Self {
        self.vacuum_threshold = rows;
        self
    }

    /// Set the retry bound for a failed block PUT.
    #[must_use]
    pub fn with_retry_bound(mut self, retries: u32) -> Self {
        self.retry_bound = retries;
        self
    }

    /// Set the acknowledge point.
    #[must_use]
    pub fn with_durability(mut self, durability: Durability) -> Self {
        self.durability = durability;
        self
    }

    /// Set the fork chain depth limit.
    #[must_use]
    pub fn with_max_fork_depth(mut self, depth: u32) -> Self {
        self.max_fork_depth = depth;
        self
    }

    /// Set the block key prefix.
    #[must_use]
    pub fn with_block_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.block_prefix = prefix.into();
        self
    }
}

/// The statements that create the two tables.
const SCHEMA: [&str; 3] = [
    "CREATE TABLE IF NOT EXISTS stream_meta (
        name TEXT PRIMARY KEY,
        content_type TEXT NOT NULL,
        len INTEGER NOT NULL,
        closed INTEGER NOT NULL,
        expires TEXT NOT NULL,
        writer TEXT NOT NULL,
        fork_source TEXT NOT NULL,
        fork_at INTEGER NOT NULL,
        last_access_ms INTEGER NOT NULL,
        trim_from INTEGER NOT NULL,
        version INTEGER NOT NULL,
        delivered_seq INTEGER NOT NULL,
        ref_count INTEGER NOT NULL,
        soft_deleted INTEGER NOT NULL,
        blocks TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS pending_data (
        stream TEXT NOT NULL,
        seq INTEGER NOT NULL,
        bytes BLOB NOT NULL,
        state TEXT NOT NULL,
        appended_ms INTEGER NOT NULL,
        PRIMARY KEY (stream, seq)
    )",
    "CREATE INDEX IF NOT EXISTS pending_data_state ON pending_data (state)",
];

/// The metadata columns of `stream_meta`, in query order.
const META_COLUMNS: &str = "name, content_type, len, closed, expires, writer, \
     fork_source, fork_at, last_access_ms, trim_from, version, delivered_seq, \
     ref_count, soft_deleted, blocks";

/// Encode an expiry for the `expires` column. An empty value means none.
fn encode_expiry(expiry: Option<&Expiry>) -> String {
    match expiry {
        None => String::new(),
        Some(Expiry::Ttl(ttl)) => format!("ttl:{}", ttl.as_millis()),
        Some(Expiry::Absolute(at)) => format!("abs:{at}"),
    }
}

fn decode_expiry(text: &str) -> Result<Option<Expiry>, SqlError> {
    if text.is_empty() {
        return Ok(None);
    }
    if let Some(value) = text.strip_prefix("ttl:") {
        let millis = value.parse::<u64>().map_err(|_| SqlError::Decode {
            column: 4,
            expected: "ttl expiry",
        })?;
        return Ok(Some(Expiry::Ttl(Duration::from_millis(millis))));
    }
    if let Some(value) = text.strip_prefix("abs:") {
        let at = value.parse::<i64>().map_err(|_| SqlError::Decode {
            column: 4,
            expected: "absolute expiry",
        })?;
        return Ok(Some(Expiry::Absolute(at)));
    }
    Err(SqlError::Decode {
        column: 4,
        expected: "expiry",
    })
}

/// Encode the writer state as JSON text.
fn encode_writer(writer: &WriterState) -> String {
    let mut producers = serde_json::Map::new();
    for (id, state) in &writer.producers {
        producers.insert(
            id.as_str().to_string(),
            serde_json::json!({ "epoch": state.epoch, "last_seq": state.last_seq }),
        );
    }
    let value = serde_json::json!({
        "stream_seq": writer.stream_seq.as_ref().map(|seq| hex(seq.as_bytes())),
        "producers": producers,
    });
    value.to_string()
}

fn decode_writer(text: &str) -> Result<WriterState, SqlError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| SqlError::Decode {
        column: 5,
        expected: "writer state",
    })?;
    let stream_seq = match value.get("stream_seq") {
        Some(serde_json::Value::String(text)) => Some(StreamSeq::from_bytes(unhex(text).ok_or(
            SqlError::Decode {
                column: 5,
                expected: "stream sequence",
            },
        )?)),
        _ => None,
    };
    let mut producers = BTreeMap::new();
    if let Some(map) = value
        .get("producers")
        .and_then(serde_json::Value::as_object)
    {
        for (id, state) in map {
            let producer = ProducerId::parse(id).map_err(|_| SqlError::Decode {
                column: 5,
                expected: "producer identifier",
            })?;
            let epoch = state
                .get("epoch")
                .and_then(serde_json::Value::as_u64)
                .ok_or(SqlError::Decode {
                    column: 5,
                    expected: "producer epoch",
                })?;
            let last_seq = state
                .get("last_seq")
                .and_then(serde_json::Value::as_u64)
                .ok_or(SqlError::Decode {
                    column: 5,
                    expected: "producer sequence",
                })?;
            producers.insert(producer, super::ProducerEpochState { epoch, last_seq });
        }
    }
    Ok(WriterState {
        stream_seq,
        producers,
    })
}

fn encode_blocks(blocks: &[BlockIndexEntry]) -> String {
    let list: Vec<serde_json::Value> = blocks
        .iter()
        .map(|entry| {
            serde_json::json!({
                "first": entry.first_seq,
                "last": entry.last_seq,
                "key": entry.key,
            })
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

fn decode_blocks(text: &str) -> Result<Vec<BlockIndexEntry>, SqlError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| SqlError::Decode {
        column: 14,
        expected: "block index",
    })?;
    let array = value.as_array().ok_or(SqlError::Decode {
        column: 14,
        expected: "block index",
    })?;
    let mut blocks = Vec::with_capacity(array.len());
    for entry in array {
        let first_seq = entry
            .get("first")
            .and_then(serde_json::Value::as_u64)
            .ok_or(SqlError::Decode {
                column: 14,
                expected: "block start",
            })?;
        let last_seq = entry
            .get("last")
            .and_then(serde_json::Value::as_u64)
            .ok_or(SqlError::Decode {
                column: 14,
                expected: "block end",
            })?;
        let key = entry
            .get("key")
            .and_then(serde_json::Value::as_str)
            .ok_or(SqlError::Decode {
                column: 14,
                expected: "block key",
            })?
            .to_string();
        blocks.push(BlockIndexEntry {
            first_seq,
            last_seq,
            key,
        });
    }
    Ok(blocks)
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
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_digit(bytes[index])?;
        let low = hex_digit(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Some(out)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
