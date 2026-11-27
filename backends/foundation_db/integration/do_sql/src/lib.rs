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
use foundation_core::durable::{
    AppendKind, AppendOutcome, AppendRequest, BadRequestKind, Chunk, CloseRequest, ContentType,
    CreateOutcome, CreateSpec, DurableService, Expiry, ForkSpec, Producer as StoreProducer,
    ProducerId, ReadFrom, ReadLimit, ReadPage, ReadRequest, StoreError, StreamName, StreamOffset,
    StreamSeq, wire,
};
use foundation_core::valtron::SimulatedClock;
use foundation_db::{D1Executor, DoSqlExecutor};
use durable_transport_matrix::{
    Driver, ErrorKind, Observation, Producer, Scenario, StepResult, run_scenario,
};
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

/// The Durable Object driver for the transport matrix.
///
/// It runs the shared scenario list over a [`DurableStream`] on the Durable
/// Object's SQLite storage. The block store is in memory. The store and the
/// model read the host wall clock, so the time-to-live (TTL) scenario expires
/// after the real wait.
struct MatrixDoDriver {
    service: DurableService<DurableStream>,
}

impl MatrixDoDriver {
    /// Build a fresh store on the Durable Object's SQLite executor.
    fn new(executor: Arc<DoSqlExecutor>) -> Self {
        let ctx = Ctx::host();
        let config = SqlVfsConfig::default().with_batch_age(Duration::ZERO);
        let blocks: Arc<dyn BlockStore> = Arc::new(MemoryBlocks::default());
        let vfs = SqlVfs::new(executor, blocks, &ctx, config);
        let stream = DurableStream::with_ctx(Arc::new(vfs), &ctx);
        Self {
            service: DurableService::new(stream),
        }
    }

    /// Create a stream.
    async fn create_result(
        &self,
        stream: &str,
        content_type: &str,
        ttl_ms: Option<u64>,
    ) -> std::result::Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let content_type = ContentType::parse(content_type)?;
        let outcome = self
            .service
            .create(
                name,
                CreateSpec {
                    content_type: Some(content_type),
                    expires: ttl_ms.map(|value| Expiry::Ttl(Duration::from_millis(value))),
                    closed: false,
                    initial: None,
                    fork: None,
                },
            )
            .await?;
        Ok(matrix_create_step(&outcome))
    }

    /// Append one chunk.
    async fn append_result(
        &self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> std::result::Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let content_type = ContentType::parse(content_type)?;
        let stream_seq = stream_seq.map(|value| StreamSeq::from_bytes(value.as_bytes().to_vec()));
        let producer = producer
            .map(|value| {
                Ok::<_, StoreError>(StoreProducer {
                    id: ProducerId::parse(&value.id)?,
                    epoch: value.epoch,
                    seq: value.seq,
                })
            })
            .transpose()?;
        let outcome = self
            .service
            .append(
                &name,
                AppendRequest {
                    chunk: Chunk::new(data.to_vec()),
                    content_type: Some(content_type),
                    stream_seq,
                    producer,
                    close,
                },
            )
            .await?;
        Ok(matrix_append_step(&outcome))
    }

    /// Close a stream.
    async fn close_result(
        &self,
        stream: &str,
        data: &[u8],
    ) -> std::result::Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let outcome = self
            .service
            .close(
                &name,
                CloseRequest {
                    final_chunk: (!data.is_empty()).then(|| Chunk::new(data.to_vec())),
                    producer: None,
                },
            )
            .await?;
        Ok(matrix_append_step(&outcome))
    }

    /// Read a bounded page.
    async fn read_result(
        &self,
        stream: &str,
        offset: Option<&str>,
    ) -> std::result::Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let from = matrix_read_from(offset)?;
        let page = self
            .service
            .read(
                &name,
                ReadRequest {
                    from,
                    limit: ReadLimit::new(None, None),
                },
            )
            .await?;
        Ok(matrix_read_step(&page))
    }

    /// Create a fork.
    async fn fork_result(
        &self,
        child: &str,
        source: &str,
        at: Option<&str>,
    ) -> std::result::Result<StepResult, StoreError> {
        let name = StreamName::parse(child)?;
        let source = StreamName::parse(source)?;
        let at = at.map(matrix_parse_offset).transpose()?;
        let outcome = self
            .service
            .fork(
                name,
                ForkSpec {
                    source,
                    at,
                    sub_offset: 0,
                },
            )
            .await?;
        Ok(matrix_create_step(&outcome))
    }
}

/// Parse a read start. `None` starts at the beginning.
fn matrix_read_from(offset: Option<&str>) -> std::result::Result<ReadFrom, StoreError> {
    match offset {
        None => Ok(ReadFrom::Start),
        Some(value) => Ok(ReadFrom::Offset(matrix_parse_offset(value)?)),
    }
}

/// Parse one wire offset.
fn matrix_parse_offset(text: &str) -> std::result::Result<StreamOffset, StoreError> {
    wire::parse_offset(text).ok_or(StoreError::BadRequest(BadRequestKind::Offset))
}

/// Build the observation of a create or fork.
fn matrix_create_step(outcome: &CreateOutcome) -> StepResult {
    let offset = wire::offset_text(&outcome.head.tail);
    let observation = Observation::status(if outcome.created { 201 } else { 200 })
        .with_next_offset(true)
        .with_closed(outcome.head.closed);
    StepResult::new(observation, Some(offset))
}

/// Build the observation of an append or close.
fn matrix_append_step(outcome: &AppendOutcome) -> StepResult {
    let offset = wire::offset_text(&outcome.cursor.next);
    let observation = Observation::status(204)
        .with_next_offset(true)
        .with_closed(outcome.closed)
        .with_duplicate(matches!(outcome.kind, AppendKind::Duplicate));
    StepResult::new(observation, Some(offset))
}

/// Build the observation of a read page.
fn matrix_read_step(page: &ReadPage) -> StepResult {
    let body: Vec<u8> = page
        .chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect();
    let offset = wire::offset_text(&page.cursor.next);
    let observation = Observation::status(200)
        .with_body(body)
        .with_next_offset(true)
        .with_up_to_date(page.cursor.up_to_date)
        .with_closed(page.cursor.closed);
    StepResult::new(observation, Some(offset))
}

/// Map one store refusal to a transport-neutral kind.
fn matrix_error_step(error: StoreError) -> StepResult {
    let kind = match error {
        StoreError::NotFound => ErrorKind::NotFound,
        StoreError::Gone => ErrorKind::Gone,
        StoreError::Closed { .. } => ErrorKind::Closed,
        StoreError::StreamSeqConflict { .. }
        | StoreError::ContentTypeMismatch { .. }
        | StoreError::ConfigConflict => ErrorKind::Conflict,
        StoreError::BadRequest(_) => ErrorKind::BadRequest,
        StoreError::Unsupported => ErrorKind::Unsupported,
        other => ErrorKind::Other(other.to_string()),
    };
    StepResult::new(Observation::error(kind), None)
}

impl Driver for MatrixDoDriver {
    async fn create(
        &mut self,
        stream: &str,
        content_type: &str,
        ttl_ms: Option<u64>,
    ) -> StepResult {
        match self.create_result(stream, content_type, ttl_ms).await {
            Ok(step) => step,
            Err(error) => matrix_error_step(error),
        }
    }

    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        match self
            .append_result(stream, data, content_type, producer, stream_seq, close)
            .await
        {
            Ok(step) => step,
            Err(error) => matrix_error_step(error),
        }
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
        match self.close_result(stream, data).await {
            Ok(step) => step,
            Err(error) => matrix_error_step(error),
        }
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        match self.read_result(stream, offset).await {
            Ok(step) => step,
            Err(error) => matrix_error_step(error),
        }
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        match self.fork_result(child, source, at).await {
            Ok(step) => step,
            Err(error) => matrix_error_step(error),
        }
    }

    async fn wait(&mut self, duration: Duration) {
        worker::Delay::from(duration).await;
    }
}

/// Run one matrix scenario and return its observation log.
async fn run_matrix(
    executor: Arc<DoSqlExecutor>,
    scenario: Scenario,
) -> std::result::Result<Vec<Observation>, String> {
    reset(executor.as_ref()).await?;
    let mut driver = MatrixDoDriver::new(executor);
    Ok(run_scenario(&mut driver, scenario).await)
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
        if _request.path() == "/matrix" {
            let url = _request.url()?;
            let name = url
                .query_pairs()
                .find(|(key, _)| key == "scenario")
                .map(|(_, value)| value.into_owned());
            let Some(name) = name else {
                return Ok(Response::from_json(&serde_json::json!({
                    "ok": false,
                    "error": "the scenario query parameter is required",
                }))?
                .with_status(400));
            };
            let Some(scenario) = Scenario::all()
                .into_iter()
                .find(|scenario| scenario.name() == name)
            else {
                return Ok(Response::from_json(&serde_json::json!({
                    "ok": false,
                    "error": format!("unknown scenario: {name}"),
                }))?
                .with_status(400));
            };
            return match run_matrix(Arc::clone(&self.executor), scenario).await {
                Ok(observations) => Response::from_json(&serde_json::json!({
                    "ok": true,
                    "scenario": name,
                    "observations": observations,
                })),
                Err(error) => Ok(Response::from_json(&serde_json::json!({
                    "ok": false,
                    "error": error,
                }))?
                .with_status(500)),
            };
        }
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
    if path == "/matrix" {
        let url = request.url()?;
        let query = url
            .query()
            .map_or_else(String::new, |value| format!("?{value}"));
        let namespace = env.durable_object("DO_SQL")?;
        let id = namespace.id_from_name("matrix")?;
        let stub = id.get_stub()?;
        return stub.fetch_with_str(&format!("https://do/matrix{query}")).await;
    }
    if path == "/run" || path == "/rollback" {
        let namespace = env.durable_object("DO_SQL")?;
        let id = namespace.id_from_name("smoke")?;
        let stub = id.get_stub()?;
        return stub.fetch_with_str(&format!("https://do{path}")).await;
    }
    Response::ok("GET /run runs the kit on a Durable Object; GET /run-d1 runs it on D1")
}
