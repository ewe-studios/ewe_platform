//! Shared process-level reactor (F40 — Decision 14 §Scope 3).
//!
//! WHY: Before this, every `FdRegistration` created its own private `Poll`
//! selector — one epoll fd per registered socket. At thousands of connections
//! that's thousands of epoll fds plus a zero-timeout `epoll_wait` syscall per
//! `is_ready()` check. A single shared epoll/kqueue fd eliminates this.
//!
//! WHAT: [`Reactor`] — a `OnceLock` singleton holding one platform selector
//! and a concurrent `Token → Ready` map. One drain thread services all
//! registered fds. `is_ready()` reads a cached atomic bit — zero syscalls.
//!
//! HOW: On first access, creates the selector and spawns the drain thread.
//! Registration calls `epoll_ctl(ADD)`. The drain thread blocks in
//! `epoll_wait(timeout)`, and on each event sets the token's readiness bits.
//! Tasks parked via `TaskStatus::Depends(event_readiness)` are re-polled by
//! the executor when `is_ready()` returns true.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::native::poll::{Events, Interest, Token};
use crate::native::poll::sys::Selector;
use crate::native::fd::Ready;

/// Per-registration readiness tracking.
struct Entry {
    /// Current readiness bits (atomically readable without lock).
    ready: Ready,
    /// Epoll interest for re-registration after one-shot fires.
    interest: Interest,
}

/// The shared process-level reactor singleton.
pub struct Reactor {
    selector: Arc<Selector>,
    /// Token → readiness cache. Locked on registration and drain.
    entries: RwLock<HashMap<Token, Entry>>,
    /// Registration lock — serialises epoll_ctl calls.
    reg_lock: Mutex<()>,
    /// Drain thread handle.
    _drain: thread::JoinHandle<()>,
    /// Signal to stop the drain thread.
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

/// Global reactor instance.
static REACTOR: OnceLock<Arc<Reactor>> = OnceLock::new();

/// Poll timeout for the drain thread.
const DRAIN_TIMEOUT_MS: u64 = 100;

impl Reactor {
    /// Get (or initialise) the shared reactor singleton.
    pub fn get() -> io::Result<Arc<Self>> {
        REACTOR
            .get_or_try_init(|| {
                let (selector, _registry) = Selector::new_with_registry()?;
                let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

                // Spawn the drain thread. We need a clone of selector + shutdown
                // that live as long as the thread.
                let drain_selector = selector.clone();
                let drain_shutdown = shutdown.clone();
                let entries: Arc<RwLock<HashMap<Token, Entry>>> =
                    Arc::new(RwLock::new(HashMap::new()));
                let drain_entries = entries.clone();

                let drain = thread::Builder::new()
                    .name("reactor-drain".into())
                    .spawn(move || {
                        drain_loop(&drain_selector, &drain_entries, &drain_shutdown);
                    })
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                Ok(Arc::new(Reactor {
                    selector,
                    entries,
                    reg_lock: Mutex::new(()),
                    _drain: drain,
                    shutdown,
                }))
            })
            .map(Arc::clone)
    }

    /// Register a file descriptor with the shared selector.
    pub fn register(
        &self,
        fd: std::os::unix::io::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        let _guard = self.reg_lock.lock().unwrap();

        // Register with epoll (edge-triggered + oneshot so we only get one event
        // per readiness transition).
        let mut epoll_interest = 0i32;
        if interest.is_readable() {
            epoll_interest |= libc::EPOLLIN | libc::EPOLLRDHUP;
        }
        if interest.is_writable() {
            epoll_interest |= libc::EPOLLOUT;
        }
        epoll_interest |= libc::EPOLLET | libc::EPOLLONESHOT;

        let mut ev = libc::epoll_event {
            events: epoll_interest as u32,
            u64: token.0 as u64,
        };

        let rc = unsafe {
            libc::epoll_ctl(
                self.selector.as_raw_fd(),
                libc::EPOLL_CTL_ADD,
                fd,
                &mut ev,
            )
        };
        if rc < 0 {
            let err = io::Error::last_os_error();
            // EEXIST: fd already registered (re-register with MOD).
            if err.raw_os_error() == Some(libc::EEXIST) {
                let rc = unsafe {
                    libc::epoll_ctl(
                        self.selector.as_raw_fd(),
                        libc::EPOLL_CTL_MOD,
                        fd,
                        &mut ev,
                    )
                };
                if rc < 0 {
                    return Err(io::Error::last_os_error());
                }
            } else {
                return Err(err);
            }
        }

        self.entries.write().unwrap().insert(
            token,
            Entry {
                ready: Ready::EMPTY,
                interest,
            },
        );
        Ok(())
    }

    /// Deregister a file descriptor and remove its readiness tracking.
    pub fn deregister(&self, fd: std::os::unix::io::RawFd, token: Token) -> io::Result<()> {
        let _guard = self.reg_lock.lock().unwrap();

        let rc = unsafe {
            libc::epoll_ctl(
                self.selector.as_raw_fd(),
                libc::EPOLL_CTL_DEL,
                fd,
                std::ptr::null_mut(),
            )
        };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOENT) {
                return Err(err);
            }
        }

        self.entries.write().unwrap().remove(&token);
        Ok(())
    }

    /// Check whether `token` is ready (zero syscalls — reads cached readiness).
    pub fn is_ready(&self, token: Token) -> bool {
        self.entries
            .read()
            .unwrap()
            .get(&token)
            .map(|e| !e.ready.is_empty())
            .unwrap_or(false)
    }

    /// Re-arm a one-shot registration after the task consumed the event.
    pub fn rearm(
        &self,
        fd: std::os::unix::io::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        self.register(fd, token, interest)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Drain thread will exit on next poll cycle.
    }
}

/// The drain loop: blocks in epoll_wait, sets readiness bits for signaled tokens.
fn drain_loop(
    selector: &Arc<Selector>,
    entries: &Arc<RwLock<HashMap<Token, Entry>>>,
    shutdown: &std::sync::atomic::AtomicBool,
) {
    let mut events = Events::with_capacity(1024);
    loop {
        if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        // Use the Selector's Poll interface for the drain.
        // Re-create Poll each iteration (it wraps the shared selector Arc).
        let poll = crate::native::poll::Poll {
            selector: selector.clone(),
        };
        events.clear();
        match poll.poll(&mut events, Some(Duration::from_millis(DRAIN_TIMEOUT_MS))) {
            Ok(()) => {
                let mut entries = entries.write().unwrap();
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

                    if let Some(entry) = entries.get_mut(&event.token()) {
                        entry.ready = entry.ready.union(ready);
                    }
                }
            }
            Err(_) => {
                // EINTR etc. — loop and retry.
            }
        }
    }
}
