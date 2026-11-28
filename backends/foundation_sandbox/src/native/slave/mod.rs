//! The slave: the engine that serves `SlaveService` in the guest.
//!
//! The same code runs as process identifier 1 in a micro virtual machine (VM),
//! as the main process or a sidecar in a container, and as a normal process in
//! a test. [`run`] is the entry point; [`Slave`] is the service.
//!
//! The order of the life of the slave is the order of the feature document:
//! process start ([`run`]), `Prepare` ([`root`]), the sessions ([`exec`]), the
//! files ([`files`]) and `Shutdown`.

/// The control role of the slave (feature 11, section 6.1): the inner control
/// plane that runs micro virtual machines (VMs) in a container.
#[cfg(all(feature = "control", feature = "firecracker"))]
pub mod control;

pub mod exec;
pub mod files;
pub mod net;
pub mod pid1;
pub mod reaper;
pub mod root;
pub mod serve;
pub mod state;
pub mod sys;
pub mod unpack;

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(feature = "control")]
use crate::shared::control::SandboxProvider;

use foundation_compact::rng::Rng;
use foundation_connectrpc::{ConnectError, ConnectResult, Request, Response, RpcCtx};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_http::ShutdownSignal;
use futures::{Stream, StreamExt};

use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::host::{GuestMounts, GuestPower, SlaveHost};
use crate::shared::ids::ExecId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    Empty, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest, ExecSignalRequest,
    FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo, UnpackAbortRequest, UnpackBeginRequest, UnpackBeginResponse, UnpackChunkRequest,
    UnpackChunkResponse, UnpackCommitRequest, UnpackCommitResponse,
};
use crate::shared::rpc::SlaveService;

pub use self::serve::ListenAddr;

use self::exec::{SessionStream, SessionTable};
use self::files::Files;
use self::reaper::ChildTable;
use self::state::{Phase, PrepareRecord, State};
use self::unpack::{ChildOptions, UnpackSessions};

/// The number of threads of the valtron pool that the slave starts when its
/// caller did not start one. Four workers keep both output pumps of one session
/// and a file call in flight without a wait.
pub const DEFAULT_POOL_THREADS: usize = 4;

/// How the slave listens and what it controls.
pub struct SlaveArgs {
    /// The address of the listener.
    pub listen: ListenAddr,
    /// Whether the slave also serves the control plane (feature 11). It is
    /// `false` in this feature.
    pub control: bool,
    /// The directory that [`crate::shared::spec::RootSource::Plain`] uses as
    /// its root. It is `/` in production. The tests set another directory for
    /// the file calls only: a program cannot start under a root other than `/`
    /// without the right to `chroot`.
    pub plain_root: PathBuf,
    /// The mode of the Unix domain socket file.
    ///
    /// Default `0o600`. A slave that serves a control plane of another user,
    /// as in a Docker container, needs a wider mode; the run directory's own
    /// mode keeps other users out.
    pub socket_mode: u32,
}

/// The slave service.
pub struct Slave {
    ctx: Ctx,
    host: Arc<SlaveHost>,
    plain_root: PathBuf,
    pid1: bool,
    state: Mutex<State>,
    sessions: Arc<SessionTable>,
    children: Arc<ChildTable>,
    files: Mutex<Option<Arc<Files>>>,
    unpack: Mutex<Option<Arc<UnpackSessions>>>,
    shutdown: Arc<ShutdownSignal>,
    /// The inner control plane, when the slave was started with `--control`.
    #[cfg(feature = "control")]
    control: Mutex<Option<Arc<dyn SandboxProvider>>>,
    /// Whether the slave can control: the build has the features, `--control`
    /// was given, and the Firecracker probe is usable (feature 11).
    can_control: AtomicBool,
}

impl Slave {
    /// A slave with the real guest effects of this host.
    #[must_use]
    pub fn new(ctx: Ctx, plain_root: PathBuf) -> Arc<Self> {
        Self::with_host(
            ctx,
            Arc::new(SlaveHost::native()),
            plain_root,
            Arc::new(ShutdownSignal::new()),
        )
    }

    /// A slave with the given guest effects. A test uses a fake for the parts
    /// that need root rights.
    #[must_use]
    pub fn with_host(
        ctx: Ctx,
        host: Arc<SlaveHost>,
        plain_root: PathBuf,
        shutdown: Arc<ShutdownSignal>,
    ) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            host,
            plain_root,
            pid1: sys::getpid() == 1,
            state: Mutex::new(State::booted()),
            sessions: Arc::new(SessionTable::new()),
            children: Arc::new(ChildTable::new()),
            files: Mutex::new(None),
            unpack: Mutex::new(None),
            shutdown,
            #[cfg(feature = "control")]
            control: Mutex::new(None),
            can_control: AtomicBool::new(false),
        })
    }

    /// Adds the inner control plane and the result of its Firecracker probe.
    ///
    /// The slave serves `SandboxService` only when this method ran (feature 11,
    /// invariant I4).
    #[cfg(feature = "control")]
    pub fn set_control(&self, provider: Arc<dyn SandboxProvider>, can_control: bool) {
        *self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(provider);
        self.can_control.store(can_control, Ordering::SeqCst);
    }

    /// The inner control plane, when the slave was started with `--control`.
    #[cfg(feature = "control")]
    #[must_use]
    pub fn control(&self) -> Option<Arc<dyn SandboxProvider>> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Whether the slave can control (feature 11).
    #[must_use]
    pub fn can_control(&self) -> bool {
        self.can_control.load(Ordering::SeqCst)
    }

    /// The signal that stops the server.
    #[must_use]
    pub fn shutdown_signal(&self) -> Arc<ShutdownSignal> {
        Arc::clone(&self.shutdown)
    }

    /// The table of the running sessions.
    #[must_use]
    pub fn sessions(&self) -> &Arc<SessionTable> {
        &self.sessions
    }

    /// The table of the children.
    #[must_use]
    pub fn children(&self) -> &Arc<ChildTable> {
        &self.children
    }

    /// Whether the process is process identifier 1.
    #[must_use]
    pub fn is_pid1(&self) -> bool {
        self.pid1
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> State {
        self.lock_state().clone()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The `Prepare` record, or `NotPrepared`.
    fn prepared(&self) -> SandboxResult<PrepareRecord> {
        let state = self.lock_state();
        if state.phase() == Phase::ShuttingDown {
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                "the slave is shutting down",
            ));
        }
        state
            .prepare
            .clone()
            .filter(|_| state.is_prepared())
            .ok_or_else(|| {
                slave_error(
                    SlaveErrorCode::NotPrepared,
                    None,
                    "the slave did not get Prepare",
                )
            })
    }

    /// The largest error message of the current limits.
    fn max_error_message(&self) -> usize {
        self.lock_state().prepare.as_ref().map_or_else(
            || SandboxLimits::default().max_error_message,
            |record| record.limits().max_message_bytes as usize,
        )
    }

    /// Prepares the root of the programs.
    ///
    /// A second call with an equal request returns the first answer. A second
    /// call with a different request fails with `AlreadyExists`, and nothing
    /// changes.
    ///
    /// # Errors
    /// [`SlaveErrorCode::RootSetupFailed`], [`SlaveErrorCode::MountFailed`],
    /// `AlreadyExists` and `BadRequest` after `Shutdown`.
    pub fn prepare_root(&self, request: &PrepareRequest) -> SandboxResult<PrepareResponse> {
        let mut state = self.lock_state();
        match state.phase() {
            Phase::ShuttingDown => {
                return Err(slave_error(
                    SlaveErrorCode::BadRequest,
                    None,
                    "the slave is shutting down",
                ));
            }
            Phase::Prepared => {
                if let Some(record) = &state.prepare {
                    if record.request == *request {
                        return Ok(PrepareResponse {
                            root_mode: record.root_mode,
                        });
                    }
                }
                return Err(SandboxError::SlaveError {
                    code: SlaveErrorCode::AlreadyExists,
                    errno: None,
                    message: "the slave is prepared already with another request".to_string(),
                }
                .into());
            }
            Phase::Booted => {}
        }
        let (root_mode, exec_root) = root::prepare_root(&self.host, request, &self.plain_root)?;
        *self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        state.prepare = Some(PrepareRecord {
            request: request.clone(),
            root_mode,
            exec_root,
        });
        state.phase = Some(Phase::Prepared);
        Ok(PrepareResponse { root_mode })
    }

    /// The file state, opened on the first file call.
    fn files(&self) -> SandboxResult<Arc<Files>> {
        let prepared = self.prepared()?;
        let mut files = self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(files) = files.as_ref() {
            return Ok(Arc::clone(files));
        }
        let limits = prepared.limits();
        let opened = Files::new(
            &prepared.exec_root,
            prepared.workdir(),
            self.ctx.clone(),
            limits.chunk_bytes,
            limits.max_exec_sessions,
            limits.upload_idle_timeout_ms,
            u32::try_from(SandboxLimits::default().max_error_message).unwrap_or(u32::MAX),
        )
        .map_err(|error| {
            slave_error(
                SlaveErrorCode::RootSetupFailed,
                error.raw_os_error(),
                format!("open the root for the file calls: {error}"),
            )
        })?;
        start_upload_timer(
            self.ctx.clone(),
            Arc::clone(&opened),
            std::time::Duration::from_millis(u64::from(limits.upload_idle_timeout_ms) / 2),
        );
        *files = Some(Arc::clone(&opened));
        Ok(opened)
    }

    /// The unpack sessions, opened on the first unpack call.
    fn unpack_sessions(&self) -> SandboxResult<Arc<UnpackSessions>> {
        let prepared = self.prepared()?;
        let mut slot = self
            .unpack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(sessions) = slot.as_ref() {
            return Ok(Arc::clone(sessions));
        }
        let limits = prepared.limits();
        let sessions = Arc::new(UnpackSessions::new(
            self.ctx.clone(),
            limits.max_exec_sessions as usize,
            std::time::Duration::from_millis(u64::from(limits.upload_idle_timeout_ms)),
            limits.max_message_bytes as usize,
        ));
        *slot = Some(Arc::clone(&sessions));
        start_unpack_timer(
            self.ctx.clone(),
            Arc::clone(&sessions),
            std::time::Duration::from_millis(u64::from(limits.upload_idle_timeout_ms) / 2),
        );
        Ok(sessions)
    }

    /// Unmounts a writable root cleanly before the machine powers off.
    ///
    /// It unmounts the bind mounts below the root, flushes the buffers and
    /// unmounts the root. When the root refuses with `EBUSY`, it remounts the
    /// root read-only, which also makes the image clean. It never fails the
    /// stop: every error is logged, and the power off runs next.
    pub fn unmount_writable_root(&self) {
        let Some(record) = self.lock_state().prepare.clone() else {
            return;
        };
        if record.root_mode != crate::shared::spec::RootMode::Writable {
            return;
        }
        let root = &record.exec_root;
        let mut targets: Vec<PathBuf> = Vec::new();
        for mount in &record.request.mounts {
            targets.push(root.join(mount.guest_path.trim_start_matches('/')));
        }
        if !record.request.dns.is_empty() {
            targets.push(root.join("etc/resolv.conf"));
        }
        for directory in ["dev", "sys", "proc"] {
            targets.push(root.join(directory));
        }
        for target in &targets {
            if let Err(error) = sys::unmount(target, 0) {
                tracing::warn!(
                    target = %target.display(),
                    error = %error,
                    "cannot unmount a bind mount of the writable root"
                );
            }
        }
        self.host.power.sync();
        if let Err(error) = sys::unmount(root, 0) {
            tracing::warn!(
                root = %root.display(),
                error = %error,
                "cannot unmount the writable root: remount it read-only"
            );
            #[allow(
                clippy::unnecessary_cast,
                reason = "the mount flag type is not the same on every system"
            )]
            let flags = (libc::MS_REMOUNT | libc::MS_RDONLY) as u64;
            if let Err(error) = self.host.mounts.mount("none", root, "", flags, "") {
                tracing::warn!(
                    root = %root.display(),
                    error = %error,
                    "cannot remount the writable root read-only"
                );
            }
        }
    }

    /// Ends every session and every upload. The server calls it when its loop
    /// ends, and `Shutdown` calls it after the grace time.
    pub fn end_everything(&self) {
        end_everything(
            &self.sessions,
            self.files
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            self.unpack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
    }
}

/// Ends every session, removes every upload and stops every unpack.
fn end_everything(
    sessions: &Arc<SessionTable>,
    files: Option<Arc<Files>>,
    unpack: Option<Arc<UnpackSessions>>,
) {
    for session in sessions.snapshot() {
        let _ = session.signal_group(libc::SIGKILL);
    }
    if let Some(files) = files {
        files.remove_all();
    }
    if let Some(unpack) = unpack {
        unpack.remove_all();
    }
}

/// Stops the inner sandboxes of a slave with the control role.
///
/// The children stop first, then the programs of the slave (feature 11,
/// section 6.1). A failed stop is not reported: the slave is going away.
#[cfg(feature = "control")]
async fn stop_inner_sandboxes(control: &Arc<dyn SandboxProvider>) {
    let Ok(sandboxes) = control.list().await else {
        return;
    };
    for info in sandboxes {
        let Ok(id) = info.id.parse() else {
            continue;
        };
        let _ = control.stop(id).await;
    }
}

/// Stops the sessions with `SIGTERM`, waits up to `grace`, then kills them and
/// stops the server.
async fn shutdown_sessions(
    ctx: Ctx,
    sessions: Arc<SessionTable>,
    files: Option<Arc<Files>>,
    unpack: Option<Arc<UnpackSessions>>,
    shutdown: Arc<ShutdownSignal>,
    grace: std::time::Duration,
    #[cfg(feature = "control")] control: Option<Arc<dyn SandboxProvider>>,
) {
    // The children stop first.
    #[cfg(feature = "control")]
    if let Some(control) = &control {
        stop_inner_sandboxes(control).await;
    }
    for session in sessions.snapshot() {
        let _ = session.signal_group(libc::SIGTERM);
    }
    let deadline = ctx.clock().now() + grace;
    while !sessions.is_empty() && ctx.clock().now() < deadline {
        let remaining = deadline.saturating_duration_since(ctx.clock().now());
        let step = remaining.min(std::time::Duration::from_millis(20));
        foundation_core::valtron::timer::sleep_for(step, &ctx).await;
    }
    end_everything(&sessions, files, unpack);
    shutdown.turn_on();
}

/// Starts the task that removes an upload that stays idle.
fn start_upload_timer(ctx: Ctx, files: Arc<Files>, interval: std::time::Duration) {
    let future = async move {
        loop {
            foundation_core::valtron::timer::sleep_for(interval, &ctx).await;
            if let Err(error) = files.expire_idle() {
                tracing::warn!(error = %error, "the slave could not remove an idle upload");
            }
        }
    };
    if let Err(error) =
        foundation_core::valtron::send(foundation_core::valtron::from_future(future))
    {
        tracing::error!(error = %error, "the slave could not start the upload timer");
    }
}

/// The child limits of one unpack.
fn unpack_child_limits(limits: crate::shared::rpc::messages::SlaveLimits) -> ChildOptions {
    ChildOptions {
        oci_whiteouts: false,
        keep_existing_dirs: false,
        skip: Vec::new(),
        max_entries: usize::try_from(limits.max_unpack_entries).unwrap_or(usize::MAX),
        max_bytes: limits.max_unpack_bytes,
        max_path_bytes: usize::try_from(limits.max_unpack_path_bytes).unwrap_or(usize::MAX),
    }
}

/// Starts the task that stops an unpack that stays idle.
fn start_unpack_timer(ctx: Ctx, unpack: Arc<UnpackSessions>, interval: std::time::Duration) {
    let future = async move {
        loop {
            foundation_core::valtron::timer::sleep_for(interval, &ctx).await;
            unpack.expire_idle();
        }
    };
    if let Err(error) =
        foundation_core::valtron::send(foundation_core::valtron::from_future(future))
    {
        tracing::error!(error = %error, "the slave could not start the unpack timer");
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the service trait declares async methods; a handler that does no I/O has nothing to await"
)]
impl SlaveService for Slave {
    async fn info(
        &self,
        _ctx: RpcCtx,
        _request: Request<InfoRequest>,
    ) -> ConnectResult<Response<SlaveInfo>> {
        let (arch, kernel) =
            sys::uname().unwrap_or_else(|_| ("unknown".to_string(), "unknown".to_string()));
        let state = self.lock_state().clone();
        Ok(Response::new(SlaveInfo {
            slave_version: env!("CARGO_PKG_VERSION").to_string(),
            arch,
            kernel,
            is_pid1: self.pid1,
            prepared: state.is_prepared(),
            can_control: self.can_control(),
        }))
    }

    async fn prepare(
        &self,
        _ctx: RpcCtx,
        request: Request<PrepareRequest>,
    ) -> ConnectResult<Response<PrepareResponse>> {
        let max = self.max_error_message();
        let answer = self.prepare_root(&request.msg);
        to_connect(answer, max).map(Response::new)
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<ExecEventMsg>> + Send>>> {
        let max = self.max_error_message();
        let prepared = to_connect(self.prepared(), max)?;
        let started = exec::start(
            &self.ctx,
            &prepared,
            &self.children,
            &self.sessions,
            &request.msg,
        )
        .await;
        match started {
            Ok((_session, stream)) => Ok(Box::pin(map_events(stream))),
            Err(error) => Err(connect_of(&error, max)),
        }
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        let max = self.max_error_message();
        let answer = self.exec_input_inner(&request.msg);
        to_connect(answer, max).map(Response::new)
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let max = self.max_error_message();
        let answer = self.exec_signal_inner(&request.msg);
        to_connect(answer, max).map(|()| Response::new(Empty {}))
    }

    async fn file_write_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteBeginRequest>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.files())
            .and_then(|files| files.begin(&request.msg.path, request.msg.mode))
            .map(|upload_id| FileWriteBeginResponse {
                upload_id: upload_id.to_string(),
            });
        to_connect(answer, max).map(Response::new)
    }

    async fn file_write_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteChunkRequest>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.files())
            .and_then(|files| {
                files.chunk(
                    &request.msg.upload_id,
                    request.msg.offset,
                    &request.msg.data,
                )
            })
            .map(|next_offset| FileWriteChunkResponse { next_offset });
        to_connect(answer, max).map(Response::new)
    }

    async fn file_write_commit(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteCommitRequest>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.files())
            .and_then(|files| files.commit(&request.msg.upload_id, request.msg.size))
            .map(|size| FileWriteCommitResponse { size });
        to_connect(answer, max).map(Response::new)
    }

    async fn file_write_abort(
        &self,
        _ctx: RpcCtx,
        request: Request<FileWriteAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.files())
            .and_then(|files| files.abort(&request.msg.upload_id));
        to_connect(answer, max).map(|()| Response::new(Empty {}))
    }

    async fn file_read(
        &self,
        _ctx: RpcCtx,
        request: Request<FileReadRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<FileChunk>> + Send>>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.files())
            .and_then(|files| files.read_file(&request.msg.path));
        match answer {
            Ok(stream) => Ok(Box::pin(stream)),
            Err(error) => Err(connect_of(&error, max)),
        }
    }

    async fn unpack_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackBeginRequest>,
    ) -> ConnectResult<Response<UnpackBeginResponse>> {
        let max = self.max_error_message();
        let answer = async {
            let prepared = self.prepared()?;
            let sessions = self.unpack_sessions()?;
            let mut child = unpack_child_limits(prepared.limits());
            child.oci_whiteouts = request.msg.oci_whiteouts;
            child.keep_existing_dirs = request.msg.keep_existing_dirs;
            child.skip.clone_from(&request.msg.skip);
            let upload_id = sessions
                .begin(&prepared.exec_root, &request.msg.dest, &child)
                .await?;
            Ok(UnpackBeginResponse {
                upload_id: upload_id.to_string(),
            })
        }
        .await;
        to_connect(answer, max).map(Response::new)
    }

    async fn unpack_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackChunkRequest>,
    ) -> ConnectResult<Response<UnpackChunkResponse>> {
        let max = self.max_error_message();
        let answer = async {
            let _ = self.prepared()?;
            let sessions = self.unpack_sessions()?;
            sessions
                .chunk(
                    &request.msg.upload_id,
                    request.msg.offset,
                    &request.msg.data,
                )
                .await
        }
        .await
        .map(|next_offset| UnpackChunkResponse { next_offset });
        to_connect(answer, max).map(Response::new)
    }

    async fn unpack_commit(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackCommitRequest>,
    ) -> ConnectResult<Response<UnpackCommitResponse>> {
        let max = self.max_error_message();
        let answer = async {
            let _ = self.prepared()?;
            let sessions = self.unpack_sessions()?;
            sessions
                .commit(&request.msg.upload_id, request.msg.size)
                .await
        }
        .await
        .map(|(entries, bytes)| UnpackCommitResponse { entries, bytes });
        to_connect(answer, max).map(Response::new)
    }

    async fn unpack_abort(
        &self,
        _ctx: RpcCtx,
        request: Request<UnpackAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let max = self.max_error_message();
        let answer = self
            .prepared()
            .and_then(|_| self.unpack_sessions())
            .map(|sessions| sessions.abort(&request.msg.upload_id));
        to_connect(answer, max).map(|()| Response::new(Empty {}))
    }

    async fn shutdown(
        &self,
        _ctx: RpcCtx,
        request: Request<ShutdownRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let first = {
            let mut state = self.lock_state();
            if state.phase() == Phase::ShuttingDown {
                false
            } else {
                state.phase = Some(Phase::ShuttingDown);
                true
            }
        };
        if first {
            let ctx = self.ctx.clone();
            let sessions = Arc::clone(&self.sessions);
            let files = self
                .files
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let unpack = self
                .unpack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let shutdown = Arc::clone(&self.shutdown);
            let grace = std::time::Duration::from_millis(u64::from(request.msg.grace_ms));
            #[cfg(feature = "control")]
            let control = self.control();
            let future = async move {
                shutdown_sessions(
                    ctx,
                    sessions,
                    files,
                    unpack,
                    shutdown,
                    grace,
                    #[cfg(feature = "control")]
                    control,
                )
                .await;
            };
            if let Err(error) =
                foundation_core::valtron::send(foundation_core::valtron::from_future(future))
            {
                tracing::error!(error = %error, "the slave could not start the shutdown");
                self.shutdown.turn_on();
            }
        }
        Ok(Response::new(Empty {}))
    }
}

/// The typed error of one shared failure, on the wire.
fn connect_of(error: &ErrorTrace<SandboxError>, max: usize) -> ErrorTrace<ConnectError> {
    ErrorTrace::new(error.current_context().to_connect(max))
}

/// One shared result as a Connect result.
fn to_connect<T>(result: SandboxResult<T>, max: usize) -> ConnectResult<T> {
    result.map_err(|error| connect_of(&error, max))
}

/// The events of one session as Connect stream items.
fn map_events(stream: SessionStream) -> impl Stream<Item = ConnectResult<ExecEventMsg>> + Send {
    stream.map(Ok)
}

impl Slave {
    /// The `exec_input` body.
    fn exec_input_inner(&self, request: &ExecInputRequest) -> SandboxResult<ExecInputResponse> {
        let prepared = self.prepared()?;
        if request.data.len() > prepared.limits().chunk_bytes as usize {
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                format!(
                    "the input chunk is longer than chunk_bytes ({} bytes)",
                    request.data.len()
                ),
            ));
        }
        let exec_id = Self::parse_exec_id(&request.exec_id)?;
        let session = self.session(&exec_id)?;
        let next_seq = match session.feed(request.seq, request.data.clone(), request.eof)? {
            exec::InputOutcome::Applied(next) | exec::InputOutcome::Duplicate(next) => next,
        };
        Ok(ExecInputResponse { next_seq })
    }

    /// The `exec_signal` body.
    fn exec_signal_inner(&self, request: &ExecSignalRequest) -> SandboxResult<()> {
        self.prepared()?;
        if !(1..=64).contains(&request.signal) {
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                Some(libc::EINVAL),
                format!("the signal number is out of range: {}", request.signal),
            ));
        }
        let exec_id = Self::parse_exec_id(&request.exec_id)?;
        let session = match self.sessions.get(&exec_id) {
            Some(session) => session,
            // A session that ended recently accepts the call and does nothing.
            None if self.sessions.is_recently_finished(&exec_id) => return Ok(()),
            None => return Err(Self::unknown_session(&exec_id)),
        };
        match session.signal_group(request.signal) {
            Ok(()) => Ok(()),
            // A program that ended already accepts the call and does nothing.
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => Ok(()),
            Err(error) => Err(slave_error(
                SlaveErrorCode::Io,
                error.raw_os_error(),
                format!("send signal {}: {error}", request.signal),
            )),
        }
    }

    fn session(&self, exec_id: &ExecId) -> SandboxResult<Arc<exec::Session>> {
        self.sessions
            .get(exec_id)
            .ok_or_else(|| Self::unknown_session(exec_id))
    }

    fn unknown_session(exec_id: &ExecId) -> foundation_errstacks::ErrorTrace<SandboxError> {
        slave_error(
            SlaveErrorCode::NotFound,
            None,
            format!("unknown exec_id: {exec_id}"),
        )
    }

    fn parse_exec_id(value: &str) -> SandboxResult<ExecId> {
        value.parse::<ExecId>().map_err(|_| {
            slave_error(
                SlaveErrorCode::BadRequest,
                None,
                format!("exec_id must be 16 lower-case hexadecimal characters: {value}"),
            )
        })
    }
}

/// Blocks `SIGCHLD` on the calling thread.
///
/// The slave calls this before it starts the valtron pool or any other thread,
/// because a signal mask belongs to a thread and a new thread copies it. A
/// block that comes later leaves threads that can take the signal, and the
/// `signalfd` then misses it.
///
/// # Errors
/// The operating system error of the signal mask call.
pub fn block_sigchld() -> std::io::Result<()> {
    sys::block_sigchld()
}

/// Runs the slave. It returns the exit code when the slave is not process
/// identifier 1, and never returns when it is.
pub fn run(ctx: Ctx, args: SlaveArgs) -> i32 {
    if let Err(error) = block_sigchld() {
        write_error(&format!("agent-slave: the signal mask failed: {error}"));
        return 70;
    }
    let pid1 = sys::getpid() == 1;
    if !pid1 {
        if let Err(error) = sys::become_subreaper() {
            tracing::warn!(error = %error, "the slave could not become a child subreaper");
        }
    }
    let host = ctx.get::<SlaveHost>().map_or_else(
        || Arc::new(SlaveHost::native()),
        |host| Arc::new((*host).clone()),
    );

    if pid1 {
        if let Err(error) = pid1::boot_duties(&host) {
            write_error(&format!("agent-slave: the PID 1 set-up failed: {error}"));
            pid1::finish_and_power_off(&host);
        }
    }

    start_pool(&ctx);

    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = match serve::bind_with_mode(&args.listen, Arc::clone(&shutdown), args.socket_mode) {
        Ok(bound) => bound,
        Err(error) => {
            write_error(&format!("agent-slave: the bind failed: {error}"));
            if pid1 {
                pid1::finish_and_power_off(&host);
            }
            return 69;
        }
    };

    let slave = Slave::with_host(
        ctx.clone(),
        Arc::clone(&host),
        args.plain_root,
        Arc::clone(&shutdown),
    );

    if args.control {
        #[cfg(all(feature = "control", feature = "firecracker"))]
        match control::build(&ctx) {
            Ok(role) => slave.set_control(role.provider, role.can_control),
            // A failure to build the inner control plane leaves the slave
            // unable to control; the control paths answer `DriverUnavailable`.
            Err(error) => {
                tracing::warn!(error = %error, "the slave could not build its control role");
            }
        }
        #[cfg(not(all(feature = "control", feature = "firecracker")))]
        tracing::warn!(
            "--control is ignored: this build has no `control` and `firecracker` features"
        );
    }

    if let Err(error) = reaper::start(Arc::clone(slave.children())) {
        write_error(&format!("agent-slave: the reaper set-up failed: {error}"));
        if pid1 {
            pid1::finish_and_power_off(&host);
        }
        return 69;
    }

    if pid1 {
        // The pid1 flag must be true in the service. It is read at construction,
        // so a process that reaches here as PID 1 holds it.
        debug_assert!(slave.is_pid1());
    }

    serve::serve(&bound, ctx, &slave);
    // The listener loop ended. The children stop first, then the sessions and
    // the uploads of the slave: nothing stays behind.
    #[cfg(feature = "control")]
    if let Some(control) = slave.control() {
        let control = Arc::clone(&control);
        foundation_core::valtron::block_on_future(async move {
            stop_inner_sandboxes(&control).await;
        });
    }
    slave.end_everything();
    // A writable image must be unmounted before the machine stops, so a later
    // mount needs no journal replay.
    slave.unmount_writable_root();

    if pid1 {
        pid1::finish_and_power_off(&host);
    }
    0
}

/// Starts the valtron pool, unless the caller started one.
fn start_pool(ctx: &Ctx) {
    if foundation_core::valtron::multi::is_pool_initialized() {
        return;
    }
    let seed = {
        let rng = ctx.rng();
        let mut rng = rng.lock().expect("the Ctx RNG lock is not poisoned");
        rng.next_u64()
    };
    let guard = foundation_core::valtron::initialize_pool_with_ctx(
        seed,
        Some(DEFAULT_POOL_THREADS),
        ctx.clone(),
    );
    // The pool lives for the whole process; the server blocks until the slave
    // stops, so the guard is not dropped before then.
    std::mem::forget(guard);
}

/// Writes one line to the console.
fn write_error(text: &str) {
    eprintln!("{text}");
}

/// The real guest effects of this host.
impl SlaveHost {
    /// The guest effects of a real Linux guest.
    #[must_use]
    pub fn native() -> Self {
        Self {
            mounts: Arc::new(NativeMounts),
            power: Arc::new(NativePower),
        }
    }
}

/// The real mounts.
struct NativeMounts;

impl GuestMounts for NativeMounts {
    fn mount(
        &self,
        source: &str,
        target: &std::path::Path,
        fstype: &str,
        flags: u64,
        data: &str,
    ) -> std::io::Result<()> {
        sys::mount(source, target, fstype, flags, data)
    }

    fn mkdir_p(&self, path: &std::path::Path, mode: u32) -> std::io::Result<()> {
        // Set the mode only on the directory that this call makes. A caller
        // that prepares a plain root may name a directory that it does not own,
        // such as `/tmp`; a `chmod` there fails with `EPERM` and it would also
        // remove a mode that the directory needs (the sticky bit of `/tmp`).
        match std::fs::create_dir(path) {
            Ok(()) => sys::set_mode(path, mode),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if path.is_dir() {
                    Ok(())
                } else {
                    Err(error)
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(path)?;
                sys::set_mode(path, mode)
            }
            Err(error) => Err(error),
        }
    }

    fn write_file(&self, path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, data)
    }
}

/// The real power state.
struct NativePower;

impl GuestPower for NativePower {
    fn sync(&self) {
        sys::sync();
    }

    fn power_off(&self) -> std::io::Result<std::convert::Infallible> {
        sys::power_off()
    }
}

// ── The wait helpers that the tasks share ───────────────────────────────────

pub(crate) mod wait {
    use std::sync::Arc;
    use std::task::Poll;

    use foundation_core::valtron::EventReadiness;
    use foundation_reactor::fd_readiness::FdReadiness;
    use futures::future::FutureExt;

    use super::exec::{InputItem, InputQueue, Notify};

    /// Waits until `condition` holds. It registers the waker before it looks,
    /// so a change that happens at the same time is not lost.
    pub async fn until(notify: Arc<Notify>, condition: impl Fn() -> bool) {
        futures::future::poll_fn(move |cx| {
            notify.register(cx.waker());
            if condition() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    /// Waits until the descriptor is ready.
    ///
    /// The caller clears the readiness before it performs its I/O attempt, not
    /// here. The shared selector is edge-triggered: a clear before the wait
    /// erases the edge that the wait exists to return, and a level condition
    /// that emits no new edge (a pending `SIGCHLD`, end of file, the rest of a
    /// pipe) then never wakes the task again.
    pub async fn ready(readiness: Arc<FdReadiness>) {
        futures::future::poll_fn(move |cx| {
            if readiness.is_ready(None) {
                return Poll::Ready(());
            }
            if !readiness.register_waker(cx.waker()) {
                // The descriptor is gone: let the caller read and see the end.
                return Poll::Ready(());
            }
            if readiness.is_ready(None) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    /// Waits until the descriptor is ready or the session stopped. It reports
    /// `true` when the session stopped.
    pub async fn ready_or_cancelled(
        readiness: Arc<FdReadiness>,
        stop: Arc<Notify>,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        let ready = Box::pin(ready(readiness)).fuse();
        let stopped = Box::pin(until(stop, cancelled)).fuse();
        futures::pin_mut!(ready, stopped);
        match futures::future::select(ready, stopped).await {
            futures::future::Either::Left(((), _)) => false,
            futures::future::Either::Right(((), _)) => true,
        }
    }

    /// Waits for one input item or for the session to stop.
    pub async fn item_or_stop(
        queue: Arc<InputQueue>,
        stop: Arc<Notify>,
        cancelled: impl Fn() -> bool,
    ) -> Option<InputItem> {
        let item = Box::pin(queue.next_item()).fuse();
        let stopped = Box::pin(until(stop, cancelled)).fuse();
        futures::pin_mut!(item, stopped);
        match futures::future::select(item, stopped).await {
            futures::future::Either::Left((item, _)) => item,
            futures::future::Either::Right(((), _)) => None,
        }
    }
}

/// The error of one slave call.
pub(crate) fn slave_error(
    code: SlaveErrorCode,
    errno: Option<i32>,
    message: impl Into<String>,
) -> ErrorTrace<SandboxError> {
    ErrorTrace::new(SandboxError::SlaveError {
        code,
        errno,
        message: message.into(),
    })
}
