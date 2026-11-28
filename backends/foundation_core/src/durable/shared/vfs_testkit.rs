//! A shared conformance kit for [`DurableStreamVfs`] implementations.
//!
//! A virtual file system (VFS) crate enables the `vfs-testkit` feature on
//! `foundation_core` in its dev-dependencies. It then calls [`run_all`] with a
//! [`StoreFactory`]. The kit runs each scenario on a fresh store and returns a
//! structured [`ScenarioObservation`]. A differential oracle compares the
//! observations of two VFS implementations.
//!
//! A broken invariant panics with the scenario name and the failed step. The
//! caller never reads a half result.

use super::{
    AppendRequest, Chunk, ContentType, CreateSpec, DurableStream, DurableStreamStore, Expiry,
    ForkInfo, ReadFrom, ReadLimit, ReadRequest, StreamName, VfsError, VfsMeta, VfsOffset,
    WriterState,
};
use crate::valtron::SimulatedClock;
use core::time::Duration;
use std::sync::Arc;

/// The scenario names, in run order.
pub const SCENARIOS: &[&str] = &[
    "create",
    "append",
    "read_byte_exact",
    "bounded_pages",
    "fork_prefix",
    "fork_append",
    "soft_delete_cascade",
    "ttl_slide",
    "version_conflict",
];

/// Builds a fresh store and the simulated clock that its context holds.
///
/// The kit calls the factory once per scenario, so no scenario sees another
/// scenario's data. The clock must be the same clock inside the store context,
/// because the time-to-live (TTL) scenario moves it.
pub trait StoreFactory {
    /// Build one store and its clock.
    fn build(&self) -> (DurableStream, Arc<SimulatedClock>);
}

impl<F> StoreFactory for F
where
    F: Fn() -> (DurableStream, Arc<SimulatedClock>),
{
    fn build(&self) -> (DurableStream, Arc<SimulatedClock>) {
        self()
    }
}

/// The comparable part of a [`VfsMeta`].
///
/// The kit reduces a metadata record to the fields both backends must agree
/// on, so a differential oracle compares stable values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetaObservation {
    /// The normalized content type.
    pub content_type: String,
    /// The stream length in bytes.
    pub len: u64,
    /// True when the stream is closed.
    pub closed: bool,
    /// The expiry in its debug form.
    pub expiry: String,
    /// The last read or write time in UNIX milliseconds.
    pub last_access_ms: i64,
    /// The first readable byte after retention.
    pub trim_from: u64,
    /// The metadata version.
    pub version: u64,
    /// The fork source and inherited prefix length.
    pub fork: Option<(String, u64)>,
    /// The last accepted `Stream-Seq`.
    pub stream_seq: Option<Vec<u8>>,
    /// The producer state: identifier, epoch and last accepted sequence.
    pub producers: Vec<(String, u64, u64)>,
}

impl From<&VfsMeta> for MetaObservation {
    fn from(meta: &VfsMeta) -> Self {
        Self {
            content_type: meta.content_type.as_str().to_string(),
            len: meta.len,
            closed: meta.closed,
            expiry: format!("{:?}", meta.expires),
            last_access_ms: meta.last_access_ms,
            trim_from: meta.trim_from,
            version: meta.version,
            fork: meta
                .fork
                .as_ref()
                .map(|fork| (fork.source.as_str().to_string(), fork.at)),
            stream_seq: meta
                .writer
                .stream_seq
                .as_ref()
                .map(|seq| seq.as_bytes().to_vec()),
            producers: meta
                .writer
                .producers
                .iter()
                .map(|(id, state)| (id.as_str().to_string(), state.epoch, state.last_seq))
                .collect(),
        }
    }
}

/// One observable value from a scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// A byte sequence.
    Bytes(Vec<u8>),
    /// A boolean flag.
    Bool(bool),
    /// An unsigned number.
    U64(u64),
    /// A text value.
    Text(String),
    /// A reduced metadata record.
    Meta(MetaObservation),
    /// The kind of an error.
    Error(String),
}

/// The result of one scenario run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioObservation {
    /// The scenario name.
    pub name: &'static str,
    /// The values the scenario observed, in order.
    pub values: Vec<Observation>,
}

impl ScenarioObservation {
    /// Start an observation list for a scenario.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            values: Vec::new(),
        }
    }

    /// Add one observed value.
    pub fn push(&mut self, value: Observation) {
        self.values.push(value);
    }
}

/// Return the stable label of a [`VfsError`] kind.
///
/// The label drops the inner error, so two backends can compare an error kind.
#[must_use]
pub fn vfs_error_label(error: &VfsError) -> &'static str {
    match error {
        VfsError::NotFound => "NotFound",
        VfsError::Gone => "Gone",
        VfsError::Trimmed => "Trimmed",
        VfsError::Conflict => "Conflict",
        VfsError::Unsupported => "Unsupported",
        VfsError::Io(_) => "Io",
    }
}

/// Reduce a result to an observation: `ok` for success, else the error kind.
#[must_use]
pub fn result_observation<T>(result: &Result<T, VfsError>) -> Observation {
    match result {
        Ok(_) => Observation::Text("ok".to_string()),
        Err(error) => Observation::Error(vfs_error_label(error).to_string()),
    }
}

/// Panic with the scenario name when an invariant fails.
///
/// # Panics
///
/// Panics when `condition` is false.
pub fn check(scenario: &'static str, condition: bool, step: &str) {
    assert!(condition, "vfs scenario '{scenario}' failed: {step}");
}

/// Run every scenario, each on a fresh store from `factory`.
pub async fn run_all(factory: &dyn StoreFactory) -> Vec<ScenarioObservation> {
    let mut results = Vec::with_capacity(SCENARIOS.len());
    results.push(scenario_create(&factory.build().0).await);
    results.push(scenario_append(&factory.build().0).await);
    results.push(scenario_read_byte_exact(&factory.build().0).await);
    results.push(scenario_bounded_pages(&factory.build().0).await);
    results.push(scenario_fork_prefix(&factory.build().0).await);
    results.push(scenario_fork_append(&factory.build().0).await);
    results.push(scenario_soft_delete_cascade(&factory.build().0).await);
    {
        let (stream, clock) = factory.build();
        results.push(scenario_ttl_slide(&stream, &clock).await);
    }
    results.push(scenario_version_conflict(&factory.build().0).await);
    results
}

/// Build the metadata for a new stream.
fn empty_meta(content_type: &str) -> VfsMeta {
    VfsMeta {
        content_type: ContentType::parse(content_type).expect("valid content type"),
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

fn stream_name(name: &str) -> StreamName {
    StreamName::parse(name).expect("valid scenario stream name")
}

/// Create an entry, prove idempotence and a configuration conflict.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_create(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "create";
    let name = stream_name("scenario/create");
    let vfs = stream.vfs();
    let created = vfs.create(&name, empty_meta("text/plain")).await;
    let again = vfs.create(&name, empty_meta("text/plain")).await;
    let conflict = vfs.create(&name, empty_meta("application/json")).await;
    let meta = vfs.meta(&name).await;

    check(NAME, matches!(created, Ok(true)), "the first create is new");
    check(
        NAME,
        matches!(again, Ok(false)),
        "a matching create is idempotent",
    );
    check(
        NAME,
        matches!(conflict, Err(VfsError::Conflict)),
        "a different content type conflicts",
    );
    let meta = meta.expect("the created entry has metadata");

    let mut result = ScenarioObservation::new(NAME);
    result.push(result_observation(&created));
    result.push(result_observation(&again));
    result.push(result_observation(&conflict));
    result.push(Observation::Meta(MetaObservation::from(&meta)));
    result
}

/// Append two chunks and observe both returned offsets.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_append(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "append";
    let name = stream_name("scenario/append");
    let vfs = stream.vfs();
    vfs.create(&name, empty_meta("application/octet-stream"))
        .await
        .expect("create");
    let first = vfs.append(&name, b"hello ").await;
    let second = vfs.append(&name, b"world").await;
    let meta = vfs.meta(&name).await.expect("metadata");

    check(
        NAME,
        matches!(&first, Ok(offset) if offset.0 == 6),
        "the first append returns six",
    );
    check(
        NAME,
        matches!(&second, Ok(offset) if offset.0 == 11),
        "the second append returns eleven",
    );
    check(NAME, meta.len == 11, "the metadata length is eleven");
    check(
        NAME,
        meta.version == 2,
        "two appends raise the version to two",
    );

    let mut result = ScenarioObservation::new(NAME);
    if let Ok(offset) = &first {
        result.push(Observation::U64(offset.0));
    }
    if let Ok(offset) = &second {
        result.push(Observation::U64(offset.0));
    }
    result.push(Observation::Meta(MetaObservation::from(&meta)));
    result
}

/// Read the full stream and one tail page, byte-exact.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_read_byte_exact(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "read_byte_exact";
    let name = stream_name("scenario/read");
    let vfs = stream.vfs();
    vfs.create(&name, empty_meta("application/octet-stream"))
        .await
        .expect("create");
    vfs.append(&name, b"hello ").await.expect("first append");
    vfs.append(&name, b"world").await.expect("second append");

    let all = vfs
        .read_at(&name, &VfsOffset(0), 1024)
        .await
        .expect("full read");
    let tail = vfs
        .read_at(&name, &VfsOffset(6), 1024)
        .await
        .expect("tail read");
    let past = vfs
        .read_at(&name, &VfsOffset(11), 1024)
        .await
        .expect("end read");

    check(NAME, all == b"hello world", "the full read is byte-exact");
    check(NAME, tail == b"world", "the tail read starts at offset six");
    check(NAME, past.is_empty(), "a read at the end returns no bytes");

    let mut result = ScenarioObservation::new(NAME);
    result.push(Observation::Bytes(all));
    result.push(Observation::Bytes(tail));
    result.push(Observation::Bytes(past));
    result
}

/// Read the stream in bounded pages and reassemble it.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_bounded_pages(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "bounded_pages";
    const PAGE: usize = 5;
    let name = stream_name("scenario/pages");
    stream
        .create(name.clone(), CreateSpec::default())
        .await
        .expect("create");
    for bytes in [b"aaaa".as_slice(), b"bb", b"cccccc"] {
        stream
            .append(&name, AppendRequest::new(Chunk::new(bytes.to_vec())))
            .await
            .expect("append");
    }

    let mut collected = Vec::new();
    let mut from = ReadFrom::Start;
    let mut pages = 0_u64;
    let mut largest = 0_usize;
    loop {
        let page = stream
            .read(
                &name,
                ReadRequest {
                    from: from.clone(),
                    limit: ReadLimit::new(Some(PAGE), None),
                },
            )
            .await
            .expect("bounded read");
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.as_bytes().to_vec())
            .collect();
        check(
            NAME,
            bytes.len() <= PAGE,
            "each page is bounded by the limit",
        );
        largest = largest.max(bytes.len());
        collected.extend_from_slice(&bytes);
        pages += 1;
        check(NAME, pages < 64, "the read makes progress");
        if page.cursor.up_to_date {
            break;
        }
        from = ReadFrom::Offset(page.cursor.next);
    }

    check(
        NAME,
        collected == b"aaaabbcccccc",
        "the pages reassemble the stream",
    );
    check(NAME, pages > 1, "a small page splits the stream");

    let mut result = ScenarioObservation::new(NAME);
    result.push(Observation::U64(pages));
    result.push(Observation::U64(u64::try_from(largest).unwrap_or(u64::MAX)));
    result.push(Observation::Bytes(collected));
    result
}

/// A fork reads only its inherited prefix, which does not grow.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_fork_prefix(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "fork_prefix";
    let source = stream_name("scenario/fork-source");
    let child = stream_name("scenario/fork-prefix");
    let vfs = stream.vfs();
    vfs.create(&source, empty_meta("application/octet-stream"))
        .await
        .expect("create source");
    vfs.append(&source, b"0123456789")
        .await
        .expect("append source");

    let mut child_meta = empty_meta("application/octet-stream");
    child_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    child_meta.len = 4;
    vfs.create(&child, child_meta).await.expect("create fork");

    let prefix = vfs
        .read_at(&child, &VfsOffset(0), 1024)
        .await
        .expect("fork read");
    vfs.append(&source, b"ABCDE")
        .await
        .expect("append source again");
    let after = vfs
        .read_at(&child, &VfsOffset(0), 1024)
        .await
        .expect("fork read again");
    let meta = vfs.meta(&child).await.expect("fork meta");

    check(NAME, prefix == b"0123", "the fork reads its prefix");
    check(
        NAME,
        after == b"0123",
        "a later source append stays invisible",
    );
    check(NAME, meta.len == 4, "the fork keeps its inherited length");

    let mut result = ScenarioObservation::new(NAME);
    result.push(Observation::Bytes(prefix));
    result.push(Observation::Bytes(after));
    result.push(Observation::Meta(MetaObservation::from(&meta)));
    result
}

/// A fork append continues after the inherited prefix.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_fork_append(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "fork_append";
    let source = stream_name("scenario/fork-append-source");
    let child = stream_name("scenario/fork-append");
    let vfs = stream.vfs();
    vfs.create(&source, empty_meta("application/octet-stream"))
        .await
        .expect("create source");
    vfs.append(&source, b"0123456789")
        .await
        .expect("append source");

    let mut child_meta = empty_meta("application/octet-stream");
    child_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    child_meta.len = 4;
    vfs.create(&child, child_meta).await.expect("create fork");
    vfs.append(&child, b"xyz").await.expect("append fork");

    let bytes = vfs
        .read_at(&child, &VfsOffset(0), 1024)
        .await
        .expect("fork read");
    let meta = vfs.meta(&child).await.expect("fork meta");

    check(
        NAME,
        bytes == b"0123xyz",
        "the fork append follows the prefix",
    );
    check(
        NAME,
        meta.len == 7,
        "the fork length counts the prefix and the append",
    );

    let mut result = ScenarioObservation::new(NAME);
    result.push(Observation::Bytes(bytes));
    result.push(Observation::Meta(MetaObservation::from(&meta)));
    result
}

/// A source with a fork is soft-deleted; removing the fork removes both.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_soft_delete_cascade(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "soft_delete_cascade";
    let source = stream_name("scenario/cascade-source");
    let child = stream_name("scenario/cascade-child");
    let vfs = stream.vfs();
    vfs.create(&source, empty_meta("text/plain"))
        .await
        .expect("create source");
    vfs.append(&source, b"abcdef").await.expect("append source");

    let mut child_meta = empty_meta("text/plain");
    child_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    child_meta.len = 4;
    vfs.create(&child, child_meta).await.expect("create fork");

    let removed_source = vfs.remove(&source).await;
    let source_meta = vfs.meta(&source).await;
    let child_read = vfs.read_at(&child, &VfsOffset(0), 1024).await;
    let removed_child = vfs.remove(&child).await;
    let child_meta = vfs.meta(&child).await;
    let source_after = vfs.meta(&source).await;

    check(
        NAME,
        matches!(removed_source, Ok(true)),
        "the referenced source soft-deletes",
    );
    check(
        NAME,
        matches!(source_meta, Err(VfsError::Gone)),
        "the soft-deleted source is gone",
    );
    check(
        NAME,
        matches!(&child_read, Ok(bytes) if bytes == b"abcd"),
        "the fork still reads",
    );
    check(
        NAME,
        matches!(removed_child, Ok(false)),
        "the fork hard-deletes",
    );
    check(
        NAME,
        matches!(child_meta, Err(VfsError::NotFound)),
        "the fork is not found",
    );
    check(
        NAME,
        matches!(source_after, Err(VfsError::NotFound)),
        "the cascade removed the source",
    );

    let mut result = ScenarioObservation::new(NAME);
    result.push(result_observation(&removed_source));
    result.push(result_observation(&source_meta));
    if let Ok(bytes) = child_read {
        result.push(Observation::Bytes(bytes));
    }
    result.push(result_observation(&removed_child));
    result.push(result_observation(&child_meta));
    result.push(result_observation(&source_after));
    result
}

/// The sliding TTL window moves on each read and then expires the stream.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_ttl_slide(
    stream: &DurableStream,
    clock: &SimulatedClock,
) -> ScenarioObservation {
    const NAME: &str = "ttl_slide";
    const TTL: Duration = Duration::from_millis(1000);
    let name = stream_name("scenario/ttl");
    stream
        .create(
            name.clone(),
            CreateSpec {
                expires: Some(Expiry::Ttl(TTL)),
                initial: Some(Chunk::new(b"data".to_vec())),
                ..CreateSpec::default()
            },
        )
        .await
        .expect("create");

    clock.advance_wall(Duration::from_millis(500));
    let first = stream.read(&name, read_from_start()).await;
    clock.advance_wall(Duration::from_millis(700));
    let second = stream.read(&name, read_from_start()).await;
    clock.advance_wall(Duration::from_millis(1100));
    let third = stream.read(&name, read_from_start()).await;

    check(NAME, first.is_ok(), "a read inside the window succeeds");
    check(NAME, second.is_ok(), "a later read slides the window");
    check(
        NAME,
        matches!(third, Err(super::StoreError::NotFound)),
        "the stream expires",
    );

    let mut result = ScenarioObservation::new(NAME);
    result.push(Observation::Bool(first.is_ok()));
    result.push(Observation::Bool(second.is_ok()));
    result.push(match &third {
        Ok(_) => Observation::Text("ok".to_string()),
        Err(error) => Observation::Error(format!("{error:?}")),
    });
    result
}

/// A stale expected version refuses the write and stores no bytes.
///
/// # Panics
///
/// Panics with the scenario name when an invariant fails.
pub async fn scenario_version_conflict(stream: &DurableStream) -> ScenarioObservation {
    const NAME: &str = "version_conflict";
    let name = stream_name("scenario/version");
    let vfs = stream.vfs();
    vfs.create(&name, empty_meta("application/octet-stream"))
        .await
        .expect("create");
    let first = vfs.meta(&name).await.expect("metadata");

    let appended = vfs
        .append_with_meta(&name, b"abc", first.clone(), first.version)
        .await;
    let stale = vfs
        .append_with_meta(&name, b"def", first.clone(), first.version)
        .await;
    let bytes = vfs.read_at(&name, &VfsOffset(0), 1024).await.expect("read");
    let meta = vfs.meta(&name).await.expect("metadata after");

    check(
        NAME,
        matches!(&appended, Ok(offset) if offset.0 == 3),
        "the current version appends",
    );
    check(
        NAME,
        matches!(stale, Err(VfsError::Conflict)),
        "a stale version conflicts",
    );
    check(NAME, bytes == b"abc", "a refused append stores no bytes");
    check(
        NAME,
        meta.len == 3,
        "the length is unchanged by the refusal",
    );

    let mut result = ScenarioObservation::new(NAME);
    if let Ok(offset) = appended {
        result.push(Observation::U64(offset.0));
    }
    result.push(result_observation(&stale));
    result.push(Observation::Bytes(bytes));
    result.push(Observation::Meta(MetaObservation::from(&meta)));
    result
}

fn read_from_start() -> ReadRequest {
    ReadRequest {
        from: ReadFrom::Start,
        limit: ReadLimit::default(),
    }
}
