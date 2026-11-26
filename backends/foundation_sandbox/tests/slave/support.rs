//! The support of the slave tests: a slave process on a Unix socket and a
//! caller for it.
//!
//! Every test sends `RootSource::Plain`. The helper starts the small program
//! `tests/support/slave_main.rs`, whose `main` blocks `SIGCHLD` before any
//! thread starts. A libtest test cannot be that process: the harness starts a
//! thread first, and the process main thread keeps `SIGCHLD` unblocked, so the
//! reaper's `signalfd` misses a child.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    ClientOptions, ConnectError, H1Transport, Request, RpcCtx, ServerStream,
};
use foundation_core::valtron::{block_on_future, initialize_pool};
use foundation_errstacks::ErrorTrace;
use foundation_netio::HttpClientBuilder;

use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::rpc::messages::{
    Empty, ExecEventKind, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest,
    ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo, SlaveLimits,
};
use foundation_sandbox::shared::rpc::SlaveServiceClient;
use foundation_sandbox::shared::spec::{RootMode, RootSetup, RootSource};

/// A temporary directory that is removed on drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A new directory below the system temporary directory.
    ///
    /// # Panics
    /// When the directory cannot be made.
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("ewe-slave-{tag}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).expect("the temporary directory");
        Self { path }
    }

    /// The path of the directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A path inside the directory.
    #[must_use]
    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A slave process that listens on a Unix socket.
pub struct SlaveProcess {
    child: Child,
    _directory: TempDir,
    /// The caller on the socket.
    pub caller: Caller,
}

impl SlaveProcess {
    /// Starts a slave and waits until it answers `Info`.
    ///
    /// # Panics
    /// When the process does not start or does not answer.
    pub fn start(tag: &str, plain_root: Option<PathBuf>, threads: usize) -> Self {
        let directory = TempDir::new(tag);
        let socket = directory.join("slave.sock");
        let plain_root = plain_root.unwrap_or_else(|| PathBuf::from("/"));
        // The driver blocks `SIGCHLD` in `main`, before any thread starts.
        let program = env!("CARGO_BIN_EXE_slave_test_driver");
        let mut child = Command::new(program)
            .env("EWE_SLAVE_SOCKET", &socket)
            .env("EWE_SLAVE_PLAIN_ROOT", &plain_root)
            .env("EWE_SLAVE_THREADS", threads.to_string())
            .env("SLAVE_SECRET", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the slave process starts");
        let caller = Caller::connect(&socket);
        let mut attempts = 0_usize;
        loop {
            if caller.info().is_ok() {
                break;
            }
            attempts += 1;
            assert!(attempts < 1_000, "the slave did not answer within 20 s");
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = &mut child;
        Self {
            child,
            _directory: directory,
            caller,
        }
    }

    /// Starts a slave with a fresh temporary plain root and prepares it.
    ///
    /// # Panics
    /// When the slave does not start or `Prepare` fails.
    pub fn start_prepared(tag: &str) -> (Self, TempDir) {
        let root = TempDir::new(&format!("{tag}-root"));
        let slave = Self::start(tag, Some(root.path().to_path_buf()), 4);
        slave.prepare(&plain_prepare("/work"));
        (slave, root)
    }

    /// The process identifier of the slave.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Waits for the slave to end and returns its exit code.
    ///
    /// # Panics
    /// When the slave does not end within 20 s.
    pub fn wait(&mut self) -> i32 {
        let mut attempts = 0_usize;
        loop {
            if let Some(status) = self.child.try_wait().expect("the slave state") {
                return status.code().unwrap_or(-1);
            }
            attempts += 1;
            assert!(attempts < 1_000, "the slave did not end within 20 s");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Prepares the root with the given request.
    ///
    /// # Panics
    /// When `Prepare` fails.
    pub fn prepare(&self, request: &PrepareRequest) -> PrepareResponse {
        self.caller.prepare(request).expect("Prepare succeeds")
    }
}

impl Drop for SlaveProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A caller on the socket of one slave.
#[derive(Clone)]
pub struct Caller {
    client: Arc<SlaveServiceClient>,
}

impl Caller {
    /// Connects to a slave socket.
    ///
    /// # Panics
    /// When the client cannot be built.
    pub fn connect(socket: &Path) -> Self {
        let http = HttpClientBuilder::new()
            .unix_socket(socket)
            .read_timeout(Duration::from_secs(60))
            .build();
        let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));
        let client = SlaveServiceClient::new(
            transport,
            "http://slave.local",
            ClientOptions::new().with_codec("json"),
        )
        .expect("the client builds");
        Self {
            client: Arc::new(client),
        }
    }

    fn rpc_ctx() -> RpcCtx {
        RpcCtx::background().with_deadline(Duration::from_secs(60))
    }

    /// Calls `Info`.
    pub fn info(&self) -> Result<SlaveInfo, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        block_on(async move {
            client
                .info(Self::rpc_ctx(), Request::new(InfoRequest {}))
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `Prepare`.
    pub fn prepare(
        &self,
        request: &PrepareRequest,
    ) -> Result<PrepareResponse, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let request = request.clone();
        block_on(async move { client.prepare(Self::rpc_ctx(), Request::new(request)).await })
            .map(|response| response.msg)
    }

    /// Opens one `Exec` stream.
    pub fn exec(
        &self,
        request: ExecRequest,
    ) -> Result<ServerStream<ExecEventMsg>, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        block_on(async move { client.exec(Self::rpc_ctx(), Request::new(request)).await })
    }

    /// The error of an `Exec` call that must fail.
    ///
    /// A server-streaming call can report the failure at open or at the first
    /// `receive`; both are the failure of the call.
    ///
    /// # Panics
    /// When the call succeeds.
    pub fn exec_error(&self, request: ExecRequest) -> SandboxError {
        match self.exec(request) {
            Err(error) => sandbox_error(&error),
            Ok(stream) => block_on(async move {
                let mut stream = stream;
                match stream.receive().await {
                    Err(error) => sandbox_error(&error),
                    Ok(event) => panic!("the call did not fail: {event:?}"),
                }
            }),
        }
    }

    /// Calls `ExecInput`.
    pub fn exec_input(
        &self,
        request: ExecInputRequest,
    ) -> Result<ExecInputResponse, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        block_on(async move {
            client
                .exec_input(Self::rpc_ctx(), Request::new(request))
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `ExecSignal`.
    pub fn exec_signal(
        &self,
        request: ExecSignalRequest,
    ) -> Result<Empty, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        block_on(async move {
            client
                .exec_signal(Self::rpc_ctx(), Request::new(request))
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `FileWriteBegin`.
    pub fn file_write_begin(
        &self,
        path: &str,
        mode: u32,
    ) -> Result<FileWriteBeginResponse, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let path = path.to_string();
        block_on(async move {
            client
                .file_write_begin(
                    Self::rpc_ctx(),
                    Request::new(FileWriteBeginRequest { path, mode }),
                )
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `FileWriteChunk`.
    pub fn file_write_chunk(
        &self,
        upload_id: &str,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<FileWriteChunkResponse, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let upload_id = upload_id.to_string();
        block_on(async move {
            client
                .file_write_chunk(
                    Self::rpc_ctx(),
                    Request::new(FileWriteChunkRequest {
                        upload_id,
                        offset,
                        data,
                    }),
                )
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `FileWriteCommit`.
    pub fn file_write_commit(
        &self,
        upload_id: &str,
        size: u64,
    ) -> Result<FileWriteCommitResponse, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let upload_id = upload_id.to_string();
        block_on(async move {
            client
                .file_write_commit(
                    Self::rpc_ctx(),
                    Request::new(FileWriteCommitRequest { upload_id, size }),
                )
                .await
        })
        .map(|response| response.msg)
    }

    /// Calls `FileWriteAbort`.
    pub fn file_write_abort(&self, upload_id: &str) -> Result<Empty, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let upload_id = upload_id.to_string();
        block_on(async move {
            client
                .file_write_abort(
                    Self::rpc_ctx(),
                    Request::new(FileWriteAbortRequest { upload_id }),
                )
                .await
        })
        .map(|response| response.msg)
    }

    /// Reads a whole file through `FileRead`.
    pub fn file_read(&self, path: &str) -> Result<Vec<FileChunk>, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        let path = path.to_string();
        block_on(async move {
            let mut stream = client
                .file_read(Self::rpc_ctx(), Request::new(FileReadRequest { path }))
                .await?;
            let mut chunks = Vec::new();
            while let Some(chunk) = stream.receive().await? {
                chunks.push(chunk);
            }
            Ok(chunks)
        })
    }

    /// Calls `Shutdown`.
    pub fn shutdown(&self, grace_ms: u32) -> Result<Empty, ErrorTrace<ConnectError>> {
        let client = Arc::clone(&self.client);
        block_on(async move {
            client
                .shutdown(Self::rpc_ctx(), Request::new(ShutdownRequest { grace_ms }))
                .await
        })
        .map(|response| response.msg)
    }
}

/// Drives one future on the test pool.
///
/// # Panics
/// When the pool cannot start.
pub fn block_on<F>(future: F) -> F::Output
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    static POOL: OnceLock<()> = OnceLock::new();
    POOL.get_or_init(|| {
        std::mem::forget(initialize_pool(1, Some(4)));
    });
    block_on_future(future)
}

/// The default limits of a test.
#[must_use]
pub fn limits() -> SlaveLimits {
    SlaveLimits {
        chunk_bytes: 64 * 1024,
        max_exec_sessions: 128,
        output_drain_timeout_ms: 2_000,
        upload_idle_timeout_ms: 60_000,
        input_queue_bytes: 4 * 64 * 1024,
        input_pipe_bytes: 64 * 1024,
        max_message_bytes: 4 * 1024 * 1024,
        event_queue_len: 4,
        max_unpack_entries: 1_000_000,
        max_unpack_bytes: 16 * 1024 * 1024 * 1024,
        max_unpack_path_bytes: 4096,
    }
}

/// A `Prepare` request with a plain root.
#[must_use]
pub fn plain_prepare(workdir: &str) -> PrepareRequest {
    PrepareRequest {
        root: RootSetup {
            source: RootSource::Plain,
            mode: RootMode::Overlay,
        },
        workdir: workdir.to_string(),
        env: Vec::new(),
        mounts: Vec::new(),
        dns: Vec::new(),
        limits: limits(),
    }
}

/// An `Exec` request.
#[must_use]
pub fn exec_request(exec_id: &str, argv: &[&str]) -> ExecRequest {
    ExecRequest {
        exec_id: exec_id.to_string(),
        argv: argv.iter().map(|value| (*value).to_string()).collect(),
        env: Vec::new(),
        cwd: None,
        stdin: false,
    }
}

/// An `Exec` request that reads standard input.
#[must_use]
pub fn exec_request_stdin(exec_id: &str, argv: &[&str]) -> ExecRequest {
    let mut request = exec_request(exec_id, argv);
    request.stdin = true;
    request
}

/// Reads every event of a stream.
///
/// # Panics
/// When the stream fails.
pub fn drain(stream: ServerStream<ExecEventMsg>) -> Vec<ExecEventMsg> {
    block_on(async move {
        let mut stream = stream;
        let mut events = Vec::new();
        while let Some(event) = stream.receive().await.expect("a stream event") {
            events.push(event);
        }
        events
    })
}

/// Reads `count` events of a stream and gives the stream back.
///
/// # Panics
/// When the stream fails.
pub fn take_events(
    stream: ServerStream<ExecEventMsg>,
    count: usize,
) -> (Vec<ExecEventMsg>, ServerStream<ExecEventMsg>) {
    let (events, stream) = block_on(async move {
        let mut stream = stream;
        let mut events = Vec::new();
        for _ in 0..count {
            match stream.receive().await.expect("a stream event") {
                Some(event) => events.push(event),
                None => break,
            }
        }
        (events, stream)
    });
    (events, stream)
}

/// The bytes of the `Stdout` events of a list.
#[must_use]
pub fn stdout_of(events: &[ExecEventMsg]) -> Vec<u8> {
    stream_of(events, true)
}

/// The bytes of the `Stderr` events of a list.
#[must_use]
pub fn stderr_of(events: &[ExecEventMsg]) -> Vec<u8> {
    stream_of(events, false)
}

fn stream_of(events: &[ExecEventMsg], stdout: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for event in events {
        match (&event.event, stdout) {
            (ExecEventKind::Stdout { data }, true) | (ExecEventKind::Stderr { data }, false) => {
                bytes.extend_from_slice(data);
            }
            _ => {}
        }
    }
    bytes
}

/// The `(code, signal)` of the one `Exit` event.
///
/// # Panics
/// When there is no `Exit` event or it is not last.
#[must_use]
pub fn exit_of(events: &[ExecEventMsg]) -> (Option<i32>, Option<i32>) {
    let last = events.last().expect("an Exit event");
    match &last.event {
        ExecEventKind::Exit { code, signal } => {
            assert!(
                events
                    .iter()
                    .filter(|event| matches!(event.event, ExecEventKind::Exit { .. }))
                    .count()
                    == 1,
                "one Exit event"
            );
            (*code, *signal)
        }
        other => panic!("the last event is not Exit: {other:?}"),
    }
}

/// The process identifier of the `Started` event.
///
/// # Panics
/// When the first event is not `Started`.
#[must_use]
pub fn started_pid(events: &[ExecEventMsg]) -> u32 {
    match &events.first().expect("a Started event").event {
        ExecEventKind::Started { pid } => *pid,
        other => panic!("the first event is not Started: {other:?}"),
    }
}

/// The `SandboxError` of a Connect failure.
#[must_use]
pub fn sandbox_error(error: &ErrorTrace<ConnectError>) -> SandboxError {
    SandboxError::from_connect(error.current_context())
}

/// The slave error code of a Connect failure, with its error number.
#[must_use]
pub fn slave_code(error: &ErrorTrace<ConnectError>) -> Option<(String, Option<i32>)> {
    match sandbox_error(error) {
        SandboxError::SlaveError { code, errno, .. } => Some((code.to_string(), errno)),
        _ => None,
    }
}

/// Waits until the condition holds.
///
/// # Panics
/// When it does not hold within `timeout`.
pub fn wait_until(mut condition: impl FnMut() -> bool, timeout: Duration) {
    let attempts = (timeout.as_millis() / 10 + 1) as usize;
    for _ in 0..attempts {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(condition(), "the condition did not hold within {timeout:?}");
}

/// One execution identifier in the `ExecId` shape.
#[must_use]
pub fn hex(value: u64) -> String {
    format!("{value:016x}")
}

/// Whether a process with this identifier exists.
#[must_use]
pub fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// The zombies of one parent, from `/proc`.
#[must_use]
pub fn zombies_of(parent: u32) -> Vec<u32> {
    let mut zombies = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return zombies;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // The format is `pid (name) state ppid ...`, and the name can hold a
        // space and a parenthesis, so the state follows the last `)`.
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if fields.len() > 1 && fields[0] == "Z" && fields[1] == parent.to_string() {
            zombies.push(pid);
        }
    }
    zombies
}
