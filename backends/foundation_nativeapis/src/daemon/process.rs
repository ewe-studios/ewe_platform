//! Managed process: spawn plumbing, output/exit event channels, and the five
//! readiness-detection strategies as valtron tasks.
//!
//! WHY: The supervisor must launch a child, observe its stdout/stderr for output
//! readiness and logging, learn when it exits (to drive restart), and confirm
//! "ready to serve" via one of several strategies — all without blocking the
//! supervisor thread.
//!
//! WHAT: `ManagedDaemon` (the supervisor's per-daemon record), `StreamKind` /
//! `OutputLine` / `DaemonExitEvent` (the channel payloads), `TimerReadiness`
//! (a duration-fired [`EventReadiness`]), and `ReadinessTask` (a
//! [`TaskIterator`] implementing all five strategies).
//!
//! HOW: Exit watching and stream reading run on the valtron background job pool
//! (`run_background_job`). Readiness is a `TaskIterator` that parks on a
//! `QueueReadiness` (output) or `TimerReadiness` (time/probe based) rather than
//! sleeping, and yields `Ready(Ok)`/`Ready(Err(timeout))` once resolved. The
//! HTTP probe is a dependency-free `TcpStream` GET to avoid a crate cycle with
//! `foundation_netio`.

use std::io::{BufRead, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{
    run_background_job, BoxedSendExecutionAction, EventReadiness, QueueReadiness, TaskIterator,
    TaskStatus,
};

use super::config::{DaemonDef, ReadinessConfig};
use super::error::ReadinessTimeout;
use super::id::{DaemonId, DaemonStatus};

/// Which standard stream a line of output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// The child's standard output.
    Stdout,
    /// The child's standard error.
    Stderr,
}

impl std::fmt::Display for StreamKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        })
    }
}

/// A single line captured from a child's stdout/stderr.
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The line text (newline stripped).
    pub text: String,
    /// Which stream produced it.
    pub stream: StreamKind,
}

/// Emitted when a managed child process exits.
#[derive(Debug, Clone)]
pub struct DaemonExitEvent {
    /// The daemon whose process exited (id travels with the event).
    pub id: DaemonId,
    /// The pid of the process that exited — lets the supervisor ignore a stale
    /// exit from a previous instance after a restart.
    pub pid: u32,
    /// The child's exit status, if `wait()` succeeded.
    pub exit_status: Option<std::process::ExitStatus>,
}

/// The supervisor's per-daemon bookkeeping record.
///
/// WHY: The supervisor tracks each daemon's current pid, lifecycle status, and
/// restart accounting to drive readiness, restart-with-backoff, and RPC status.
///
/// WHAT: The definition plus mutable runtime state.
///
/// HOW: Cloned out of the supervisor's mutex when a background job needs an
/// owned snapshot (e.g. the restart closure).
#[derive(Debug, Clone)]
pub struct ManagedDaemon {
    /// This daemon's identity.
    pub id: DaemonId,
    /// The definition it was built from.
    pub def: DaemonDef,
    /// The running child's pid, if any.
    pub pid: Option<u32>,
    /// Current lifecycle status.
    pub status: DaemonStatus,
    /// How many times it has been restarted.
    pub restart_count: u32,
    /// When the current process was last started.
    pub last_start: Option<Instant>,
}

impl ManagedDaemon {
    /// Create a `Pending` record for a definition.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(id: DaemonId, def: DaemonDef) -> Self {
        Self {
            id,
            def,
            pid: None,
            status: DaemonStatus::Pending,
            restart_count: 0,
            last_start: None,
        }
    }
}

/// A readiness signal that fires `true` after a duration elapses.
///
/// WHY: Time-based readiness (`Delay`) and periodic re-probing (`Http`, `Port`,
/// `Cmd`) need the executor to park the task and wake it later — without a busy
/// sleep inside `next_status`.
///
/// WHAT: An [`EventReadiness`] backed by an `Arc<AtomicBool>` that a background
/// timer thread flips after the requested duration.
///
/// HOW: [`TimerReadiness::reset`] clears the bool and spawns a short-lived timer
/// thread that sleeps then stores `true`; `is_ready` reads the bool.
#[derive(Debug, Clone)]
pub struct TimerReadiness {
    signal: Arc<AtomicBool>,
}

impl TimerReadiness {
    /// Create an unfired timer.
    ///
    /// # Panics
    /// Never panics.
    pub fn new() -> Self {
        Self {
            signal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Clear the signal and (re)arm it to fire after `dur`.
    ///
    /// WHY: Probe strategies re-arm the timer between checks.
    ///
    /// WHAT: Resets to unfired, then schedules a fire after `dur`.
    ///
    /// HOW: Stores `false`, then a background job sleeps `dur` and stores `true`.
    /// Uses the valtron background pool so no unmanaged thread leaks.
    ///
    /// # Panics
    /// Never panics (a closed background pool is logged, not fatal — the task
    /// will time out instead of parking forever).
    pub fn reset(&self, dur: Duration) {
        self.signal.store(false, Ordering::SeqCst);
        let signal = self.signal.clone();
        if let Err(e) = run_background_job(move || {
            std::thread::sleep(dur);
            signal.store(true, Ordering::SeqCst);
        }) {
            tracing::warn!("timer readiness could not arm: {e}");
        }
    }
}

impl Default for TimerReadiness {
    fn default() -> Self {
        Self::new()
    }
}

impl EventReadiness for TimerReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.signal.load(Ordering::SeqCst)
    }
}

/// A [`TaskIterator`] that resolves one readiness strategy for one daemon.
///
/// WHY: Dependents must not start until a daemon is actually ready; readiness
/// varies by process, so this task encodes all five strategies uniformly and
/// parks (never spins) between checks.
///
/// WHAT: Yields exactly one `Ready(Ok(()))` on success or
/// `Ready(Err(ReadinessTimeout))` if the timeout elapses first, then completes.
///
/// HOW: Output readiness pops from the shared output queue and parks on its
/// `QueueReadiness`; time/probe strategies check, then park on a `TimerReadiness`
/// re-armed for the next attempt. The overall deadline is enforced on every poll.
pub struct ReadinessTask {
    id: DaemonId,
    strategy: ReadinessConfig,
    output_queue: Arc<ConcurrentQueue<OutputLine>>,
    timer: TimerReadiness,
    started: Instant,
    timeout: Duration,
    done: bool,
}

impl ReadinessTask {
    /// Build a readiness task for a daemon.
    ///
    /// WHY: The supervisor constructs one per daemon at spawn time.
    ///
    /// WHAT: A task bound to the daemon's strategy, its output queue (for the
    /// `Output` strategy), and a timeout.
    ///
    /// HOW: Records the start instant for deadline accounting.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(
        id: DaemonId,
        strategy: ReadinessConfig,
        output_queue: Arc<ConcurrentQueue<OutputLine>>,
        timeout: Duration,
    ) -> Self {
        Self {
            id,
            strategy,
            output_queue,
            timer: TimerReadiness::new(),
            started: Instant::now(),
            timeout,
            done: false,
        }
    }

    fn timed_out(&self) -> bool {
        self.started.elapsed() >= self.timeout
    }

    fn timeout_err(&self) -> ReadinessTimeout {
        ReadinessTimeout {
            id: self.id.clone(),
            timeout_secs: self.timeout.as_secs(),
        }
    }

    /// Minimal dependency-free HTTP GET probe: connect, send request, check for
    /// a 2xx status line.
    ///
    /// WHY: Avoids pulling `foundation_netio` into this crate (which would risk a
    /// dependency cycle) just to check liveness.
    ///
    /// WHAT: `true` if the URL returns a `2xx` status within a short timeout.
    ///
    /// HOW: Parses `http://host[:port]/path`, opens a `TcpStream`, writes an
    /// HTTP/1.0 `GET`, and parses the status code from the first response line.
    ///
    /// # Panics
    /// Never panics.
    fn http_check(url: &str) -> bool {
        let Some((host, port, path)) = parse_http_url(url) else {
            return false;
        };
        let addr = format!("{host}:{port}");
        let Ok(mut resolved) = std::net::ToSocketAddrs::to_socket_addrs(&addr) else {
            return false;
        };
        let Some(sockaddr) = resolved.next() else {
            return false;
        };
        let Ok(mut stream) =
            std::net::TcpStream::connect_timeout(&sockaddr, Duration::from_millis(200))
        else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
        let request =
            format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut buf = [0u8; 128];
        let Ok(n) = stream.read(&mut buf) else {
            return false;
        };
        let head = String::from_utf8_lossy(&buf[..n]);
        // Status line: "HTTP/1.1 200 OK". The status code is the second token.
        head.split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .is_some_and(|code| (200..300).contains(&code))
    }

    fn port_check(port: u16) -> bool {
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            Duration::from_millis(50),
        )
        .is_ok()
    }

    fn cmd_check(args: &[String]) -> bool {
        if args.is_empty() {
            return false;
        }
        std::process::Command::new(&args[0])
            .args(&args[1..])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

impl TaskIterator for ReadinessTask {
    type Ready = Result<(), ReadinessTimeout>;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(
        &mut self,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.done {
            return None;
        }
        if self.timed_out() {
            self.done = true;
            return Some(TaskStatus::Ready(Err(self.timeout_err())));
        }

        match &self.strategy {
            ReadinessConfig::Immediate => {
                self.done = true;
                Some(TaskStatus::Ready(Ok(())))
            }

            ReadinessConfig::Delay(target) => {
                if self.started.elapsed() >= *target {
                    self.done = true;
                    Some(TaskStatus::Ready(Ok(())))
                } else {
                    self.timer
                        .reset(target.saturating_sub(self.started.elapsed()));
                    Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
                }
            }

            ReadinessConfig::Output(pattern) => {
                while let Ok(line) = self.output_queue.pop() {
                    if pattern.is_match(&line.text) {
                        self.done = true;
                        return Some(TaskStatus::Ready(Ok(())));
                    }
                }
                Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                    self.output_queue.clone(),
                ))))
            }

            ReadinessConfig::Http(url) => {
                if Self::http_check(url) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(Duration::from_millis(500));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }

            ReadinessConfig::Port(port) => {
                if Self::port_check(*port) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(Duration::from_millis(100));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }

            ReadinessConfig::Cmd(args) => {
                if Self::cmd_check(args) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(Duration::from_millis(500));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }
        }
    }
}

/// Parse a minimal `http://host[:port]/path` URL for the readiness probe.
///
/// WHY: The probe needs host/port/path without a URL-parsing dependency.
///
/// WHAT: `(host, port, path)`, defaulting port 80 and path `/`.
///
/// HOW: Strips the `http://` scheme, splits authority from path, then host from
/// port. Returns `None` for non-http schemes or empty hosts.
///
/// # Panics
/// Never panics.
fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(80)),
        None => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return None;
    }
    Some((host, port, path.to_string()))
}

/// Submit an exit watcher to the valtron background pool.
///
/// WHY: `Child::wait()` blocks; it must not run on a task/supervisor thread.
///
/// WHAT: Owns the `Child`, blocks on `wait()`, and pushes a `DaemonExitEvent`.
///
/// HOW: `run_background_job`. On a closed pool the daemon simply won't observe
/// the exit (logged), rather than panicking the caller.
///
/// # Panics
/// Never panics.
pub fn spawn_exit_watcher(
    mut child: std::process::Child,
    id: DaemonId,
    pid: u32,
    queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
) {
    if let Err(e) = run_background_job(move || {
        let exit_status = child.wait().ok();
        let _ = queue.push(DaemonExitEvent {
            id,
            pid,
            exit_status,
        });
    }) {
        tracing::error!("could not submit exit watcher: {e}");
    }
}

/// Submit a stream reader to the valtron background pool.
///
/// WHY: Reading a child's stdout/stderr blocks; it feeds output readiness and
/// structured logs.
///
/// WHAT: Reads the stream line by line, pushing each to the readiness queue and
/// logging it under the daemon's id.
///
/// HOW: `run_background_job` with a `BufReader`. A closed queue stops the reader.
///
/// # Panics
/// Never panics.
pub fn spawn_stream_reader<R>(
    id: DaemonId,
    stream: R,
    kind: StreamKind,
    readiness_queue: Arc<ConcurrentQueue<OutputLine>>,
) where
    R: Read + Send + 'static,
{
    if let Err(e) = run_background_job(move || {
        for line in std::io::BufReader::new(stream).lines() {
            match line {
                Ok(text) => {
                    tracing::info!(daemon = %id, stream = %kind, "{text}");
                    if readiness_queue
                        .push(OutputLine { text, stream: kind })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(daemon = %id, "stream read error: {e}");
                    break;
                }
            }
        }
    }) {
        tracing::error!("could not submit stream reader: {e}");
    }
}
