//! A worker fixture that runs the Durable Streams SQL virtual file system
//! (VFS) conformance kit against the two real Cloudflare SQL backends.
//!
//! WHY: the Durable Object (DO) SQL application programming interface (API)
//! only exists inside the Workers runtime. A native test cannot call it.
//! WHAT: `wrangler dev --local` starts workerd. A `GET /run` request reaches
//! the `DoSqlObject` Durable Object, which runs the kit over `DoSqlExecutor`.
//! A `GET /run-d1` request runs the same kit over the local D1 binding.
//! HOW: both executors use the real storage handle. The block store is in
//! memory, because this check covers the SQL executors, not the object store.

use foundation_context::Ctx;
use foundation_core::durable::DurableStream;
use foundation_core::durable::sql::{
    BlockError, BlockFuture, BlockStore, SqlExecutor, SqlStatement, SqlVfs, SqlVfsConfig,
};
use foundation_core::durable::vfs_testkit::{StoreFactory, run_all};
use foundation_core::valtron::SimulatedClock;
use foundation_db::{D1Executor, DoSqlExecutor};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wasm_bindgen::JsValue;
use worker::{
    Context, DurableObject, Env, Request, Response, Result, State, durable_object, event,
};

/// An in-memory block store for the cold tier of the kit.
#[derive(Default)]
struct MemoryBlocks {
    blocks: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl BlockStore for MemoryBlocks {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BlockFuture<'_, ()> {
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
        Box::pin(async move { Ok(piece) })
    }
}

/// Build a fresh VFS and clock for one scenario.
struct Factory<E: SqlExecutor> {
    executor: Arc<E>,
    blocks: Arc<dyn BlockStore>,
}

impl<E: SqlExecutor + 'static> StoreFactory for Factory<E> {
    fn build(&self) -> (DurableStream, Arc<SimulatedClock>) {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let config = SqlVfsConfig::default().with_batch_age(Duration::ZERO);
        let vfs = SqlVfs::new(
            Arc::clone(&self.executor),
            Arc::clone(&self.blocks),
            &ctx,
            config,
        );
        (DurableStream::with_ctx(Arc::new(vfs), &ctx), clock)
    }
}

/// Drop the two tables, so a second request starts from empty storage.
async fn reset<E: SqlExecutor>(executor: &E) -> std::result::Result<(), String> {
    for statement in [
        "DROP TABLE IF EXISTS pending_data",
        "DROP TABLE IF EXISTS stream_meta",
    ] {
        executor
            .execute(SqlStatement::plain(statement))
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Run every kit scenario and return their names in order.
async fn run_suite<E: SqlExecutor + 'static>(
    executor: Arc<E>,
) -> std::result::Result<Vec<&'static str>, String> {
    reset(executor.as_ref()).await?;
    let factory = Factory {
        executor,
        blocks: Arc::new(MemoryBlocks::default()),
    };
    let report = run_all(&factory).await;
    Ok(report.iter().map(|scenario| scenario.name).collect())
}

/// Prove that a failed statement rolls the whole transaction back.
async fn check_rollback(
    executor: &DoSqlExecutor,
) -> std::result::Result<serde_json::Value, String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS rollback_probe (x INTEGER)",
        "DELETE FROM rollback_probe",
    ] {
        executor
            .execute(SqlStatement::plain(statement))
            .await
            .map_err(|error| error.to_string())?;
    }
    let failed = executor
        .transaction(vec![
            SqlStatement::plain("INSERT INTO rollback_probe VALUES (1)"),
            SqlStatement::plain("INSERT INTO table_that_does_not_exist VALUES (2)"),
        ])
        .await
        .is_err();
    let rows = executor
        .query(SqlStatement::plain("SELECT count(*) FROM rollback_probe"))
        .await
        .map_err(|error| error.to_string())?;
    let remaining = rows
        .first()
        .and_then(|row| row.integer(0).ok())
        .unwrap_or(-1);
    Ok(serde_json::json!({
        "ok": failed && remaining == 0,
        "failed": failed,
        "remaining": remaining,
    }))
}

/// The Durable Object that owns the SQLite storage.
#[durable_object]
pub struct DoSqlObject {
    executor: Arc<DoSqlExecutor>,
}

impl DurableObject for DoSqlObject {
    fn new(state: State, _env: Env) -> Self {
        let executor =
            DoSqlExecutor::from_state(state).expect("the Durable Object has SQLite storage");
        Self {
            executor: Arc::new(executor),
        }
    }

    async fn fetch(&self, _request: Request) -> Result<Response> {
        if _request.path() == "/rollback" {
            return match check_rollback(&self.executor).await {
                Ok(value) => Response::from_json(&value),
                Err(error) => Ok(Response::from_json(&serde_json::json!({
                    "ok": false,
                    "error": error,
                }))?
                .with_status(500)),
            };
        }
        match run_suite(Arc::clone(&self.executor)).await {
            Ok(scenarios) => Response::from_json(&serde_json::json!({
                "ok": true,
                "scenarios": scenarios,
            })),
            Err(error) => Ok(Response::from_json(&serde_json::json!({
                "ok": false,
                "error": error,
            }))?
            .with_status(500)),
        }
    }
}

/// Run the kit over the real local D1 binding named `DB`.
async fn run_d1(env: Env) -> std::result::Result<Vec<&'static str>, String> {
    let env: JsValue = env.into();
    let executor = D1Executor::from_env(&env, "DB").map_err(|error| format!("{error:?}"))?;
    run_suite(Arc::new(executor)).await
}

/// Route `GET /run` to the Durable Object.
#[event(fetch)]
async fn main(request: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = request.path();
    if path == "/run-d1" {
        return match run_d1(env).await {
            Ok(scenarios) => Response::from_json(&serde_json::json!({
                "ok": true,
                "backend": "d1",
                "scenarios": scenarios,
            })),
            Err(error) => Ok(Response::from_json(&serde_json::json!({
                "ok": false,
                "backend": "d1",
                "error": error,
            }))?
            .with_status(500)),
        };
    }
    if path == "/run" || path == "/rollback" {
        let namespace = env.durable_object("DO_SQL")?;
        let id = namespace.id_from_name("smoke")?;
        let stub = id.get_stub()?;
        return stub.fetch_with_str(&format!("https://do{path}")).await;
    }
    Response::ok("GET /run runs the kit on a Durable Object; GET /run-d1 runs it on D1")
}
