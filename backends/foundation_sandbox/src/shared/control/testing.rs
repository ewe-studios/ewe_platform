//! Simulation helpers for the control-plane tests.
//!
//! These helpers give no isolation. They run the real client, the real codec
//! and the real router in the same process, with no socket. Use them in tests
//! only.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Waker;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use foundation_connectrpc::shared::transport::{
    body_stream_from_pipe, head_stream_from_pipe, BoxFuture as ConnectBoxFuture, Transport,
    TransportCapabilities, TransportError, TransportStream,
};
use foundation_connectrpc::{
    CallHooks, ConnectError, ConnectResult, ConnectRpcHandler, Request, RequestBody, Response,
    ResponseSink, Router, RpcCtx,
};
use foundation_context::Ctx;
use foundation_core::valtron::{from_future, send, Pipe, PipeSender, Stream as ValtronStream};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SendSafeBody, SimpleHeaders, SimpleIncomingRequest, Status,
};
use futures::Stream;

use crate::shared::container::{
    ContainerControl, ContainerHandle, ContainerId, ContainerInfo, ContainerSpec,
};
use crate::shared::control::record::RecordStore;
use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs as _;
use crate::shared::host::SlaveConnector;
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    Empty, ExecEventKind, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest,
    ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo, UnpackAbortRequest, UnpackBeginRequest, UnpackBeginResponse, UnpackChunkRequest,
    UnpackChunkResponse, UnpackCommitRequest, UnpackCommitResponse,
};
use crate::shared::rpc::{
    register_sandbox_service, register_slave_service, SandboxService, SlaveService,
};
use crate::shared::spec::{GuestMount, RootMode, RootSetup, RootSource, SandboxSpec};

/// The capabilities of a host that can run everything.
#[must_use]
pub fn full_capabilities() -> Capabilities {
    Capabilities {
        rootfs_directory: true,
        rootfs_ext4: true,
        rootfs_squashfs: true,
        rootfs_container_image: true,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly, RootMode::Writable],
        host_mounts: true,
        network_none: true,
        network_host: true,
        max_vcpus: 8,
    }
}

/// One operation that a simulated substrate recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubstrateOp {
    /// `terminate` was called.
    Terminate,
    /// `kill` was called.
    Kill,
    /// `cleanup` was called.
    Cleanup,
}

#[derive(Default)]
struct ExitState {
    exit: Option<SubstrateExit>,
    wakers: Vec<Waker>,
}

/// A substrate that a test controls.
pub struct SimSubstrate {
    state: Mutex<ExitState>,
    ops: Mutex<Vec<SubstrateOp>>,
    log_tail: Mutex<Vec<u8>>,
    container: Option<ContainerHandle>,
    exit_error: Option<SandboxError>,
}

impl SimSubstrate {
    /// A substrate that runs until the test says otherwise.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ExitState::default()),
            ops: Mutex::new(Vec::new()),
            log_tail: Mutex::new(Vec::new()),
            container: None,
            exit_error: None,
        }
    }

    /// A substrate with a log tail.
    #[must_use]
    pub fn with_log_tail(mut self, bytes: Vec<u8>) -> Self {
        *self
            .log_tail
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner) = bytes;
        self
    }

    /// A substrate with a container handle.
    #[must_use]
    pub fn with_container(mut self, container: ContainerHandle) -> Self {
        self.container = Some(container);
        self
    }

    /// A substrate with an exact exit error.
    #[must_use]
    pub fn with_exit_error(mut self, error: SandboxError) -> Self {
        self.exit_error = Some(error);
        self
    }

    /// Clears the exit, so a later launch of the same scripted substrate runs.
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.exit = None;
        state.wakers.clear();
    }

    /// Makes the substrate exit now.
    pub fn exit_now(&self, exit: SubstrateExit) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.exit = Some(exit);
        for waker in state.wakers.drain(..) {
            waker.wake();
        }
    }

    /// The operations that the control plane called.
    #[must_use]
    pub fn ops(&self) -> Vec<SubstrateOp> {
        self.ops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How often one operation was called.
    #[must_use]
    pub fn op_count(&self, op: SubstrateOp) -> usize {
        self.ops()
            .iter()
            .filter(|recorded| **recorded == op)
            .count()
    }

    fn record(&self, op: SubstrateOp) {
        self.ops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(op);
    }
}

impl Default for SimSubstrate {
    fn default() -> Self {
        Self::new()
    }
}

/// The handle wrapper that gives a [`SimSubstrate`] to the control plane.
struct SimHandle(Arc<SimSubstrate>);

impl SubstrateHandle for SimHandle {
    fn exited(&self) -> crate::shared::BoxFuture<'_, SubstrateExit> {
        let substrate = Arc::clone(&self.0);
        Box::pin(futures::future::poll_fn(move |context| {
            substrate.poll_exit(context)
        }))
    }

    fn terminate(
        &self,
        _grace: Duration,
    ) -> crate::shared::BoxFuture<'_, SandboxResult<SubstrateExit>> {
        self.0.record(SubstrateOp::Terminate);
        let exit = SubstrateExit {
            code: None,
            signal: Some(9),
            forced: true,
        };
        self.0.exit_now(exit);
        Box::pin(async move { Ok(exit) })
    }

    fn kill(&self) -> crate::shared::BoxFuture<'_, SandboxResult<SubstrateExit>> {
        self.0.record(SubstrateOp::Kill);
        let exit = SubstrateExit {
            code: None,
            signal: Some(9),
            forced: true,
        };
        self.0.exit_now(exit);
        Box::pin(async move { Ok(exit) })
    }

    fn log_tail(&self, max: usize) -> crate::shared::BoxFuture<'_, Vec<u8>> {
        let mut bytes = self
            .0
            .log_tail
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        bytes.truncate(max);
        Box::pin(async move { bytes })
    }

    fn cleanup(&self) -> crate::shared::BoxFuture<'_, SandboxResult<()>> {
        self.0.record(SubstrateOp::Cleanup);
        Box::pin(async { Ok(()) })
    }

    fn container(&self) -> Option<ContainerHandle> {
        self.0.container.clone()
    }

    fn exit_error(&self, exit: &SubstrateExit, log_tail: Vec<u8>) -> SandboxError {
        self.0
            .exit_error
            .clone()
            .unwrap_or(SandboxError::SubstrateExited {
                code: exit.code,
                log_tail,
            })
    }
}

impl SimSubstrate {
    fn poll_exit(&self, context: &mut Context<'_>) -> Poll<SubstrateExit> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(exit) = state.exit {
            return Poll::Ready(exit);
        }
        state.wakers.push(context.waker().clone());
        if let Some(exit) = state.exit {
            return Poll::Ready(exit);
        }
        Poll::Pending
    }
}

/// How the simulated driver answers `attach`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttachMode {
    /// The substrate does not exist.
    #[default]
    None,
    /// The substrate exists.
    Found,
}

struct DriverScript {
    progress: Vec<CreateProgress>,
    launch_error: Option<SandboxError>,
    launch_calls: usize,
    probe_calls: usize,
    attach_calls: usize,
    substrate: Arc<SimSubstrate>,
    endpoint: SlaveEndpoint,
    root: RootSetup,
    mounts: Vec<GuestMount>,
    attach: AttachMode,
    launch_spec: Option<SandboxSpec>,
    fresh_substrate: bool,
}

/// A driver that a test scripts.
pub struct SimDriver {
    id: DriverId,
    report: HostReport,
    script: Mutex<DriverScript>,
    container: Option<Arc<dyn ContainerControl>>,
}

impl SimDriver {
    /// A usable driver with the full capabilities and no progress values.
    #[must_use]
    pub fn new(id: DriverId) -> Self {
        Self::with_report(HostReport {
            driver: id,
            usable: true,
            checks: Vec::new(),
            capabilities: full_capabilities(),
        })
    }

    /// A driver with an exact report.
    #[must_use]
    pub fn with_report(report: HostReport) -> Self {
        let id = report.driver;
        let root = RootSetup {
            source: RootSource::Plain,
            mode: RootMode::Overlay,
        };
        Self {
            id,
            report,
            script: Mutex::new(DriverScript {
                progress: vec![CreateProgress::Preparing, CreateProgress::SubstrateStarted],
                launch_error: None,
                launch_calls: 0,
                probe_calls: 0,
                attach_calls: 0,
                substrate: Arc::new(SimSubstrate::new()),
                endpoint: SlaveEndpoint::Unix {
                    path: std::path::PathBuf::from("/tmp/sim-slave.sock"),
                },
                root,
                mounts: Vec::new(),
                attach: AttachMode::None,
                launch_spec: None,
                fresh_substrate: false,
            }),
            container: None,
        }
    }

    /// A driver that cannot run on this host.
    #[must_use]
    pub fn unusable(id: DriverId, reason: &str) -> Self {
        let driver = Self::with_report(HostReport {
            driver: id,
            usable: false,
            checks: vec![HostCheck {
                name: "usable".to_string(),
                ok: false,
                message: reason.to_string(),
            }],
            capabilities: full_capabilities(),
        });
        driver
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .progress
            .clear();
        driver
    }

    /// Replaces the launch progress values.
    #[must_use]
    pub fn with_progress(self, progress: Vec<CreateProgress>) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .progress = progress;
        self
    }

    /// Makes the launch stream fail.
    #[must_use]
    pub fn with_launch_error(self, error: SandboxError) -> Self {
        self.set_launch_error(error);
        self
    }

    /// Makes the launch stream fail, on a driver that a test already holds.
    pub fn set_launch_error(&self, error: SandboxError) {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .launch_error = Some(error);
    }

    /// Replaces the root that the driver gives the slave.
    #[must_use]
    pub fn with_root(self, root: RootSetup) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .root = root;
        self
    }

    /// Replaces the mounts that the driver gives the slave.
    #[must_use]
    pub fn with_mounts(self, mounts: Vec<GuestMount>) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .mounts = mounts;
        self
    }

    /// Sets the endpoint that the driver reports.
    #[must_use]
    pub fn with_endpoint(self, endpoint: SlaveEndpoint) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .endpoint = endpoint;
        self
    }

    /// Sets the attach answer.
    #[must_use]
    pub fn with_attach(self, attach: AttachMode) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .attach = attach;
        self
    }

    /// Makes every launch start from a substrate that did not exit.
    ///
    /// A test that stops several sandboxes on one driver needs this: a
    /// simulated substrate stays exited after its first stop.
    #[must_use]
    pub fn with_fresh_substrate(self) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fresh_substrate = true;
        self
    }

    /// Gives the driver a container control.
    #[must_use]
    pub fn with_container(mut self, control: Arc<dyn ContainerControl>) -> Self {
        self.container = Some(control);
        self
    }

    /// The simulated substrate of this driver.
    #[must_use]
    pub fn substrate(&self) -> Arc<SimSubstrate> {
        Arc::clone(
            &self
                .script
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .substrate,
        )
    }

    /// How often the driver was probed.
    #[must_use]
    pub fn probe_calls(&self) -> usize {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .probe_calls
    }

    /// How often the driver was asked to launch.
    #[must_use]
    pub fn launch_calls(&self) -> usize {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .launch_calls
    }

    /// How often the driver was asked to attach.
    #[must_use]
    pub fn attach_calls(&self) -> usize {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .attach_calls
    }

    /// The spec of the last launch.
    #[must_use]
    pub fn launch_spec(&self) -> Option<SandboxSpec> {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .launch_spec
            .clone()
    }
}

impl SandboxDriver for SimDriver {
    fn id(&self) -> DriverId {
        self.id
    }

    fn probe(&self, _ctx: Ctx) -> crate::shared::BoxFuture<'static, HostReport> {
        let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
        script.probe_calls += 1;
        drop(script);
        let report = self.report.clone();
        Box::pin(async move { report })
    }

    fn launch(
        &self,
        _ctx: Ctx,
        _id: SandboxId,
        spec: SandboxSpec,
        _deadline: foundation_compact::Instant,
    ) -> LaunchStream {
        let (progress, error, substrate, endpoint, root, mounts) = {
            let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
            script.launch_calls += 1;
            script.launch_spec = Some(spec);
            if script.fresh_substrate {
                script.substrate.reset();
            }
            (
                script.progress.clone(),
                script.launch_error.clone(),
                Arc::clone(&script.substrate),
                script.endpoint.clone(),
                script.root.clone(),
                script.mounts.clone(),
            )
        };
        let mut items: Vec<ValtronStream<SandboxResult<Substrate>, CreateProgress>> =
            progress.into_iter().map(ValtronStream::Pending).collect();
        if let Some(error) = error {
            items.push(ValtronStream::Next(Err(error.into())));
        } else {
            let substrate = Substrate {
                endpoint,
                root,
                mounts,
                handle: Box::new(SimHandle(substrate)),
                calls: None,
                slave: None,
            };
            items.push(ValtronStream::Next(Ok(substrate)));
        }
        Box::new(items.into_iter())
    }

    fn attach(
        &self,
        _ctx: Ctx,
        record: &StoredRecord,
    ) -> crate::shared::BoxFuture<'_, SandboxResult<Option<Substrate>>> {
        let (attach, substrate, endpoint, root, mounts) = {
            let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
            script.attach_calls += 1;
            (
                script.attach,
                Arc::clone(&script.substrate),
                record
                    .endpoint
                    .clone()
                    .unwrap_or_else(|| script.endpoint.clone()),
                script.root.clone(),
                script.mounts.clone(),
            )
        };
        Box::pin(async move {
            match attach {
                AttachMode::None => Ok(None),
                AttachMode::Found => Ok(Some(Substrate {
                    endpoint,
                    root,
                    mounts,
                    handle: Box::new(SimHandle(substrate)),
                    calls: None,
                    slave: None,
                })),
            }
        })
    }

    fn containers(&self) -> Option<Arc<dyn ContainerControl>> {
        self.container.clone()
    }
}

/// One call that the simulated slave received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlaveCall {
    /// `Info`.
    Info,
    /// `Prepare`.
    Prepare {
        /// The work directory.
        workdir: String,
        /// The root.
        root: RootSetup,
    },
    /// `Exec`.
    Exec {
        /// The execution identifier.
        exec_id: String,
        /// The argument vector.
        argv: Vec<String>,
        /// Whether the program reads input.
        stdin: bool,
        /// The user of the command (feature 12).
        user: Option<String>,
    },
    /// `ExecInput`.
    ExecInput {
        /// The sequence number.
        seq: u64,
        /// Whether the chunk ends the input.
        eof: bool,
        /// The data length.
        len: usize,
    },
    /// `ExecSignal`.
    ExecSignal {
        /// The signal number.
        signal: i32,
    },
    /// `FileWriteBegin`.
    FileWriteBegin {
        /// The path.
        path: String,
    },
    /// `FileWriteChunk`.
    FileWriteChunk {
        /// The offset.
        offset: u64,
    },
    /// `FileWriteCommit`.
    FileWriteCommit {
        /// The size.
        size: u64,
    },
    /// `FileWriteAbort`.
    FileWriteAbort {
        /// The upload.
        upload_id: String,
    },
    /// `FileRead`.
    FileRead {
        /// The path.
        path: String,
    },
    /// `UnpackBegin`.
    UnpackBegin {
        /// Whether an existing directory keeps its mode.
        keep_existing_dirs: bool,
    },
    /// `Shutdown`.
    Shutdown {
        /// The grace in milliseconds.
        grace_ms: u32,
    },
}

impl SlaveCall {
    /// The method name.
    #[must_use]
    pub fn method(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Prepare { .. } => "prepare",
            Self::Exec { .. } => "exec",
            Self::ExecInput { .. } => "exec_input",
            Self::ExecSignal { .. } => "exec_signal",
            Self::FileWriteBegin { .. } => "file_write_begin",
            Self::FileWriteChunk { .. } => "file_write_chunk",
            Self::FileWriteCommit { .. } => "file_write_commit",
            Self::FileWriteAbort { .. } => "file_write_abort",
            Self::FileRead { .. } => "file_read",
            Self::UnpackBegin { .. } => "unpack_begin",
            Self::Shutdown { .. } => "shutdown",
        }
    }
}

/// One input chunk that the simulated slave applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedInput {
    /// The sequence number.
    pub seq: u64,
    /// The bytes.
    pub data: Vec<u8>,
    /// Whether the chunk ends the input.
    pub eof: bool,
}

/// One scripted answer of `ExecInput`.
#[derive(Clone, Debug)]
struct InputScript {
    seq: u64,
    kind: InputScriptKind,
    remaining: usize,
}

#[derive(Clone, Copy, Debug)]
enum InputScriptKind {
    QueueFull,
    TransportError,
}

/// The gate that a gated `Exec` waits on, released by `ExecSignal`.
#[derive(Default)]
struct SignalGate {
    state: Mutex<GateState>,
}

#[derive(Default)]
struct GateState {
    released: bool,
    wakers: Vec<Waker>,
}

impl SignalGate {
    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.released = true;
        for waker in state.wakers.drain(..) {
            waker.wake();
        }
    }

    async fn wait(&self) {
        futures::future::poll_fn(|context| {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.released {
                return Poll::Ready(());
            }
            state.wakers.push(context.waker().clone());
            Poll::Pending
        })
        .await;
    }
}

enum ExecScript {
    Events(Vec<ExecEventMsg>),
    Error(SandboxError),
    /// These messages, then a stream error.
    StreamError {
        before: Vec<ExecEventMsg>,
        error: SandboxError,
    },
    Gated {
        before: Vec<ExecEventMsg>,
        exit: ExecEventMsg,
        gate: Arc<SignalGate>,
    },
}

struct ScriptedInner {
    calls: Mutex<Vec<SlaveCall>>,
    info_results: Mutex<VecDeque<Result<SlaveInfo, SandboxError>>>,
    default_info: Mutex<SlaveInfo>,
    prepare_results: Mutex<VecDeque<Result<PrepareResponse, SandboxError>>>,
    exec_scripts: Mutex<VecDeque<ExecScript>>,
    input_scripts: Mutex<VecDeque<InputScript>>,
    next_input_seq: Mutex<u64>,
    applied: Mutex<Vec<AppliedInput>>,
    signals: Mutex<Vec<i32>>,
    gates: Mutex<HashMap<String, Arc<SignalGate>>>,
    chunk_fail_at: Mutex<Option<(usize, SandboxError)>>,
    chunk_calls: Mutex<usize>,
    read_chunks: Mutex<VecDeque<FileChunk>>,
    shutdown_grace: Mutex<Option<u32>>,
    shutdown_exit: Mutex<Option<Arc<SimSubstrate>>>,
    shutdown_count: Mutex<usize>,
    info_pending: AtomicBool,
    shutdown_error: Mutex<Option<SandboxError>>,
    unpack_error: Mutex<Option<SandboxError>>,
}

/// A `SlaveService` implementation that a test scripts.
pub struct ScriptedSlave {
    inner: Arc<ScriptedInner>,
}

impl ScriptedSlave {
    /// An empty script. Every call answers a default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ScriptedInner {
                calls: Mutex::new(Vec::new()),
                info_results: Mutex::new(VecDeque::new()),
                default_info: Mutex::new(SlaveInfo {
                    slave_version: "test".to_string(),
                    arch: "x86_64".to_string(),
                    kernel: "test".to_string(),
                    is_pid1: false,
                    prepared: false,
                    can_control: false,
                }),
                prepare_results: Mutex::new(VecDeque::new()),
                exec_scripts: Mutex::new(VecDeque::new()),
                input_scripts: Mutex::new(VecDeque::new()),
                next_input_seq: Mutex::new(0),
                applied: Mutex::new(Vec::new()),
                signals: Mutex::new(Vec::new()),
                gates: Mutex::new(HashMap::new()),
                chunk_fail_at: Mutex::new(None),
                chunk_calls: Mutex::new(0),
                read_chunks: Mutex::new(VecDeque::new()),
                shutdown_grace: Mutex::new(None),
                shutdown_exit: Mutex::new(None),
                shutdown_count: Mutex::new(0),
                info_pending: AtomicBool::new(false),
                shutdown_error: Mutex::new(None),
                unpack_error: Mutex::new(None),
            }),
        }
    }

    /// Makes the next `UnpackCommit` fail with `error`.
    pub fn with_unpack_error(&self, error: SandboxError) {
        *self
            .inner
            .unpack_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error);
    }

    /// Makes `Shutdown` fail.
    pub fn with_shutdown_error(&self, error: SandboxError) {
        *self
            .inner
            .shutdown_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error);
    }

    /// Makes every `Info` call wait forever, as a slave that never answers.
    pub fn with_info_pending(&self) {
        self.inner.info_pending.store(true, Ordering::SeqCst);
    }

    /// Refuses the next `count` `Info` calls with an input or output error.
    ///
    /// The error is not a transport error, so the typed client sends the call
    /// one time and the readiness wait of the create owns the back-off.
    pub fn with_info_refusals(&self, count: usize) {
        let mut queue = self
            .inner
            .info_results
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for _ in 0..count {
            queue.push_back(Err(SandboxError::Io {
                op: "info".to_string(),
                message: "the slave is not ready".to_string(),
            }));
        }
    }

    /// Refuses the next `count` `Info` calls with a call-deadline error.
    ///
    /// A container that is still starting can miss the call deadline of one
    /// attempt. The readiness wait of the create owns the back-off.
    pub fn with_info_timeouts(&self, count: usize) {
        let mut queue = self
            .inner
            .info_results
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for _ in 0..count {
            queue.push_back(Err(SandboxError::Rpc {
                code: "deadline_exceeded".to_string(),
                message: "call deadline exceeded".to_string(),
            }));
        }
    }

    /// Sets whether the default `Info` answer reports `can_control`.
    pub fn with_can_control(&self, can_control: bool) {
        self.inner
            .default_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .can_control = can_control;
    }

    /// Sets whether the default `Info` answer is prepared.
    pub fn with_info_prepared(&self, prepared: bool) {
        self.inner
            .default_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .prepared = prepared;
    }

    /// Makes the next `Prepare` call fail.
    pub fn with_prepare_error(&self, error: SandboxError) {
        self.inner
            .prepare_results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(Err(error));
    }

    /// Scripts the events of the next `Exec` call, in order from sequence zero.
    pub fn with_exec_events(&self, events: Vec<ExecEventKind>) {
        let messages = events
            .into_iter()
            .enumerate()
            .map(|(seq, event)| ExecEventMsg {
                seq: u64::try_from(seq).unwrap_or(u64::MAX),
                event,
            })
            .collect();
        self.with_exec_messages(messages);
    }

    /// Scripts the exact messages of the next `Exec` call.
    pub fn with_exec_messages(&self, messages: Vec<ExecEventMsg>) {
        self.inner
            .exec_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(ExecScript::Events(messages));
    }

    /// Makes the next `Exec` stream send `before`, then a stream error.
    pub fn with_exec_stream_error(&self, before: Vec<ExecEventMsg>, error: SandboxError) {
        self.inner
            .exec_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(ExecScript::StreamError { before, error });
    }

    /// Makes the next `Exec` call fail.
    pub fn with_exec_error(&self, error: SandboxError) {
        self.inner
            .exec_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(ExecScript::Error(error));
    }

    /// Scripts an `Exec` that gives `before`, waits for a signal, then gives
    /// `exit`.
    pub fn with_gated_exec(&self, before: Vec<ExecEventKind>, exit: ExecEventKind) {
        let gate = Arc::new(SignalGate::default());
        let mut seq = 0_u64;
        let mut messages = Vec::new();
        for event in before {
            messages.push(ExecEventMsg { seq, event });
            seq += 1;
        }
        let exit = ExecEventMsg { seq, event: exit };
        self.inner
            .exec_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(ExecScript::Gated {
                before: messages,
                exit,
                gate,
            });
    }

    /// Makes `ExecInput` for `seq` answer `InputQueueFull` `count` times.
    pub fn with_input_queue_full(&self, seq: u64, count: usize) {
        self.inner
            .input_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(InputScript {
                seq,
                kind: InputScriptKind::QueueFull,
                remaining: count,
            });
    }

    /// Makes `ExecInput` for `seq` answer a transport error `count` times.
    pub fn with_input_transport_error(&self, seq: u64, count: usize) {
        self.inner
            .input_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(InputScript {
                seq,
                kind: InputScriptKind::TransportError,
                remaining: count,
            });
    }

    /// Makes the `index`-th `FileWriteChunk` call (1 based) fail.
    pub fn with_chunk_failure(&self, index: usize, error: SandboxError) {
        *self
            .inner
            .chunk_fail_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((index, error));
    }

    /// Scripts the chunks of the next `FileRead` call.
    pub fn with_read_chunks(&self, chunks: Vec<FileChunk>) {
        let mut queue = self
            .inner
            .read_chunks
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        queue.clear();
        queue.extend(chunks);
    }

    /// Makes `Shutdown` exit this substrate.
    pub fn with_shutdown_exit(&self, substrate: Arc<SimSubstrate>) {
        *self
            .inner
            .shutdown_exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(substrate);
    }

    /// Every call that the slave received.
    #[must_use]
    pub fn calls(&self) -> Vec<SlaveCall> {
        self.inner
            .calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How often one method was called.
    #[must_use]
    pub fn call_count(&self, method: &str) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.method() == method)
            .count()
    }

    /// The input chunks that the slave applied, in order.
    #[must_use]
    pub fn applied_input(&self) -> Vec<AppliedInput> {
        self.inner
            .applied
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The signals that the slave received.
    #[must_use]
    pub fn signals(&self) -> Vec<i32> {
        self.inner
            .signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The grace of the one `Shutdown` call.
    #[must_use]
    pub fn shutdown_grace(&self) -> Option<u32> {
        *self
            .inner
            .shutdown_grace
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn record(&self, call: SlaveCall) {
        self.inner
            .calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }
}

impl Default for ScriptedSlave {
    fn default() -> Self {
        Self::new()
    }
}

fn slave_connect_error(error: &SandboxError) -> ErrorTrace<ConnectError> {
    ErrorTrace::new(error.to_connect(SandboxLimits::default().max_error_message))
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the service trait declares async methods; the bodies answer from a script"
)]
impl SlaveService for ScriptedSlave {
    async fn info(
        &self,
        _ctx: RpcCtx,
        _request: Request<InfoRequest>,
    ) -> ConnectResult<Response<SlaveInfo>> {
        self.record(SlaveCall::Info);
        if self.inner.info_pending.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        let next = self
            .inner
            .info_results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        match next {
            Some(Ok(info)) => Ok(Response::new(info)),
            Some(Err(error)) => Err(slave_connect_error(&error)),
            None => Ok(Response::new(
                self.inner
                    .default_info
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone(),
            )),
        }
    }

    async fn prepare(
        &self,
        _ctx: RpcCtx,
        request: Request<PrepareRequest>,
    ) -> ConnectResult<Response<PrepareResponse>> {
        self.record(SlaveCall::Prepare {
            workdir: request.msg.workdir.clone(),
            root: request.msg.root.clone(),
        });
        let next = self
            .inner
            .prepare_results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        match next {
            Some(Ok(response)) => {
                self.inner
                    .default_info
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .prepared = true;
                Ok(Response::new(response))
            }
            Some(Err(error)) => Err(slave_connect_error(&error)),
            None => {
                self.inner
                    .default_info
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .prepared = true;
                Ok(Response::new(PrepareResponse {
                    root_mode: RootMode::Overlay,
                }))
            }
        }
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<ExecEventMsg>> + Send>>> {
        self.record(SlaveCall::Exec {
            exec_id: request.msg.exec_id.clone(),
            argv: request.msg.argv.clone(),
            stdin: request.msg.stdin,
            user: request.msg.user.clone(),
        });
        let script = self
            .inner
            .exec_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| {
                ExecScript::Events(vec![
                    ExecEventMsg {
                        seq: 0,
                        event: ExecEventKind::Started { pid: 1 },
                    },
                    ExecEventMsg {
                        seq: 1,
                        event: ExecEventKind::Exit {
                            code: Some(0),
                            signal: None,
                        },
                    },
                ])
            });
        match script {
            ExecScript::Error(error) => Err(slave_connect_error(&error)),
            ExecScript::Events(messages) => Ok(Box::pin(futures::stream::iter(
                messages.into_iter().map(Ok),
            ))),
            ExecScript::StreamError { before, error } => {
                let mut items: Vec<ConnectResult<ExecEventMsg>> =
                    before.into_iter().map(Ok).collect();
                items.push(Err(slave_connect_error(&error)));
                Ok(Box::pin(futures::stream::iter(items)))
            }
            ExecScript::Gated { before, exit, gate } => {
                self.inner
                    .gates
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(request.msg.exec_id.clone(), Arc::clone(&gate));
                Ok(Box::pin(futures::stream::unfold(
                    (before.into_iter(), Some(exit), gate),
                    |(mut messages, exit, gate)| async move {
                        if let Some(message) = messages.next() {
                            return Some((Ok(message), (messages, exit, gate)));
                        }
                        if let Some(exit) = exit {
                            gate.wait().await;
                            return Some((Ok(exit), (messages, None, gate)));
                        }
                        None
                    },
                )))
            }
        }
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        self.record(SlaveCall::ExecInput {
            seq: request.msg.seq,
            eof: request.msg.eof,
            len: request.msg.data.len(),
        });
        let seq = request.msg.seq;
        let mut next = self
            .inner
            .next_input_seq
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if seq < *next {
            return Ok(Response::new(ExecInputResponse { next_seq: *next }));
        }
        if seq > *next {
            return Err(slave_connect_error(&SandboxError::SequenceGap));
        }
        // The scripted answer is for this sequence number.
        let mut scripts = self
            .inner
            .input_scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(script) = scripts.front_mut() {
            if script.seq == seq && script.remaining > 0 {
                script.remaining -= 1;
                let kind = script.kind;
                if script.remaining == 0 {
                    scripts.pop_front();
                }
                drop(scripts);
                return match kind {
                    InputScriptKind::QueueFull => {
                        Err(slave_connect_error(&SandboxError::InputQueueFull))
                    }
                    InputScriptKind::TransportError => Err(ErrorTrace::new(
                        ConnectError::unavailable("the transport failed"),
                    )),
                };
            }
        }
        drop(scripts);
        *next += 1;
        drop(next);
        self.inner
            .applied
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(AppliedInput {
                seq,
                data: request.msg.data,
                eof: request.msg.eof,
            });
        Ok(Response::new(ExecInputResponse { next_seq: seq + 1 }))
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        self.record(SlaveCall::ExecSignal {
            signal: request.msg.signal,
        });
        self.inner
            .signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.msg.signal);
        let gate = self
            .inner
            .gates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&request.msg.exec_id)
            .cloned();
        if let Some(gate) = gate {
            gate.release();
        }
        Ok(Response::new(Empty {}))
    }

    async fn file_write_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteBeginRequest>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>> {
        self.record(SlaveCall::FileWriteBegin {
            path: request.msg.path.clone(),
        });
        Ok(Response::new(FileWriteBeginResponse {
            upload_id: "upload-0".to_string(),
        }))
    }

    async fn file_write_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteChunkRequest>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>> {
        self.record(SlaveCall::FileWriteChunk {
            offset: request.msg.offset,
        });
        let mut calls = self
            .inner
            .chunk_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *calls += 1;
        let index = *calls;
        drop(calls);
        let failure = self
            .inner
            .chunk_fail_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some((at, error)) = failure {
            if at == index {
                return Err(slave_connect_error(&error));
            }
        }
        let next_offset = request
            .msg
            .offset
            .saturating_add(u64::try_from(request.msg.data.len()).unwrap_or(u64::MAX));
        Ok(Response::new(FileWriteChunkResponse { next_offset }))
    }

    async fn file_write_commit(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteCommitRequest>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>> {
        self.record(SlaveCall::FileWriteCommit {
            size: request.msg.size,
        });
        Ok(Response::new(FileWriteCommitResponse {
            size: request.msg.size,
        }))
    }

    async fn file_write_abort(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        self.record(SlaveCall::FileWriteAbort {
            upload_id: request.msg.upload_id.clone(),
        });
        Ok(Response::new(Empty {}))
    }

    async fn file_read(
        &self,
        _ctx: RpcCtx,
        request: Request<FileReadRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<FileChunk>> + Send>>> {
        self.record(SlaveCall::FileRead {
            path: request.msg.path.clone(),
        });
        let chunks: Vec<FileChunk> = self
            .inner
            .read_chunks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect();
        Ok(Box::pin(futures::stream::iter(chunks.into_iter().map(Ok))))
    }

    async fn shutdown(
        &self,
        _ctx: RpcCtx,
        request: Request<ShutdownRequest>,
    ) -> ConnectResult<Response<Empty>> {
        self.record(SlaveCall::Shutdown {
            grace_ms: request.msg.grace_ms,
        });
        *self
            .inner
            .shutdown_grace
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(request.msg.grace_ms);
        *self
            .inner
            .shutdown_count
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        let error = self
            .inner
            .shutdown_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(error) = error {
            return Err(slave_connect_error(&error));
        }
        let substrate = self
            .inner
            .shutdown_exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(substrate) = substrate {
            substrate.exit_now(SubstrateExit {
                code: Some(0),
                signal: None,
                forced: false,
            });
        }
        Ok(Response::new(Empty {}))
    }

    async fn unpack_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackBeginRequest>,
    ) -> ConnectResult<Response<UnpackBeginResponse>> {
        self.record(SlaveCall::UnpackBegin {
            keep_existing_dirs: request.msg.keep_existing_dirs,
        });
        Ok(Response::new(UnpackBeginResponse {
            upload_id: "unpack-00000001".to_string(),
        }))
    }

    async fn unpack_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackChunkRequest>,
    ) -> ConnectResult<Response<UnpackChunkResponse>> {
        Ok(Response::new(UnpackChunkResponse {
            next_offset: request.msg.offset + request.msg.data.len() as u64,
        }))
    }

    async fn unpack_commit(
        &self,
        _ctx: RpcCtx,
        _request: Request<UnpackCommitRequest>,
    ) -> ConnectResult<Response<UnpackCommitResponse>> {
        let error = self
            .inner
            .unpack_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(error) = error {
            return Err(slave_connect_error(&error));
        }
        Ok(Response::new(UnpackCommitResponse {
            entries: 0,
            bytes: 0,
        }))
    }

    async fn unpack_abort(
        &self,
        _ctx: RpcCtx,
        _request: Request<UnpackAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        Ok(Response::new(Empty {}))
    }
}

/// A transport to a router in the same process, with no socket.
struct InMemoryTransport {
    handler: Arc<ConnectRpcHandler>,
    ctx: Ctx,
}

impl Transport for InMemoryTransport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            full_duplex: false,
            h2_trailers: false,
            http_versions: &[Proto::HTTP11],
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let (send_body, request_rx) = Pipe::<Bytes>::with_depth(4);
        let (response_tx, response_rx) = Pipe::<Bytes>::with_depth(4);
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::with_depth(1);
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        drop(trailer_tx);
        let handler = Arc::clone(&self.handler);
        let ctx = self.ctx.clone();
        let future = async move {
            let full_url = request.request_url.url;
            let path = full_url
                .split_once("://")
                .and_then(|(_, rest)| rest.find('/').map(|slash| rest[slash..].to_string()))
                .unwrap_or(full_url);
            let incoming = match SimpleIncomingRequest::builder()
                .with_url(foundation_netio::shared::http::SimpleUrl::url_only(path))
                .with_uri(request.request_uri)
                .with_proto(request.proto)
                .with_method(request.method)
                .with_headers(request.headers)
                .build()
            {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!(error = %error, "the in-memory request is not valid");
                    let _ = head_tx
                        .send((Status::InternalServerError, SimpleHeaders::new()))
                        .await;
                    let _ = head_tx.close();
                    let _ = response_tx.close();
                    return;
                }
            };
            let body: RequestBody<'static> =
                Box::pin(futures::stream::unfold(request_rx, |rx| async move {
                    rx.receive().await.map(|bytes| (Ok(bytes), rx))
                }));
            let sink = PipeSink {
                head: head_tx,
                body: response_tx,
            };
            if let Err(error) = handler
                .dispatch_to(ctx, incoming, body, Box::new(sink), CallHooks::default())
                .await
            {
                tracing::warn!(error = %error, "the in-memory dispatch failed");
            }
        };
        if let Err(error) = send(from_future(future)) {
            return Err(TransportError::Protocol(error.to_string()));
        }
        Ok(TransportStream {
            send_body: Arc::new(send_body),
            head: head_stream_from_pipe(head_rx),
            recv_body: body_stream_from_pipe(response_rx),
            trailers: trailer_rx,
        })
    }
}

/// The response sink of the in-memory transport. It writes each frame as the
/// handler produces it.
struct PipeSink {
    head: PipeSender<(Status, SimpleHeaders)>,
    body: PipeSender<Bytes>,
}

impl ResponseSink for PipeSink {
    fn whole(
        &mut self,
        response: foundation_netio::shared::http::SimpleOutgoingResponse,
    ) -> ConnectBoxFuture<'_, std::io::Result<()>> {
        Box::pin(async move {
            let _ = self.head.send((response.status, response.headers)).await;
            let _ = self.head.close();
            let body = match response.body {
                Some(SendSafeBody::Bytes(bytes)) => bytes,
                Some(SendSafeBody::Text(text)) => text.into_bytes(),
                _ => Vec::new(),
            };
            if !body.is_empty() {
                let _ = self.body.send(Bytes::from(body)).await;
            }
            let _ = self.body.close();
            Ok(())
        })
    }

    fn open(
        &mut self,
        head: foundation_netio::shared::http::SimpleOutgoingResponse,
    ) -> ConnectBoxFuture<'_, std::io::Result<()>> {
        Box::pin(async move {
            let _ = self.head.send((head.status, head.headers)).await;
            let _ = self.head.close();
            Ok(())
        })
    }

    fn chunk(&mut self, bytes: Bytes) -> ConnectBoxFuture<'_, std::io::Result<()>> {
        Box::pin(async move {
            let _ = self.body.send(bytes).await;
            Ok(())
        })
    }

    fn finish(&mut self) -> ConnectBoxFuture<'_, std::io::Result<()>> {
        Box::pin(async move {
            let _ = self.body.close();
            Ok(())
        })
    }
}

/// A connector whose transport calls a router in the same process.
struct InMemoryConnector {
    handler: Arc<ConnectRpcHandler>,
}

impl SlaveConnector for InMemoryConnector {
    fn connect(
        &self,
        ctx: &Ctx,
        _endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn Transport>> {
        Ok(Arc::new(InMemoryTransport {
            handler: Arc::clone(&self.handler),
            ctx: ctx.clone(),
        }))
    }
}

/// A connector whose transport calls a router in the same process.
#[must_use]
pub fn in_memory_connector(slave: Arc<ScriptedSlave>) -> Arc<dyn SlaveConnector> {
    let mut router = Router::new();
    register_slave_service(&mut router, slave);
    Arc::new(InMemoryConnector {
        handler: Arc::new(router.into_handler()),
    })
}

/// A connector to a slave that serves both services, as a nested host does
/// (feature 11).
///
/// The `SlaveService` calls go to `slave`; the `SandboxService` calls go to
/// `control`. A nested child reaches the second service through this
/// connector.
#[must_use]
pub fn in_memory_dual_connector<S>(
    _ctx: Ctx,
    slave: Arc<ScriptedSlave>,
    control: Arc<S>,
) -> Arc<dyn SlaveConnector>
where
    S: SandboxService + 'static,
{
    let mut router = Router::new();
    register_slave_service(&mut router, slave);
    register_sandbox_service(&mut router, control);
    Arc::new(InMemoryConnector {
        handler: Arc::new(router.into_handler()),
    })
}

/// A transport to a `SandboxService` router in the same process.
#[must_use]
pub fn in_memory_control_transport<S>(ctx: Ctx, service: Arc<S>) -> Arc<dyn Transport>
where
    S: SandboxService + 'static,
{
    let mut router = Router::new();
    register_sandbox_service(&mut router, service);
    Arc::new(InMemoryTransport {
        handler: Arc::new(router.into_handler()),
        ctx,
    })
}

/// A record store that counts its calls.
#[derive(Debug, Default)]
pub struct CountingStore {
    inner: Mutex<HashMap<SandboxId, StoredRecord>>,
    puts: Mutex<usize>,
    removes: Mutex<usize>,
}

impl CountingStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How often `put` was called.
    #[must_use]
    pub fn puts(&self) -> usize {
        *self.puts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How often `remove` was called.
    #[must_use]
    pub fn removes(&self) -> usize {
        *self.removes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The stored records.
    #[must_use]
    pub fn records(&self) -> Vec<StoredRecord> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }
}

impl RecordStore for CountingStore {
    fn put(&self, record: StoredRecord) -> crate::shared::BoxFuture<'_, SandboxResult<()>> {
        *self.puts.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        let id = record.id.clone();
        let records = &self.inner;
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id, record);
            Ok(())
        })
    }

    fn remove(&self, id: &SandboxId) -> crate::shared::BoxFuture<'_, SandboxResult<()>> {
        *self.removes.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        let id = id.clone();
        let records = &self.inner;
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id);
            Ok(())
        })
    }

    fn list(&self) -> crate::shared::BoxFuture<'_, SandboxResult<Vec<StoredRecord>>> {
        let records = &self.inner;
        Box::pin(async move {
            Ok(records
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .values()
                .cloned()
                .collect())
        })
    }
}

/// A store whose `put` fails.
#[derive(Debug)]
pub struct FailingStore {
    /// How many `put` calls fail before the store accepts a record.
    pub fail_puts: usize,
    inner: CountingStore,
}

impl FailingStore {
    /// A store with `fail_puts` failing calls.
    #[must_use]
    pub fn new(fail_puts: usize) -> Self {
        Self {
            fail_puts,
            inner: CountingStore::new(),
        }
    }
}

impl RecordStore for FailingStore {
    fn put(&self, record: StoredRecord) -> crate::shared::BoxFuture<'_, SandboxResult<()>> {
        if *self
            .inner
            .puts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            < self.fail_puts
        {
            *self
                .inner
                .puts
                .lock()
                .unwrap_or_else(PoisonError::into_inner) += 1;
            return Box::pin(async {
                Err(SandboxError::Io {
                    op: "store".to_string(),
                    message: "the store is full".to_string(),
                }
                .into())
            });
        }
        self.inner.put(record)
    }

    fn remove(&self, id: &SandboxId) -> crate::shared::BoxFuture<'_, SandboxResult<()>> {
        self.inner.remove(id)
    }

    fn list(&self) -> crate::shared::BoxFuture<'_, SandboxResult<Vec<StoredRecord>>> {
        self.inner.list()
    }
}

/// A container control that a test uses only to prove the manager finds it.
pub struct FakeContainerControl;

impl ContainerControl for FakeContainerControl {
    fn start(
        &self,
        _ctx: Ctx,
        _spec: ContainerSpec,
    ) -> crate::shared::BoxFuture<'_, SandboxResult<ContainerHandle>> {
        Box::pin(async {
            Err(SandboxError::UnsupportedCapability {
                capability: "container start".to_string(),
            }
            .into())
        })
    }

    fn get(
        &self,
        _ctx: Ctx,
        _id: &ContainerId,
    ) -> crate::shared::BoxFuture<'_, SandboxResult<Option<ContainerHandle>>> {
        Box::pin(async { Ok(None) })
    }

    fn list(&self, _ctx: Ctx) -> crate::shared::BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// A `SandboxSpec` with a directory root and the full capabilities.
#[must_use]
pub fn spec() -> SandboxSpec {
    SandboxSpec::new(crate::shared::spec::RootFs::Directory(
        std::path::PathBuf::from("/tmp/root"),
    ))
}

/// An in-memory [`HostFs`](crate::shared::host::HostFs).
///
/// The fake keeps files, directories, modification times and the
/// read-and-write flag in one map. A test seeds it with the files that the
/// driver must find.
#[derive(Clone, Default)]
pub struct FakeHostFs {
    state: Arc<Mutex<FsState>>,
}

#[derive(Default)]
struct FsState {
    files: HashMap<PathBuf, Vec<u8>>,
    dirs: std::collections::HashSet<PathBuf>,
    modified: HashMap<PathBuf, foundation_compact::SystemTime>,
    read_write: HashMap<PathBuf, bool>,
    locked: std::collections::HashSet<PathBuf>,
}

impl FakeHostFs {
    /// An empty host file system.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file with its contents.
    pub fn insert_file(&self, path: impl Into<PathBuf>, data: impl Into<Vec<u8>>) {
        let path = path.into();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(parent) = path.parent() {
            state.dirs.insert(parent.to_path_buf());
        }
        state.files.insert(path, data.into());
    }

    /// Adds a directory.
    pub fn insert_dir(&self, path: impl Into<PathBuf>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .dirs
            .insert(path.into());
    }

    /// Sets the modification time of a path.
    pub fn set_modified(&self, path: impl Into<PathBuf>, time: foundation_compact::SystemTime) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .modified
            .insert(path.into(), time);
    }

    /// Sets whether a path can be opened for reading and writing.
    pub fn set_read_write(&self, path: impl Into<PathBuf>, ok: bool) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .read_write
            .insert(path.into(), ok);
    }

    /// The contents of a file.
    #[must_use]
    pub fn file(&self, path: &Path) -> Option<Vec<u8>> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .files
            .get(path)
            .cloned()
    }

    /// Whether a file exists.
    #[must_use]
    pub fn has_file(&self, path: &Path) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .files
            .contains_key(path)
    }

    /// Whether a directory exists.
    #[must_use]
    pub fn has_dir(&self, path: &Path) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .dirs
            .contains(path)
    }
}

impl crate::shared::host::HostFs for FakeHostFs {
    fn create_dir_all(&self, path: &Path, _mode: u32) -> SandboxResult<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut current = Some(path);
        while let Some(directory) = current {
            state.dirs.insert(directory.to_path_buf());
            current = directory.parent();
        }
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.dirs.remove(path);
        state.files.retain(|file, _| !file.starts_with(path));
        state
            .dirs
            .retain(|dir| !dir.starts_with(path) || dir == path);
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> SandboxResult<()> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .files
            .remove(path);
        Ok(())
    }

    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(parent) = path.parent() {
            state.dirs.insert(parent.to_path_buf());
        }
        state.files.insert(path.to_path_buf(), data.to_vec());
        Ok(())
    }

    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let data = state
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| SandboxError::Io {
                op: "read".to_string(),
                message: format!("{}: not found", path.display()),
            })?;
        Ok(data[..data.len().min(max)].to_vec())
    }

    fn open_read(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send + Sync>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let data = state
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| SandboxError::Io {
                op: "open_read".to_string(),
                message: format!("{}: not found", path.display()),
            })?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }

    fn file_size(&self, path: &Path) -> SandboxResult<u64> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.files.get(path).map_or_else(
            || {
                Err(SandboxError::Io {
                    op: "file_size".to_string(),
                    message: format!("{}: not found", path.display()),
                }
                .into())
            },
            |data| Ok(u64::try_from(data.len()).unwrap_or(u64::MAX)),
        )
    }

    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let data = state
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| SandboxError::Io {
                op: "tail".to_string(),
                message: format!("{}: not found", path.display()),
            })?;
        let start = data.len().saturating_sub(max);
        Ok(data[start..].to_vec())
    }

    fn exists(&self, path: &Path) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.files.contains_key(path) || state.dirs.contains(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(data) = state.files.remove(from) {
            state.files.insert(to.to_path_buf(), data);
            return Ok(());
        }
        if state.dirs.remove(from) {
            // A directory keeps its children: move each path below it.
            let prefix = from.to_path_buf();
            let moved_files: Vec<(PathBuf, Vec<u8>)> = state
                .files
                .iter()
                .filter(|(path, _)| path.starts_with(&prefix))
                .map(|(path, data)| (path.clone(), data.clone()))
                .collect();
            for (path, data) in moved_files {
                state.files.remove(&path);
                let suffix = path.strip_prefix(&prefix).unwrap_or(&path);
                state.files.insert(to.join(suffix), data);
            }
            let moved_dirs: Vec<PathBuf> = state
                .dirs
                .iter()
                .filter(|path| path.starts_with(&prefix))
                .cloned()
                .collect();
            for path in moved_dirs {
                state.dirs.remove(&path);
                let suffix = path.strip_prefix(&prefix).unwrap_or(&path);
                state.dirs.insert(to.join(suffix));
            }
            state.dirs.insert(to.to_path_buf());
            return Ok(());
        }
        Err(SandboxError::Io {
            op: "rename".to_string(),
            message: format!("{}: not found", from.display()),
        }
        .into())
    }

    fn create_file(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Write + Send>> {
        Ok(Box::new(FakeFileWriter {
            fs: self.clone(),
            path: path.to_path_buf(),
            buffer: Vec::new(),
        }))
    }

    fn modified(&self, path: &Path) -> Option<foundation_compact::SystemTime> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .modified
            .get(path)
            .copied()
    }

    fn read_dir(&self, path: &Path) -> SandboxResult<Vec<PathBuf>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut entries: Vec<PathBuf> = state
            .dirs
            .iter()
            .chain(state.files.keys())
            .filter(|entry| entry.parent() == Some(path))
            .cloned()
            .collect();
        entries.sort();
        entries.dedup();
        Ok(entries)
    }

    fn is_openable_read_write(&self, path: &Path) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .read_write
            .get(path)
            .copied()
            .unwrap_or_else(|| state.files.contains_key(path) || state.dirs.contains(path))
    }

    fn entry_meta(&self, path: &Path) -> SandboxResult<crate::shared::host::EntryMeta> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(data) = state.files.get(path) {
            return Ok(crate::shared::host::EntryMeta {
                kind: crate::shared::host::EntryKind::File,
                mode: 0o644,
                size: u64::try_from(data.len()).unwrap_or(u64::MAX),
            });
        }
        if state.dirs.contains(path) {
            return Ok(crate::shared::host::EntryMeta {
                kind: crate::shared::host::EntryKind::Directory,
                mode: 0o755,
                size: 0,
            });
        }
        Err(SandboxError::Io {
            op: "entry_meta".to_string(),
            message: format!("{}: not found", path.display()),
        }
        .into())
    }

    fn canonicalize(&self, path: &Path) -> SandboxResult<PathBuf> {
        if self.exists(path) {
            Ok(path.to_path_buf())
        } else {
            Err(SandboxError::Io {
                op: "canonicalize".to_string(),
                message: format!("{}: not found", path.display()),
            }
            .into())
        }
    }

    fn read_stream(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send>> {
        let data = self.file(path).ok_or_else(|| SandboxError::Io {
            op: "read_stream".to_string(),
            message: format!("{}: not found", path.display()),
        })?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }

    fn clone_file(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        let data = self.file(from).ok_or_else(|| SandboxError::Io {
            op: "clone_file".to_string(),
            message: format!("{}: not found", from.display()),
        })?;
        self.write(to, &data)
    }

    fn lock_exclusive(&self, path: &Path) -> SandboxResult<crate::shared::host::FileLock> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.locked.insert(path.to_path_buf()) {
            return Err(SandboxError::Io {
                op: "lock_exclusive".to_string(),
                message: format!("{}: the lock is held", path.display()),
            }
            .into());
        }
        drop(state);
        Ok(crate::shared::host::FileLock::new(FakeLock {
            state: Arc::clone(&self.state),
            path: path.to_path_buf(),
        }))
    }
}

/// The token of one [`FakeHostFs`] lock. Its drop releases the lock.
struct FakeLock {
    state: Arc<Mutex<FsState>>,
    path: PathBuf,
}

impl Drop for FakeLock {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .locked
            .remove(&self.path);
    }
}

/// A writer over one [`FakeHostFs`] file.
struct FakeFileWriter {
    fs: FakeHostFs,
    path: PathBuf,
    buffer: Vec<u8>,
}

impl std::io::Write for FakeFileWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        let _ = self.fs.write(&self.path, &self.buffer);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for FakeFileWriter {
    fn drop(&mut self) {
        // `create_file` replaces the file contents, so an empty writer still
        // makes an empty file. A caller that drops the writer without a write
        // relies on it (the image store makes the work file this way).
        let _ = self.fs.write(&self.path, &self.buffer);
    }
}

/// One signal that a fake child received.
#[cfg(unix)]
struct FakeChildState {
    exit: Mutex<Option<crate::shared::host::ProcessExit>>,
    signals: Mutex<Vec<i32>>,
    wakers: Mutex<Vec<std::task::Waker>>,
}

/// The control of one fake child process.
#[cfg(unix)]
#[derive(Clone)]
pub struct FakeChildControl {
    pid: u32,
    state: Arc<FakeChildState>,
    live: Arc<Mutex<bool>>,
}

#[cfg(unix)]
impl FakeChildControl {
    /// The process identifier.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Makes the process exit now.
    pub fn exit_now(&self, exit: crate::shared::host::ProcessExit) {
        self.record(exit);
    }

    /// The signals that the process received.
    #[must_use]
    pub fn signals(&self) -> Vec<i32> {
        self.state
            .signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Whether the process still runs.
    #[must_use]
    pub fn is_live(&self) -> bool {
        *self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn record(&self, exit: crate::shared::host::ProcessExit) {
        self.state
            .exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(exit);
        *self.live.lock().unwrap_or_else(PoisonError::into_inner) = false;
        let wakers = std::mem::take(
            &mut *self
                .state
                .wakers
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for waker in wakers {
            waker.wake();
        }
    }
}

/// One fake child process.
#[cfg(unix)]
pub struct FakeChildProcess {
    pid: u32,
    state: Arc<FakeChildState>,
    live: Arc<Mutex<bool>>,
}

#[cfg(unix)]
impl crate::shared::host::ChildProcess for FakeChildProcess {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn signal(&self, signal: i32) -> SandboxResult<()> {
        self.state
            .signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(signal);
        if signal == libc::SIGKILL || signal == libc::SIGTERM {
            let control = FakeChildControl {
                pid: self.pid,
                state: Arc::clone(&self.state),
                live: Arc::clone(&self.live),
            };
            control.record(crate::shared::host::ProcessExit {
                code: None,
                signal: Some(signal),
            });
        }
        Ok(())
    }

    fn wait(
        &self,
    ) -> crate::shared::BoxFuture<'_, SandboxResult<crate::shared::host::ProcessExit>> {
        let state = Arc::clone(&self.state);
        Box::pin(futures::future::poll_fn(move |context| {
            if let Some(exit) = *state.exit.lock().unwrap_or_else(PoisonError::into_inner) {
                return Poll::Ready(Ok(exit));
            }
            state
                .wakers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(context.waker().clone());
            let exit = *state.exit.lock().unwrap_or_else(PoisonError::into_inner);
            match exit {
                Some(exit) => Poll::Ready(Ok(exit)),
                None => Poll::Pending,
            }
        }))
    }

    fn try_wait(&self) -> SandboxResult<Option<crate::shared::host::ProcessExit>> {
        Ok(*self
            .state
            .exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner))
    }
}

/// A process spawner that a test scripts.
#[cfg(unix)]
#[derive(Clone, Default)]
pub struct FakeProcessSpawner {
    state: Arc<Mutex<SpawnerState>>,
}

/// The recorded fields of one spawned process.
///
/// The file descriptors of the original spec cannot be copied, so the fake keeps
/// the fields that a test reads.
#[cfg(unix)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRecord {
    /// The program path.
    pub program: PathBuf,
    /// The arguments after the program.
    pub args: Vec<String>,
    /// The full environment.
    pub env: Vec<(String, String)>,
    /// The working directory.
    pub cwd: PathBuf,
    /// The standard output file.
    pub stdout: PathBuf,
    /// The standard error file.
    pub stderr: PathBuf,
    /// Whether the child dies with this process.
    pub kill_with_parent: bool,
}

#[cfg(unix)]
impl ProcessRecord {
    fn of(spec: &crate::shared::host::ProcessSpec) -> Self {
        Self {
            program: spec.program.clone(),
            args: spec.args.clone(),
            env: spec.env.clone(),
            cwd: spec.cwd.clone(),
            stdout: spec.stdout.clone(),
            stderr: spec.stderr.clone(),
            kill_with_parent: spec.kill_with_parent,
        }
    }
}

#[cfg(unix)]
#[derive(Default)]
struct SpawnerState {
    specs: Vec<ProcessRecord>,
    children: Vec<FakeChildControl>,
    next_pid: u32,
    fail_with: Option<String>,
    version_output: Option<String>,
    fs: Option<FakeHostFs>,
    exit_next_with: Option<NextExit>,
    program_outputs: HashMap<String, (String, FakeHostFs)>,
}

/// How the next fake child ends at once.
#[derive(Clone, Copy, Debug)]
enum NextExit {
    /// The child exits with this code.
    Code(i32),
    /// The child dies by a signal.
    Signal,
}

#[cfg(unix)]
impl FakeProcessSpawner {
    /// A spawner that starts every process.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the next spawn fail with an input or output error.
    pub fn fail_next(&self, message: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fail_with = Some(message.to_string());
    }

    /// Makes a `--version` child write `text` and exit at once.
    ///
    /// The output goes to the fake file system, which must be the same one that
    /// the driver reads.
    pub fn with_version_output(&self, fs: FakeHostFs, text: &str) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.version_output = Some(text.to_string());
        state.fs = Some(fs);
    }

    /// Makes a child of `program` write `text` and exit with code 0 at once.
    ///
    /// The host probe runs `codesign`, `sysctl` and `sandbox-krun --probe`.
    /// The runner itself has a different argument list, so this rule does not
    /// hide a scripted runner exit.
    pub fn with_program_output(&self, fs: FakeHostFs, program: &str, text: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .program_outputs
            .insert(program.to_string(), (text.to_string(), fs));
    }

    /// Makes the next non-`--version` child exit at once with `code`.
    pub fn exit_next_with(&self, code: i32) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .exit_next_with = Some(NextExit::Code(code));
    }

    /// Makes the next non-`--version` child die by a signal at once.
    pub fn exit_next_by_signal(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .exit_next_with = Some(NextExit::Signal);
    }

    /// The specs of every spawn, in order.
    #[must_use]
    pub fn specs(&self) -> Vec<ProcessRecord> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .specs
            .clone()
    }

    /// The control of the last child.
    #[must_use]
    pub fn last_child(&self) -> Option<FakeChildControl> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .children
            .last()
            .cloned()
    }
}

#[cfg(unix)]
impl crate::shared::host::ProcessSpawner for FakeProcessSpawner {
    fn spawn(
        &self,
        spec: crate::shared::host::ProcessSpec,
    ) -> SandboxResult<Box<dyn crate::shared::host::ChildProcess>> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(message) = state.fail_with.take() {
            return Err(SandboxError::Io {
                op: "spawn process".to_string(),
                message,
            }
            .into());
        }
        state.specs.push(ProcessRecord::of(&spec));
        state.next_pid += 1;
        let pid = state.next_pid;
        let child_state = Arc::new(FakeChildState {
            exit: Mutex::new(None),
            signals: Mutex::new(Vec::new()),
            wakers: Mutex::new(Vec::new()),
        });
        let live = Arc::new(Mutex::new(true));
        let control = FakeChildControl {
            pid,
            state: Arc::clone(&child_state),
            live: Arc::clone(&live),
        };
        state.children.push(control.clone());
        // A `--version` probe exits at once with its output.
        if spec.args == ["--version"] {
            if let (Some(text), Some(fs)) = (state.version_output.clone(), state.fs.clone()) {
                let _ = fs.write(&spec.stdout, text.as_bytes());
            }
            drop(state);
            control.record(crate::shared::host::ProcessExit {
                code: Some(0),
                signal: None,
            });
            return Ok(Box::new(FakeChildProcess {
                pid,
                state: child_state,
                live,
            }));
        }
        // A scripted probe command exits at once with its output.
        let program = spec
            .program
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let is_probe =
            program == "sandbox-krun" && spec.args.first().is_some_and(|arg| arg == "--probe");
        let scripted = state
            .program_outputs
            .get(program)
            .filter(|_| is_probe || program != "sandbox-krun")
            .cloned();
        if let Some((text, fs)) = scripted {
            let _ = fs.write(&spec.stdout, text.as_bytes());
            drop(state);
            control.record(crate::shared::host::ProcessExit {
                code: Some(0),
                signal: None,
            });
            return Ok(Box::new(FakeChildProcess {
                pid,
                state: child_state,
                live,
            }));
        }
        let exit_code = state.exit_next_with.take();
        drop(state);
        match exit_code {
            Some(NextExit::Code(code)) => control.record(crate::shared::host::ProcessExit {
                code: Some(code),
                signal: None,
            }),
            Some(NextExit::Signal) => control.record(crate::shared::host::ProcessExit {
                code: None,
                signal: Some(libc::SIGKILL),
            }),
            None => {}
        }
        Ok(Box::new(FakeChildProcess {
            pid,
            state: child_state,
            live,
        }))
    }

    fn is_running(&self, pid: u32) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .children
            .iter()
            .find(|child| child.pid == pid)
            .is_some_and(FakeChildControl::is_live)
    }
}
