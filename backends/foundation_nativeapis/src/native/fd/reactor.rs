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
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::native::fd::Ready;
use crate::native::poll::{Backend, BackendPreference, Events, Interest, Poll, Registry, Token};

/// Per-registration readiness tracking.
///
/// `ready` is atomic so the drain thread can OR bits in, and consumers can mask
/// bits out, while both hold only a *read* lock on the entry map.
struct Entry {
    ready: AtomicU8,
    #[allow(dead_code)]
    interest: Interest,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("ready", &Ready::from_bits(self.ready.load(Ordering::Acquire)))
            .field("interest", &self.interest)
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
    /// Registration lock — serialises `epoll_ctl` calls.
    reg_lock: Mutex<()>,
    /// Drain thread.
    _drain: thread::JoinHandle<()>,
    /// Signal the drain thread to stop.
    shutdown: Arc<AtomicBool>,
    /// Event generation + condvar (F50 B2). The drain thread bumps the counter and
    /// notifies after each batch of latched readiness, so a *non-task* thread (e.g.
    /// a proxy splice) can block until the reactor observes new events instead of
    /// busy-polling. The generation makes the wait lost-wakeup-safe: a waiter that
    /// snapshots the count before reading its fds only sleeps if nothing has
    /// happened since.
    events_gen: Arc<(Mutex<u64>, Condvar)>,
}

impl std::fmt::Debug for Reactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reactor")
            .field("registered", &self.entries.read().map(|e| e.len()).unwrap_or(0))
            .field("shutdown", &self.shutdown.load(Ordering::Acquire))
            .finish()
    }
}

/// Global singleton.
static REACTOR: OnceLock<Arc<Reactor>> = OnceLock::new();

/// Error returned by [`Reactor::init`] when a reactor is already running on
/// a different backend than the one requested.
///
/// WHY: the reactor is process-global (D14 OQ#14.1). Two servers in one process
/// asking for different backends cannot both be satisfied, and silently giving
/// the second the first's backend defeats the point of explicit selection — a
/// perf-critical deploy would discover from latency graphs that it never got
/// io_uring. This error makes that failure a startup crash with both backends
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

/// Poll timeout for the drain thread. Bounds shutdown latency only — a real
/// event returns from `poll()` immediately.
const DRAIN_TIMEOUT_MS: u64 = 100;

impl Reactor {
    /// WHY: every `FdRegistration` parks on this one instance instead of
    /// building a private epoll fd per socket.
    ///
    /// WHAT: get or initialise the shared reactor.
    ///
    /// HOW: `OnceLock` doesn't support fallible init, so this checks for an
    /// existing instance, otherwise builds one and races to `set()` it; a
    /// loser of that race drops its instance and returns the winner's.
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
        Self::init_inner(BackendPreference::Auto)
    }

    /// WHY: an operator must be able to demand a specific backend (F48 —
    /// `ServerIo::Completion` needs `Reactor::init(Uring)`), and must be told
    /// when that demand cannot be met. `get()` always auto-selects, silently
    /// falling back to epoll on a host without io_uring. That silent fallback is
    /// precisely the no-silent-defaults failure mode Decision 14 OQ#14.3 calls
    /// "the worst."
    ///
    /// WHAT: initialise the shared reactor on a specific backend. First call
    /// wins; the backend is a process-level decision.
    ///
    /// HOW: [`BackendPreference::Auto`] walks the probe ladder.
    /// [`BackendPreference::Uring`] fails hard with the probe detail if io_uring
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
        if let Some(existing) = REACTOR.get() {
            let running = existing.backend();
            // If the running backend matches the preference, return it.
            // Mismatch: the caller asked for something different than what is
            // already running.
            if !backend_matches(running, preference) {
                return Err(AlreadyInitialised { running, requested: preference }.into());
            }
            tracing::debug!(
                running = %running,
                ?preference,
                "reactor already initialised on a matching backend; returning existing instance"
            );
            return Ok(Arc::clone(existing));
        }
        Self::init_inner(preference)
    }

    /// Shared init body — builds the reactor and races to install it.
    fn init_inner(preference: BackendPreference) -> io::Result<Arc<Self>> {
        let poll = Arc::new(Self::make_poll(preference)?);
        let registry = poll.registry();
        let shutdown = Arc::new(AtomicBool::new(false));

        let entries: Arc<RwLock<HashMap<Token, Entry>>> = Arc::new(RwLock::new(HashMap::new()));

        // The drain thread polls the *same* selector registrations land in.
        // Handing it a second `Poll` instance here silently breaks every wake.
        let events_gen: Arc<(Mutex<u64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));

        let drain_poll = Arc::clone(&poll);
        let drain_entries = Arc::clone(&entries);
        let drain_shutdown = Arc::clone(&shutdown);
        let drain_events_gen = Arc::clone(&events_gen);

        let drain = thread::Builder::new()
            .name("reactor-drain".into())
            .spawn(move || {
                drain_loop(&drain_poll, &drain_entries, &drain_shutdown, &drain_events_gen);
            })?;

        let reactor = Arc::new(Reactor {
            poll,
            registry,
            entries,
            reg_lock: Mutex::new(()),
            _drain: drain,
            shutdown,
            events_gen,
        });

        match REACTOR.set(Arc::clone(&reactor)) {
            Ok(()) => {
                tracing::info!(backend = ?reactor.backend(), "shared fd reactor initialised");
                Ok(reactor)
            }
            // Lost the init race — the winner's instance is authoritative.
            Err(_) => Ok(Arc::clone(REACTOR.get().expect("set() raced, so it is populated"))),
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
                    "backend {:?} is not available on this platform; only Auto (kqueue) is supported",
                    preference,
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

    /// WHY: one `epoll_ctl` per connection, into the shared selector.
    ///
    /// WHAT: register `fd` under `token` with `interest`, and start tracking
    /// its readiness.
    ///
    /// HOW: takes `reg_lock` to serialise selector mutation, registers, then
    /// installs a zeroed cache entry.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` if registration fails; the cache entry
    /// is not installed in that case.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn register(&self, fd: std::os::fd::RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        self.registry.register_fd(fd, token, interest)?;
        self.entries.write().expect("entries lock poisoned").insert(
            token,
            Entry { ready: AtomicU8::new(Ready::EMPTY.bits()), interest },
        );
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
            Entry { ready: AtomicU8::new(Ready::EMPTY.bits()), interest },
        );
        Ok(armed)
    }

    /// WHY: a closed connection must stop consuming a selector slot and a cache
    /// entry.
    ///
    /// WHAT: deregister `fd` and drop its readiness tracking.
    ///
    /// HOW: `ENOENT` from the selector is expected (the fd may already be gone,
    /// e.g. closed before deregistration) and is treated as success.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` for any failure other than `ENOENT`.
    ///
    /// # Panics
    /// Panics if the registration map lock is poisoned.
    pub fn deregister(&self, fd: std::os::fd::RawFd, token: Token) -> io::Result<()> {
        let _guard = self.reg_lock.lock().expect("reg_lock poisoned");
        if let Err(e) = self.registry.deregister_fd(fd) {
            if e.raw_os_error() != Some(libc::ENOENT) {
                return Err(e);
            }
        }
        self.entries.write().expect("entries lock poisoned").remove(&token);
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

    /// WHY: `poll_readable` must distinguish READABLE from READ_CLOSED/ERROR;
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
            .map(|e| Ready::from_bits(e.ready.load(Ordering::Acquire)))
            .unwrap_or(Ready::EMPTY)
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
        if let Some(entry) = self.entries.read().expect("entries lock poisoned").get(&token) {
            entry.ready.fetch_and(!ready.bits(), Ordering::AcqRel);
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
    pub fn take_completions(&self, token: Token) -> Option<Vec<super::completion::Completion>> {
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
    pub fn submit_send(&self, token: Token, fd: std::os::fd::RawFd, data: &[u8]) -> io::Result<usize> {
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
    pub fn submit_send_zc(&self, token: Token, fd: std::os::fd::RawFd, data: &[u8]) -> io::Result<usize> {
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
        buf: crate::native::poll::sys::unix::selector::bufring::ProvidedBuf,
    ) -> io::Result<usize> {
        self.poll.submit_send_zc_direct(token, fd, buf)
    }

    /// Take finished sends for `token` (drained by a transport's `flush`).
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn take_send_completions(&self, token: Token) -> Vec<super::completion::SendCompletion> {
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
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
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
) {
    let mut events = Events::with_capacity(1024);
    while !shutdown.load(Ordering::Acquire) {
        events.clear();
        match poll.poll(&mut events, Some(Duration::from_millis(DRAIN_TIMEOUT_MS))) {
            Ok(()) => {
                let mut latched = 0usize;
                {
                    // Read lock only: bits are latched with an atomic OR.
                    let entries = entries.read().expect("entries lock poisoned");
                    for event in events.iter() {
                        let mut ready = Ready::EMPTY;
                        if event.is_readable() { ready = ready.union(Ready::READABLE); }
                        if event.is_writable() { ready = ready.union(Ready::WRITABLE); }
                        if event.is_read_closed() { ready = ready.union(Ready::READ_CLOSED); }
                        if event.is_write_closed() { ready = ready.union(Ready::WRITE_CLOSED); }
                        if event.is_error() { ready = ready.union(Ready::ERROR); }

                        if let Some(entry) = entries.get(&event.token()) {
                            entry.ready.fetch_or(ready.bits(), Ordering::AcqRel);
                        }
                        latched += 1;
                    }
                }
                // Publish a new event generation *after* the readiness bits are
                // latched, so any thread woken here sees them. Notify parked
                // non-task waiters (F50 B2). Only on real events — a bare poll
                // timeout latched nothing.
                if latched > 0 {
                    let (lock, cvar) = &**events_gen;
                    let mut gen = lock.lock().expect("events_gen lock poisoned");
                    *gen = gen.wrapping_add(1);
                    cvar.notify_all();
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
    pub fn new(fd: std::os::fd::RawFd, reactor: Arc<Reactor>, token: Token, interest: Interest) -> io::Result<Self> {
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
}
