//! The SQL virtual file system (VFS) runs against a real SQLite engine.
//!
//! The executor is libsql, the engine the workspace already builds for
//! `foundation_db`. The block store is an in-memory map that records every
//! range read. No fake SQL engine is used.

use foundation_context::Ctx;
#[cfg(feature = "vfs-testkit")]
use foundation_core::durable::DurableStream;
use foundation_core::durable::sql::{
    BlockError, BlockFuture, BlockStore, Durability, FlushReport, SqlError, SqlExecutor, SqlFuture,
    SqlRow, SqlStatement, SqlValue, SqlVfs, SqlVfsConfig, block_key,
};
#[cfg(feature = "vfs-testkit")]
use foundation_core::durable::vfs_testkit::{SCENARIOS, run_all};
use foundation_core::durable::{
    ContentType, DurableStreamVfs, StreamName, VfsError, VfsMeta, WriterState,
};
use foundation_core::valtron::{SimulatedClock, block_on_future_scoped};
use libsql_backend as libsql;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A libsql-backed executor.
struct LibsqlExecutor {
    conn: Arc<libsql::Connection>,
}

fn io_error(error: libsql::Error) -> SqlError {
    SqlError::Io(Box::new(error))
}

fn to_params(values: Vec<SqlValue>) -> Vec<libsql::Value> {
    values
        .into_iter()
        .map(|value| match value {
            SqlValue::Null => libsql::Value::Null,
            SqlValue::Integer(value) => libsql::Value::Integer(value),
            SqlValue::Real(value) => libsql::Value::Real(value),
            SqlValue::Text(value) => libsql::Value::Text(value),
            SqlValue::Blob(value) => libsql::Value::Blob(value),
        })
        .collect()
}

fn from_value(value: libsql::Value) -> SqlValue {
    match value {
        libsql::Value::Null => SqlValue::Null,
        libsql::Value::Integer(value) => SqlValue::Integer(value),
        libsql::Value::Real(value) => SqlValue::Real(value),
        libsql::Value::Text(value) => SqlValue::Text(value),
        libsql::Value::Blob(value) => SqlValue::Blob(value),
    }
}

impl SqlExecutor for LibsqlExecutor {
    fn execute(&self, statement: SqlStatement) -> SqlFuture<'_, u64> {
        Box::pin(async move {
            self.conn
                .execute(&statement.sql, to_params(statement.params))
                .await
                .map_err(io_error)
        })
    }

    fn query(&self, statement: SqlStatement) -> SqlFuture<'_, Vec<SqlRow>> {
        Box::pin(async move {
            let mut rows = self
                .conn
                .query(&statement.sql, to_params(statement.params))
                .await
                .map_err(io_error)?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().await.map_err(io_error)? {
                let columns = row.column_count();
                let mut values = Vec::with_capacity(usize::try_from(columns).unwrap_or(0));
                for index in 0..columns {
                    values.push(from_value(row.get_value(index).map_err(io_error)?));
                }
                out.push(SqlRow::new(values));
            }
            Ok(out)
        })
    }

    fn transaction(&self, statements: Vec<SqlStatement>) -> SqlFuture<'_, Vec<u64>> {
        Box::pin(async move {
            let transaction = self.conn.transaction().await.map_err(io_error)?;
            let mut counts = Vec::with_capacity(statements.len());
            for statement in statements {
                counts.push(
                    transaction
                        .execute(&statement.sql, to_params(statement.params))
                        .await
                        .map_err(io_error)?,
                );
            }
            transaction.commit().await.map_err(io_error)?;
            Ok(counts)
        })
    }
}

/// An in-memory block store with fault injection and a range-read log.
#[derive(Default)]
struct MemoryBlockStore {
    blocks: Mutex<BTreeMap<String, Vec<u8>>>,
    fail_puts: AtomicUsize,
    reads: Mutex<Vec<(u64, usize)>>,
}

impl MemoryBlockStore {
    fn keys(&self) -> Vec<String> {
        self.blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }

    fn block(&self, key: &str) -> Option<Vec<u8>> {
        self.blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    fn remove(&self, key: &str) {
        self.blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
    }

    fn reads(&self) -> Vec<(u64, usize)> {
        self.reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn clear_reads(&self) {
        self.reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl BlockStore for MemoryBlockStore {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BlockFuture<'_, ()> {
        if self.fail_puts.load(Ordering::SeqCst) > 0 {
            self.fail_puts.fetch_sub(1, Ordering::SeqCst);
            return Box::pin(async {
                Err(BlockError::Io(Box::new(std::io::Error::other(
                    "injected put failure",
                ))))
            });
        }
        let mut blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = blocks.get(key) {
            if existing != &bytes {
                return Box::pin(async { Err(BlockError::Invalid("a block is immutable")) });
            }
        }
        blocks.insert(key.to_string(), bytes);
        Box::pin(async { Ok(()) })
    }

    fn get_range(&self, key: &str, start: u64, len: usize) -> BlockFuture<'_, Vec<u8>> {
        let blocks = self
            .blocks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(bytes) = blocks.get(key) else {
            return Box::pin(async { Err(BlockError::NotFound) });
        };
        let Ok(from) = usize::try_from(start) else {
            return Box::pin(async { Err(BlockError::Invalid("range start overflows")) });
        };
        let Some(end) = from.checked_add(len) else {
            return Box::pin(async { Err(BlockError::Invalid("range end overflows")) });
        };
        if end > bytes.len() {
            return Box::pin(async { Err(BlockError::Invalid("short block")) });
        }
        let piece = bytes[from..end].to_vec();
        drop(blocks);
        self.reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((start, len));
        Box::pin(async move { Ok(piece) })
    }
}

struct Engine {
    executor: Arc<LibsqlExecutor>,
    blocks: Arc<MemoryBlockStore>,
    ctx: Ctx,
    clock: Arc<SimulatedClock>,
    config: SqlVfsConfig,
}

impl Engine {
    async fn new(config: SqlVfsConfig) -> Self {
        let database = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("open the in-memory database");
        let conn = database.connect().expect("connect");
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        Self {
            executor: Arc::new(LibsqlExecutor {
                conn: Arc::new(conn),
            }),
            blocks: Arc::new(MemoryBlockStore::default()),
            ctx,
            clock,
            config,
        }
    }

    fn vfs(&self) -> SqlVfs<LibsqlExecutor> {
        SqlVfs::new(
            self.executor.clone(),
            self.blocks.clone(),
            &self.ctx,
            self.config.clone(),
        )
    }

    async fn scalar(&self, sql: &str, params: Vec<SqlValue>) -> i64 {
        let rows = self
            .executor
            .query(SqlStatement::new(sql, params))
            .await
            .expect("query");
        rows.first().expect("one row").integer(0).expect("integer")
    }

    async fn pending_states(&self, name: &StreamName) -> Vec<String> {
        let rows = self
            .executor
            .query(SqlStatement::new(
                "SELECT state FROM pending_data WHERE stream = ? ORDER BY seq",
                vec![SqlValue::Text(name.as_str().to_string())],
            ))
            .await
            .expect("pending states");
        rows.iter()
            .map(|row| row.text(0).expect("state").to_string())
            .collect()
    }
}

fn stream_name(text: &str) -> StreamName {
    StreamName::parse(text).expect("valid stream name")
}

fn meta() -> VfsMeta {
    VfsMeta {
        content_type: ContentType::parse("application/octet-stream").expect("content type"),
        len: 0,
        closed: false,
        expires: None,
        writer: WriterState::default(),
        fork: None,
        last_access_ms: 0,
        trim_from: 0,
        version: 0,
    }
}

fn zero_age() -> SqlVfsConfig {
    SqlVfsConfig::default().with_batch_age(Duration::ZERO)
}

#[test]
fn sql_vfs_create_append_read() {
    block_on_future_scoped(async {
        let engine = Engine::new(zero_age()).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/basic");
        assert!(vfs.create(&name, meta()).await.expect("create"));
        assert_eq!(vfs.append(&name, b"hello ").await.expect("append").0, 6);
        assert_eq!(vfs.append(&name, b"world").await.expect("append").0, 11);
        let bytes = vfs
            .read_at(&name, &foundation_core::durable::VfsOffset(0), 1024)
            .await
            .expect("read");
        assert_eq!(bytes, b"hello world");
        let stored = vfs.meta(&name).await.expect("meta");
        assert_eq!(stored.len, 11);
        assert_eq!(stored.version, 2);
    });
}

#[cfg(feature = "vfs-testkit")]
#[test]
fn sql_vfs_passes_conformance() {
    block_on_future_scoped(async {
        let mut engines = Vec::with_capacity(SCENARIOS.len());
        for _ in 0..SCENARIOS.len() {
            engines.push(Engine::new(zero_age()).await);
        }
        let engines = Mutex::new(engines.into_iter());
        let make = move || {
            let engine = engines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next()
                .expect("one engine per scenario");
            (
                DurableStream::with_ctx(
                    Arc::new(SqlVfs::new(
                        engine.executor.clone(),
                        engine.blocks.clone(),
                        &engine.ctx,
                        engine.config.clone(),
                    )),
                    &engine.ctx,
                ),
                engine.clock.clone(),
            )
        };
        let report = run_all(&make).await;
        let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();
        assert_eq!(names, SCENARIOS, "the kit runs every scenario in order");
    });
}

#[test]
fn sql_vfs_batches_pending_rows_into_one_block() {
    block_on_future_scoped(async {
        let engine = Engine::new(zero_age().with_max_block_bytes(1024)).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/batch");
        vfs.create(&name, meta()).await.expect("create");
        for bytes in [b"aaaa".as_slice(), b"bb", b"cccccc"] {
            vfs.append(&name, bytes).await.expect("append");
        }
        let report = vfs.flush().await.expect("flush");
        assert_eq!(
            report,
            FlushReport {
                blocks_put: 1,
                rows_delivered: 3,
            }
        );
        assert_eq!(
            engine.pending_states(&name).await,
            vec!["delivered", "delivered", "delivered"]
        );
        let key = block_key("durable", &name, 0, 12);
        let block = engine.blocks.block(&key).expect("one deterministic block");
        assert_eq!(&block[..8], b"DSBLK001");
        assert_eq!(
            engine
                .scalar(
                    "SELECT delivered_seq FROM stream_meta WHERE name = ?",
                    vec![SqlValue::Text(name.as_str().to_string())]
                )
                .await,
            12
        );
        let bytes = vfs
            .read_at(&name, &foundation_core::durable::VfsOffset(0), 1024)
            .await
            .expect("read");
        assert_eq!(bytes, b"aaaabbcccccc");
    });
}

#[test]
fn sql_vfs_batch_age_waits_for_the_clock() {
    block_on_future_scoped(async {
        let config = SqlVfsConfig::default()
            .with_batch_age(Duration::from_millis(1000))
            .with_max_block_bytes(1024);
        let engine = Engine::new(config).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/age");
        vfs.create(&name, meta()).await.expect("create");
        vfs.append(&name, b"young").await.expect("append");

        let early = vfs.flush().await.expect("early flush");
        assert_eq!(early.blocks_put, 0, "a young chunk waits for its age");

        engine.clock.advance_wall(Duration::from_millis(1000));
        let late = vfs.flush().await.expect("late flush");
        assert_eq!(late.blocks_put, 1, "an old chunk flushes");
    });
}

#[test]
fn sql_vfs_crash_replays_pending_rows() {
    block_on_future_scoped(async {
        let engine = Engine::new(zero_age().with_max_block_bytes(1024)).await;
        let first = engine.vfs();
        let name = stream_name("sql/crash");
        first.create(&name, meta()).await.expect("create");
        first.append(&name, b"abc").await.expect("append");
        drop(first);

        // A new VFS over the same database replays the pending row.
        let second = engine.vfs();
        let report = second.flush().await.expect("replay flush");
        assert_eq!(report.blocks_put, 1);
        assert_eq!(
            second
                .read_at(&name, &foundation_core::durable::VfsOffset(0), 1024)
                .await
                .expect("read"),
            b"abc"
        );
        // The deterministic key makes a second flush idempotent.
        let again = second.flush().await.expect("idempotent flush");
        assert_eq!(again.blocks_put, 0);
        assert_eq!(
            engine.blocks.keys(),
            vec![block_key("durable", &name, 0, 3)]
        );
    });
}

#[test]
fn sql_vfs_vacuum_removes_delivered_rows() {
    block_on_future_scoped(async {
        let config = zero_age()
            .with_max_block_bytes(1024)
            .with_vacuum_threshold(1);
        let engine = Engine::new(config).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/vacuum");
        vfs.create(&name, meta()).await.expect("create");
        for bytes in [b"1111".as_slice(), b"2222", b"3333"] {
            vfs.append(&name, bytes).await.expect("append");
        }
        vfs.flush().await.expect("flush");
        let report = vfs.vacuum().await.expect("vacuum");
        assert_eq!(report.rows_removed, 3);
        assert_eq!(
            engine
                .scalar("SELECT COUNT(*) FROM pending_data", vec![])
                .await,
            0
        );
        let reads_before = engine.blocks.reads().len();
        let bytes = vfs
            .read_at(&name, &foundation_core::durable::VfsOffset(4), 8)
            .await
            .expect("vacuumed read");
        assert_eq!(bytes, b"22223333");
        assert!(
            engine.blocks.reads().len() > reads_before,
            "a vacuumed read comes from the block store"
        );
    });
}

#[test]
fn sql_vfs_range_read_is_bounded() {
    block_on_future_scoped(async {
        let config = zero_age()
            .with_max_block_bytes(256)
            .with_vacuum_threshold(1);
        let engine = Engine::new(config).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/range");
        vfs.create(&name, meta()).await.expect("create");
        let chunk = [b'x'; 64];
        for _ in 0..16 {
            vfs.append(&name, &chunk).await.expect("append");
        }
        vfs.flush().await.expect("flush");
        vfs.vacuum().await.expect("vacuum");
        engine.blocks.clear_reads();

        let bytes = vfs
            .read_at(&name, &foundation_core::durable::VfsOffset(300), 5)
            .await
            .expect("page read");
        assert_eq!(bytes, b"xxxxx");
        let fetched: usize = engine.blocks.reads().iter().map(|(_, len)| len).sum();
        assert!(
            fetched < 1024,
            "a five byte page fetches less than the whole stream, fetched {fetched}"
        );
    });
}

#[test]
fn sql_vfs_retries_a_failed_put() {
    block_on_future_scoped(async {
        let engine = Engine::new(zero_age().with_max_block_bytes(1024).with_retry_bound(3)).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/retry");
        vfs.create(&name, meta()).await.expect("create");
        vfs.append(&name, b"abc").await.expect("append");

        engine.blocks.fail_puts.store(2, Ordering::SeqCst);
        let report = vfs.flush().await.expect("a retry recovers");
        assert_eq!(report.blocks_put, 1);
        assert_eq!(engine.pending_states(&name).await, vec!["delivered"]);

        vfs.append(&name, b"def").await.expect("second append");
        engine.blocks.fail_puts.store(9, Ordering::SeqCst);
        assert!(vfs.flush().await.is_err(), "the retry bound is respected");
        assert_eq!(
            engine.pending_states(&name).await,
            vec!["delivered", "pending"],
            "a failed PUT keeps the row pending"
        );
        engine.blocks.fail_puts.store(0, Ordering::SeqCst);
        assert_eq!(vfs.flush().await.expect("later flush").blocks_put, 1);
    });
}

#[test]
fn sql_vfs_object_durability_acknowledges_after_put() {
    block_on_future_scoped(async {
        let hot = Engine::new(zero_age().with_max_block_bytes(1024)).await;
        let hot_vfs = hot.vfs();
        let hot_name = stream_name("sql/hot");
        hot_vfs.create(&hot_name, meta()).await.expect("create");
        hot_vfs.append(&hot_name, b"abc").await.expect("append");
        assert!(hot.blocks.keys().is_empty(), "hot acknowledges the commit");

        let object = Engine::new(
            zero_age()
                .with_max_block_bytes(1024)
                .with_durability(Durability::Object),
        )
        .await;
        let object_vfs = object.vfs();
        let object_name = stream_name("sql/object");
        object_vfs
            .create(&object_name, meta())
            .await
            .expect("create");
        object_vfs
            .append(&object_name, b"abc")
            .await
            .expect("append");
        assert_eq!(
            object.blocks.keys(),
            vec![block_key("durable", &object_name, 0, 3)],
            "object durability waits for the PUT"
        );
    });
}

#[test]
fn sql_vfs_missing_block_is_an_error() {
    block_on_future_scoped(async {
        let config = zero_age()
            .with_max_block_bytes(1024)
            .with_vacuum_threshold(1);
        let engine = Engine::new(config).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/missing");
        vfs.create(&name, meta()).await.expect("create");
        vfs.append(&name, b"abc").await.expect("append");
        vfs.flush().await.expect("flush");
        vfs.vacuum().await.expect("vacuum");
        engine.blocks.remove(&block_key("durable", &name, 0, 3));

        let result = vfs
            .read_at(&name, &foundation_core::durable::VfsOffset(0), 1024)
            .await;
        assert!(
            matches!(result, Err(VfsError::Io(_))),
            "a missing live block is an error, never zero bytes"
        );
    });
}

#[test]
fn sql_vfs_covers_every_method() {
    block_on_future_scoped(async {
        let engine = Engine::new(zero_age().with_max_block_bytes(1024)).await;
        let vfs = engine.vfs();
        let name = stream_name("sql/cover");
        assert!(vfs.create(&name, meta()).await.expect("create"));
        assert!(!vfs.create(&name, meta()).await.expect("idempotent create"));

        let notifier = vfs.notifier(&name).expect("notifier");
        let fired = Arc::new(AtomicUsize::new(0));
        let counter = fired.clone();
        notifier.register(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        vfs.append(&name, b"abcd").await.expect("append");
        assert_eq!(fired.load(Ordering::SeqCst), 1, "an append notifies");

        vfs.touch(&name, 42).await.expect("touch");
        assert_eq!(vfs.meta(&name).await.expect("meta").last_access_ms, 42);
        vfs.trim(&name, foundation_core::durable::VfsOffset(2))
            .await
            .expect("trim");
        assert!(matches!(
            vfs.read_at(&name, &foundation_core::durable::VfsOffset(0), 8)
                .await,
            Err(VfsError::Trimmed)
        ));
        vfs.close(&name).await.expect("close");
        assert!(vfs.meta(&name).await.expect("closed meta").closed);
        assert_eq!(vfs.list(&stream_name("sql")).await.expect("list").len(), 1);
        assert!(!vfs.remove(&name).await.expect("remove"));
        assert!(matches!(vfs.meta(&name).await, Err(VfsError::NotFound)));
    });
}
