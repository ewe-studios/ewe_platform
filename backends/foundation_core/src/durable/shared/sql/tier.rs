//! The batcher and the vacuum.
//!
//! Both are functions over the executor, the block store and the injected
//! clock. The SQL VFS calls them; a host schedules [`super::SqlVfs::flush`] on
//! a timer and [`super::SqlVfs::vacuum`] after a batch.

use super::super::{StreamName, VfsError};
use super::block::{block_key, encode_block, BlockIndexEntry, BlockStore};
use super::{decode_blocks, encode_blocks, SqlExecutor, SqlStatement, SqlValue, SqlVfsConfig};
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;

/// What one flush did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlushReport {
    /// The number of blocks put in the object store.
    pub blocks_put: usize,
    /// The number of rows marked delivered.
    pub rows_delivered: u64,
}

/// What one vacuum did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumReport {
    /// The number of delivered rows removed.
    pub rows_removed: u64,
}

struct PendingRow {
    seq: u64,
    bytes: Vec<u8>,
    appended_ms: i64,
}

struct BlockDraft {
    chunks: Vec<(u64, Vec<u8>)>,
    bytes: usize,
    first_seq: u64,
    last_seq: u64,
}

impl BlockDraft {
    fn new(seq: u64) -> Self {
        Self {
            chunks: Vec::new(),
            bytes: 0,
            first_seq: seq,
            last_seq: seq,
        }
    }

    fn push(&mut self, seq: u64, bytes: Vec<u8>) {
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.bytes = self.bytes.saturating_add(bytes.len());
        self.last_seq = seq.saturating_add(len);
        self.chunks.push((seq, bytes));
    }
}

/// Put the eligible pending blocks and mark their rows delivered.
///
/// A block is eligible when its oldest chunk reached the batch age, or when its
/// bytes reached the block cap. When `only` names a stream, the call flushes
/// that stream now.
///
/// # Errors
///
/// Returns a storage error when a block PUT still fails after the retry bound,
/// or when a SQL statement fails.
pub(crate) async fn flush_stream<E: SqlExecutor + ?Sized>(
    executor: &E,
    blocks: &dyn BlockStore,
    ctx: &Ctx,
    config: &SqlVfsConfig,
    only: Option<&StreamName>,
) -> Result<FlushReport, VfsError> {
    let now = unix_millis(ctx.clock().wall_now());
    let streams = pending_streams(executor, only).await?;
    let mut report = FlushReport::default();
    for text in streams {
        let name = StreamName::parse(&text).map_err(|_| VfsError::Conflict)?;
        let Some(tail) = load_tail(executor, &name).await? else {
            continue;
        };
        let rows = load_pending(executor, &name).await?;
        if rows.is_empty() || (only.is_none() && !eligible(&rows, now, config)) {
            continue;
        }
        let one = flush_one(executor, blocks, config, &name, tail, rows).await?;
        report.blocks_put = report.blocks_put.saturating_add(one.blocks_put);
        report.rows_delivered = report.rows_delivered.saturating_add(one.rows_delivered);
    }
    Ok(report)
}

async fn pending_streams<E: SqlExecutor + ?Sized>(
    executor: &E,
    only: Option<&StreamName>,
) -> Result<Vec<String>, VfsError> {
    let statement = match only {
        Some(name) => SqlStatement::new(
            "SELECT DISTINCT stream FROM pending_data WHERE state = 'pending' AND stream = ?",
            vec![SqlValue::Text(name.as_str().to_string())],
        ),
        None => {
            SqlStatement::plain("SELECT DISTINCT stream FROM pending_data WHERE state = 'pending'")
        }
    };
    let rows = executor.query(statement).await?;
    let mut streams = Vec::with_capacity(rows.len());
    for row in &rows {
        streams.push(row.text(0)?.to_string());
    }
    Ok(streams)
}

async fn load_tail<E: SqlExecutor + ?Sized>(
    executor: &E,
    name: &StreamName,
) -> Result<Option<(u64, Vec<BlockIndexEntry>)>, VfsError> {
    let rows = executor
        .query(SqlStatement::new(
            "SELECT delivered_seq, blocks FROM stream_meta WHERE name = ?",
            vec![SqlValue::Text(name.as_str().to_string())],
        ))
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let delivered = u64::try_from(row.integer(0)?).map_err(|_| VfsError::Conflict)?;
    Ok(Some((delivered, decode_blocks(row.text(1)?)?)))
}

async fn load_pending<E: SqlExecutor + ?Sized>(
    executor: &E,
    name: &StreamName,
) -> Result<Vec<PendingRow>, VfsError> {
    let rows = executor
        .query(SqlStatement::new(
            "SELECT seq, bytes, appended_ms FROM pending_data \
             WHERE stream = ? AND state = 'pending' ORDER BY seq",
            vec![SqlValue::Text(name.as_str().to_string())],
        ))
        .await?;
    let mut pending = Vec::with_capacity(rows.len());
    for row in &rows {
        pending.push(PendingRow {
            seq: u64::try_from(row.integer(0)?).map_err(|_| VfsError::Conflict)?,
            bytes: row.blob(1)?.to_vec(),
            appended_ms: row.integer(2)?,
        });
    }
    Ok(pending)
}

fn eligible(rows: &[PendingRow], now_ms: i64, config: &SqlVfsConfig) -> bool {
    let total: usize = rows.iter().map(|row| row.bytes.len()).sum();
    if total >= config.max_block_bytes {
        return true;
    }
    let Some(oldest) = rows.iter().map(|row| row.appended_ms).min() else {
        return false;
    };
    let age_ms = i64::try_from(config.batch_age.as_millis()).unwrap_or(i64::MAX);
    now_ms.saturating_sub(oldest) >= age_ms
}

struct Batch<'a, E: SqlExecutor + ?Sized> {
    executor: &'a E,
    blocks: &'a dyn BlockStore,
    config: &'a SqlVfsConfig,
    name: &'a StreamName,
    delivered_seq: u64,
    index: Vec<BlockIndexEntry>,
    report: FlushReport,
}

async fn flush_one<E: SqlExecutor + ?Sized>(
    executor: &E,
    blocks: &dyn BlockStore,
    config: &SqlVfsConfig,
    name: &StreamName,
    tail: (u64, Vec<BlockIndexEntry>),
    rows: Vec<PendingRow>,
) -> Result<FlushReport, VfsError> {
    let (delivered_seq, index) = tail;
    let mut batch = Batch {
        executor,
        blocks,
        config,
        name,
        delivered_seq,
        index,
        report: FlushReport::default(),
    };
    let mut draft = BlockDraft::new(rows[0].seq);
    for row in rows {
        if !draft.chunks.is_empty()
            && draft.bytes.saturating_add(row.bytes.len()) > config.max_block_bytes
        {
            batch
                .seal(std::mem::replace(&mut draft, BlockDraft::new(row.seq)))
                .await?;
        }
        draft.push(row.seq, row.bytes);
    }
    if !draft.chunks.is_empty() {
        batch.seal(draft).await?;
    }
    Ok(batch.report)
}

impl<E: SqlExecutor + ?Sized> Batch<'_, E> {
    async fn seal(&mut self, draft: BlockDraft) -> Result<(), VfsError> {
        let key = block_key(
            &self.config.block_prefix,
            self.name,
            draft.first_seq,
            draft.last_seq,
        );
        let chunks: Vec<(u64, &[u8])> = draft
            .chunks
            .iter()
            .map(|(seq, bytes)| (*seq, bytes.as_slice()))
            .collect();
        let bytes = encode_block(&chunks)?;
        put_with_retry(self.blocks, &key, bytes, self.config.retry_bound).await?;
        self.report.blocks_put = self.report.blocks_put.saturating_add(1);

        let mut next_index = self.index.clone();
        next_index.push(BlockIndexEntry {
            first_seq: draft.first_seq,
            last_seq: draft.last_seq,
            key,
        });
        let next_delivered = self.delivered_seq.max(draft.last_seq);
        // The metadata update goes first and guards both statements. A second
        // batcher that loses the guard also leaves its rows pending, so no row
        // moves to delivered without an index entry.
        let counts = self
            .executor
            .transaction(vec![
                SqlStatement::new(
                    "UPDATE stream_meta SET delivered_seq = ?, blocks = ? \
                     WHERE name = ? AND delivered_seq = ?",
                    vec![
                        integer_params(next_delivered)?,
                        SqlValue::Text(encode_blocks(&next_index)),
                        SqlValue::Text(self.name.as_str().to_string()),
                        integer_params(self.delivered_seq)?,
                    ],
                ),
                SqlStatement::new(
                    "UPDATE pending_data SET state = 'delivered' \
                     WHERE stream = ? AND seq >= ? AND seq < ? AND state = 'pending' \
                     AND EXISTS (SELECT 1 FROM stream_meta \
                                 WHERE name = ? AND delivered_seq = ?)",
                    vec![
                        SqlValue::Text(self.name.as_str().to_string()),
                        integer_params(draft.first_seq)?,
                        integer_params(draft.last_seq)?,
                        SqlValue::Text(self.name.as_str().to_string()),
                        integer_params(next_delivered)?,
                    ],
                ),
            ])
            .await?;
        if counts.first().copied().unwrap_or(0) == 1 {
            self.delivered_seq = next_delivered;
            self.index = next_index;
            self.report.rows_delivered = self
                .report
                .rows_delivered
                .saturating_add(counts.get(1).copied().unwrap_or(0));
        }
        Ok(())
    }
}

fn integer_params(value: u64) -> Result<SqlValue, VfsError> {
    Ok(SqlValue::Integer(
        i64::try_from(value).map_err(|_| VfsError::Conflict)?,
    ))
}

async fn put_with_retry(
    blocks: &dyn BlockStore,
    key: &str,
    bytes: Vec<u8>,
    retry_bound: u32,
) -> Result<(), VfsError> {
    let mut retries = 0;
    loop {
        match blocks.put(key, bytes.clone()).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                if retries >= retry_bound {
                    return Err(error.into());
                }
                retries = retries.saturating_add(1);
            }
        }
    }
}

/// Remove delivered rows whose bytes the object store already holds.
///
/// # Errors
///
/// Returns a storage error when a SQL statement fails.
pub(crate) async fn vacuum_delivered<E: SqlExecutor + ?Sized>(
    executor: &E,
    config: &SqlVfsConfig,
    only: Option<&StreamName>,
) -> Result<VacuumReport, VfsError> {
    let threshold = i64::try_from(config.vacuum_threshold).unwrap_or(i64::MAX);
    let (sql, params) = match only {
        Some(name) => (
            "SELECT stream FROM pending_data WHERE state = 'delivered' AND stream = ? \
             GROUP BY stream HAVING COUNT(*) >= ?",
            vec![
                SqlValue::Text(name.as_str().to_string()),
                SqlValue::Integer(threshold),
            ],
        ),
        None => (
            "SELECT stream FROM pending_data WHERE state = 'delivered' \
             GROUP BY stream HAVING COUNT(*) >= ?",
            vec![SqlValue::Integer(threshold)],
        ),
    };
    let streams = executor.query(SqlStatement::new(sql, params)).await?;
    let mut report = VacuumReport::default();
    for row in &streams {
        let name = row.text(0)?;
        let Some(delivered) = load_delivered_seq(executor, name).await? else {
            continue;
        };
        let removed = executor
            .execute(SqlStatement::new(
                "DELETE FROM pending_data WHERE stream = ? AND state = 'delivered' \
                 AND seq + length(bytes) <= ?",
                vec![
                    SqlValue::Text(name.to_string()),
                    SqlValue::Integer(i64::try_from(delivered).map_err(|_| VfsError::Conflict)?),
                ],
            ))
            .await?;
        report.rows_removed = report.rows_removed.saturating_add(removed);
    }
    Ok(report)
}

async fn load_delivered_seq<E: SqlExecutor + ?Sized>(
    executor: &E,
    name: &str,
) -> Result<Option<u64>, VfsError> {
    let rows = executor
        .query(SqlStatement::new(
            "SELECT delivered_seq FROM stream_meta WHERE name = ?",
            vec![SqlValue::Text(name.to_string())],
        ))
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(
        u64::try_from(row.integer(0)?).map_err(|_| VfsError::Conflict)?,
    ))
}
