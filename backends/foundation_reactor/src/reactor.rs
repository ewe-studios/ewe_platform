//! Shared process-level reactor (F40 — Decision 14 §Scope 3).
//!
//! WHY: Before F40, every `FdRegistration` created its own private `Poll`
//! selector — one epoll fd per registered socket. At thousands of
//! connections that's thousands of epoll fds plus a zero-timeout
//! `epoll_wait` syscall per `is_ready()` check. A single shared epoll/kqueue
//! fd eliminates this.
//!
//! WHAT: [`Reactor`] — a `OnceLock` singleton holding one platform selector
//! and a concurrent `Token → Ready` cache. One drain thread services all
//! registered fds. `is_ready()` reads a cached atomic bit — zero syscalls.
//!
//! HOW: On first access, creates the selector and spawns the drain thread.
//! Registration calls `epoll_ctl(ADD)`. The drain thread blocks in
//! `poll(timeout)` on **the same selector registrations land in**, and on each
//! event ORs the token's readiness bits into its cache entry. Tasks parked via
//! `TaskStatus::Depends(event_readiness)` are re-polled by the executor when
//! `is_ready()` returns true.
//!
//! ## The edge-triggered clear protocol
//!
//! The Linux selector registers fds edge-triggered (`EPOLLET`), so the kernel
//! reports a readiness transition exactly once. The cache therefore *latches*
//! that edge: bits stay set until a consumer clears them. A consumer that reads
//! until `WouldBlock` must call [`Reactor::clear`] (via
//! `ReadyGuard::clear_ready`), otherwise the entry stays permanently ready and
//! the task spins instead of parking. Clearing re-arms the entry: the next
//! kernel edge sets the bit again.
//!
//! Set (drain thread) and clear (consumer) race by construction. They are both
//! atomic read-modify-writes on the same `AtomicU8`, and the ordering that
//! matters is the safe one: a `fetch_or` landing after a `fetch_and` leaves the
//! bit *set* — a spurious wake, which every consumer already tolerates by
//! re-reading and getting `WouldBlock`. The unsafe direction (losing a real
//! edge) cannot happen, because the kernel only reports the edge after the data
//! is queued, so the `fetch_or` always happens-after the data is observable.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::task::Waker;
use std::thread;
use std::time::Duration;

use crate::poll::{Backend, BackendPreference, Events, Interest, Poll, Registry, Token};
use crate::ready::Ready;
use foundation_core::valtron::{ReadinessNotifier, ReadinessWaker};

/// Per-registration readiness tracking.
///
/// `ready` is atomic so the drain thread can OR bits in, and consumers can mask
/// bits out, while both hold only a *read* lock on the entry map.
struct Entry {
    /// The registered fd, kept so [`Reactor::clear`] can re-arm the selector's
    /// read edge (a kqueue-only concern; see [`Poll::rearm_readable`]).
    fd: std::os::fd::RawFd,
    ready: AtomicU8,
    #[allow(dead_code)]
    interest: Interest,
    /// A `std::task::Waker` armed by [`Reactor::register_waker`]. The drain
    /// thread wakes it after latching readiness, so a native-executor task
    /// (tokio/smol) can park on the same edge the valtron task path uses.
    waker: Mutex<Option<Waker>>,
    /// The valtron tasks parked on this fd's readiness
    /// ([`Reactor::register_task_waker`]). The drain thread wakes them after it
    /// latched readiness; each wake reaches only that task's executor.
    tasks: Arc<ReadinessNotifier>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("fd", &self.fd)
            .field(
                "ready",
                &Ready::from_bits(self.ready.load(Ordering::Acquire)),
            )
            .field("interest", &self.interest)
            .field("waker", &self.waker)
            .field("tasks", &self.tasks)
            .finish()
    }
}

/// The shared process-level reactor singleton.
pub struct Reactor {
    /// The one selector. Shared with the drain thread — registrations and
    /// `poll()` **must** target the same instance or no wake ever fires.
    poll: Arc<Poll>,
    /// Registry over `poll`'s selector.
    registry: Registry,
    /// Token → readiness cache. Arc'd so the drain thread holds a ref.
    entries: Arc<RwLock<HashMap<Token, Entry>>>,
    /// The live token for each registered fd number.
    ///
    /// The selector keys a registration by fd number alone, so a late
    /// [`Reactor::deregister`] cannot tell whether the number still belongs to
    /// its token. This map answers that question: the entry is inserted after
    /// the selector call succeeds and removed together with the selector entry.
    /// It holds one token per open fd, so it is bounded by the open-fd count.
    /// Guarded by `reg_lock`, which already serialises selector changes, so it
    /// adds no second lock order.
    fd_tokens: Mutex<HashMap<std::os::fd::RawFd, Token>>,
    /// Registration lock — serialises `epoll_ctl` calls.
    reg_lock: Mutex<()>,
    /// Drain thread. `Some` for the process lifetime; [`Drop`] takes it so it
    /// can join instead of merely detaching the handle.
    drain: Option<thread::JoinHandle<()>>,
    /// Signal the drain thread to stop.
    shutdown: Arc<AtomicBool>,
    /// Event generation + condvar (F50 B2). The drain thread bumps the counter and
    /// notifies after each batch of latched readiness, so a *non-task* thread (e.g.
    /// a proxy splice) can block until the reactor observes new events instead of
    /// busy-polling. The generation makes the wait lost-wakeup-safe: a waiter that
    /// snapshots the count before reading its fds only sleeps if nothing has
    /// happened since.
    events_gen: Arc<(Mutex<u64>, Condvar)>,
    /// Selector poll timeout for the drain thread (rule 8; see
    /// [`ReactorConfig::drain_poll_timeout`]).
    drain_poll_timeout: Duration,
}

impl std::fmt::Debug for Reactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reactor")
            .field("registered", &self.entries.read().map_or(0, |e| e.len()))
            .field("fd_tokens", &self.fd_tokens)
            .field("shutdown", &self.shutdown.load(Ordering::Acquire))
            .field("events_gen", &self.events_gen)
            .field("drain_poll_timeout", &self.drain_poll_timeout)
            .finish_non_exhaustive()
    }
}

/// Global singleton.
static REACTOR: OnceLock<Arc<Reactor>> = OnceLock::new();

/// Serialises reactor construction (issue #240).
///
/// WHY: `REACTOR` alone makes *installing* the reactor atomic, but not
/// *building* it. Before this lock, every thread that missed `REACTOR.get()`
/// went on to build its own `Reactor` and spawn its own `reactor-drain` thread;
/// all but one then lost `REACTOR.set` and dropped their instance. This lock
/// makes the whole "miss → build → install" sequence single-flight, so only the
/// winner ever constructs a selector or starts a thread.
///
/// WHAT: held across the second `REACTOR.get()` check and the whole of
/// [`Reactor::init_inner`], released once `REACTOR.set` has run.
static INIT_LOCK: Mutex<()> = Mutex::new(());

/// Number of `reactor-drain` threads alive in this process.
///
/// WHY: issue #240 was a drain thread leaked by every initialisation-race
/// loser. A unit test cannot see that leak through the public behaviour, so
/// this counter makes it observable. It is test instrumentation only; nothing
/// in the reactor reads it.
static LIVE_DRAIN_THREADS: AtomicUsize = AtomicUsize::new(0);

/// Test hook: the number of live `reactor-drain` threads (issue #240).
///
/// WHY: a lost reactor-initialisation race must not leave a drain thread
/// behind. External test binaries count this to prove it, without reading
/// `/proc` (which is Linux-only).
///
/// WHAT: the current value of the process-wide live-drain-thread counter.
///
/// HOW: the counter is incremented when the drain thread starts and decremented
/// when its loop returns, through an RAII guard so a panic cannot skew it.
///
/// # Panics
/// Never panics.
#[doc(hidden)]
pub fn live_drain_threads() -> usize {
    LIVE_DRAIN_THREADS.load(Ordering::SeqCst)
}

/// Number of `Reactor::init_inner` entries in this process.
///
/// Test instrumentation: single-flight initialisation must let exactly one
/// thread build the reactor. The live-thread counter above proves the drain
/// thread side; this counter proves no second build was even attempted.
static INIT_BUILDS: AtomicUsize = AtomicUsize::new(0);

/// Test hook: how many threads have entered the reactor's build body.
///
/// WHY: issue #240 was every losing initialiser building (and spawning) its own
/// reactor. Counting build entries pins the single-flight guarantee directly.
///
/// WHAT: how many times `init_inner` has been entered since process start.
///
/// HOW: read the process-wide counter. Nothing in the reactor reads it.
///
/// # Panics
/// Never panics.
#[doc(hidden)]
pub fn init_build_attempts() -> usize {
    INIT_BUILDS.load(Ordering::SeqCst)
}

/// Number of `reactor-drain` threads spawned in this process.
///
/// Test instrumentation: the old race spawned one per init race loser. A build
/// entry does not prove a thread was spawned (the selector could fail first),
/// and a live-thread count does not prove how many were ever spawned (a detached
/// loser thread exits as soon as it observes `shutdown`). This counter is
/// incremented in the parent immediately after `spawn` returns.
static DRAIN_THREADS_SPAWNED: AtomicUsize = AtomicUsize::new(0);

/// Test hook: how many `reactor-drain` threads have been spawned.
///
/// WHY: issue #240 leaked one drain thread per initialisation-race loser. This
/// counts them directly, so the test can show "one per loser" instead of racing
/// a transient live count.
///
/// WHAT: how many successful `reactor-drain` spawns have happened since process
/// start.
///
/// HOW: read the process-wide counter. Nothing in the reactor reads it.
///
/// # Panics
/// Never panics.
#[doc(hidden)]
pub fn drain_thread_spawn_count() -> usize {
    DRAIN_THREADS_SPAWNED.load(Ordering::SeqCst)
}

/// Test-only pause, in milliseconds, applied inside the reactor build body.
///
/// WHY: the window between the `REACTOR.get()` miss and `REACTOR.set` is only
/// microseconds wide, so a race test is scheduling-dependent: sometimes one
/// thread finishes before the others even check. Holding every builder inside
/// the body makes the race reproduction deterministic.
///
/// WHAT: zero (the default) disables the pause; anything else sleeps that many
/// milliseconds at the start of `init_inner`.
///
/// HOW: set by `#[doc(hidden)]` test hook [`set_init_build_delay`]. Normal code
/// never writes it.
static INIT_BUILD_DELAY_MS: AtomicU64 = AtomicU64::new(0);

/// Test hook: pause every reactor builder for `delay` (issue #240 race tests).
///
/// WHY: reproduce the initialisation race deterministically. See
/// [`INIT_BUILD_DELAY_MS`].
///
/// WHAT: set the delay applied at the start of `init_inner`. Zero disables it.
///
/// HOW: stores milliseconds in a process-wide atomic.
///
/// # Panics
/// Never panics.
#[doc(hidden)]
pub fn set_init_build_delay(delay: Duration) {
    let millis = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
    INIT_BUILD_DELAY_MS.store(millis, Ordering::SeqCst);
}

/// RAII guard that keeps [`LIVE_DRAIN_THREADS`] accurate across panics.
struct LiveDrainGuard;

impl LiveDrainGuard {
    fn enter() -> Self {
        LIVE_DRAIN_THREADS.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for LiveDrainGuard {
    fn drop(&mut self) {
        LIVE_DRAIN_THREADS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Error returned by [`Reactor::init`] when a reactor is already running on
/// a different backend than the one requested.
///
/// WHY: the reactor is process-global (D14 OQ#14.1). Two servers in one process
/// asking for different backends cannot both be satisfied, and silently giving
/// the second the first's backend defeats the point of explicit selection — a
/// perf-critical deploy would discover from latency graphs that it never got
/// `io_uring`. This error makes that failure a startup crash with both backends
/// named.
#[derive(Debug, Clone)]
pub struct AlreadyInitialised {
    /// The backend that is already running.
    pub running: Backend,
    /// What the caller asked for.
    pub requested: BackendPreference,
}

impl std::fmt::Display for AlreadyInitialised {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "reactor already initialised with backend {} — cannot honour request for {:?}",
            self.running, self.requested,
        )
    }
}

impl std::error::Error for AlreadyInitialised {}

impl From<AlreadyInitialised> for io::Error {
    fn from(e: AlreadyInitialised) -> io::Error {
        io::Error::new(io::ErrorKind::AlreadyExists, e.to_string())
    }
}

/// Default selector poll timeout for the reactor drain thread (rule 8).
///
/// It bounds shutdown latency only: a real event returns from `poll()`
/// immediately, and an idle drain thread checks the shutdown flag at least this
/// often. Pick: 100 ms — the same idle wake-up bound valtron uses for its worker
/// re-check (`DEFAULT_OP_READ_TIME`), so an idle reactor wakes ~10×/s and a
/// shutdown is noticed within a tenth of a second. Referenced only by
/// [`ReactorConfig::default`].
pub const DEFAULT_DRAIN_POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Configuration for the shared reactor (rule 8 — limits are configuration).
///
/// WHAT: `drain_poll_timeout` is the one knob the drain loop has: how long it
/// blocks in `poll()` when no fd is ready. A real event still returns from
/// `poll()` immediately, so the knob bounds shutdown latency and the idle
/// wake-up rate, not readiness latency.
#[derive(Debug, Clone, Copy)]
pub struct ReactorConfig {
    drain_poll_timeout: Duration,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        Self {
            drain_poll_timeout: DEFAULT_DRAIN_POLL_TIMEOUT,
        }
    }
}

impl ReactorConfig {
    /// Set the selector poll timeout for the drain thread. See
    /// [`DEFAULT_DRAIN_POLL_TIMEOUT`].
    #[must_use]
    pub fn with_drain_poll_timeout(mut self, timeout: Duration) -> Self {
        self.drain_poll_timeout = timeout;
        self
    }

    /// The configured drain poll timeout.
    #[must_use]
    pub fn drain_poll_timeout(&self) -> Duration {
        self.drain_poll_timeout
    }
}

impl Reactor {
    /// WHY: every `FdRegistration` parks on this one instance instead of
    /// building a private epoll fd per socket.
    ///
    /// WHAT: get or initialise the shared reactor.
    ///
    /// HOW: `OnceLock` doesn't support fallible init, so this delegates to
    /// [`Reactor::init_with_config`], whose [`INIT_LOCK`] makes the check and the
    /// build single-flight; a loser of the build waits and returns the winner's
    /// instance.
    ///
    /// The reactor is initialised with [`BackendPreference::Auto`] — the probe
    /// ladder picks the best available backend. Call [`Reactor::init`] instead
    /// to require a specific backend.
    ///
    /// # Errors
    /// Returns the underlying `io::Error` if the platform selector or the drain
    /// thread cannot be created.
    ///
    /// # Panics
    /// Never panics.
    pub fn get() -> io::Result<Arc<Self>> {
        if let Some(reactor) = REACTOR.get() {
            return Ok(Arc::clone(reactor));
        }
        Self::init_with_config(BackendPreference::Auto, ReactorConfig::default())
    }

    /// WHY: an operator must be able to demand a specific backend (F48 —
    /// `ServerIo::Completion` needs `Reactor::init(Uring)`), and must be told
    /// when that demand cannot be met. `get()` always auto-selects, silently
    /// falling back to epoll on a host without `io_uring`. That silent fallback is
    /// precisely the no-silent-defaults failure mode Decision 14 OQ#14.3 calls
    /// "the worst."
    ///
    /// WHAT: initialise the shared reactor on a specific backend. First call
    /// wins; the backend is a process-level decision.
    ///
    /// HOW: [`BackendPreference::Auto`] walks the probe ladder.
    /// [`BackendPreference::Uring`] fails hard with the probe detail if `io_uring`
    /// is unavailable. [`BackendPreference::Epoll`] skips the probe. See
    /// [`AlreadyInitialised`] for the conflict case.
    ///
    /// # Errors
    /// - The probe's concrete failure if `preference` is unavailable
    ///   (`Uring` on `kernel.io_uring_disabled=2`).
    /// - [`AlreadyInitialised`] if the reactor is already running on a
    ///   *different* backend than requested.
    /// - The platform selector's own `io::Error`.
    ///
    /// # Panics
    /// Never panics.
    pub fn init(preference: BackendPreference) -> io::Result<Arc<Self>> {
        Self::init_with_config(preference, ReactorConfig::default())
    }

    /// WHY: rule 8 — a caller that needs a non-default drain poll timeout (or a
    /// future reactor knob) must be able to set it on the one shared instance.
    ///
    /// WHAT: initialise the shared reactor on `preference` with `config`. First
    /// call wins; a later call returns the existing instance and ignores
    /// `config`, exactly as [`Reactor::init`] ignores a later preference.
    ///
    /// HOW: takes [`INIT_LOCK`] so construction is single-flight, applies the
    /// backend-match policy to an existing instance, and otherwise builds via
    /// [`Reactor::init_inner`] while holding the lock.
    ///
    /// # Errors
    /// - [`AlreadyInitialised`] if the reactor is already running on a
    ///   *different* backend than requested.
    /// - The probe's concrete failure or the selector's own `io::Error`.
    ///
    /// # Panics
    /// Never panics.
    pub fn init_with_config(
        preference: BackendPreference,
        config: ReactorConfig,
    ) -> io::Result<Arc<Self>> {
        // Fast path: already initialised, so no build (and no drain thread) is
        // needed. Kept lock-free so every ordinary caller pays only an atomic
        // load.
        if let Some(existing) = REACTOR.get() {
            return Self::already_initialised(existing, preference);
        }

        // Single-flight (issue #240): the window between the miss above and
        // `REACTOR.set` used to allow every racer to build its own reactor and
        // leak its own drain thread. Holding this lock across the second check
        // and `init_inner` means exactly one thread builds; the rest wait, then
        // see the winner's instance on the re-check.
        let _init = INIT_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(existing) = REACTOR.get() {
            return Self::already_initialised(existing, preference);
        }

        Self::init_inner(preference, config)
    }

    /// Return an existing reactor, or the conflict error if `preference` does
    /// not match the running backend.
    ///
    /// WHY: both the lock-free fast path and the post-lock re-check in
    /// [`Reactor::init_with_config`] apply the same backend-match policy.
    ///
    /// WHAT: first-call-wins for a matching preference; [`AlreadyInitialised`]
    /// for a mismatch.
    ///
    /// HOW: compares the running backend through [`backend_matches`].
    ///
    /// # Errors
    /// [`AlreadyInitialised`] when the request conflicts with the running
    /// backend.
    ///
    /// # Panics
    /// Never panics.
    fn already_initialised(
        existing: &Arc<Self>,
        preference: BackendPreference,
    ) -> io::Result<Arc<Self>> {
        let running = existing.backend();
        // If the running backend matches the preference, return it.
        // Mismatch: the caller asked for something different than what is
        // already running.
        if !backend_matches(running, preference) {
            return Err(AlreadyInitialised {
                running,
                requested: preference,
            }
            .into());
        }
        tracing::debug!(
            running = %running,
            ?preference,
            "reactor already initialised on a matching backend; returning existing instance (config ignored)"
        );
        Ok(Arc::clone(existing))
    }

    /// Shared init body — builds the reactor and installs it.
    ///
    /// Callers must hold [`INIT_LOCK`], which makes this the only build in
    /// flight, so [`REACTOR.set`] cannot actually race. The loser branch below
    /// is kept as a defensive fallback.
    fn init_inner(preference: BackendPreference, config: ReactorConfig) -> io::Result<Arc<Self>> {
        INIT_BUILDS.fetch_add(1, Ordering::SeqCst);

        // Test instrumentation (issue #240): widen the race window so a test can
        // make every contender reach this point before any of them finishes.
        let delay_ms = INIT_BUILD_DELAY_MS.load(Ordering::SeqCst);
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        let poll = Arc::new(Self::make_poll(preference)?);
        let registry = poll.registry();
        let shutdown = Arc::new(AtomicBool::new(false));

        let entries: Arc<RwLock<HashMap<Token, Entry>>> = Arc::new(RwLock::new(HashMap::new()));

        // The drain thread polls the *same* selector registrations land in.
        // Handing it a second `Poll` instance here silently breaks every wake.
        let events_gen: Arc<(Mutex<u64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));
        let drain_timeout = config.drain_poll_timeout();

        let drain_poll = Arc::clone(&poll);
        let drain_entries = Arc::clone(&entries);
        let drain_shutdown = Arc::clone(&shutdown);
        let drain_events_gen = Arc::clone(&events_gen);

        let drain = thread::Builder::new()
            .name("reactor-drain".into())
            .spawn(move || {
                let _live = LiveDrainGuard::enter();
                drain_loop(
                    &drain_poll,
                    &drain_entries,
                    &drain_shutdown,
                    &drain_events_gen,
                    drain_timeout,
                );
            })?;
        DRAIN_THREADS_SPAWNED.fetch_add(1, Ordering::SeqCst);

        let reactor = Arc::new(Reactor {
            poll,
            registry,
            entries,
            fd_tokens: Mutex::new(HashMap::new()),
            reg_lock: Mutex::new(()),
            drain: Some(drain),
            shutdown,
            events_gen,
            drain_poll_timeout: drain_timeout,
        });

        match REACTOR.set(Arc::clone(&reactor)) {
            Ok(()) => {
                tracing::info!(backend = ?reactor.backend(), "shared fd reactor initialised");
                Ok(reactor)
            }
            // Lost the init race — the winner's instance is authoritative.
            // Unreachable while callers hold `INIT_LOCK`; kept as a defensive
            // fallback so a future caller cannot silently drop a built reactor.
            Err(_) => Ok(Arc::clone(
                REACTOR.get().expect("set() raced, so it is populated"),
            )),
        }
    }

    /// Construct the platform poller. On Linux, honours the preference through
    /// the probe ladder. On non-Linux, any explicit preference other than `Auto`
    /// is a hard error — the only available backend is kqueue.
    #[cfg(target_os = "linux")]
    fn make_poll(preference: BackendPreference) -> io::Result<Poll> {
        Poll::with_preference(preference)
    }

    #[cfg(not(target_os = "linux"))]
    fn make_poll(preference: BackendPreference) -> io::Result<Poll> {
        if !matches!(preference, BackendPreference::Auto) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "backend {preference:?} is not available on this platform; only Auto (kqueue) is supported",
                ),
            ));
        }
        Poll::new()
    }

    /// WHY: callers that need to drive the selector directly (tests, examples)
    /// should share the reactor's selector rather than build their own.
    ///
    /// WHAT: the shared `Registry`.
    ///
    /// HOW: returns a borrow of the registry cloned from the shared `Poll`.
    ///
    /// # Panics
    /// Never panics.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// WHY: observability — which backend the process actually selected
    /// (Decision 14 OQ#14.3). Operators need the backend in use, not the one
    /// that was preferred.
    ///
    /// WHAT: the selector backend this reactor is driving.
    ///
    /// HOW: asks the selector, which was chosen at construction by the F42
    /// probe ladder.
    ///
    /// # Panics
    /// Never panics.
    pub fn backend(&self) -> Backend {
        self.poll.backend()
    }

    /// WHY: observability — a rule-8 config field must be inspectable, and an
    /// operator or test needs to confirm the shared reactor actually holds the
    /// value that was configured (not just that the config object held it).
    ///
    /// WHAT: the drain poll timeout the shared reactor was initialised with.
    ///
    /// # Panics
    /// Never panics.
    pub fn drain_poll_timeout(&self) -> Duration {
        self.drain_poll_timeout
    }

    /// WHY: one `epoll_ctl` per connection, into the shared selector.
    ///
    /// WHAT: register `fd` under `token` with `interest`, and start tracking
    /// its readiness.
    ///
    /// HOW: takes `reg_lock` to serialise selector mutation, registers, then
    /// installs a zeroed cache entry and records the fd's live token.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` if registration fails; the cache entry
    /// is not installed in that case.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn register(
        &self,
        fd: std::os::fd::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        self.registry.register_fd(fd, token, interest)?;
        self.entries.write().expect("entries lock poisoned").insert(
            token,
            Entry {
                fd,
                ready: AtomicU8::new(Ready::EMPTY.bits()),
                interest,
                waker: Mutex::new(None),
                tasks: Arc::new(ReadinessNotifier::new()),
            },
        );
        // The latest `register` owns the number: replace any earlier mapping,
        // so a stale deregistration for the old token can no longer touch the
        // selector.
        self.fd_tokens
            .lock()
            .expect("fd_tokens lock poisoned")
            .insert(fd, token);
        Ok(())
    }

    /// WHY: a task parks on an fd for read readiness and only wants write
    /// readiness while it has output queued to flush. Watching write readiness
    /// all the time spins the drain thread — an idle socket is always writable,
    /// so the selector reports it ready on every poll.
    ///
    /// WHAT: change the interest an already-registered fd is watched with.
    ///
    /// HOW: takes `reg_lock` to serialise selector mutation, re-registers the
    /// fd, then records the new interest in the cache entry for observability.
    /// The fd-to-token mapping stays as it is: the fd number and its token do
    /// not change.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` if re-registration fails.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn reregister(
        &self,
        fd: std::os::fd::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        self.registry.reregister_fd(fd, token, interest)?;
        if let Some(entry) = self
            .entries
            .write()
            .expect("entries lock poisoned")
            .get_mut(&token)
        {
            entry.interest = interest;
        }
        Ok(())
    }

    /// WHY: [`Reactor::register`] is byte-transparent — the fd's bytes stay in
    /// the socket and the caller reads them. Completion mode is opted into here,
    /// per registration, so no existing `Poll` user has the kernel start
    /// draining its sockets behind its back.
    ///
    /// WHAT: register `fd` and, on the completion backend, arm a multishot
    /// `RECV`. Returns whether the kernel will now read this fd for us.
    ///
    /// HOW: `false` on every backend without an inbox, and on fds that cannot
    /// receive (non-sockets, listeners, write-only interests). The caller must
    /// keep its own read path for those.
    ///
    /// # Errors
    /// The selector's `io::Error` if registration fails.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn register_completion(
        &self,
        fd: std::os::fd::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<bool> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        let armed = self.poll.register_recv_fd(fd, token, interest)?;
        self.entries.write().expect("entries lock poisoned").insert(
            token,
            Entry {
                fd,
                ready: AtomicU8::new(Ready::EMPTY.bits()),
                interest,
                waker: Mutex::new(None),
                tasks: Arc::new(ReadinessNotifier::new()),
            },
        );
        // The latest `register_completion` owns the number, as in `register`.
        self.fd_tokens
            .lock()
            .expect("fd_tokens lock poisoned")
            .insert(fd, token);
        Ok(armed)
    }

    /// WHY: a closed connection must stop consuming a selector slot and a cache
    /// entry. The selector is keyed by fd number alone, so a late call — after
    /// the number was closed and reused — must not remove the new socket's
    /// registration. See the `fd_tokens` map.
    ///
    /// WHAT: deregister `fd` and drop its readiness tracking, but only when the
    /// number still belongs to `token`.
    ///
    /// HOW: always remove the token's cache entry. Then compare `fd`'s live
    /// token with `token`: only a match removes the mapping and calls the
    /// selector's deregister. A mismatch is a stale drop after fd reuse; it is
    /// logged at debug level and the selector is left untouched. `ENOENT` from
    /// the selector is expected (the fd may already be closed, e.g. before
    /// deregistration) and is treated as success.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` for any failure other than `ENOENT`.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn deregister(&self, fd: std::os::fd::RawFd, token: Token) -> io::Result<()> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        self.entries
            .write()
            .expect("entries lock poisoned")
            .remove(&token);

        let owns_number = {
            let mut fd_tokens = self.fd_tokens.lock().expect("fd_tokens lock poisoned");
            match fd_tokens.get(&fd).copied() {
                Some(live) if live == token => {
                    fd_tokens.remove(&fd);
                    true
                }
                Some(live) => {
                    tracing::debug!(
                        fd,
                        stale_token = ?token,
                        live_token = ?live,
                        "fd number was reused; keeping the live registration"
                    );
                    false
                }
                // No live registration for this number (for example, the
                // selector already forgot it), so there is nothing to remove.
                None => false,
            }
        };
        if !owns_number {
            return Ok(());
        }

        if let Err(e) = self.registry.deregister_fd(fd) {
            if e.raw_os_error() != Some(libc::ENOENT) {
                return Err(e);
            }
        }
        Ok(())
    }

    /// WHY: the `EventReadiness` check runs on every scheduler pass, so it must
    /// not syscall.
    ///
    /// WHAT: whether `token` has any latched readiness.
    ///
    /// HOW: an atomic load behind a read lock. Unregistered tokens are not
    /// ready.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn is_ready(&self, token: Token) -> bool {
        !self.readiness(token).is_empty()
    }

    /// WHY: `poll_readable` must distinguish READABLE from `READ_CLOSED/ERROR`;
    /// collapsing them loses EOF and error detection.
    ///
    /// WHAT: the latched readiness flags for `token`.
    ///
    /// HOW: an atomic load behind a read lock. Unregistered tokens read
    /// `Ready::EMPTY`.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn readiness(&self, token: Token) -> Ready {
        self.entries
            .read()
            .expect("entries lock poisoned")
            .get(&token)
            .map_or(Ready::EMPTY, |e| {
                Ready::from_bits(e.ready.load(Ordering::Acquire))
            })
    }

    /// WHY: edge-triggered registration reports each transition once. A
    /// consumer that has drained the fd to `WouldBlock` must clear the latched
    /// bit, or `is_ready()` stays true forever and the task spins.
    ///
    /// WHAT: clear the flags in `ready` from `token`'s latched readiness.
    ///
    /// HOW: `fetch_and` of the complement — atomic, and safe against a
    /// concurrent `fetch_or` from the drain thread (worst case: a bit the drain
    /// thread just set survives, producing a spurious wake).
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn clear(&self, token: Token, ready: Ready) {
        let fd = {
            let entries = self.entries.read().expect("entries lock poisoned");
            let Some(entry) = entries.get(&token) else {
                return;
            };
            entry.ready.fetch_and(!ready.bits(), Ordering::AcqRel);
            // Clearing readiness also disarms any armed waker. The register-
            // before-check protocol re-arms it on the next park, so a stale
            // edge cannot re-wake a reader that has already drained the fd.
            *entry.waker.lock().expect("waker lock poisoned") = None;
            entry.fd
        };
        // A consumer that cleared READABLE has drained the fd, so re-arm its
        // read edge for the next arrival. A no-op on backends that re-arm
        // themselves (epoll/io_uring).
        if ready.is_readable() {
            self.poll.rearm_readable(fd);
        }
    }

    /// WHY: a native-executor task (tokio/smol) cannot park on the valtron
    /// `EventReadiness` path; it parks by returning `Pending` and being woken
    /// by the `std::task::Waker` it hands its executor. This is the reactor
    /// side of that contract (Feature 29 / Feature 03).
    ///
    /// WHAT: store `waker` for `token` so the drain thread wakes it after the
    /// next readiness edge. Returns whether `token` is still registered.
    ///
    /// HOW: re-arm the selector's read edge, then store the waker in an
    /// interior-mutable slot on the cache entry. The re-arm lives here, so a
    /// caller cannot forget it. Callers still follow the register-before-check
    /// protocol: register the waker, then read once more before parking. Clear
    /// readiness (which disarms the waker) before the next read so a stale
    /// latched flag does not re-wake forever.
    ///
    /// # Panics
    /// Panics if the registration map lock or the waker slot is poisoned.
    pub fn register_waker(&self, token: Token, waker: Waker) -> bool {
        if let Some(entry) = self
            .entries
            .read()
            .expect("entries lock poisoned")
            .get(&token)
        {
            // Forget the last delivered read event. The kqueue selector
            // suppresses a readable event when the fd's byte count did not
            // grow, so a drained fd that receives a smaller frame produces no
            // new edge until it is re-armed. A no-op on epoll and io_uring.
            self.poll.rearm_readable(entry.fd);
            *entry.waker.lock().expect("waker lock poisoned") = Some(waker);
            true
        } else {
            false
        }
    }

    /// Register the waker of a valtron task that parks on `token`'s readiness.
    ///
    /// WHAT: the drain thread wakes it (only that task's executor) every time it
    /// latches readiness for `token`. Unlike [`Reactor::register_waker`] it is not
    /// disarmed by [`Reactor::clear`]: a task registers again on every park, and a
    /// waker of a task that runs again is a no-op. Returns whether `token` is
    /// still registered.
    ///
    /// HOW: re-arm the selector's read edge before the task parks, for the same
    /// reason as [`Reactor::register_waker`]. The re-arm is a no-op on epoll and
    /// `io_uring`.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn register_task_waker(&self, token: Token, waker: &ReadinessWaker) -> bool {
        match self
            .entries
            .read()
            .expect("entries lock poisoned")
            .get(&token)
        {
            Some(entry) => {
                self.poll.rearm_readable(entry.fd);
                entry.tasks.register(waker)
            }
            None => false,
        }
    }

    /// WHY: completion mode's whole point — the bytes are already here, so the
    /// transport takes them instead of calling `read(2)` (Decision 14 F4).
    ///
    /// WHAT: drain the kernel-delivered buffers queued for `token`.
    ///
    /// HOW: forwards to the selector's inbox. Returns `None` on backends without
    /// one (epoll, kqueue, io_uring readiness mode), which tells the caller to
    /// read the fd itself.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn take_completions(
        &self,
        token: Token,
    ) -> Option<Vec<crate::poll::sys::unix::selector::uring_completion::Completion>> {
        self.poll.take_completions(token)
    }

    /// Whether `token` has kernel-delivered bytes waiting.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_completions(&self, token: Token) -> bool {
        self.poll.has_completions(token)
    }

    /// Whether `token`'s bytes arrive as completions rather than needing a read.
    ///
    /// Non-destructive, unlike [`Reactor::take_completions`].
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn is_recv_token(&self, token: Token) -> bool {
        self.poll.is_recv_token(token)
    }

    /// WHY: the SEND mirror of [`Reactor::take_completions`] — a write costs no
    /// `write(2)`; the kernel copies from an owned buffer and reports later (F49).
    ///
    /// WHAT: submit an `IORING_OP_SEND` of up to one pool buffer of `data` for
    /// `token`/`fd`; returns the bytes taken (a short write on overflow).
    ///
    /// # Errors
    /// `WouldBlock` when the send pool is exhausted; `Unsupported` off the
    /// completion backend; the kernel's submission error otherwise.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send(
        &self,
        token: Token,
        fd: std::os::fd::RawFd,
        data: &[u8],
    ) -> io::Result<usize> {
        self.poll.submit_send(token, fd, data)
    }

    /// Zero-copy send: submit `IORING_OP_SEND_ZC` for `token`/`fd` (F50 Part C).
    /// The kernel sends directly from the pinned pool buffer without copying it
    /// into the socket send buffer.
    ///
    /// # Errors
    /// `WouldBlock` when the send pool is exhausted; `Unsupported` off the
    /// completion backend; the kernel's submission error otherwise.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send_zc(
        &self,
        token: Token,
        fd: std::os::fd::RawFd,
        data: &[u8],
    ) -> io::Result<usize> {
        self.poll.submit_send_zc(token, fd, data)
    }

    /// Zero-copy send from an owned `ProvidedBuf` — the direct handoff (F50 Part C
    /// tail). The buffer's backing memory (in the RECV ring) goes straight to the
    /// kernel with no intermediate copy.
    ///
    /// # Errors
    /// The kernel's submission error; `Unsupported` off the completion backend.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send_zc_direct(
        &self,
        token: Token,
        fd: std::os::fd::RawFd,
        buf: crate::poll::sys::unix::selector::bufring::ProvidedBuf,
    ) -> io::Result<usize> {
        self.poll.submit_send_zc_direct(token, fd, buf)
    }

    /// Take finished sends for `token` (drained by a transport's `flush`).
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn take_send_completions(
        &self,
        token: Token,
    ) -> Vec<crate::poll::sys::unix::selector::uring_completion::SendCompletion> {
        self.poll.take_send_completions(token)
    }

    /// Whether finished sends are waiting for `token`.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_send_completions(&self, token: Token) -> bool {
        self.poll.has_send_completions(token)
    }

    /// Whether `token` has an unfinished SEND in flight — a transport's `flush`
    /// parks while this is true.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_pending_sends(&self, token: Token) -> bool {
        self.poll.has_pending_sends(token)
    }

    /// Whether this reactor delivers bytes through completions rather than
    /// readiness.
    ///
    /// # Panics
    /// Never panics.
    pub fn is_completion_mode(&self) -> bool {
        self.backend() == Backend::UringCompletion
    }

    /// The current event generation (F50 B2). Snapshot this *before* reading your
    /// registered fds; pass it to [`Reactor::wait_for_events`] to block only if the
    /// reactor has observed no new events since — the lost-wakeup guard.
    ///
    /// # Panics
    /// Never panics (poisoned lock aside).
    #[must_use]
    pub fn events_generation(&self) -> u64 {
        *self.events_gen.0.lock().expect("events_gen lock poisoned")
    }

    /// Block the calling thread until the reactor latches new readiness (its
    /// generation advances past `since`) or `timeout` elapses; returns the
    /// generation observed on wake. Lets a **non-task** thread — e.g. a proxy
    /// splice — park on reactor activity instead of a fixed sleep, without
    /// busy-polling (F50 B2). A caller loops:
    ///
    /// ```ignore
    /// let gen = reactor.events_generation();
    /// // ... read both registered fds; if data moved, continue ...
    /// reactor.wait_for_events(gen, Duration::from_millis(50));
    /// ```
    ///
    /// Because the generation is snapshotted before the fds are read, an event
    /// that lands in the race window bumps the generation and this returns at once
    /// rather than sleeping through it.
    ///
    /// # Panics
    /// Never panics (poisoned lock aside).
    pub fn wait_for_events(&self, since: u64, timeout: Duration) -> u64 {
        let (lock, cvar) = &*self.events_gen;
        let gen = lock.lock().expect("events_gen lock poisoned");
        if *gen != since {
            return *gen;
        }
        let (gen, _timed_out) = cvar
            .wait_timeout(gen, timeout)
            .expect("events_gen lock poisoned");
        *gen
    }

    /// Wake every thread parked in [`Reactor::wait_for_events`] without a real
    /// fd event.
    ///
    /// WHY: a non-task thread (e.g. the accept loop) parks on this reactor's
    /// generation condvar. Some external condition — a shutdown signal — can
    /// change without any registered fd becoming ready, so the drain thread
    /// never bumps the generation and the parked thread sleeps out its timeout.
    ///
    /// WHAT: simulate a synthetic event: advance the generation and notify all
    /// waiters. A waiter's lost-wakeup guard then returns at once and it
    /// re-checks its own condition.
    ///
    /// HOW: the same generation bump and `notify_all` the drain thread performs
    /// when it latches readiness (see [`Reactor::wait_for_events`]).
    ///
    /// # Panics
    /// Never panics (poisoned lock aside).
    pub fn wake_waiters(&self) {
        let (lock, cvar) = &*self.events_gen;
        let mut gen = lock.lock().expect("events_gen lock poisoned");
        *gen = gen.wrapping_add(1);
        cvar.notify_all();
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        // Tell the drain loop to stop, then join it. Dropping a `JoinHandle`
        // only detaches the thread: before this join, a loser of the
        // initialisation race could leave a `reactor-drain` thread polling an
        // empty selector (issue #240).
        self.shutdown.store(true, Ordering::SeqCst);
        // Release any thread parked in `wait_for_events`. The drain loop itself
        // waits in `poll()`, not on this condvar, so it exits when the poll
        // timeout expires; joining below waits at most `drain_poll_timeout`.
        self.wake_waiters();
        if let Some(drain) = self.drain.take() {
            if drain.join().is_err() {
                tracing::warn!("reactor drain thread panicked during shutdown");
            }
        }
    }
}

/// Whether `running` satisfies `preference`.
///
/// `Auto` matches anything — the caller deferred to the probe ladder.
/// `Uring` matches both readiness and completion modes.
/// `Epoll` matches only epoll.
fn backend_matches(running: Backend, preference: BackendPreference) -> bool {
    match preference {
        BackendPreference::Auto => true,
        BackendPreference::Uring => running.is_uring(),
        BackendPreference::Epoll => running == Backend::Epoll,
    }
}

/// The drain loop: blocks in `poll()`, latches readiness bits for signalled tokens.
///
/// Takes the reactor's own `Poll` by shared reference — a second selector here
/// would receive no registrations and no wake would ever fire.
fn drain_loop(
    poll: &Poll,
    entries: &Arc<RwLock<HashMap<Token, Entry>>>,
    shutdown: &AtomicBool,
    events_gen: &Arc<(Mutex<u64>, Condvar)>,
    timeout: Duration,
) {
    let mut events = Events::with_capacity(1024);
    while !shutdown.load(Ordering::Acquire) {
        events.clear();
        match poll.poll(&mut events, Some(timeout)) {
            Ok(()) => {
                let mut latched = 0usize;
                // Wakers armed via `register_waker` are woken *after* the read
                // lock drops, alongside the valtron resume, so a wake never
                // re-enters the reactor while it holds the entry map lock.
                let mut to_wake: Vec<Waker> = Vec::new();
                let mut tasks_to_wake: Vec<Arc<ReadinessNotifier>> = Vec::new();
                {
                    // Read lock only: bits are latched with an atomic OR.
                    let entries = entries.read().expect("entries lock poisoned");
                    for event in events.iter() {
                        let mut ready = Ready::EMPTY;
                        if event.is_readable() {
                            ready = ready.union(Ready::READABLE);
                        }
                        if event.is_writable() {
                            ready = ready.union(Ready::WRITABLE);
                        }
                        if event.is_read_closed() {
                            ready = ready.union(Ready::READ_CLOSED);
                        }
                        if event.is_write_closed() {
                            ready = ready.union(Ready::WRITE_CLOSED);
                        }
                        if event.is_error() {
                            ready = ready.union(Ready::ERROR);
                        }

                        if let Some(entry) = entries.get(&event.token()) {
                            entry.ready.fetch_or(ready.bits(), Ordering::AcqRel);
                            if entry.tasks.has_waiters() {
                                tasks_to_wake.push(Arc::clone(&entry.tasks));
                            }
                            if let Some(waker) =
                                entry.waker.lock().expect("waker lock poisoned").as_ref()
                            {
                                to_wake.push(waker.clone());
                            }
                        }
                        latched += 1;
                    }
                }
                // Publish a new event generation *after* the readiness bits are
                // latched, so any thread woken here sees them. Notify parked
                // non-task waiters (F50 B2). Only on real events — a bare poll
                // timeout latched nothing.
                if latched > 0 {
                    {
                        let (lock, cvar) = &**events_gen;
                        let mut gen = lock.lock().expect("events_gen lock poisoned");
                        *gen = gen.wrapping_add(1);
                        cvar.notify_all();
                    }
                    // Wake `std::task::Waker`-registered readers (native
                    // executor tasks parked on `Pending`) before the valtron
                    // tasks, so both paths observe the latched readiness.
                    for waker in to_wake.drain(..) {
                        waker.wake();
                    }
                    // Tasks parked on `Depends(reactor readiness)`: wake each one,
                    // and only its executor, now that the readiness is latched.
                    for tasks in tasks_to_wake.drain(..) {
                        tasks.notify();
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                tracing::warn!(error = %e, "reactor drain poll failed; retrying");
            }
        }
    }
}

/// Wrap a raw fd into an `EventReadiness` backed by the shared reactor.
pub struct SharedReadiness {
    reactor: Arc<Reactor>,
    token: Token,
}

impl std::fmt::Debug for SharedReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedReadiness")
            .field("token", &self.token)
            .field("ready", &self.reactor.readiness(self.token))
            .finish()
    }
}

impl SharedReadiness {
    /// WHY: lets a caller park on an fd without owning an `FdRegistration`.
    ///
    /// WHAT: register `fd` with the shared reactor and return an
    /// `EventReadiness` view of its latched readiness.
    ///
    /// HOW: delegates to [`Reactor::register`].
    ///
    /// # Errors
    /// Returns the selector's `io::Error` if registration fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(
        fd: std::os::fd::RawFd,
        reactor: Arc<Reactor>,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        reactor.register(fd, token, interest)?;
        Ok(Self { reactor, token })
    }

    /// WHY: an edge consumed without clearing latches the entry ready forever.
    ///
    /// WHAT: clear `ready` from this token's latched readiness.
    ///
    /// HOW: delegates to [`Reactor::clear`].
    ///
    /// # Panics
    /// Never panics.
    pub fn clear(&self, ready: Ready) {
        self.reactor.clear(self.token, ready);
    }
}

impl foundation_core::valtron::EventReadiness for SharedReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.reactor.is_ready(self.token)
    }

    /// Readiness is latched only by the reactor's event drain, which wakes the
    /// registered tasks after every batch of events. Registration also re-arms
    /// the selector's read edge, so a task gets the next edge without clearing
    /// the latched flags first; the clear is still needed for the flags
    /// themselves.
    fn register(&self, waker: &foundation_core::valtron::ReadinessWaker) -> bool {
        self.reactor.register_task_waker(self.token, waker)
    }
}
