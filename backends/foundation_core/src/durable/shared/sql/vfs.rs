//! The SQL-backed virtual file system (VFS).

use super::super::{
    DurableStreamVfs, ForkInfo, Notifier, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture,
    VfsMeta, VfsOffset,
};
use super::block::{
    decode_index, extract_range, payload_range, read_header, BlockError, BlockIndexEntry,
    BlockStore,
};
use super::{
    decode_blocks, decode_expiry, decode_writer, encode_expiry, encode_writer, Durability,
    SqlError, SqlExecutor, SqlRow, SqlStatement, SqlValue, SqlVfsConfig, META_COLUMNS, SCHEMA,
};
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A one-shot append notifier.
#[derive(Default)]
struct SqlNotifier {
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + 'static>>>,
}

impl SqlNotifier {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Box<dyn Fn() + Send + 'static>>> {
        self.callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Notifier for SqlNotifier {
    fn register(&self, cb: Box<dyn Fn() + Send + 'static>) {
        self.lock().push(cb);
    }

    fn notify(&self) {
        let callbacks = std::mem::take(&mut *self.lock());
        for callback in callbacks {
            callback();
        }
    }
}

/// A `u64` that does not fit the signed SQLite integer.
#[derive(Debug)]
struct SqlRangeError;

impl fmt::Display for SqlRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("value does not fit a signed 64-bit SQLite integer")
    }
}

impl core::error::Error for SqlRangeError {}

fn sql_int(value: u64) -> Result<i64, VfsError> {
    i64::try_from(value).map_err(|_| VfsError::Io(Box::new(SqlRangeError)))
}

fn sql_u64(value: i64, column: usize) -> Result<u64, SqlError> {
    u64::try_from(value).map_err(|_| SqlError::Decode {
        column,
        expected: "unsigned integer",
    })
}

/// One row of `stream_meta` with the engine columns.
#[derive(Clone, Debug)]
struct MetaRow {
    meta: VfsMeta,
    ref_count: u32,
    soft_deleted: bool,
    blocks: Vec<BlockIndexEntry>,
}

impl MetaRow {
    fn base_len(&self) -> u64 {
        self.meta.fork.as_ref().map_or(0, |fork| fork.at)
    }
}

/// The metadata change of one append.
#[derive(Clone, Copy)]
enum AppendPlan<'a> {
    /// Keep the metadata record and advance the length and the version.
    Length,
    /// Replace the metadata record fields from `meta`.
    Meta(&'a VfsMeta),
}

/// A SQL-backed VFS with an object-store cold tier.
///
/// The executor runs statements. The block store holds vacuumed blocks. The
/// injected clock drives the batch age and the append time. No global state is
/// read.
pub struct SqlVfs<E: SqlExecutor> {
    executor: Arc<E>,
    blocks: Arc<dyn BlockStore>,
    ctx: Ctx,
    config: SqlVfsConfig,
    schema_ready: AtomicBool,
    notifiers: Mutex<HashMap<StreamName, Arc<SqlNotifier>>>,
}

impl<E: SqlExecutor> SqlVfs<E> {
    /// Build a VFS.
    ///
    /// The clock comes from the injected context, so a test can move time.
    #[must_use]
    pub fn new(
        executor: Arc<E>,
        blocks: Arc<dyn BlockStore>,
        ctx: &Ctx,
        config: SqlVfsConfig,
    ) -> Self {
        Self {
            executor,
            blocks,
            ctx: ctx.clone(),
            config,
            schema_ready: AtomicBool::new(false),
            notifiers: Mutex::new(HashMap::new()),
        }
    }

    /// Return the configuration.
    #[must_use]
    pub fn config(&self) -> &SqlVfsConfig {
        &self.config
    }

    /// Return the executor.
    #[must_use]
    pub fn executor(&self) -> &Arc<E> {
        &self.executor
    }

    /// Return the block store.
    #[must_use]
    pub fn blocks(&self) -> &Arc<dyn BlockStore> {
        &self.blocks
    }

    /// Create the two tables and the index.
    ///
    /// This runs automatically on the first operation. A host may call it at
    /// start to fail early.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a schema statement fails.
    pub async fn init_schema(&self) -> Result<(), VfsError> {
        for statement in SCHEMA {
            self.executor
                .execute(SqlStatement::plain(statement))
                .await?;
        }
        self.schema_ready.store(true, Ordering::Release);
        Ok(())
    }

    async fn ensure_schema(&self) -> Result<(), VfsError> {
        if self.schema_ready.load(Ordering::Acquire) {
            return Ok(());
        }
        self.init_schema().await
    }

    /// Put every eligible pending block. A block is eligible when its oldest
    /// chunk reached the batch age or its bytes reached the block cap.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a block PUT still fails after the retry
    /// bound, or when a SQL statement fails.
    pub async fn flush(&self) -> Result<super::FlushReport, VfsError> {
        self.ensure_schema().await?;
        super::tier::flush_stream(
            self.executor.as_ref(),
            self.blocks.as_ref(),
            &self.ctx,
            &self.config,
            None,
        )
        .await
    }

    /// Put every pending block of one stream now, regardless of age.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a block PUT still fails after the retry
    /// bound.
    pub async fn flush_stream(&self, name: &StreamName) -> Result<super::FlushReport, VfsError> {
        self.ensure_schema().await?;
        super::tier::flush_stream(
            self.executor.as_ref(),
            self.blocks.as_ref(),
            &self.ctx,
            &self.config,
            Some(name),
        )
        .await
    }

    /// Remove delivered rows once their delivered sequence is durable.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a SQL statement fails.
    pub async fn vacuum(&self) -> Result<super::VacuumReport, VfsError> {
        self.ensure_schema().await?;
        super::tier::vacuum_delivered(self.executor.as_ref(), &self.config, None).await
    }

    fn now_ms(&self) -> i64 {
        unix_millis(self.ctx.clock().wall_now())
    }

    fn notifier_for(&self, name: &StreamName) -> Arc<SqlNotifier> {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(name.clone())
            .or_default()
            .clone()
    }

    fn forget_notifier(&self, name: &StreamName) {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }

    fn load_row<'a>(
        &'a self,
        name: &StreamName,
    ) -> Pin<Box<dyn Future<Output = Result<Option<MetaRow>, VfsError>> + Send + 'a>> {
        let statement = SqlStatement::new(
            format!("SELECT {META_COLUMNS} FROM stream_meta WHERE name = ?"),
            vec![SqlValue::Text(name.as_str().to_string())],
        );
        let executor = self.executor.clone();
        Box::pin(async move {
            let rows = executor.query(statement).await?;
            rows.first()
                .map(row_to_meta)
                .transpose()
                .map_err(Into::into)
        })
    }

    async fn load_required(&self, name: &StreamName) -> Result<MetaRow, VfsError> {
        self.load_row(name).await?.ok_or(VfsError::NotFound)
    }

    fn check_existing(row: &MetaRow, meta: &VfsMeta) -> Result<bool, VfsError> {
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        if row.meta.content_type != meta.content_type
            || row.meta.closed != meta.closed
            || row.meta.expires != meta.expires
            || row.meta.fork != meta.fork
        {
            return Err(VfsError::Conflict);
        }
        Ok(false)
    }

    async fn create_entry(&self, name: &StreamName, meta: &VfsMeta) -> Result<bool, VfsError> {
        if let Some(existing) = self.load_row(name).await? {
            return Self::check_existing(&existing, meta);
        }
        if let Some(info) = &meta.fork {
            match self.load_row(&info.source).await? {
                None => return Err(VfsError::NotFound),
                Some(source) if source.soft_deleted => return Err(VfsError::Gone),
                Some(source) if meta.len > source.meta.len => return Err(VfsError::Conflict),
                Some(_) => {}
            }
        }

        let insert = SqlStatement::new(
            "INSERT INTO stream_meta (name, content_type, len, closed, expires, writer, \
             fork_source, fork_at, last_access_ms, trim_from, version, delivered_seq, \
             ref_count, soft_deleted, blocks) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, '[]') \
             ON CONFLICT(name) DO NOTHING",
            vec![
                SqlValue::Text(name.as_str().to_string()),
                SqlValue::Text(meta.content_type.as_str().to_string()),
                SqlValue::Integer(sql_int(meta.len)?),
                SqlValue::Integer(i64::from(meta.closed)),
                SqlValue::Text(encode_expiry(meta.expires.as_ref())),
                SqlValue::Text(encode_writer(&meta.writer)),
                SqlValue::Text(
                    meta.fork
                        .as_ref()
                        .map_or_else(String::new, |fork| fork.source.as_str().to_string()),
                ),
                SqlValue::Integer(sql_int(meta.fork.as_ref().map_or(0, |fork| fork.at))?),
                SqlValue::Integer(meta.last_access_ms),
                SqlValue::Integer(sql_int(meta.trim_from)?),
            ],
        );
        let mut statements = vec![insert];
        if let Some(fork) = &meta.fork {
            // `changes()` is the insert count, so a lost race adds no reference.
            statements.push(SqlStatement::new(
                "UPDATE stream_meta SET ref_count = ref_count + \
                 CASE WHEN changes() = 1 THEN 1 ELSE 0 END WHERE name = ?",
                vec![SqlValue::Text(fork.source.as_str().to_string())],
            ));
        }
        let counts = self.executor.transaction(statements).await?;
        if counts.first().copied().unwrap_or(0) == 1 {
            return Ok(true);
        }
        match self.load_row(name).await? {
            Some(existing) => Self::check_existing(&existing, meta),
            None => Err(VfsError::Conflict),
        }
    }

    async fn append_entry(
        &self,
        name: &StreamName,
        bytes: &[u8],
        plan: AppendPlan<'_>,
    ) -> Result<VfsOffset, VfsError> {
        let row = self.load_required(name).await?;
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        if row.meta.closed {
            return Err(VfsError::Conflict);
        }
        let base = row.base_len();
        let own_len = row.meta.len.saturating_sub(base);
        let new_len = row
            .meta
            .len
            .checked_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            .ok_or(VfsError::Conflict)?;
        let expected_version = sql_int(row.meta.version)?;
        let now = self.now_ms();

        let mut statements = Vec::with_capacity(2);
        if !bytes.is_empty() {
            statements.push(conditional_insert(
                name,
                own_len,
                bytes,
                now,
                expected_version,
            )?);
        }
        statements.push(update_statement(name, new_len, expected_version, plan)?);
        let update_index = statements.len() - 1;
        let counts = self.executor.transaction(statements).await?;
        if counts.get(update_index).copied().unwrap_or(0) != 1 {
            return Err(VfsError::Conflict);
        }

        if self.config.durability == Durability::Object && !bytes.is_empty() {
            self.flush_stream(name).await?;
        }
        self.notifier_for(name).notify();
        Ok(VfsOffset(new_len))
    }

    fn read_chain(
        &self,
        name: StreamName,
        at: u64,
        limit: usize,
        depth: u32,
        direct: bool,
    ) -> VfsFuture<'_, Vec<u8>> {
        Box::pin(async move {
            if depth > self.config.max_fork_depth {
                return Err(VfsError::Conflict);
            }
            let Some(row) = self.load_row(&name).await? else {
                return Err(VfsError::NotFound);
            };
            if direct && row.soft_deleted {
                return Err(VfsError::Gone);
            }
            if at < row.meta.trim_from {
                return Err(VfsError::Trimmed);
            }
            if limit == 0 {
                return Ok(Vec::new());
            }

            let base = row.base_len();
            let mut out = Vec::new();
            if at < base {
                let take = usize::try_from(base - at).unwrap_or(usize::MAX).min(limit);
                if let Some(fork) = &row.meta.fork {
                    out.extend(
                        self.read_chain(fork.source.clone(), at, take, depth + 1, false)
                            .await?,
                    );
                }
            }
            if out.len() < limit {
                let next = at.saturating_add(u64::try_from(out.len()).unwrap_or(u64::MAX));
                let own_at = next.saturating_sub(base);
                let own_len = row.meta.len.saturating_sub(base);
                let room = u64::try_from(limit - out.len()).unwrap_or(u64::MAX);
                let own_end = own_at.saturating_add(room).min(own_len);
                if own_at < own_end {
                    out.extend(self.read_own(&name, &row, own_at, own_end).await?);
                }
            }
            Ok(out)
        })
    }

    async fn read_own(
        &self,
        name: &StreamName,
        row: &MetaRow,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>, VfsError> {
        let statement = SqlStatement::new(
            "SELECT seq, length(bytes), substr(bytes, max(?, seq) - seq + 1, \
             min(?, seq + length(bytes)) - max(?, seq)) \
             FROM pending_data \
             WHERE stream = ? AND seq < ? AND seq + length(bytes) > ? ORDER BY seq",
            vec![
                SqlValue::Integer(sql_int(start)?),
                SqlValue::Integer(sql_int(end)?),
                SqlValue::Integer(sql_int(start)?),
                SqlValue::Text(name.as_str().to_string()),
                SqlValue::Integer(sql_int(end)?),
                SqlValue::Integer(sql_int(start)?),
            ],
        );
        let rows = self.executor.query(statement).await?;
        let mut out = Vec::new();
        let mut cursor = start;
        for row_entry in &rows {
            let seq = sql_u64(row_entry.integer(0)?, 0)?;
            let len = sql_u64(row_entry.integer(1)?, 1)?;
            let row_start = seq.max(start);
            let row_end = seq.saturating_add(len).min(end);
            if row_start >= row_end {
                continue;
            }
            if row_start > cursor {
                self.fill_from_blocks(&row.blocks, cursor, row_start, &mut out)
                    .await?;
            }
            out.extend_from_slice(row_entry.blob(2)?);
            cursor = row_end;
        }
        if cursor < end {
            self.fill_from_blocks(&row.blocks, cursor, end, &mut out)
                .await?;
        }
        let expected = usize::try_from(end - start).unwrap_or(usize::MAX);
        if out.len() != expected {
            return Err(VfsError::Io(Box::new(BlockError::NotFound)));
        }
        Ok(out)
    }

    async fn fill_from_blocks(
        &self,
        blocks: &[BlockIndexEntry],
        start: u64,
        end: u64,
        out: &mut Vec<u8>,
    ) -> Result<(), VfsError> {
        let mut cursor = start;
        while cursor < end {
            let Some(entry) = blocks
                .iter()
                .find(|block| block.first_seq <= cursor && cursor < block.last_seq)
            else {
                return Err(VfsError::Io(Box::new(BlockError::NotFound)));
            };
            let block_end = end.min(entry.last_seq);
            out.extend(self.read_block(&entry.key, cursor, block_end).await?);
            cursor = block_end;
        }
        Ok(())
    }

    async fn read_block(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>, VfsError> {
        let header = self.blocks.get_range(key, 0, 12).await?;
        let count = read_header(&header)?;
        let cap = u64::try_from(self.config.max_block_bytes.max(1)).unwrap_or(u64::MAX);
        if u64::from(count) > cap {
            return Err(BlockError::Invalid("block index is larger than a block").into());
        }
        let index_start = super::block::BLOCK_HEADER_BYTES;
        let index_len = u64::from(count).saturating_mul(super::block::BLOCK_ENTRY_BYTES);
        let index = if index_len == 0 {
            Vec::new()
        } else {
            self.blocks
                .get_range(
                    key,
                    index_start,
                    usize::try_from(index_len).unwrap_or(usize::MAX),
                )
                .await?
        };
        let entries = decode_index(&index, count)?;
        let Some(range) = payload_range(&entries, start, end) else {
            return Err(BlockError::Invalid("block does not cover the read").into());
        };
        if range.len == 0 {
            return Ok(Vec::new());
        }
        let payload_start = index_start
            .saturating_add(index_len)
            .saturating_add(range.start);
        let payload = self
            .blocks
            .get_range(
                key,
                payload_start,
                usize::try_from(range.len).unwrap_or(usize::MAX),
            )
            .await?;
        if payload.len() != usize::try_from(range.len).unwrap_or(usize::MAX) {
            return Err(BlockError::Invalid("short block payload").into());
        }
        Ok(extract_range(&entries, &payload, range.start, start, end))
    }

    async fn touch_entry(&self, name: &StreamName, now_ms: i64) -> Result<(), VfsError> {
        let row = self.load_required(name).await?;
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        self.executor
            .execute(SqlStatement::new(
                "UPDATE stream_meta SET last_access_ms = ? WHERE name = ?",
                vec![
                    SqlValue::Integer(now_ms),
                    SqlValue::Text(name.as_str().to_string()),
                ],
            ))
            .await?;
        Ok(())
    }

    async fn trim_entry(&self, name: &StreamName, at: u64) -> Result<(), VfsError> {
        let row = self.load_required(name).await?;
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        let trim_from = at.min(row.meta.len).max(row.meta.trim_from);
        self.executor
            .execute(SqlStatement::new(
                "UPDATE stream_meta SET trim_from = ?, version = version + 1 WHERE name = ?",
                vec![
                    SqlValue::Integer(sql_int(trim_from)?),
                    SqlValue::Text(name.as_str().to_string()),
                ],
            ))
            .await?;
        Ok(())
    }

    async fn close_entry(&self, name: &StreamName) -> Result<(), VfsError> {
        let row = self.load_required(name).await?;
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        self.executor
            .execute(SqlStatement::new(
                "UPDATE stream_meta SET closed = 1, version = version + 1 WHERE name = ?",
                vec![SqlValue::Text(name.as_str().to_string())],
            ))
            .await?;
        self.notifier_for(name).notify();
        Ok(())
    }

    async fn remove_entry(&self, name: &StreamName) -> Result<bool, VfsError> {
        let row = self.load_required(name).await?;
        if row.soft_deleted {
            return Err(VfsError::Gone);
        }
        if row.ref_count > 0 {
            self.executor
                .execute(SqlStatement::new(
                    "UPDATE stream_meta SET soft_deleted = 1, version = version + 1 WHERE name = ?",
                    vec![SqlValue::Text(name.as_str().to_string())],
                ))
                .await?;
            self.notifier_for(name).notify();
            return Ok(true);
        }
        self.delete_stream(name, row.meta.fork.as_ref().map(|fork| &fork.source))
            .await?;
        self.forget_notifier(name);
        self.cascade().await?;
        Ok(false)
    }

    async fn delete_stream(
        &self,
        name: &StreamName,
        fork_source: Option<&StreamName>,
    ) -> Result<(), VfsError> {
        let mut statements = vec![
            SqlStatement::new(
                "DELETE FROM pending_data WHERE stream = ?",
                vec![SqlValue::Text(name.as_str().to_string())],
            ),
            SqlStatement::new(
                "DELETE FROM stream_meta WHERE name = ?",
                vec![SqlValue::Text(name.as_str().to_string())],
            ),
        ];
        if let Some(source) = fork_source {
            statements.push(SqlStatement::new(
                "UPDATE stream_meta SET ref_count = max(ref_count - 1, 0) WHERE name = ?",
                vec![SqlValue::Text(source.as_str().to_string())],
            ));
        }
        self.executor.transaction(statements).await?;
        Ok(())
    }

    /// Remove every soft-deleted entry whose reference count reached zero.
    async fn cascade(&self) -> Result<(), VfsError> {
        loop {
            let rows = self
                .executor
                .query(SqlStatement::plain(
                    "SELECT name, fork_source FROM stream_meta \
                     WHERE soft_deleted = 1 AND ref_count = 0 LIMIT 1",
                ))
                .await?;
            let Some(row) = rows.first() else {
                return Ok(());
            };
            let name = parse_name(row.text(0)?)?;
            let source = match row.opt_text(1)? {
                None | Some("") => None,
                Some(source) => Some(parse_name(source)?),
            };
            self.delete_stream(&name, source.as_ref()).await?;
            self.forget_notifier(&name);
        }
    }

    async fn list_entries(&self, prefix: &StreamName) -> Result<Vec<VfsEntry>, VfsError> {
        let length = i64::try_from(prefix.as_str().chars().count())
            .map_err(|_| VfsError::Io(Box::new(SqlRangeError)))?;
        let rows = self
            .executor
            .query(SqlStatement::new(
                format!(
                    "SELECT {META_COLUMNS} FROM stream_meta \
                     WHERE soft_deleted = 0 AND substr(name, 1, ?) = ? ORDER BY name"
                ),
                vec![
                    SqlValue::Integer(length),
                    SqlValue::Text(prefix.as_str().to_string()),
                ],
            ))
            .await?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            entries.push(VfsEntry {
                name: parse_name(row.text(0)?)?,
                meta: row_to_meta(row)?.meta,
            });
        }
        Ok(entries)
    }
}

fn conditional_insert(
    name: &StreamName,
    own_len: u64,
    bytes: &[u8],
    appended_ms: i64,
    expected_version: i64,
) -> Result<SqlStatement, VfsError> {
    Ok(SqlStatement::new(
        "INSERT INTO pending_data (stream, seq, bytes, state, appended_ms) \
         SELECT ?, ?, ?, 'pending', ? \
         WHERE (SELECT version FROM stream_meta \
                WHERE name = ? AND soft_deleted = 0 AND closed = 0) = ?",
        vec![
            SqlValue::Text(name.as_str().to_string()),
            SqlValue::Integer(sql_int(own_len)?),
            SqlValue::Blob(bytes.to_vec()),
            SqlValue::Integer(appended_ms),
            SqlValue::Text(name.as_str().to_string()),
            SqlValue::Integer(expected_version),
        ],
    ))
}

fn update_statement(
    name: &StreamName,
    new_len: u64,
    expected_version: i64,
    plan: AppendPlan<'_>,
) -> Result<SqlStatement, VfsError> {
    let name = SqlValue::Text(name.as_str().to_string());
    let sql = match plan {
        AppendPlan::Length => {
            "UPDATE stream_meta SET len = ?, version = version + 1 \
             WHERE name = ? AND version = ? AND soft_deleted = 0 AND closed = 0"
        }
        AppendPlan::Meta(_) => {
            "UPDATE stream_meta SET content_type = ?, len = ?, closed = ?, expires = ?, \
             writer = ?, last_access_ms = ?, version = ? \
             WHERE name = ? AND version = ? AND soft_deleted = 0 AND closed = 0"
        }
    };
    let params = match plan {
        AppendPlan::Length => vec![
            SqlValue::Integer(sql_int(new_len)?),
            name,
            SqlValue::Integer(expected_version),
        ],
        AppendPlan::Meta(meta) => vec![
            SqlValue::Text(meta.content_type.as_str().to_string()),
            SqlValue::Integer(sql_int(new_len)?),
            SqlValue::Integer(i64::from(meta.closed)),
            SqlValue::Text(encode_expiry(meta.expires.as_ref())),
            SqlValue::Text(encode_writer(&meta.writer)),
            SqlValue::Integer(meta.last_access_ms),
            SqlValue::Integer(expected_version.saturating_add(1)),
            name,
            SqlValue::Integer(expected_version),
        ],
    };
    Ok(SqlStatement::new(sql, params))
}

fn parse_name(text: &str) -> Result<StreamName, VfsError> {
    StreamName::parse(text).map_err(|_| {
        SqlError::Decode {
            column: 0,
            expected: "stream name",
        }
        .into()
    })
}

fn row_to_meta(row: &SqlRow) -> Result<MetaRow, SqlError> {
    let content_type = row
        .text(1)
        .ok()
        .and_then(|text| super::super::ContentType::parse(text).ok())
        .ok_or(SqlError::Decode {
            column: 1,
            expected: "content type",
        })?;
    let fork = match row.opt_text(6)? {
        None | Some("") => None,
        Some(source) => Some(ForkInfo {
            source: super::super::StreamName::parse(source).map_err(|_| SqlError::Decode {
                column: 6,
                expected: "fork source",
            })?,
            at: sql_u64(row.integer(7)?, 7)?,
        }),
    };
    let meta = VfsMeta {
        content_type,
        len: sql_u64(row.integer(2)?, 2)?,
        closed: row.integer(3)? != 0,
        expires: decode_expiry(row.text(4)?)?,
        writer: decode_writer(row.text(5)?)?,
        fork,
        last_access_ms: row.integer(8)?,
        trim_from: sql_u64(row.integer(9)?, 9)?,
        version: sql_u64(row.integer(10)?, 10)?,
    };
    Ok(MetaRow {
        meta,
        ref_count: u32::try_from(row.integer(12)?).map_err(|_| SqlError::Decode {
            column: 12,
            expected: "reference count",
        })?,
        soft_deleted: row.integer(13)? != 0,
        blocks: decode_blocks(row.text(14)?)?,
    })
}

impl<E: SqlExecutor + 'static> DurableStreamVfs for SqlVfs<E> {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.create_entry(&name, &meta).await
        })
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.append_entry(&name, &bytes, AppendPlan::Length).await
        })
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move {
            self.ensure_schema().await?;
            let row = self.load_required(&name).await?;
            if row.soft_deleted {
                return Err(VfsError::Gone);
            }
            if row.meta.version != expected_version || row.meta.closed {
                return Err(VfsError::Conflict);
            }
            self.append_entry(&name, &bytes, AppendPlan::Meta(&meta))
                .await
        })
    }

    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        let name = name.clone();
        let at = at.0;
        Box::pin(async move {
            self.ensure_schema().await?;
            self.read_chain(name, at, limit, 0, true).await
        })
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            let row = self.load_required(&name).await?;
            if row.soft_deleted {
                return Err(VfsError::Gone);
            }
            Ok(row.meta)
        })
    }

    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.touch_entry(&name, now_ms).await
        })
    }

    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.trim_entry(&name, at.0).await
        })
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.close_entry(&name).await
        })
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        let name = name.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.remove_entry(&name).await
        })
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        let prefix = prefix.clone();
        Box::pin(async move {
            self.ensure_schema().await?;
            self.list_entries(&prefix).await
        })
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        Some(self.notifier_for(name) as NotifierRef)
    }
}
