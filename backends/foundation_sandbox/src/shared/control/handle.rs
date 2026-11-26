//! The handle behaviour: exec sessions, input, files and `run`.
//!
//! The code works on [`ExecFileCalls`], so the local slave client and the
//! remote `SandboxService` client share it.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{ClientOptions, ConnectError, Request, RpcCtx};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::{Stream, StreamExt};

use crate::shared::command::{Command, ExecEvent, ExecOutput, ExitStatus, StdinMode};
use crate::shared::control::manager::Inner;
use crate::shared::control::record::SandboxRecord;
use crate::shared::control::slave_client::{chunk_stream, event_stream};
use crate::shared::control::stop::{kill_record, spawn_detached, stop_record};
use crate::shared::control::{send_box, wait_until};
use crate::shared::control::{ExitReport, SandboxInfo, SandboxOps, SandboxState};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::{
    ExecEvents, ExecFileCalls, ExecHandle, ExecHandleOps, ExecStdin, ExecStdinOps, FileStream,
};
use crate::shared::ids::ExecId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    ExecEventKind, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest,
    ExecSignalRequest, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, SandboxRef, Scoped, UnpackAbortRequest, UnpackBeginRequest,
    UnpackBeginResponse, UnpackChunkRequest, UnpackChunkResponse, UnpackCommitRequest,
    UnpackCommitResponse,
};
use crate::shared::rpc::SandboxServiceClient;
use crate::shared::sequence::{Sequence, SequenceCheck};
use crate::shared::unpack::{UnpackOptions, UnpackReport, UNPACK_LIMIT_MESSAGE};
use crate::shared::{BoxByteStream, BoxFuture, ExecEventStream};

/// The address that the remote client uses in the request line.
const CONTROL_AUTHORITY: &str = "http://control.local";

/// An iterator that is `Send` on every target.
///
/// On a native target the wrapped iterator is `Send` already. On single
/// threaded wasm [`foundation_compact::SendWrapper`] supplies the bound, and
/// the value never crosses a thread because the target has one.
pub(crate) struct SendIterator<I>(foundation_compact::SendWrapper<I>);

impl<I> SendIterator<I> {
    pub(crate) fn new(inner: I) -> Self {
        Self(foundation_compact::SendWrapper::new(inner))
    }
}

impl<I: Iterator> Iterator for SendIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// The state that one execution session shares between its handle and its
/// input writer.
struct ExecShared {
    ctx: Ctx,
    calls: Arc<dyn ExecFileCalls>,
    exec_id: ExecId,
    limits: SandboxLimits,
    next_input_seq: AtomicU64,
    eof_sent: AtomicBool,
    timed_out: AtomicBool,
    exit: Mutex<Option<ExitStatus>>,
}

/// The operations behind one [`ExecHandle`] of this crate.
struct LocalExecOps {
    shared: Arc<ExecShared>,
    pid: u32,
    stream: Mutex<Option<ExecEventStream>>,
    stdin_pending: AtomicBool,
}

impl ExecHandleOps for LocalExecOps {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn stdin(&self) -> Option<ExecStdin> {
        if !self.stdin_pending.swap(false, Ordering::SeqCst) {
            return None;
        }
        Some(ExecStdin::new(Arc::new(LocalExecStdin {
            shared: Arc::clone(&self.shared),
        })))
    }

    fn events(&self) -> ExecEvents {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        match stream {
            Some(stream) => checked_events(stream, Arc::clone(&self.shared)),
            None => ExecEvents::new(Box::pin(futures::stream::empty())),
        }
    }

    fn signal(&self, signal: i32) -> BoxFuture<'_, SandboxResult<()>> {
        let shared = Arc::clone(&self.shared);
        send_box(async move {
            shared
                .calls
                .exec_signal(ExecSignalRequest {
                    exec_id: shared.exec_id.to_string(),
                    signal,
                })
                .await
        })
    }

    fn timed_out(&self) -> bool {
        self.shared.timed_out.load(Ordering::SeqCst)
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<ExitStatus>> {
        let shared = Arc::clone(&self.shared);
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        send_box(async move {
            if let Some(stream) = stream {
                let mut events = checked_events(stream, Arc::clone(&shared));
                while let Some(item) = events.next().await {
                    item?;
                }
            }
            let exit = *shared
                .exit
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match exit {
                Some(_status) if shared.timed_out.load(Ordering::SeqCst) => {
                    Err(SandboxError::ExecTimeout.into())
                }
                Some(status) => Ok(status),
                None => Err(SandboxError::StreamLost.into()),
            }
        })
    }
}

/// The input writer of one execution session.
struct LocalExecStdin {
    shared: Arc<ExecShared>,
}

impl ExecStdinOps for LocalExecStdin {
    fn write(&self, bytes: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>> {
        let shared = Arc::clone(&self.shared);
        send_box(async move {
            let chunk = shared.limits.chunk_bytes.max(1);
            for piece in bytes.chunks(chunk) {
                let seq = shared.next_input_seq.load(Ordering::SeqCst);
                let next = send_input(&shared, seq, piece.to_vec(), false).await?;
                shared.next_input_seq.store(next, Ordering::SeqCst);
            }
            Ok(())
        })
    }

    fn close(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let shared = Arc::clone(&self.shared);
        send_box(async move { send_eof(&shared).await })
    }
}

impl Drop for LocalExecStdin {
    fn drop(&mut self) {
        if self.shared.eof_sent.load(Ordering::SeqCst) {
            return;
        }
        let shared = Arc::clone(&self.shared);
        spawn_detached(async move {
            let _ = send_eof(&shared).await;
        });
    }
}

/// Sends the end of the input one time.
async fn send_eof(shared: &Arc<ExecShared>) -> SandboxResult<()> {
    if shared.eof_sent.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let seq = shared.next_input_seq.load(Ordering::SeqCst);
    let next = send_input(shared, seq, Vec::new(), true).await?;
    shared.next_input_seq.store(next, Ordering::SeqCst);
    Ok(())
}

/// Sends one input chunk, with the retry rules of the design.
async fn send_input(
    shared: &Arc<ExecShared>,
    seq: u64,
    data: Vec<u8>,
    eof: bool,
) -> SandboxResult<u64> {
    let started = shared.ctx.clock().now();
    let mut attempt = 0_u32;
    let mut transport_retried = false;
    loop {
        let request = ExecInputRequest {
            exec_id: shared.exec_id.to_string(),
            seq,
            data: data.clone(),
            eof,
        };
        match shared.calls.exec_input(request).await {
            Ok(response) => return Ok(response.next_seq),
            Err(error) => match error.current_context() {
                SandboxError::InputQueueFull => {
                    let now = shared.ctx.clock().now();
                    if now.saturating_duration_since(started) >= shared.limits.input_retry_timeout {
                        return Err(SandboxError::InputQueueFull.into());
                    }
                    let backoff = retry_backoff(
                        attempt,
                        Duration::from_millis(5),
                        Duration::from_millis(200),
                    );
                    attempt = attempt.saturating_add(1);
                    wait_until(now + backoff, &shared.ctx).await?;
                }
                SandboxError::Rpc { code, .. } if code == "unavailable" && !transport_retried => {
                    transport_retried = true;
                }
                _ => return Err(error),
            },
        }
    }
}

/// `base * 2^attempt`, cut at `cap`.
fn retry_backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let factor = 1_u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
    base.saturating_mul(factor).min(cap)
}

/// The event stream of one session, with the sequence check.
fn checked_events(stream: ExecEventStream, shared: Arc<ExecShared>) -> ExecEvents {
    let state = EventsState {
        stream: Some(stream),
        expected: 1,
        done: false,
        shared,
    };
    ExecEvents::new(Box::pin(futures::stream::unfold(
        state,
        |state| async move {
            let (item, state) = state.next_event().await;
            item.map(|item| (item, state))
        },
    )))
}

struct EventsState {
    stream: Option<ExecEventStream>,
    expected: u64,
    done: bool,
    shared: Arc<ExecShared>,
}

impl EventsState {
    async fn next_event(mut self) -> (Option<SandboxResult<ExecEvent>>, Self) {
        if self.done {
            return (None, self);
        }
        let Some(stream) = self.stream.as_mut() else {
            self.done = true;
            return (None, self);
        };
        match stream.next().await {
            Some(Ok(message)) => {
                if message.seq != self.expected {
                    self.done = true;
                    return (Some(Err(SandboxError::StreamLost.into())), self);
                }
                self.expected += 1;
                match message.event {
                    ExecEventKind::Stdout { data } => (Some(Ok(ExecEvent::Stdout(data))), self),
                    ExecEventKind::Stderr { data } => (Some(Ok(ExecEvent::Stderr(data))), self),
                    ExecEventKind::Exit { code, signal } => {
                        let status = match (code, signal) {
                            (Some(code), _) => ExitStatus::code(code),
                            (None, Some(signal)) => ExitStatus::signal(signal),
                            (None, None) => ExitStatus::code(0),
                        };
                        *self
                            .shared
                            .exit
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status);
                        self.done = true;
                        (Some(Ok(ExecEvent::Exit(status))), self)
                    }
                    ExecEventKind::Started { .. } => {
                        self.done = true;
                        (Some(Err(SandboxError::StreamLost.into())), self)
                    }
                }
            }
            Some(Err(_error)) => {
                self.done = true;
                (Some(Err(SandboxError::StreamLost.into())), self)
            }
            None => {
                self.done = true;
                (Some(Err(SandboxError::StreamLost.into())), self)
            }
        }
    }
}

/// Opens one execution session and returns its handle.
///
/// It reads the `Started` event before it returns, so the caller reads the
/// program events from the first output byte.
///
/// # Errors
/// The error of the `Exec` call, or [`SandboxError::StreamLost`] when the
/// first event is not `Started` with sequence zero.
pub async fn exec_session(
    ctx: Ctx,
    limits: SandboxLimits,
    calls: Arc<dyn ExecFileCalls>,
    command: Command,
) -> SandboxResult<ExecHandle> {
    let exec_id = ExecId::generate(&ctx);
    exec_session_with_id(ctx, limits, calls, command, exec_id).await
}

/// Opens one execution session with the caller's execution identifier.
///
/// # Errors
/// The error of the `Exec` call, or [`SandboxError::StreamLost`] when the
/// first event is not `Started` with sequence zero.
pub async fn exec_session_with_id(
    ctx: Ctx,
    limits: SandboxLimits,
    calls: Arc<dyn ExecFileCalls>,
    command: Command,
    exec_id: ExecId,
) -> SandboxResult<ExecHandle> {
    if command.timeout.is_some() && ctx.require_env().is_err() {
        return Err(SandboxError::HostNotConfigured.into());
    }
    let stdin_requested = !matches!(&command.stdin, StdinMode::Null);
    let timeout_ms = command
        .timeout
        .map(|timeout| u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
    let request = ExecRequest {
        exec_id: exec_id.to_string(),
        argv: command.argv.clone(),
        env: command.env.clone(),
        cwd: command.cwd.clone(),
        stdin: stdin_requested,
        timeout_ms,
        user: command.user.clone(),
    };
    let mut stream = calls.exec(request).await?;
    let Some(Ok(ExecEventMsg {
        seq: 0,
        event: ExecEventKind::Started { pid },
    })) = stream.next().await
    else {
        return Err(SandboxError::StreamLost.into());
    };

    let shared = Arc::new(ExecShared {
        ctx: ctx.clone(),
        calls,
        exec_id: exec_id.clone(),
        limits: limits.clone(),
        next_input_seq: AtomicU64::new(0),
        eof_sent: AtomicBool::new(false),
        timed_out: AtomicBool::new(false),
        exit: Mutex::new(None),
    });

    if let StdinMode::Bytes(bytes) = &command.stdin {
        let writer = LocalExecStdin {
            shared: Arc::clone(&shared),
        };
        let bytes = bytes.clone();
        spawn_detached(async move {
            if writer.write(bytes).await.is_ok() {
                let _ = writer.close().await;
            }
        });
    }

    if let Some(timeout) = command.timeout {
        let deadline = ctx.clock().now() + timeout;
        let timer_shared = Arc::clone(&shared);
        spawn_detached(async move {
            if wait_until(deadline, &timer_shared.ctx).await.is_err() {
                return;
            }
            if timer_shared
                .exit
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            {
                return;
            }
            timer_shared.timed_out.store(true, Ordering::SeqCst);
            let _ = timer_shared
                .calls
                .exec_signal(ExecSignalRequest {
                    exec_id: timer_shared.exec_id.to_string(),
                    signal: 9,
                })
                .await;
        });
    }

    let ops = LocalExecOps {
        shared,
        pid,
        stream: Mutex::new(Some(stream)),
        stdin_pending: AtomicBool::new(matches!(&command.stdin, StdinMode::Piped)),
    };
    Ok(ExecHandle::new(exec_id, Arc::new(ops)))
}

/// Runs one command and captures at most `max` bytes of each stream.
///
/// # Errors
/// The error of the `Exec` call or of the event stream.
pub async fn run_capture(mut handle: ExecHandle, max: usize) -> SandboxResult<ExecOutput> {
    let mut events = handle.events();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;
    let mut status = None;
    while let Some(item) = events.next().await {
        match item? {
            ExecEvent::Stdout(data) => {
                append_capped(&mut stdout, &data, max, &mut stdout_truncated);
            }
            ExecEvent::Stderr(data) => {
                append_capped(&mut stderr, &data, max, &mut stderr_truncated);
            }
            ExecEvent::Exit(exit) => {
                status = Some(exit);
                break;
            }
        }
    }
    let status = status.ok_or(SandboxError::StreamLost)?;
    if handle.timed_out() {
        return Err(SandboxError::ExecTimeout.into());
    }
    Ok(ExecOutput {
        stdout,
        stderr,
        status,
        stdout_truncated,
        stderr_truncated,
    })
}

fn append_capped(target: &mut Vec<u8>, data: &[u8], max: usize, truncated: &mut bool) {
    let room = max.saturating_sub(target.len());
    if data.len() > room {
        *truncated = true;
        target.extend_from_slice(&data[..room]);
    } else {
        target.extend_from_slice(data);
    }
}

/// The abort guard of one file write. It aborts the upload when the caller
/// drops the future before the commit.
struct WriteAbort {
    calls: Arc<dyn ExecFileCalls>,
    upload_id: Option<String>,
    armed: bool,
}

impl WriteAbort {
    fn disarm(&mut self) {
        self.armed = false;
    }

    async fn abort_now(&mut self) {
        self.armed = false;
        if let Some(upload_id) = self.upload_id.take() {
            let _ = self
                .calls
                .file_write_abort(FileWriteAbortRequest { upload_id })
                .await;
        }
    }
}

impl Drop for WriteAbort {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(upload_id) = self.upload_id.take() else {
            return;
        };
        let calls = Arc::clone(&self.calls);
        spawn_detached(async move {
            let _ = calls
                .file_write_abort(FileWriteAbortRequest { upload_id })
                .await;
        });
    }
}

/// Writes one file from a stream of byte chunks.
///
/// # Errors
/// The error of the begin, chunk or commit call, or the error of the byte
/// stream.
pub async fn write_file_stream(
    limits: SandboxLimits,
    calls: Arc<dyn ExecFileCalls>,
    path: String,
    mode: u32,
    data: BoxByteStream,
) -> SandboxResult<u64> {
    let begin = calls
        .file_write_begin(FileWriteBeginRequest { path, mode })
        .await?;
    let mut guard = WriteAbort {
        calls: Arc::clone(&calls),
        upload_id: Some(begin.upload_id.clone()),
        armed: true,
    };
    let mut data = data;
    let mut offset = 0_u64;
    let chunk = limits.chunk_bytes.max(1);
    loop {
        match data.next().await {
            None => break,
            Some(Err(error)) => {
                guard.abort_now().await;
                return Err(error);
            }
            Some(Ok(bytes)) => {
                for piece in bytes.chunks(chunk) {
                    let answer = calls
                        .file_write_chunk(FileWriteChunkRequest {
                            upload_id: begin.upload_id.clone(),
                            offset,
                            data: piece.to_vec(),
                        })
                        .await;
                    match answer {
                        Ok(response) => {
                            offset = response.next_offset;
                        }
                        Err(error) => {
                            guard.abort_now().await;
                            return Err(error);
                        }
                    }
                }
            }
        }
    }
    let answer = calls
        .file_write_commit(FileWriteCommitRequest {
            upload_id: begin.upload_id,
            size: offset,
        })
        .await;
    match answer {
        Ok(response) => {
            guard.disarm();
            Ok(response.size)
        }
        Err(error) => {
            guard.abort_now().await;
            Err(error)
        }
    }
}

/// The abort guard of one unpack. It aborts the session when the caller drops
/// the future before the commit.
struct UnpackAbort {
    calls: Arc<dyn ExecFileCalls>,
    upload_id: Option<String>,
    armed: bool,
}

impl UnpackAbort {
    fn disarm(&mut self) {
        self.armed = false;
    }

    async fn abort_now(&mut self) {
        self.armed = false;
        if let Some(upload_id) = self.upload_id.take() {
            let _ = self
                .calls
                .unpack_abort(UnpackAbortRequest { upload_id })
                .await;
        }
    }
}

impl Drop for UnpackAbort {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(upload_id) = self.upload_id.take() else {
            return;
        };
        let calls = Arc::clone(&self.calls);
        spawn_detached(async move {
            let _ = calls.unpack_abort(UnpackAbortRequest { upload_id }).await;
        });
    }
}

/// Applies one uncompressed tar stream to `dest` in the sandbox.
///
/// The stream is written in bounded chunks, so the control plane never holds
/// the archive. A limit failure of the slave becomes
/// [`SandboxError::ArtifactTooLarge`].
///
/// # Errors
/// The error of the begin, chunk or commit call, or the error of the byte
/// stream.
pub async fn unpack_stream(
    limits: SandboxLimits,
    calls: Arc<dyn ExecFileCalls>,
    dest: String,
    options: UnpackOptions,
    tar: BoxByteStream,
) -> SandboxResult<UnpackReport> {
    let begin = calls
        .unpack_begin(UnpackBeginRequest {
            dest,
            oci_whiteouts: options.oci_whiteouts,
            skip: options.skip,
        })
        .await?;
    let mut guard = UnpackAbort {
        calls: Arc::clone(&calls),
        upload_id: Some(begin.upload_id.clone()),
        armed: true,
    };
    let mut tar = tar;
    let mut offset = 0_u64;
    let chunk = limits.chunk_bytes.max(1);
    loop {
        match tar.next().await {
            None => break,
            Some(Err(error)) => {
                guard.abort_now().await;
                return Err(error);
            }
            Some(Ok(bytes)) => {
                for piece in bytes.chunks(chunk) {
                    let answer = calls
                        .unpack_chunk(UnpackChunkRequest {
                            upload_id: begin.upload_id.clone(),
                            offset,
                            data: piece.to_vec(),
                        })
                        .await;
                    match answer {
                        Ok(response) => offset = response.next_offset,
                        Err(error) => {
                            guard.abort_now().await;
                            return Err(error);
                        }
                    }
                }
            }
        }
    }
    let answer = calls
        .unpack_commit(UnpackCommitRequest {
            upload_id: begin.upload_id,
            size: offset,
        })
        .await;
    match answer {
        Ok(response) => {
            guard.disarm();
            Ok(UnpackReport {
                entries: response.entries,
                bytes: response.bytes,
            })
        }
        Err(error) => {
            guard.abort_now().await;
            Err(map_unpack_error(error, limits.max_unpack_bytes))
        }
    }
}

/// Maps the slave limit failure of an unpack to `ArtifactTooLarge`.
fn map_unpack_error(error: ErrorTrace<SandboxError>, max_bytes: u64) -> ErrorTrace<SandboxError> {
    if let SandboxError::SlaveError {
        code: crate::shared::error::SlaveErrorCode::Io,
        message,
        errno: _,
    } = error.current_context()
    {
        if message.contains(UNPACK_LIMIT_MESSAGE) {
            return SandboxError::ArtifactTooLarge {
                limit: max_bytes,
                message: message.clone(),
            }
            .into();
        }
    }
    error
}

/// Reads one file as a byte stream, with the offset check.
///
/// # Errors
/// The error of the `FileRead` call.
pub async fn read_file_stream(
    calls: Arc<dyn ExecFileCalls>,
    path: String,
) -> SandboxResult<FileStream> {
    let stream = calls.file_read(FileReadRequest { path }).await?;
    let state = ReadState {
        stream: Some(stream),
        sequence: Sequence::new(),
        done: false,
    };
    let inner: Pin<Box<dyn Stream<Item = SandboxResult<Vec<u8>>> + Send>> =
        Box::pin(futures::stream::unfold(state, |state| async move {
            let (item, state) = state.next_chunk().await;
            item.map(|item| (item, state))
        }));
    Ok(FileStream::new(inner))
}

struct ReadState {
    stream: Option<crate::shared::FileChunkStream>,
    sequence: Sequence,
    done: bool,
}

impl ReadState {
    async fn next_chunk(mut self) -> (Option<SandboxResult<Vec<u8>>>, Self) {
        if self.done {
            return (None, self);
        }
        loop {
            let Some(stream) = self.stream.as_mut() else {
                self.done = true;
                return (None, self);
            };
            match stream.next().await {
                None => {
                    self.done = true;
                    return (None, self);
                }
                Some(Err(error)) => {
                    self.done = true;
                    return (Some(Err(error)), self);
                }
                Some(Ok(chunk)) => {
                    let len = u64::try_from(chunk.data.len()).unwrap_or(u64::MAX);
                    match self.sequence.accept_span(chunk.offset, len) {
                        SequenceCheck::Duplicate => {}
                        SequenceCheck::Gap => {
                            self.done = true;
                            return (Some(Err(SandboxError::StreamLost.into())), self);
                        }
                        SequenceCheck::Apply => {
                            if chunk.eof {
                                self.done = true;
                            }
                            return (Some(Ok(chunk.data)), self);
                        }
                    }
                }
            }
        }
    }
}

/// The command with the image user when it names none (invariant I10).
fn with_image_user(
    mut command: Command,
    manifest: Option<crate::shared::images::ImageManifest>,
) -> Command {
    if command.user.is_none() {
        command.user = manifest.and_then(|manifest| manifest.user);
    }
    command
}

fn ensure_ready(record: &SandboxRecord) -> SandboxResult<()> {
    let state = record
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.state == SandboxState::Ready {
        Ok(())
    } else {
        Err(SandboxError::SandboxStopped.into())
    }
}

fn record_calls(record: &SandboxRecord) -> Arc<dyn ExecFileCalls> {
    let state = record
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // A nested child carries its calls already: they go to the parent's inner
    // control plane (feature 11).
    if let Some(calls) = &state.calls {
        return Arc::clone(calls);
    }
    match &state.client {
        Some(client) => Arc::clone(client) as Arc<dyn ExecFileCalls>,
        None => Arc::new(MissingCalls),
    }
}

/// The calls of a record that has no client. Every call is `SandboxStopped`.
struct MissingCalls;

impl ExecFileCalls for MissingCalls {
    fn exec(&self, _request: ExecRequest) -> BoxFuture<'_, SandboxResult<ExecEventStream>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn exec_input(
        &self,
        _request: ExecInputRequest,
    ) -> BoxFuture<'_, SandboxResult<ExecInputResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn exec_signal(&self, _request: ExecSignalRequest) -> BoxFuture<'_, SandboxResult<()>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn file_write_begin(
        &self,
        _request: FileWriteBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteBeginResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn file_write_chunk(
        &self,
        _request: FileWriteChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteChunkResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn file_write_commit(
        &self,
        _request: FileWriteCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteCommitResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn file_write_abort(
        &self,
        _request: FileWriteAbortRequest,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn file_read(
        &self,
        _request: FileReadRequest,
    ) -> BoxFuture<'_, SandboxResult<crate::shared::FileChunkStream>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn unpack_begin(
        &self,
        _request: UnpackBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackBeginResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn unpack_chunk(
        &self,
        _request: UnpackChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackChunkResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn unpack_commit(
        &self,
        _request: UnpackCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackCommitResponse>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }

    fn unpack_abort(&self, _request: UnpackAbortRequest) -> BoxFuture<'_, SandboxResult<()>> {
        send_box(async { Err(SandboxError::SandboxStopped.into()) })
    }
}

/// The operations of one local sandbox.
pub struct LocalSandboxOps {
    inner: Arc<Inner>,
    record: Arc<SandboxRecord>,
    ctx: Ctx,
    limits: SandboxLimits,
}

impl LocalSandboxOps {
    /// The operations of the record of one sandbox.
    #[must_use]
    pub fn new(
        inner: Arc<Inner>,
        record: Arc<SandboxRecord>,
        ctx: Ctx,
        limits: SandboxLimits,
    ) -> Self {
        Self {
            inner,
            record,
            ctx,
            limits,
        }
    }
}

impl SandboxOps for LocalSandboxOps {
    fn info(&self) -> SandboxInfo {
        self.record.info()
    }

    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let command = with_image_user(command, self.manifest());
        let result = ensure_ready(&self.record);
        let calls = record_calls(&self.record);
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        send_box(async move {
            result?;
            exec_session(ctx, limits, calls, command).await
        })
    }

    fn run(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecOutput>> {
        let command = with_image_user(command, self.manifest());
        let result = ensure_ready(&self.record);
        let calls = record_calls(&self.record);
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        send_box(async move {
            result?;
            let handle = exec_session(ctx, limits.clone(), calls, command).await?;
            run_capture(handle, limits.max_captured_output).await
        })
    }

    fn write_file(
        &self,
        path: String,
        mode: u32,
        data: BoxByteStream,
    ) -> BoxFuture<'_, SandboxResult<u64>> {
        let result = ensure_ready(&self.record);
        let calls = record_calls(&self.record);
        let limits = self.limits.clone();
        send_box(async move {
            result?;
            write_file_stream(limits, calls, path, mode, data).await
        })
    }

    fn read_file(&self, path: String) -> BoxFuture<'_, SandboxResult<FileStream>> {
        let result = ensure_ready(&self.record);
        let calls = record_calls(&self.record);
        send_box(async move {
            result?;
            read_file_stream(calls, path).await
        })
    }

    fn unpack(
        &self,
        dest: String,
        options: UnpackOptions,
        tar: BoxByteStream,
    ) -> BoxFuture<'_, SandboxResult<UnpackReport>> {
        let result = ensure_ready(&self.record);
        let calls = record_calls(&self.record);
        let limits = self.limits.clone();
        send_box(async move {
            result?;
            unpack_stream(limits, calls, dest, options, tar).await
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        send_box(stop_record(
            Arc::clone(&self.inner),
            Arc::clone(&self.record),
        ))
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        send_box(kill_record(
            Arc::clone(&self.inner),
            Arc::clone(&self.record),
        ))
    }

    fn container(&self) -> Option<crate::shared::container::ContainerHandle> {
        self.record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .substrate
            .as_ref()
            .and_then(|substrate| substrate.container())
    }

    fn calls(&self) -> Arc<dyn ExecFileCalls> {
        record_calls(&self.record)
    }

    fn manifest(&self) -> Option<crate::shared::images::ImageManifest> {
        self.record.manifest.clone()
    }
}

/// The scoped calls of one sandbox on a remote `SandboxService`.
pub struct ScopedCalls {
    ctx: Ctx,
    client: Arc<SandboxServiceClient>,
    sandbox_id: String,
    limits: SandboxLimits,
}

impl ScopedCalls {
    /// Calls on the named sandbox.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the generated client cannot be built.
    pub fn new(
        ctx: Ctx,
        transport: Arc<dyn Transport>,
        sandbox_id: String,
        limits: SandboxLimits,
    ) -> SandboxResult<Self> {
        let client = SandboxServiceClient::new(
            transport,
            CONTROL_AUTHORITY,
            ClientOptions::new().with_codec("json"),
        )
        .map_err(|error| SandboxError::Io {
            op: "control_client".to_string(),
            message: error.to_string(),
        })?;
        Ok(Self {
            ctx,
            client: Arc::new(client),
            sandbox_id,
            limits,
        })
    }

    /// The same calls with another client, which carries the token header.
    #[must_use]
    pub fn with_client(
        ctx: Ctx,
        client: Arc<SandboxServiceClient>,
        sandbox_id: String,
        limits: SandboxLimits,
    ) -> Self {
        Self {
            ctx,
            client,
            sandbox_id,
            limits,
        }
    }

    fn rpc_ctx(&self, timeout: Duration) -> RpcCtx {
        RpcCtx::client(self.ctx.clone()).with_deadline(timeout)
    }

    /// The context of a streaming call. A stream has no unary deadline.
    fn stream_ctx(&self) -> RpcCtx {
        RpcCtx::client(self.ctx.clone())
    }

    fn scoped<T>(&self, inner: T) -> Scoped<T> {
        Scoped {
            sandbox_id: self.sandbox_id.clone(),
            inner,
        }
    }

    /// Stops the remote sandbox.
    ///
    /// # Errors
    /// The mapped error of the call.
    pub async fn stop(&self) -> SandboxResult<ExitReport> {
        let timeout = self.limits.rpc_timeout;
        self.client
            .stop(
                self.rpc_ctx(timeout),
                Request::new(SandboxRef {
                    sandbox_id: self.sandbox_id.clone(),
                }),
            )
            .await
            .map(|response| response.msg)
            .map_err(|error| connect_to_sandbox(&error))
    }

    /// Kills the remote sandbox.
    ///
    /// # Errors
    /// The mapped error of the call.
    pub async fn kill(&self) -> SandboxResult<ExitReport> {
        let timeout = self.limits.rpc_timeout;
        self.client
            .kill(
                self.rpc_ctx(timeout),
                Request::new(SandboxRef {
                    sandbox_id: self.sandbox_id.clone(),
                }),
            )
            .await
            .map(|response| response.msg)
            .map_err(|error| connect_to_sandbox(&error))
    }
}

macro_rules! scoped_retry {
    ($self:ident, $call:ident, $request:expr, $timeout:expr) => {{
        let first = $self
            .client
            .$call(
                $self.rpc_ctx($timeout),
                Request::new($self.scoped($request.clone())),
            )
            .await
            .map_err(|error| connect_to_sandbox(&error));
        match first {
            Ok(response) => Ok(response.msg),
            Err(error) => {
                if is_transport_error(&error) {
                    $self
                        .client
                        .$call(
                            $self.rpc_ctx($timeout),
                            Request::new($self.scoped($request)),
                        )
                        .await
                        .map(|response| response.msg)
                        .map_err(|error| connect_to_sandbox(&error))
                } else {
                    Err(error)
                }
            }
        }
    }};
}

impl ExecFileCalls for ScopedCalls {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<ExecEventStream>> {
        send_box(async move {
            let stream = self
                .client
                .exec(self.stream_ctx(), Request::new(self.scoped(request)))
                .await
                .map_err(|error| connect_to_sandbox(&error))?;
            Ok(event_stream(stream))
        })
    }

    fn exec_input(
        &self,
        request: ExecInputRequest,
    ) -> BoxFuture<'_, SandboxResult<ExecInputResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { scoped_retry!(self, exec_input, request, timeout) })
    }

    fn exec_signal(&self, request: ExecSignalRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<crate::shared::rpc::messages::Empty> =
                scoped_retry!(self, exec_signal, request, timeout);
            answer.map(|_empty| ())
        })
    }

    fn file_write_begin(
        &self,
        request: FileWriteBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteBeginResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .file_write_begin(self.rpc_ctx(timeout), Request::new(self.scoped(request)))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn file_write_chunk(
        &self,
        request: FileWriteChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteChunkResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { scoped_retry!(self, file_write_chunk, request, timeout) })
    }

    fn file_write_commit(
        &self,
        request: FileWriteCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteCommitResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .file_write_commit(self.rpc_ctx(timeout), Request::new(self.scoped(request)))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn file_write_abort(&self, request: FileWriteAbortRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<crate::shared::rpc::messages::Empty> =
                scoped_retry!(self, file_write_abort, request, timeout);
            answer.map(|_empty| ())
        })
    }

    fn file_read(
        &self,
        request: FileReadRequest,
    ) -> BoxFuture<'_, SandboxResult<crate::shared::FileChunkStream>> {
        send_box(async move {
            let stream = self
                .client
                .file_read(self.stream_ctx(), Request::new(self.scoped(request)))
                .await
                .map_err(|error| connect_to_sandbox(&error))?;
            Ok(chunk_stream(stream))
        })
    }

    fn unpack_begin(
        &self,
        request: UnpackBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackBeginResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .unpack_begin(self.rpc_ctx(timeout), Request::new(self.scoped(request)))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn unpack_chunk(
        &self,
        request: UnpackChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackChunkResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { scoped_retry!(self, unpack_chunk, request, timeout) })
    }

    fn unpack_commit(
        &self,
        request: UnpackCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackCommitResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .unpack_commit(self.rpc_ctx(timeout), Request::new(self.scoped(request)))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn unpack_abort(&self, request: UnpackAbortRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<crate::shared::rpc::messages::Empty> =
                scoped_retry!(self, unpack_abort, request, timeout);
            answer.map(|_empty| ())
        })
    }
}

/// The operations of one sandbox on a remote `SandboxService`.
pub struct RemoteSandboxOps {
    info: SandboxInfo,
    calls: Arc<ScopedCalls>,
    ctx: Ctx,
    limits: SandboxLimits,
}

impl RemoteSandboxOps {
    /// The operations of the named sandbox.
    #[must_use]
    pub fn new(
        info: SandboxInfo,
        calls: Arc<ScopedCalls>,
        ctx: Ctx,
        limits: SandboxLimits,
    ) -> Self {
        Self {
            info,
            calls,
            ctx,
            limits,
        }
    }
}

impl SandboxOps for RemoteSandboxOps {
    fn info(&self) -> SandboxInfo {
        self.info.clone()
    }

    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let command = with_image_user(command, self.manifest());
        let ready = self.info.state == SandboxState::Ready;
        let calls = Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>;
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        send_box(async move {
            if !ready {
                return Err(SandboxError::SandboxStopped.into());
            }
            exec_session(ctx, limits, calls, command).await
        })
    }

    fn run(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecOutput>> {
        let command = with_image_user(command, self.manifest());
        let ready = self.info.state == SandboxState::Ready;
        let calls = Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>;
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        send_box(async move {
            if !ready {
                return Err(SandboxError::SandboxStopped.into());
            }
            let handle = exec_session(ctx, limits.clone(), calls, command).await?;
            run_capture(handle, limits.max_captured_output).await
        })
    }

    fn write_file(
        &self,
        path: String,
        mode: u32,
        data: BoxByteStream,
    ) -> BoxFuture<'_, SandboxResult<u64>> {
        let ready = self.info.state == SandboxState::Ready;
        let calls = Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>;
        let limits = self.limits.clone();
        send_box(async move {
            if !ready {
                return Err(SandboxError::SandboxStopped.into());
            }
            write_file_stream(limits, calls, path, mode, data).await
        })
    }

    fn read_file(&self, path: String) -> BoxFuture<'_, SandboxResult<FileStream>> {
        let ready = self.info.state == SandboxState::Ready;
        let calls = Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>;
        send_box(async move {
            if !ready {
                return Err(SandboxError::SandboxStopped.into());
            }
            read_file_stream(calls, path).await
        })
    }

    fn unpack(
        &self,
        dest: String,
        options: UnpackOptions,
        tar: BoxByteStream,
    ) -> BoxFuture<'_, SandboxResult<UnpackReport>> {
        let ready = self.info.state == SandboxState::Ready;
        let calls = Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>;
        let limits = self.limits.clone();
        send_box(async move {
            if !ready {
                return Err(SandboxError::SandboxStopped.into());
            }
            unpack_stream(limits, calls, dest, options, tar).await
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let calls = Arc::clone(&self.calls);
        send_box(async move { calls.stop().await })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let calls = Arc::clone(&self.calls);
        send_box(async move { calls.kill().await })
    }

    fn calls(&self) -> Arc<dyn ExecFileCalls> {
        Arc::clone(&self.calls) as Arc<dyn ExecFileCalls>
    }

    fn manifest(&self) -> Option<crate::shared::images::ImageManifest> {
        self.info.manifest.clone()
    }
}

fn is_transport_error(error: &ErrorTrace<SandboxError>) -> bool {
    matches!(
        error.current_context(),
        SandboxError::Rpc { code, .. } if code == "unavailable"
    )
}

fn connect_to_sandbox(error: &ErrorTrace<ConnectError>) -> ErrorTrace<SandboxError> {
    SandboxError::from_connect(error.current_context()).into()
}
