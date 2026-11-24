//! The sessions: start a program, move its bytes and cut it off.
//!
//! Each session owns a program, its process group, its pipes, its input queue
//! and its event sink. The pumps move bytes between the pipes and the sink. The
//! exit watch sends the one `Exit` event and removes the session.

use std::collections::{HashMap, VecDeque};
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use foundation_context::Ctx;
use foundation_core::valtron::timer::sleep_for;
use foundation_errstacks::ErrorTrace;
use foundation_reactor::fd_readiness::FdReadiness;
use futures::Stream;

use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::ids::ExecId;
use crate::shared::rpc::messages::{ExecEventKind, ExecEventMsg, ExecRequest, SlaveLimits};
use crate::shared::sequence::Sequence;

use super::reaper::ChildTable;
use super::slave_error;
use super::state::PrepareRecord;
use super::sys;

/// The number of bytes that one pipe read takes at most.
const EVENT_POLL_BYTES: usize = 64 * 1024;
/// The default `PATH` that the slave adds when the environment has none.
const DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

// ── A small wake channel ────────────────────────────────────────────────────

/// A set of parked tasks that one state change wakes.
#[derive(Debug, Default)]
pub(crate) struct Notify {
    wakers: Mutex<Vec<Waker>>,
}

impl Notify {
    /// A signal with no parked task.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Wakes every parked task.
    pub fn notify(&self) {
        let wakers = {
            let mut parked = self
                .wakers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *parked)
        };
        for waker in wakers {
            waker.wake();
        }
    }

    /// Parks `waker` until the next [`Self::notify`].
    pub(crate) fn register(&self, waker: &Waker) {
        let mut parked = self
            .wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if parked.iter().any(|known| known.will_wake(waker)) {
            return;
        }
        parked.push(waker.clone());
    }
}

// ── The event sink ──────────────────────────────────────────────────────────

/// What one look at the event sink found.
enum PopState {
    /// One event waits.
    Event(ExecEventMsg),
    /// The sink closed.
    Closed,
    /// No event waits and the sink is open.
    Empty,
}

/// A queue of at most `capacity` events, with the sequence number of each one.
///
/// The sequence number is assigned in the same lock that appends the event, so
/// the order of the queue is the order of the sequence numbers.
#[derive(Debug)]
pub(crate) struct EventSink {
    inner: Mutex<SinkInner>,
    changed: Arc<Notify>,
    /// Notified when a receiver takes an event, so a watcher that waits for
    /// room wakes. It is the session's change signal.
    room: Arc<Notify>,
    capacity: usize,
}

#[derive(Debug)]
struct SinkInner {
    queue: VecDeque<ExecEventMsg>,
    closed: bool,
    next_seq: u64,
}

impl EventSink {
    /// A sink of `capacity` events. `room` is notified when a receiver takes
    /// an event.
    fn new(capacity: usize, room: Arc<Notify>) -> Self {
        Self {
            inner: Mutex::new(SinkInner {
                queue: VecDeque::new(),
                closed: false,
                next_seq: 0,
            }),
            changed: Arc::new(Notify::new()),
            room,
            capacity: capacity.max(1),
        }
    }

    /// Adds an event. It waits for room and gives `false` when the sink closed.
    async fn send(&self, kind: ExecEventKind) -> bool {
        loop {
            {
                let mut inner = self.lock();
                if inner.closed {
                    return false;
                }
                if inner.queue.len() < self.capacity {
                    let seq = inner.next_seq;
                    inner.next_seq += 1;
                    inner.queue.push_back(ExecEventMsg { seq, event: kind });
                    drop(inner);
                    self.changed.notify();
                    return true;
                }
            }
            let changed = Arc::clone(&self.changed);
            super::wait::until(changed, || {
                let inner = self.lock();
                inner.closed || inner.queue.len() < self.capacity
            })
            .await;
        }
    }

    /// Whether a sender can add an event without a wait.
    fn has_room(&self) -> bool {
        let inner = self.lock();
        inner.closed || inner.queue.len() < self.capacity
    }

    /// Closes the sink. Every parked sender and receiver wakes.
    fn close(&self) {
        self.lock().closed = true;
        self.changed.notify();
    }

    /// Takes one event, or parks until one arrives or the sink closes.
    fn poll_recv(&self, cx: &mut Context<'_>) -> Poll<Option<ExecEventMsg>> {
        match self.poll_pop() {
            PopState::Event(event) => Poll::Ready(Some(event)),
            PopState::Closed => Poll::Ready(None),
            PopState::Empty => {
                self.changed.register(cx.waker());
                // A push may have arrived between the first look and the
                // registration.
                match self.poll_pop() {
                    PopState::Event(event) => Poll::Ready(Some(event)),
                    PopState::Closed => Poll::Ready(None),
                    PopState::Empty => Poll::Pending,
                }
            }
        }
    }

    /// Takes one event, or reports why none waits.
    fn poll_pop(&self) -> PopState {
        let mut inner = self.lock();
        if let Some(event) = inner.queue.pop_front() {
            drop(inner);
            self.changed.notify();
            self.room.notify();
            return PopState::Event(event);
        }
        if inner.closed {
            return PopState::Closed;
        }
        PopState::Empty
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SinkInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ── The input queue ─────────────────────────────────────────────────────────

/// What an `ExecInput` call did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputOutcome {
    /// The chunk was queued. The value is the next accepted sequence number.
    Applied(u64),
    /// The sequence number was applied before. The value is the next one.
    Duplicate(u64),
}

/// The bytes that wait for the standard input of one program.
#[derive(Debug)]
pub(crate) struct InputQueue {
    inner: Mutex<InputInner>,
    changed: Arc<Notify>,
    limit: usize,
}

#[derive(Debug)]
struct InputInner {
    items: VecDeque<InputItem>,
    bytes: usize,
    sequence: Sequence,
    eof_applied: bool,
    closed: bool,
}

/// One queued piece of input.
#[derive(Debug)]
pub(crate) struct InputItem {
    data: Vec<u8>,
    eof: bool,
}

impl InputQueue {
    /// A queue of at most `limit` bytes.
    fn new(limit: usize) -> Self {
        Self {
            inner: Mutex::new(InputInner {
                items: VecDeque::new(),
                bytes: 0,
                sequence: Sequence::new(),
                eof_applied: false,
                closed: false,
            }),
            changed: Arc::new(Notify::new()),
            limit,
        }
    }

    /// Applies one sequence number.
    ///
    /// # Errors
    /// [`SandboxError::SequenceGap`] for a number that is not the next one,
    /// [`SandboxError::InputQueueFull`] when the queue cannot hold the bytes,
    /// and a `BadRequest` slave error after `eof`.
    pub(crate) fn accept(&self, seq: u64, data: Vec<u8>, eof: bool) -> SandboxResult<InputOutcome> {
        let mut inner = self.lock();
        let next = inner.sequence.next();
        if seq < next {
            return Ok(InputOutcome::Duplicate(next));
        }
        if seq > next {
            return Err(SandboxError::SequenceGap.into());
        }
        if inner.eof_applied {
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                "the input of the program ended already",
            ));
        }
        if inner.bytes.saturating_add(data.len()) > self.limit {
            return Err(SandboxError::InputQueueFull.into());
        }
        inner.sequence.accept(seq);
        inner.bytes += data.len();
        inner.eof_applied = eof;
        inner.items.push_back(InputItem { data, eof });
        let next = inner.sequence.next();
        drop(inner);
        self.changed.notify();
        Ok(InputOutcome::Applied(next))
    }

    /// Takes the next piece, or parks until one arrives or the queue closes.
    pub(crate) async fn next_item(&self) -> Option<InputItem> {
        loop {
            {
                let mut inner = self.lock();
                if let Some(item) = inner.items.pop_front() {
                    inner.bytes = inner.bytes.saturating_sub(item.data.len());
                    drop(inner);
                    self.changed.notify();
                    return Some(item);
                }
                if inner.closed {
                    return None;
                }
            }
            let changed = Arc::clone(&self.changed);
            super::wait::until(changed, || {
                let inner = self.lock();
                inner.closed || !inner.items.is_empty()
            })
            .await;
        }
    }

    /// Ends the queue. Every parked task wakes.
    fn close(&self) {
        self.lock().closed = true;
        self.changed.notify();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, InputInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ── The session ─────────────────────────────────────────────────────────────

/// The changeable state of one session.
#[derive(Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is one independent fact of the session"
)]
struct SessionState {
    exit: Option<(Option<i32>, Option<i32>)>,
    stdout_done: bool,
    stderr_done: bool,
    finished: bool,
    caller_gone: bool,
    stopped: bool,
}

/// A part of the state that a watcher reads.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the snapshot copies the independent flags of the session"
)]
struct Snapshot {
    exit: Option<(Option<i32>, Option<i32>)>,
    stdout_done: bool,
    stderr_done: bool,
    caller_gone: bool,
    stopped: bool,
}

impl Snapshot {
    fn streams_done(&self) -> bool {
        self.stdout_done && self.stderr_done
    }
}

/// One running `Exec`: a program, its process group, its pipes, its input queue
/// and its event sink.
pub struct Session {
    /// The identifier that the caller chose.
    pub exec_id: ExecId,
    /// The process identifier, which is also the process group identifier.
    pub pid: i32,
    sink: EventSink,
    input: Option<Arc<InputQueue>>,
    state: Mutex<SessionState>,
    changed: Arc<Notify>,
    stop: Arc<Notify>,
    limits: SlaveLimits,
    ctx: Ctx,
}

impl Session {
    fn lock(&self) -> std::sync::MutexGuard<'_, SessionState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn snapshot(&self) -> Snapshot {
        let state = self.lock();
        Snapshot {
            exit: state.exit,
            stdout_done: state.stdout_done,
            stderr_done: state.stderr_done,
            caller_gone: state.caller_gone,
            stopped: state.stopped,
        }
    }

    /// Stores the status that the reaper collected.
    pub(crate) fn set_exit(&self, code: Option<i32>, signal: Option<i32>) {
        self.lock().exit = Some((code, signal));
        self.changed.notify();
    }

    /// Marks one output stream as ended.
    fn mark_stream_done(&self, stdout: bool) {
        {
            let mut state = self.lock();
            if stdout {
                state.stdout_done = true;
            } else {
                state.stderr_done = true;
            }
        }
        self.changed.notify();
    }

    /// Records that the caller let the response stream go.
    fn mark_caller_gone(&self) {
        self.lock().caller_gone = true;
        self.changed.notify();
        self.stop_pumps();
    }

    /// Whether the session completed.
    fn is_finished(&self) -> bool {
        self.lock().finished
    }

    /// Marks the session complete.
    fn mark_finished(&self) {
        self.lock().finished = true;
    }

    /// Sends `signal` to the process group of the program.
    ///
    /// # Errors
    /// The operating system error of `kill`.
    pub(crate) fn signal_group(&self, signal: i32) -> io::Result<()> {
        sys::kill_group(self.pid, signal)
    }

    /// Ends both pumps, which close their pipes.
    fn stop_pumps(&self) {
        self.lock().stopped = true;
        self.stop.notify();
    }

    /// Adds one input chunk.
    ///
    /// # Errors
    /// The errors of [`InputQueue::accept`] and a `BadRequest` when the program
    /// reads no input.
    pub(crate) fn feed(&self, seq: u64, data: Vec<u8>, eof: bool) -> SandboxResult<InputOutcome> {
        match &self.input {
            Some(queue) => queue.accept(seq, data, eof),
            None => Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                "the program reads no input",
            )),
        }
    }
}

// ── The session table ───────────────────────────────────────────────────────

/// Every live session, by identifier.
#[derive(Default)]
pub struct SessionTable {
    sessions: Mutex<HashMap<ExecId, Arc<Session>>>,
    /// The identifiers that ended most recently, bounded by the session limit.
    ///
    /// A signal can arrive just after the session left the table. The feature
    /// says that a session whose program already exited accepts the call and
    /// does nothing, so the slave remembers the last finished identifiers.
    finished: Mutex<VecDeque<(ExecId, usize)>>,
}

impl SessionTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ExecId, Arc<Session>>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Adds a session. It reports `false` when a running session holds the
    /// identifier. A session that ended does not hold it.
    fn insert(&self, session: Arc<Session>) -> bool {
        let mut sessions = self.lock();
        if let Some(existing) = sessions.get(&session.exec_id) {
            if !existing.is_finished() {
                return false;
            }
        }
        sessions.insert(session.exec_id.clone(), session);
        true
    }

    /// Remembers that `exec_id` ended, so a late signal is accepted.
    pub(crate) fn remember_finished(&self, exec_id: &ExecId, limit: usize) {
        let mut finished = self
            .finished
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        finished.push_back((exec_id.clone(), limit.max(1)));
        let total: usize = finished.iter().map(|(_, limit)| *limit).max().unwrap_or(1);
        while finished.len() > total {
            finished.pop_front();
        }
    }

    /// Whether `exec_id` ended recently.
    #[must_use]
    pub fn is_recently_finished(&self, exec_id: &ExecId) -> bool {
        self.finished
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|(known, _)| known == exec_id)
    }

    /// Finds a session.
    #[must_use]
    pub fn get(&self, exec_id: &ExecId) -> Option<Arc<Session>> {
        self.lock().get(exec_id).cloned()
    }

    /// Removes a session.
    pub(crate) fn remove(&self, exec_id: &ExecId) {
        self.lock().remove(exec_id);
    }

    /// The number of live sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether there is no live session.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every live session.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Arc<Session>> {
        self.lock().values().cloned().collect()
    }
}

// ── The response stream ─────────────────────────────────────────────────────

/// The events of one `Exec` call.
///
/// When the caller drops it before the program ended, the slave kills the
/// process group and removes the session.
pub struct SessionStream {
    session: Arc<Session>,
    table: Arc<SessionTable>,
}

impl Stream for SessionStream {
    type Item = ExecEventMsg;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.session.sink.poll_recv(cx)
    }
}

impl Drop for SessionStream {
    fn drop(&mut self) {
        if !self.session.is_finished() {
            self.session.mark_caller_gone();
            let _ = self.session.signal_group(libc::SIGKILL);
        }
        // The identifier is free at once, so a new `Exec` may use it. A late
        // signal is still accepted.
        self.table.remove(&self.session.exec_id);
        self.table.remember_finished(
            &self.session.exec_id,
            self.session.limits.max_exec_sessions as usize,
        );
        self.session.sink.close();
        self.session.stop_pumps();
    }
}

// ── Start of one session ────────────────────────────────────────────────────

/// The parts of one started program.
struct Spawned {
    session: Arc<Session>,
    stdout: Arc<OwnedFd>,
    stderr: Arc<OwnedFd>,
    stdin: Option<Arc<OwnedFd>>,
}

/// Starts one program and returns its session and its event stream.
///
/// The caller checks the state before this call.
///
/// # Errors
/// The checks of §6.6, the program lookup and the start of the program.
pub(crate) async fn start(
    ctx: &Ctx,
    prepare: &PrepareRecord,
    children: &Arc<ChildTable>,
    table: &Arc<SessionTable>,
    request: &ExecRequest,
) -> SandboxResult<(Arc<Session>, SessionStream)> {
    let exec_id = parse_exec_id(&request.exec_id)?;
    if request.argv.is_empty() {
        return Err(slave_error(
            SlaveErrorCode::BadRequest,
            None,
            "argv is empty",
        ));
    }
    let limits = prepare.limits();
    if table.len() >= limits.max_exec_sessions as usize {
        return Err(SandboxError::TooManySessions.into());
    }
    let cwd = match &request.cwd {
        Some(cwd) => {
            if !is_guest_absolute(cwd) {
                return Err(slave_error(
                    SlaveErrorCode::BadRequest,
                    None,
                    format!("cwd must be absolute: {cwd}"),
                ));
            }
            cwd.clone()
        }
        None => prepare.workdir().to_string(),
    };
    if !is_guest_absolute(&cwd) {
        return Err(slave_error(
            SlaveErrorCode::BadRequest,
            None,
            format!("the work directory must be absolute: {cwd}"),
        ));
    }
    if table
        .get(&exec_id)
        .is_some_and(|session| !session.is_finished())
    {
        return Err(SandboxError::SlaveError {
            code: SlaveErrorCode::AlreadyExists,
            errno: None,
            message: format!("exec_id {} is in use", request.exec_id),
        }
        .into());
    }
    let exec_root = prepare.exec_root.clone();
    let environment = merge_environment(prepare, request);
    let program = resolve_program(&exec_root, &request.argv[0], &environment)?;

    let spawned = spawn_process(
        ctx,
        prepare,
        children,
        exec_id.clone(),
        request,
        &cwd,
        &environment,
        &program,
    )
    .await?;

    if !table.insert(Arc::clone(&spawned.session)) {
        // Another call took the identifier between the checks and here.
        children.lock().remove(&spawned.session.pid);
        let _ = spawned.session.signal_group(libc::SIGKILL);
        return Err(SandboxError::SlaveError {
            code: SlaveErrorCode::AlreadyExists,
            errno: None,
            message: format!("exec_id {} is in use", request.exec_id),
        }
        .into());
    }

    // `Started` is sequence number zero, so it goes before any pump starts.
    let started = spawned
        .session
        .sink
        .send(ExecEventKind::Started {
            pid: u32::try_from(spawned.session.pid).expect("a process identifier is positive"),
        })
        .await;
    if !started {
        table.remove(&spawned.session.exec_id);
        return Err(SandboxError::StreamLost.into());
    }

    spawn_pumps(&spawned, table);
    let stream = SessionStream {
        session: Arc::clone(&spawned.session),
        table: Arc::clone(table),
    };
    Ok((spawned.session, stream))
}

/// Forks the program and waits until it started or failed.
#[allow(
    clippy::too_many_arguments,
    reason = "each argument is one prepared value of the child"
)]
async fn spawn_process(
    ctx: &Ctx,
    prepare: &PrepareRecord,
    children: &Arc<ChildTable>,
    exec_id: ExecId,
    request: &ExecRequest,
    cwd: &str,
    environment: &[(String, String)],
    program: &str,
) -> SandboxResult<Spawned> {
    let limits = prepare.limits();
    let (stdout_read, stdout_write) = sys::pipe().map_err(|error| spawn_io(&error))?;
    let (stderr_read, stderr_write) = sys::pipe().map_err(|error| spawn_io(&error))?;
    let (stdin_read, stdin_write) = if request.stdin {
        let (read, write) = sys::pipe().map_err(|error| spawn_io(&error))?;
        (Some(read), Some(write))
    } else {
        let null = sys::open_null().map_err(|error| spawn_io(&error))?;
        (Some(null), None)
    };
    let (status_read, status_write) = sys::pipe().map_err(|error| spawn_io(&error))?;

    let argv = strings_to_c(request.argv.iter().map(String::as_str))?;
    let environment_lines: Vec<String> = environment
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    let envp = strings_to_c(environment_lines.iter().map(String::as_str))?;
    let program_c = CString::new(program).map_err(|_| {
        slave_error(
            SlaveErrorCode::BadRequest,
            None,
            "the program path has a zero byte",
        )
    })?;
    let cwd_c = CString::new(cwd)
        .map_err(|_| slave_error(SlaveErrorCode::BadRequest, None, "cwd has a zero byte"))?;
    let chroot_c = if prepare.exec_root == Path::new("/") {
        None
    } else {
        Some(
            CString::new(prepare.exec_root.as_os_str().as_encoded_bytes()).map_err(|_| {
                slave_error(
                    SlaveErrorCode::Internal,
                    None,
                    "the root path has a zero byte",
                )
            })?,
        )
    };
    // The lock covers the fork and the insert, so a child that exits at once is
    // in the table before the reaper looks.
    let (pid, session) = {
        let mut child_guard = children.lock();
        let pid = fork_process(&ForkInputs {
            stdin_fd: stdin_read.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            stdout_fd: stdout_write.as_raw_fd(),
            stderr_fd: stderr_write.as_raw_fd(),
            chroot: chroot_c.as_ref(),
            cwd: &cwd_c,
            program: &program_c,
            argv: &argv,
            envp: &envp,
            status_fd: status_write.as_raw_fd(),
        })
        .map_err(|error| spawn_io(&error))?;
        let changed = Arc::new(Notify::new());
        let session = Arc::new(Session {
            exec_id,
            pid,
            sink: EventSink::new(limits.event_queue_len as usize, Arc::clone(&changed)),
            input: if request.stdin {
                Some(Arc::new(InputQueue::new(limits.input_queue_bytes as usize)))
            } else {
                None
            },
            state: Mutex::new(SessionState::default()),
            changed,
            stop: Arc::new(Notify::new()),
            limits,
            ctx: ctx.clone(),
        });
        child_guard.insert(pid, Arc::clone(&session));
        (pid, session)
    };

    // The parent ends.
    drop(stdin_read);
    drop(stdout_write);
    drop(stderr_write);
    drop(status_write);

    for fd in [
        stdout_read.as_raw_fd(),
        stderr_read.as_raw_fd(),
        status_read.as_raw_fd(),
    ] {
        sys::set_nonblocking(fd).map_err(|error| spawn_io(&error))?;
    }
    if let Some(write) = &stdin_write {
        sys::set_nonblocking(write.as_raw_fd()).map_err(|error| spawn_io(&error))?;
    }

    if let Err(error) = wait_for_start(&status_read).await {
        children.lock().remove(&pid);
        return Err(error);
    }

    Ok(Spawned {
        session,
        stdout: Arc::new(stdout_read),
        stderr: Arc::new(stderr_read),
        stdin: stdin_write.map(Arc::new),
    })
}

/// The values of one `fork`.
struct ForkInputs<'a> {
    stdin_fd: RawFd,
    stdout_fd: RawFd,
    stderr_fd: RawFd,
    chroot: Option<&'a CString>,
    cwd: &'a CString,
    program: &'a CString,
    argv: &'a [CString],
    envp: &'a [CString],
    status_fd: RawFd,
}

/// Forks the program. The child never returns; the parent gets its identifier.
///
/// This function is synchronous on purpose: the pointer lists that `execve`
/// takes live across the call and nowhere else, so no raw pointer is held
/// across an `await`.
///
/// # Errors
/// The operating system error of `fork`.
fn fork_process(inputs: &ForkInputs<'_>) -> io::Result<i32> {
    let argv = pointer_list(inputs.argv);
    let envp = pointer_list(inputs.envp);
    let pid = sys::fork()?;
    if pid == 0 {
        // SAFETY: this is the child of `fork`. Every call below and inside
        // `child_start` is async-signal-safe, and every pointer refers to
        // memory that this process owns and does not free.
        unsafe {
            child_start(
                inputs.stdin_fd,
                inputs.stdout_fd,
                inputs.stderr_fd,
                inputs.chroot,
                inputs.cwd,
                inputs.program,
                &argv,
                &envp,
                inputs.status_fd,
            );
        }
    }
    Ok(pid)
}

/// The child side of `fork`.
///
/// # Safety
/// Runs in the child of `fork`. Every call is async-signal-safe.
#[allow(
    clippy::too_many_arguments,
    reason = "the child side takes every prepared value"
)]
unsafe fn child_start(
    stdin_fd: RawFd,
    stdout_fd: RawFd,
    stderr_fd: RawFd,
    chroot_path: Option<&CString>,
    cwd: &CString,
    program: &CString,
    argv: &[*const libc::c_char],
    envp: &[*const libc::c_char],
    status_fd: RawFd,
) -> ! {
    let mut step = 1_u8;
    let mut errno = sys::child_setsid();
    if errno == 0 {
        step = 2;
        errno = sys::child_dup2(stdin_fd, 0);
        if errno == 0 {
            errno = sys::child_dup2(stdout_fd, 1);
        }
        if errno == 0 {
            errno = sys::child_dup2(stderr_fd, 2);
        }
    }
    if errno == 0 {
        step = 3;
        if let Some(path) = chroot_path {
            errno = sys::child_chroot(path);
        }
    }
    if errno == 0 {
        step = 4;
        errno = sys::child_chdir(cwd);
    }
    if errno == 0 {
        step = 5;
        // The child unblocks every signal and returns each common disposition
        // to the default before the new image starts.
        sys::child_reset_signals();
        errno = sys::child_execve(program, argv, envp);
    }
    // The start failed: report the step and the error, then end.
    let mut report = [0_u8; 8];
    report[0] = step;
    report[4..].copy_from_slice(&errno.to_ne_bytes());
    let _ = sys::child_write(status_fd, &report);
    sys::child_exit(127)
}

/// Waits until the program started, or returns the typed start failure.
async fn wait_for_start(status: &OwnedFd) -> SandboxResult<()> {
    let readiness =
        Arc::new(FdReadiness::register(status.as_raw_fd()).map_err(|error| spawn_io(&error))?);
    let mut report = [0_u8; 8];
    let bytes = loop {
        super::wait::ready(Arc::clone(&readiness)).await;
        match sys::read(status.as_raw_fd(), &mut report) {
            Ok(bytes) => break bytes,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(spawn_io(&error)),
        }
    };
    if bytes == 0 {
        // The write end closed on `execve`: the program started.
        return Ok(());
    }
    let mut errno_bytes = [0_u8; 4];
    errno_bytes.copy_from_slice(&report[4..8]);
    let errno = i32::from_ne_bytes(errno_bytes);
    Err(start_failure(report[0], errno))
}

/// The typed error of one failed start step.
fn start_failure(step: u8, errno: i32) -> ErrorTrace<SandboxError> {
    let code = match errno {
        libc::ENOENT => SlaveErrorCode::NotFound,
        libc::EACCES => SlaveErrorCode::PermissionDenied,
        _ => SlaveErrorCode::SpawnFailed,
    };
    slave_error(
        code,
        Some(errno),
        format!("start step {step} failed: {}", sys::error_of(errno)),
    )
}

/// The start error of one operating system error.
fn spawn_io(error: &io::Error) -> ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::SpawnFailed,
        error.raw_os_error(),
        format!("start of the program failed: {error}"),
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

/// Whether a guest path is absolute.
fn is_guest_absolute(path: &str) -> bool {
    path.starts_with('/')
}

/// The environment of the program: `Prepare.env`, then `ExecRequest.env`, with
/// a later key winning. Nothing comes from the slave's own environment.
fn merge_environment(prepare: &PrepareRecord, request: &ExecRequest) -> Vec<(String, String)> {
    let mut environment: Vec<(String, String)> = prepare.env().to_vec();
    for (key, value) in &request.env {
        if let Some(entry) = environment.iter_mut().find(|(known, _)| known == key) {
            entry.1.clone_from(value);
        } else {
            environment.push((key.clone(), value.clone()));
        }
    }
    if !environment.iter().any(|(key, _)| key == "PATH") {
        environment.push(("PATH".to_string(), DEFAULT_PATH.to_string()));
    }
    environment
}

/// Finds the program path in the guest. The result is the guest path that
/// `execve` receives after the `chroot`.
fn resolve_program(
    exec_root: &Path,
    argv0: &str,
    environment: &[(String, String)],
) -> SandboxResult<String> {
    if argv0.contains('/') {
        let guest = if argv0.starts_with('/') {
            argv0.to_string()
        } else {
            format!("/{argv0}")
        };
        check_executable(exec_root, &guest)?;
        return Ok(guest);
    }
    let path = environment
        .iter()
        .find(|(key, _)| key == "PATH")
        .map_or("", |(_, value)| value.as_str());
    for directory in path.split(':').filter(|entry| !entry.is_empty()) {
        let guest = format!("{}/{}", directory.trim_end_matches('/'), argv0);
        if check_executable(exec_root, &guest).is_ok() {
            return Ok(guest);
        }
    }
    Err(slave_error(
        SlaveErrorCode::NotFound,
        Some(libc::ENOENT),
        format!("program not found: {argv0}"),
    ))
}

/// Checks that `guest` is a regular file with an execute bit below `exec_root`.
fn check_executable(exec_root: &Path, guest: &str) -> SandboxResult<()> {
    let host = exec_root.join(guest.trim_start_matches('/'));
    let c_path = CString::new(host.as_os_str().as_encoded_bytes()).map_err(|_| {
        slave_error(
            SlaveErrorCode::BadRequest,
            None,
            format!("the program path has a zero byte: {guest}"),
        )
    })?;
    let fd = sys::open_path(&c_path, libc::O_PATH, 0).map_err(|error| {
        let code = match error.raw_os_error() {
            Some(libc::EACCES) => SlaveErrorCode::PermissionDenied,
            _ => SlaveErrorCode::NotFound,
        };
        slave_error(
            code,
            error.raw_os_error(),
            format!("program not found: {guest}"),
        )
    })?;
    let stat = sys::fstat(fd.as_raw_fd()).map_err(|error| spawn_io(&error))?;
    let regular = stat.st_mode & libc::S_IFMT == libc::S_IFREG;
    if !regular {
        return Err(slave_error(
            SlaveErrorCode::PermissionDenied,
            Some(libc::EACCES),
            format!("not a regular file: {guest}"),
        ));
    }
    if stat.st_mode & 0o111 == 0 {
        return Err(slave_error(
            SlaveErrorCode::PermissionDenied,
            Some(libc::EACCES),
            format!("the file has no execute bit: {guest}"),
        ));
    }
    Ok(())
}

/// The NUL-terminated strings of one list.
fn strings_to_c<'a>(values: impl Iterator<Item = &'a str>) -> SandboxResult<Vec<CString>> {
    values
        .map(|value| {
            CString::new(value).map_err(|_| {
                slave_error(SlaveErrorCode::BadRequest, None, "a string has a zero byte")
            })
        })
        .collect()
}

/// The `argv`-style pointer list, terminated by a null pointer.
fn pointer_list(values: &[CString]) -> Vec<*const libc::c_char> {
    let mut list: Vec<*const libc::c_char> = values.iter().map(|value| value.as_ptr()).collect();
    list.push(std::ptr::null());
    list
}

// ── Watches ─────────────────────────────────────────────────────────────────

/// Starts the pumps and the exit watch of one session.
fn spawn_pumps(spawned: &Spawned, table: &Arc<SessionTable>) {
    let stdout = Arc::clone(&spawned.stdout);
    let session = Arc::clone(&spawned.session);
    send_task(async move { output_pump(stdout, true, session).await });
    let stderr = Arc::clone(&spawned.stderr);
    let session = Arc::clone(&spawned.session);
    send_task(async move { output_pump(stderr, false, session).await });
    if let Some(stdin) = &spawned.stdin {
        let stdin = Arc::clone(stdin);
        let session = Arc::clone(&spawned.session);
        send_task(async move { input_pump(stdin, session).await });
    }
    let session = Arc::clone(&spawned.session);
    let table = Arc::clone(table);
    send_task(async move { exit_watch(session, table).await });
}

fn send_task<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Err(error) =
        foundation_core::valtron::send(foundation_core::valtron::from_future(future))
    {
        tracing::error!(error = %error, "the slave could not start a session task");
    }
}

/// Moves bytes from one pipe to the event sink.
async fn output_pump(pipe: Arc<OwnedFd>, stdout: bool, session: Arc<Session>) {
    let readiness = match FdReadiness::register(pipe.as_raw_fd()) {
        Ok(readiness) => Arc::new(readiness),
        Err(error) => {
            tracing::warn!(error = %error, "the slave could not watch an output pipe");
            session.mark_stream_done(stdout);
            return;
        }
    };
    let chunk = (session.limits.chunk_bytes as usize).clamp(1, EVENT_POLL_BYTES);
    let mut buffer = vec![0_u8; chunk];
    loop {
        let stop = Arc::clone(&session.stop);
        let state = Arc::clone(&session);
        if super::wait::ready_or_cancelled(Arc::clone(&readiness), stop, move || {
            state.snapshot().stopped
        })
        .await
        {
            break;
        }
        match sys::read(pipe.as_raw_fd(), &mut buffer) {
            Ok(0) => break,
            Ok(bytes) => {
                let data = buffer[..bytes].to_vec();
                let kind = if stdout {
                    ExecEventKind::Stdout { data }
                } else {
                    ExecEventKind::Stderr { data }
                };
                if !session.sink.send(kind).await {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::debug!(error = %error, "an output pipe ended with an error");
                break;
            }
        }
    }
    // The pump owns the read end: when it ends, the pipe closes.
    drop(readiness);
    drop(pipe);
    session.mark_stream_done(stdout);
}

/// Moves queued bytes to the standard input of the program.
async fn input_pump(pipe: Arc<OwnedFd>, session: Arc<Session>) {
    let Some(queue) = session.input.clone() else {
        return;
    };
    let readiness = match FdReadiness::register(pipe.as_raw_fd()) {
        Ok(readiness) => {
            if let Err(error) = readiness.set_writable(true) {
                tracing::warn!(error = %error, "the slave could not watch the input pipe");
            }
            Arc::new(readiness)
        }
        Err(error) => {
            tracing::warn!(error = %error, "the slave could not watch the input pipe");
            return;
        }
    };
    loop {
        let stop = Arc::clone(&session.stop);
        let state = Arc::clone(&session);
        let Some(item) =
            super::wait::item_or_stop(Arc::clone(&queue), stop, move || state.snapshot().stopped)
                .await
        else {
            break;
        };
        let mut written = 0;
        let mut broken = false;
        while written < item.data.len() {
            match sys::write(pipe.as_raw_fd(), &item.data[written..]) {
                Ok(0) => {
                    broken = true;
                    break;
                }
                Ok(bytes) => written += bytes,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let stop = Arc::clone(&session.stop);
                    let state = Arc::clone(&session);
                    if super::wait::ready_or_cancelled(Arc::clone(&readiness), stop, move || {
                        state.snapshot().stopped
                    })
                    .await
                    {
                        broken = true;
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.raw_os_error() == Some(libc::EPIPE) => {
                    // The program closed its input: drop the rest of the queue.
                    queue.close();
                    broken = true;
                    break;
                }
                Err(error) => {
                    tracing::debug!(error = %error, "the input pipe ended with an error");
                    queue.close();
                    broken = true;
                    break;
                }
            }
        }
        if broken || item.eof {
            break;
        }
    }
    // The write end closes here, so the program sees the end of its input.
    drop(readiness);
    drop(pipe);
}

/// Why the exit watch stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitWait {
    /// The caller let the stream go.
    CallerGone,
    /// The program ended and both output streams reached their end.
    Drained,
    /// The program ended and the drain time passed.
    TimedOut,
}

/// Waits for the exit status, then sends the one `Exit` event.
async fn exit_watch(session: Arc<Session>, table: Arc<SessionTable>) {
    let waited = wait_for_exit(&session).await;
    if waited == ExitWait::CallerGone {
        session.mark_finished();
        session.sink.close();
        session.stop_pumps();
        table.remove(&session.exec_id);
        table.remember_finished(&session.exec_id, session.limits.max_exec_sessions as usize);
        return;
    }
    if waited == ExitWait::TimedOut {
        // The time-out closes the pipes; later output is lost.
        session.stop_pumps();
    }
    let (code, signal) = session.snapshot().exit.unwrap_or((None, None));
    let _ = session
        .sink
        .send(ExecEventKind::Exit { code, signal })
        .await;
    session.sink.close();
    session.mark_finished();
    table.remove(&session.exec_id);
    table.remember_finished(&session.exec_id, session.limits.max_exec_sessions as usize);
    session.stop_pumps();
}

/// Waits until the program ended and its output is drained or the drain time
/// passed.
async fn wait_for_exit(session: &Arc<Session>) -> ExitWait {
    let drain_timeout =
        std::time::Duration::from_millis(u64::from(session.limits.output_drain_timeout_ms));
    let mut deadline: Option<foundation_compact::Instant> = None;
    loop {
        let now = session.ctx.clock().now();
        let snapshot = session.snapshot();
        if snapshot.caller_gone {
            return ExitWait::CallerGone;
        }
        if snapshot.exit.is_some() {
            if snapshot.streams_done() {
                return ExitWait::Drained;
            }
            let deadline = *deadline.get_or_insert(now + drain_timeout);
            if now >= deadline && session.sink.has_room() {
                return ExitWait::TimedOut;
            }
        }
        let changed = Arc::clone(&session.changed);
        let state = Arc::clone(session);
        // The wake of this wait:
        // - the caller went, or a stream ended;
        // - before the deadline is set, the exit arrives (this call sets it);
        // - with a deadline, the deadline passed and the sink has room. A
        //   receiver that takes an event notifies the session through the
        //   sink's `room` signal, so this branch does not poll.
        let wait = super::wait::until(changed, move || {
            let snapshot = state.snapshot();
            if snapshot.caller_gone || snapshot.streams_done() {
                return true;
            }
            match deadline {
                None => snapshot.exit.is_some(),
                Some(deadline) => state.ctx.clock().now() >= deadline && state.sink.has_room(),
            }
        });
        match deadline {
            Some(deadline) if now < deadline => {
                use futures::future::FutureExt;
                let sleep = sleep_for(deadline.saturating_duration_since(now), &session.ctx);
                let wait = Box::pin(wait).fuse();
                let sleep = Box::pin(sleep).fuse();
                futures::pin_mut!(wait, sleep);
                let _ = futures::future::select(wait, sleep).await;
            }
            _ => wait.await,
        }
    }
}
