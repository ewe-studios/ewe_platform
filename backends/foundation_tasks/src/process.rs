//! Process and output interfaces supplied by the executable host.
use crate::{TaskError, TaskResult, TaskRunner};
use foundation_context::Ctx;
use foundation_core::valtron::{self, TaskIterator, TaskStatus};
use foundation_nativeapis::signal::{signal_task, SignalEvent, SignalKind, SignalTask};
use futures_util::{
    future::{try_join3, BoxFuture, FutureExt},
    select_biased,
};
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Command, ExitCode, ExitStatus, Stdio},
    sync::{Arc, OnceLock},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

pub trait TaskOutput: Send + Sync {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>>;
    fn flush(&self) -> BoxFuture<'_, io::Result<()>>;
}

pub trait TaskProcess: Send + Sync {
    fn run(
        &self,
        command: ProcessCommand,
        output: Arc<dyn TaskOutput>,
        cancellation: Cancellation,
    ) -> BoxFuture<'_, TaskResult>;
}

pub struct TaskEnv {
    pub process: Arc<dyn TaskProcess>,
    pub output: Arc<dyn TaskOutput>,
    pub cancellation: Cancellation,
}

#[derive(Clone)]
pub struct Cancellation(async_channel::Sender<()>, async_channel::Receiver<()>);

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        Self(sender, receiver)
    }

    pub fn cancel(&self) {
        self.0.close();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        // The signal has no data. The channel closes when cancellation occurs.
        self.0.is_closed()
    }

    pub async fn cancelled(&self) {
        let _ = self.1.recv().await;
    }
}

#[derive(Clone)]
enum ProcessInput {
    Once(Vec<u8>),
    Repeat(Vec<u8>),
}

impl ProcessInput {
    fn write(self, mut pipe: impl Write) -> io::Result<()> {
        let pattern = match self {
            Self::Once(bytes) => return pipe.write_all(&bytes),
            Self::Repeat(pattern) => pattern,
        };
        if pattern.is_empty() {
            return Ok(());
        }
        let mut buffered = io::BufWriter::new(pipe);
        loop {
            match buffered.write_all(&pattern) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }
}

#[derive(Clone)]
pub struct ProcessCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, Option<OsString>)>,
    clear_env: bool,
    input: Option<ProcessInput>,
}

impl ProcessCommand {
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            clear_env: false,
            input: None,
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args<I, T>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), Some(value.into())));
        self
    }

    #[must_use]
    pub fn env_remove(mut self, key: impl Into<OsString>) -> Self {
        self.env.push((key.into(), None));
        self
    }

    #[must_use]
    pub fn env_clear(mut self) -> Self {
        self.clear_env = true;
        self
    }

    /// Send a small private input without adding it to the argument list.
    #[must_use]
    pub fn private_input(mut self, bytes: Vec<u8>) -> Self {
        self.input = Some(ProcessInput::Once(bytes));
        self
    }

    /// Repeat the input until the child closes its pipe. An empty pattern closes the pipe immediately.
    #[must_use]
    pub fn repeated_input(mut self, pattern: Vec<u8>) -> Self {
        self.input = Some(ProcessInput::Repeat(pattern));
        self
    }

    fn into_native(self) -> (Command, Option<ProcessInput>) {
        let ProcessCommand {
            program,
            args,
            cwd,
            env,
            clear_env,
            input,
        } = self;
        let mut process = Command::new(program);
        process
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }
        if clear_env {
            process.env_clear();
        }
        for (key, value) in env {
            if let Some(value) = value {
                process.env(key, value);
            } else {
                process.env_remove(key);
            }
        }
        (process, input)
    }
}

#[derive(Clone, Copy)]
pub struct ProcessConfig {
    chunk_bytes: std::num::NonZeroUsize,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        // Rust's standard buffered reader uses an 8 KiB buffer.
        Self {
            chunk_bytes: std::num::NonZeroUsize::new(8192).expect("positive buffer size"),
        }
    }
}

impl ProcessConfig {
    #[must_use]
    pub fn with_chunk_bytes(mut self, capacity: std::num::NonZeroUsize) -> Self {
        self.chunk_bytes = capacity;
        self
    }
}

pub struct NativeProcess {
    config: ProcessConfig,
    signal: Option<HostSignal>,
}

struct HostSignal {
    ctx: Ctx,
    registered: OnceLock<Result<(), String>>,
}

struct CancelOnSignal {
    task: SignalTask,
    cancellation: Cancellation,
}

impl TaskIterator for CancelOnSignal {
    type Ready = SignalEvent;
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<SignalEvent, ()>> {
        match self.task.next_status(ctx) {
            Some(TaskStatus::Ready(event))
                if matches!(
                    event.kind,
                    SignalKind::Interrupt | SignalKind::Terminate | SignalKind::Quit
                ) =>
            {
                self.cancellation.cancel();
                None
            }
            step => step,
        }
    }
}

impl NativeProcess {
    #[must_use]
    pub fn new(config: ProcessConfig) -> Self {
        Self {
            config,
            signal: None,
        }
    }

    fn for_host(ctx: &Ctx, config: ProcessConfig) -> Self {
        Self {
            config,
            signal: Some(HostSignal {
                ctx: ctx.with_empty_map(),
                registered: OnceLock::new(),
            }),
        }
    }

    fn register_signal(&self, cancellation: &Cancellation) -> TaskResult {
        if let Some(host) = &self.signal {
            host.registered
                .get_or_init(|| {
                    let (task, _) =
                        signal_task(host.ctx.clone()).map_err(|error| error.to_string())?;
                    valtron::send(CancelOnSignal {
                        task,
                        cancellation: cancellation.clone(),
                    })
                    .map_err(|error| error.to_string())
                })
                .as_ref()
                .map_err(|error| TaskError::operation(error.clone()))?;
        }
        Ok(())
    }
}

impl TaskProcess for NativeProcess {
    fn run(
        &self,
        command: ProcessCommand,
        output: Arc<dyn TaskOutput>,
        cancellation: Cancellation,
    ) -> BoxFuture<'_, TaskResult> {
        Box::pin(async move {
            self.register_signal(&cancellation)?;
            run_child(command, output, cancellation, self.config).await
        })
    }
}

/// Use the Valtron background pool for a short blocking operation.
async fn background<T: Send + 'static>(
    operation: impl FnOnce() -> T + Send + 'static,
) -> io::Result<T> {
    let (result, receiver) = async_channel::bounded(1);
    valtron::run_background_job(move || {
        if result.is_closed() {
            return;
        }
        let value = operation();
        // The caller can stop waiting before the background operation ends.
        let _ = result.try_send(value);
    })
    .map_err(|error| io::Error::other(error.to_string()))?;
    receiver.recv().await.map_err(io::Error::other)
}

type Pipe = async_channel::Receiver<io::Result<Vec<u8>>>;

struct ChildStreams {
    stdout: Pipe,
    stderr: Pipe,
    input: async_channel::Receiver<io::Result<()>>,
    observed: async_channel::Receiver<()>,
    exit: async_channel::Receiver<io::Result<ExitStatus>>,
    owner: ChildOwner,
}

struct ChildOwner {
    tree: Arc<tree::Tree>,
    release: Option<std::sync::mpsc::Sender<()>>,
}

impl ChildOwner {
    fn finish(&mut self) {
        self.release.take();
    }

    fn terminate(&mut self) -> io::Result<()> {
        let result = self.tree.terminate();
        self.finish();
        result
    }
}

impl Drop for ChildOwner {
    fn drop(&mut self) {
        if self.release.is_some() {
            if let Err(error) = self.terminate() {
                tracing::error!(%error, "Cannot terminate an abandoned child tree");
            }
        }
    }
}

async fn run_child(
    command: ProcessCommand,
    output: Arc<dyn TaskOutput>,
    cancellation: Cancellation,
    config: ProcessConfig,
) -> TaskResult {
    let program = command.program.clone();
    let fail = |source| TaskError::Process {
        program: program.clone(),
        source,
    };
    let before_spawn = cancellation.clone();
    let child = background(move || start_child(command, &before_spawn, config))
        .await
        .map_err(fail)??;
    let ChildStreams {
        stdout,
        stderr,
        input,
        observed,
        exit,
        mut owner,
    } = child;
    let result = {
        let joined = async {
            try_join3(
                drain(stdout, OutputStream::Stdout, output.as_ref()),
                drain(stderr, OutputStream::Stderr, output.as_ref()),
                async { input.recv().await.map_err(io::Error::other)? },
            )
            .await?;
            observed.recv().await.map_err(io::Error::other)
        };
        let cancellation = cancellation.cancelled();
        futures_util::pin_mut!(joined, cancellation);
        select_biased! {
            () = cancellation.fuse() => Err(TaskError::Cancelled),
            result = joined.fuse() => result.map_err(fail),
        }
    };
    let outcome = match result {
        Ok(()) => {
            owner.finish();
            Ok(())
        }
        Err(error) => with_cleanup(error, owner.terminate().map_err(fail)),
    };
    let reaped = exit
        .recv()
        .await
        .map_err(io::Error::other)
        .and_then(|status| status)
        .map_err(fail);
    let status = match outcome {
        Ok(()) => reaped?,
        Err(error) => return with_cleanup(error, reaped.map(drop)),
    };
    if status.success() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Err(TaskError::Signal { program, signal });
        }
    }
    Err(TaskError::Exit {
        program,
        code: status.code().unwrap_or(1),
    })
}

async fn drain(pipe: Pipe, stream: OutputStream, output: &dyn TaskOutput) -> io::Result<()> {
    while let Ok(chunk) = pipe.recv().await {
        output.write(stream, &chunk?).await?;
    }
    Ok(())
}

fn pump(
    mut reader: impl Read,
    chunks: &async_channel::Sender<io::Result<Vec<u8>>>,
    config: ProcessConfig,
) {
    loop {
        let mut bytes = vec![0; config.chunk_bytes.get()];
        match reader.read(&mut bytes) {
            Ok(0) => return,
            Ok(size) => {
                bytes.truncate(size);
                if chunks.send_blocking(Ok(bytes)).is_err() {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                let _ = chunks.send_blocking(Err(error));
                return;
            }
        }
    }
}

fn start_child(
    command: ProcessCommand,
    cancellation: &Cancellation,
    config: ProcessConfig,
) -> Result<ChildStreams, TaskError> {
    if cancellation.is_cancelled() {
        return Err(TaskError::Cancelled);
    }
    let (mut process, input) = command.into_native();
    let program = process.get_program().to_owned();
    let fail = |source| TaskError::Process {
        program: program.clone(),
        source,
    };
    tree::configure(&mut process);
    let mut child = process.spawn().map_err(fail)?;
    let tree = match tree::Tree::attach(&child) {
        Ok(tree) => Arc::new(tree),
        Err(error) => {
            let stopped = child.kill();
            return Err(cleanup_error(
                fail(error),
                after_cleanup(stopped, child.wait().map(drop)).map_err(fail),
            ));
        }
    };
    let (stdout, out) = async_channel::bounded(1);
    let (stderr, err) = async_channel::bounded(1);
    let (sent, input_status) = async_channel::bounded(1);
    let (ready, observed) = async_channel::bounded(1);
    let (finished, exit) = async_channel::bounded(1);
    let (release, permission_to_reap) = std::sync::mpsc::channel();
    let (handoff, receive) = std::sync::mpsc::sync_channel(1);
    let owned = tree.clone();
    // A Valtron pool can have one background worker. The child wait and each
    // pipe must block independently, so the supervisor owns their OS threads.
    let supervisor = std::thread::Builder::new()
        .name("task-child".into())
        .spawn(move || {
            let Ok(mut child) = receive.recv() else {
                return;
            };
            let supervision = supervise(&mut child, &owned, stdout, stderr, sent, input, config);
            let _ = ready.try_send(());
            // The owner closes this channel after its last possible group signal.
            let _ = permission_to_reap.recv();
            let status = after_cleanup(supervision, child.wait());
            if let Err(error) = finished.try_send(status) {
                if let Err(error) = error.into_inner() {
                    tracing::error!(%error, "Cannot reap an abandoned child");
                }
            }
        });
    if let Err(error) = supervisor {
        let stopped = stop_tree(&tree, &mut child);
        let waited = child.wait().map(drop);
        return Err(cleanup_error(
            fail(error),
            after_cleanup(stopped, waited).map_err(fail),
        ));
    }
    if let Err(error) = handoff.send(child) {
        let mut child = error.0;
        let stopped = stop_tree(&tree, &mut child);
        let waited = child.wait().map(drop);
        return Err(cleanup_error(
            fail(io::Error::other("The child supervisor stopped")),
            after_cleanup(stopped, waited).map_err(fail),
        ));
    }
    Ok(ChildStreams {
        stdout: out,
        stderr: err,
        input: input_status,
        observed,
        exit,
        owner: ChildOwner {
            tree,
            release: Some(release),
        },
    })
}

fn supervise(
    child: &mut Child,
    tree: &tree::Tree,
    stdout: async_channel::Sender<io::Result<Vec<u8>>>,
    stderr: async_channel::Sender<io::Result<Vec<u8>>>,
    input_status: async_channel::Sender<io::Result<()>>,
    input: Option<ProcessInput>,
    config: ProcessConfig,
) -> io::Result<()> {
    std::thread::scope(|scope| {
        let result = (|| {
            let out = child.stdout.take().expect("standard output pipe");
            let err = child.stderr.take().expect("standard error pipe");
            std::thread::Builder::new()
                .name("task-stdout".into())
                .spawn_scoped(scope, move || pump(out, &stdout, config))?;
            std::thread::Builder::new()
                .name("task-stderr".into())
                .spawn_scoped(scope, move || pump(err, &stderr, config))?;
            if let Some(input) = input {
                let pipe = child.stdin.take().expect("private input pipe");
                std::thread::Builder::new()
                    .name("task-stdin".into())
                    .spawn_scoped(scope, move || {
                        let _ = input_status.try_send(input.write(pipe));
                    })?;
            } else {
                let _ = input_status.try_send(Ok(()));
            }
            tree::observe_exit(child)
        })();
        if let Err(error) = result {
            return after_cleanup(Err(error), stop_tree(tree, child));
        }
        result
    })
}

fn stop_tree(tree: &tree::Tree, child: &mut Child) -> io::Result<()> {
    match tree.terminate() {
        Ok(()) => Ok(()),
        Err(error) => after_cleanup(Err(error), child.kill()),
    }
}

fn after_cleanup<T>(operation: io::Result<()>, cleanup: io::Result<T>) -> io::Result<T> {
    match (operation, cleanup) {
        (Ok(()), result) => result,
        (Err(error), Ok(_)) => Err(error),
        (Err(error), Err(cleanup)) => Err(io::Error::other(format!(
            "{error}; cleanup failed: {cleanup}"
        ))),
    }
}

fn cleanup_error(source: TaskError, cleanup: TaskResult) -> TaskError {
    match cleanup {
        Ok(()) => source,
        Err(cleanup) => TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        },
    }
}

fn with_cleanup(source: TaskError, cleanup: TaskResult) -> TaskResult {
    match cleanup {
        Ok(()) => Err(source),
        Err(cleanup) => Err(TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        }),
    }
}

#[cfg(unix)]
mod tree {
    use super::{io, Child, Command};

    pub(super) fn configure(command: &mut Command) {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }

    pub(super) fn observe_exit(child: &Child) -> io::Result<()> {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::uninit();
        loop {
            // The kernel writes the result. WNOWAIT reserves the group leader until the owner releases it.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOWAIT,
                )
            };
            if result == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    pub(super) struct Tree(libc::pid_t);

    impl Tree {
        pub(super) fn attach(child: &Child) -> io::Result<Self> {
            libc::pid_t::try_from(child.id())
                .map(Self)
                .map_err(io::Error::other)
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            // The negative identifier targets the process group created at launch.
            if unsafe { libc::kill(-self.0, libc::SIGKILL) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            #[cfg(target_vendor = "apple")]
            if error.raw_os_error() == Some(libc::EPERM) && self.only_exited_leader() {
                return Ok(());
            }
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error)
            }
        }

        #[cfg(target_vendor = "apple")]
        fn only_exited_leader(&self) -> bool {
            // Darwin denies a group signal when only the exited leader remains.
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            // The zeroed result identifies the case where the child still runs.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.0.cast_unsigned(),
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
                )
            };
            if result != 0 || unsafe { info.assume_init() }.si_pid != self.0 {
                return false;
            }
            // A second slot detects another group member that might still need a signal.
            let mut members = [0_i32; 2];
            let size = libc::c_int::try_from(std::mem::size_of_val(&members))
                .expect("two process identifiers fit");
            let count =
                unsafe { libc::proc_listpgrppids(self.0, members.as_mut_ptr().cast(), size) };
            count == 1 && members[0] == self.0
        }
    }
}

#[cfg(windows)]
mod tree {
    use super::{io, Child, Command};
    use std::{
        mem::size_of,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    };
    use windows_sys::Win32::System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{
            OpenProcess, OpenThread, ResumeThread, CREATE_SUSPENDED, PROCESS_SET_QUOTA,
            PROCESS_TERMINATE, THREAD_SUSPEND_RESUME,
        },
    };

    pub(super) fn configure(command: &mut Command) {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_SUSPENDED);
    }

    pub(super) fn observe_exit(child: &mut Child) -> io::Result<()> {
        // The job handle preserves the tree identity after the process exits.
        child.wait().map(drop)
    }

    pub(super) struct Tree(OwnedHandle);

    impl Tree {
        pub(super) fn attach(child: &Child) -> io::Result<Self> {
            let id = child.id();
            // The returned handles belong to this function.
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = unsafe { OwnedHandle::from_raw_handle(job) };
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let bytes = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                .expect("job structure fits");
            // The structure remains valid until Windows copies it.
            if unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    bytes,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Open the process handle with only the access that job assignment requires.
            let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, id) };
            if process.is_null() {
                return Err(io::Error::last_os_error());
            }
            let process = unsafe { OwnedHandle::from_raw_handle(process) };
            if unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) }
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            resume_primary_thread(id)?;
            Ok(Self(job))
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            if unsafe { TerminateJobObject(self.0.as_raw_handle(), 130) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    fn resume_primary_thread(id: u32) -> io::Result<()> {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        // A suspended process cannot create any descendants before assignment.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
        let mut entry = THREADENTRY32::default();
        entry.dwSize = u32::try_from(size_of::<THREADENTRY32>()).expect("thread structure fits");
        let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
        while found != 0 {
            if entry.th32OwnerProcessID == id {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
                if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                return Ok(());
            }
            found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
        }
        Err(io::Error::other(
            "The suspended child has no primary thread",
        ))
    }
}

struct Console;

impl TaskOutput for Console {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        let bytes = bytes.to_vec();
        Box::pin(async move {
            background(move || match stream {
                OutputStream::Stdout => io::stdout().lock().write_all(&bytes),
                OutputStream::Stderr => io::stderr().lock().write_all(&bytes),
            })
            .await?
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async {
            background(|| {
                let stdout = io::stdout().lock().flush();
                let stderr = io::stderr().lock().flush();
                stdout.and(stderr)
            })
            .await?
        })
    }
}

/// Run the application entry and provide native process and output services.
#[doc(hidden)]
pub fn run_entry(
    label: &'static str,
    entry: impl FnOnce(TaskRunner) -> BoxFuture<'static, TaskResult>,
) -> ExitCode {
    match host(label, entry) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let diagnostic = error.to_string();
            let suffix = if diagnostic.ends_with('\n') { "" } else { "\n" };
            // There is no second output path if the diagnostic stream itself fails.
            let _ = io::stderr()
                .lock()
                .write_all(format!("{diagnostic}{suffix}").as_bytes());
            ExitCode::from(error.exit_code())
        }
    }
}

fn host(
    label: &'static str,
    entry: impl FnOnce(TaskRunner) -> BoxFuture<'static, TaskResult>,
) -> TaskResult {
    let ctx = Ctx::try_host().map_err(TaskError::operation)?;
    foundation_core::env::install(&ctx, foundation_core::env::system)
        .map_err(TaskError::operation)?;
    let cancellation = Cancellation::new();
    let output = Arc::new(Console);
    ctx.store(TaskEnv {
        process: Arc::new(NativeProcess::for_host(&ctx, ProcessConfig::default())),
        output: output.clone(),
        cancellation,
    });
    let guard = foundation_core::valtron::initialize_pool_with_ctx(100, None, ctx.clone());
    let runner =
        TaskRunner::new(ctx, std::env::args_os().collect()).with_name(label, "Run available tasks");
    let entry = entry(runner);
    let outcome = foundation_core::valtron::block_on_future(async move {
        let result = entry.await;
        let flushed = output.flush().await.map_err(TaskError::operation);
        match result {
            Ok(()) => flushed,
            Err(error) => with_cleanup(error, flushed),
        }
    });
    drop(guard);
    outcome
}
