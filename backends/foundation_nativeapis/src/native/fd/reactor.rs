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
//! `poll(timeout)`, and on each event sets the token's readiness bits.
//! Tasks parked via `TaskStatus::Depends(event_readiness)` are re-polled by
//! the executor when `is_ready()` returns true.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::native::poll::{Events, Interest, Poll, Registry, Token};
use crate::native::fd::Ready;

/// Per-registration readiness tracking.
struct Entry {
    ready: Ready,
    interest: Interest,
}

/// The shared process-level reactor singleton.
pub struct Reactor {
    poll: Poll,
    registry: Registry,
    /// Token → readiness cache. Arc'd so the drain thread holds a ref.
    entries: Arc<RwLock<HashMap<Token, Entry>>>,
    /// Registration lock — serialises epoll_ctl calls.
    reg_lock: Mutex<()>,
    /// Drain thread.
    _drain: thread::JoinHandle<()>,
    /// Signal the drain thread to stop.
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

/// Global singleton.
static REACTOR: OnceLock<Arc<Reactor>> = OnceLock::new();

/// Poll timeout for the drain thread.
const DRAIN_TIMEOUT_MS: u64 = 100;

impl Reactor {
    /// Get or initialise the shared reactor.
    /// Returns `Err` if the reactor could not be created (unsupported platform).
    pub fn get() -> io::Result<Arc<Self>> {
        // get_or_init doesn't support Result, so we use a two-phase approach:
        // if already initialised, return it. Otherwise, init and store.
        if let Some(reactor) = REACTOR.get() {
            return Ok(Arc::clone(reactor));
        }

        let poll = Poll::new()?;
        let registry = poll.registry();
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let entries = Arc::new(RwLock::new(HashMap::new()));
        let drain_entries = entries.clone();
        let drain_poll = Poll::new()?;
        let drain_shutdown = shutdown.clone();

        let drain = thread::Builder::new()
            .name("reactor-drain".into())
            .spawn(move || {
                drain_loop(drain_poll, &drain_entries, &drain_shutdown);
            })?;

        let reactor = Arc::new(Reactor {
            poll,
            registry,
            entries,
            reg_lock: Mutex::new(()),
            _drain: drain,
            shutdown,
        });

        // set() returns Err(reactor) if already initialised (race).
        // In that case, just return the already-initialised instance.
        match REACTOR.set(Arc::clone(&reactor)) {
            Ok(()) => Ok(reactor),
            Err(_) => Ok(Arc::clone(REACTOR.get().unwrap())),
        }
    }

    /// Access the shared Registry (for direct epoll ops).
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Register a raw fd with the shared selector and readiness cache.
    pub fn register(&self, fd: std::os::fd::RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let _guard = self.reg_lock.lock().unwrap();
        self.registry.register_fd(fd, token, interest)?;
        self.entries.write().unwrap().insert(
            token,
            Entry { ready: Ready::EMPTY, interest },
        );
        Ok(())
    }

    /// Deregister a raw fd and remove its readiness tracking.
    pub fn deregister(&self, fd: std::os::fd::RawFd, token: Token) -> io::Result<()> {
        let _guard = self.reg_lock.lock().unwrap();
        // ENOENT is expected if the fd was already removed.
        if let Err(e) = self.registry.deregister_fd(fd) {
            if e.raw_os_error() != Some(libc::ENOENT) {
                return Err(e);
            }
        }
        self.entries.write().unwrap().remove(&token);
        Ok(())
    }

    /// Check whether `token` is ready — reads cached readiness, zero syscalls.
    pub fn is_ready(&self, token: Token) -> bool {
        self.entries.read().unwrap()
            .get(&token)
            .map(|e| !e.ready.is_empty())
            .unwrap_or(false)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The drain loop: blocks in poll(), updates readiness bits for signaled tokens.
fn drain_loop(
    poll: Poll,
    entries: &Arc<RwLock<HashMap<Token, Entry>>>,
    shutdown: &std::sync::atomic::AtomicBool,
) {
    let mut events = Events::with_capacity(1024);
    loop {
        if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        events.clear();
        match poll.poll(&mut events, Some(Duration::from_millis(DRAIN_TIMEOUT_MS))) {
            Ok(()) => {
                let mut entries = entries.write().unwrap();
                for event in events.iter() {
                    let mut ready = Ready::EMPTY;
                    if event.is_readable() { ready = ready.union(Ready::READABLE); }
                    if event.is_writable() { ready = ready.union(Ready::WRITABLE); }
                    if event.is_read_closed() { ready = ready.union(Ready::READ_CLOSED); }
                    if event.is_write_closed() { ready = ready.union(Ready::WRITE_CLOSED); }
                    if event.is_error() { ready = ready.union(Ready::ERROR); }
                    if let Some(entry) = entries.get_mut(&event.token()) {
                        entry.ready = entry.ready.union(ready);
                    }
                }
            }
            Err(_) => {
                // EINTR etc. — retry.
            }
        }
    }
}

/// Convenience: wrap a raw fd into an `EventReadiness` via the shared reactor.
pub struct SharedReadiness {
    reactor: Arc<Reactor>,
    token: Token,
}

impl SharedReadiness {
    pub fn new(fd: std::os::fd::RawFd, reactor: Arc<Reactor>, token: Token, interest: Interest) -> io::Result<Self> {
        reactor.register(fd, token, interest)?;
        Ok(Self { reactor, token })
    }
}

impl foundation_core::valtron::EventReadiness for SharedReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.reactor.is_ready(self.token)
    }
}
