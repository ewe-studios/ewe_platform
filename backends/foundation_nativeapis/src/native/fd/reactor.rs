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
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::native::fd::Ready;
use crate::native::poll::{Backend, Events, Interest, Poll, Registry, Token};

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

        let poll = Arc::new(Poll::new()?);
        let registry = poll.registry();
        let shutdown = Arc::new(AtomicBool::new(false));

        let entries: Arc<RwLock<HashMap<Token, Entry>>> = Arc::new(RwLock::new(HashMap::new()));

        // The drain thread polls the *same* selector registrations land in.
        // Handing it a second `Poll::new()` here silently breaks every wake.
        let drain_poll = Arc::clone(&poll);
        let drain_entries = Arc::clone(&entries);
        let drain_shutdown = Arc::clone(&shutdown);

        let drain = thread::Builder::new()
            .name("reactor-drain".into())
            .spawn(move || {
                drain_loop(&drain_poll, &drain_entries, &drain_shutdown);
            })?;

        let reactor = Arc::new(Reactor {
            poll,
            registry,
            entries,
            reg_lock: Mutex::new(()),
            _drain: drain,
            shutdown,
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
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
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
) {
    let mut events = Events::with_capacity(1024);
    while !shutdown.load(Ordering::Acquire) {
        events.clear();
        match poll.poll(&mut events, Some(Duration::from_millis(DRAIN_TIMEOUT_MS))) {
            Ok(()) => {
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
