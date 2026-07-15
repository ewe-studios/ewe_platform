//! uring↔epoll behavioural parity suite (F41 — Decision 14 OQ#14.2).
//!
//! WHY: Decision 14 makes this suite normative twice over. It is F41's own
//! acceptance criterion ("same events for same stimuli, edge triggers, fd close
//! mid-poll, drain ordering"), and it is the gate on F43: *"that suite later
//! defines what F4's fallback must preserve. F4 does not start until it is
//! green."* Without it, swapping the Linux reactor backend is an unobserved
//! change to the wake semantics every parked task depends on.
//!
//! WHAT: each scenario drives one stimulus against **both** selectors and
//! asserts they observe the same readiness. Divergence fails the test naming
//! both backends and what each saw.
//!
//! HOW: `epoll::Selector` and `uring::Selector` are both compiled on Linux (see
//! `sys/unix/mod.rs`), so a single test process instantiates one of each behind
//! a `Selector` trait and feeds them identical pipes and sockets. Readiness is
//! compared as a token → flag-set map, which is order-insensitive within a
//! drain — the selectors are allowed to batch differently, not to disagree.

#![cfg(all(target_os = "linux", feature = "uring"))]

use std::collections::BTreeMap;
use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::poll::sys::unix::selector::{epoll, uring};
use foundation_nativeapis::{Events, Interest, Token};

// ── Backend abstraction ─────────────────────────────────────────────────────

/// The internal selector interface, as both backends implement it.
trait Selector {
    fn name(&self) -> &'static str;
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;
    fn deregister(&self, fd: RawFd) -> io::Result<()>;
    fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()>;
}

impl Selector for epoll::Selector {
    fn name(&self) -> &'static str {
        "epoll"
    }
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.register_fd(fd, token, interest)
    }
    fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.deregister_fd(fd)
    }
    fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.poll(events, timeout)
    }
}

impl Selector for uring::Selector {
    fn name(&self) -> &'static str {
        "io_uring"
    }
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.register_fd(fd, token, interest)
    }
    fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.deregister_fd(fd)
    }
    fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.poll(events, timeout)
    }
}

/// Build one selector of each backend.
fn backends() -> Vec<Box<dyn Selector>> {
    vec![
        Box::new(epoll::Selector::new().expect("epoll selector")),
        Box::new(uring::Selector::new().expect("io_uring selector")),
    ]
}

// ── Observation model ───────────────────────────────────────────────────────

/// The readiness flags a selector reported for one token, as a comparable set.
///
/// Compared rather than raw `Event`s because the two backends are permitted to
/// batch and order completions differently — they are not permitted to report
/// different readiness.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Flags {
    readable: bool,
    writable: bool,
    read_closed: bool,
    write_closed: bool,
    error: bool,
}

impl Flags {
    fn merge(self, other: Flags) -> Flags {
        Flags {
            readable: self.readable || other.readable,
            writable: self.writable || other.writable,
            read_closed: self.read_closed || other.read_closed,
            write_closed: self.write_closed || other.write_closed,
            error: self.error || other.error,
        }
    }
}

/// A drain's worth of observations: token → merged flags.
type Observed = BTreeMap<usize, Flags>;

/// Poll once and collect the reported readiness.
fn drain_once(sel: &dyn Selector, timeout: Option<Duration>) -> Observed {
    let mut events = Events::with_capacity(64);
    sel.poll(&mut events, timeout).expect("poll must not fail");

    let mut observed = Observed::new();
    for event in events.iter() {
        let flags = Flags {
            readable: event.is_readable(),
            writable: event.is_writable(),
            read_closed: event.is_read_closed(),
            write_closed: event.is_write_closed(),
            error: event.is_error(),
        };
        let slot = observed.entry(event.token().0).or_default();
        *slot = slot.merge(flags);
    }
    observed
}

/// Poll until a drain reports something, or `budget` expires.
///
/// Both backends may return an empty drain before the stimulus lands (a timeout
/// tick, a benign `ETIME`), so a single empty drain is not evidence of absence.
fn drain_until_nonempty(sel: &dyn Selector, budget: Duration) -> Observed {
    let deadline = Instant::now() + budget;
    loop {
        let observed = drain_once(sel, Some(Duration::from_millis(20)));
        if !observed.is_empty() {
            return observed;
        }
        if Instant::now() >= deadline {
            return Observed::new();
        }
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Create a nonblocking pipe, returning `(read_fd, write_fd)`.
fn nonblocking_pipe() -> (RawFd, RawFd) {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe2 to fill.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    assert!(rc >= 0, "pipe2: {}", io::Error::last_os_error());
    (fds[0], fds[1])
}

fn write_byte(fd: RawFd) {
    // SAFETY: writing one byte from a valid local buffer to an owned pipe fd.
    let n = unsafe { libc::write(fd, b"x".as_ptr().cast(), 1) };
    assert_eq!(n, 1, "write: {}", io::Error::last_os_error());
}

fn close(fd: RawFd) {
    // SAFETY: `fd` is owned by the caller and not used again.
    unsafe { libc::close(fd) };
}

fn drain_fd(fd: RawFd) {
    let mut buf = [0u8; 64];
    // SAFETY: reading into a valid local buffer from an owned pipe fd.
    while unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) } > 0 {}
}

/// Run `scenario` against every backend and assert they all observed the same thing.
///
/// Agreement alone is a weak claim — two identically-wrong backends agree. Each
/// scenario therefore also asserts the *expected* readiness inside its closure,
/// where it knows the stimulus. This function only catches divergence.
fn assert_parity(what: &str, scenario: impl Fn(&dyn Selector) -> Observed) {
    let mut results: Vec<(&'static str, Observed)> = Vec::new();
    for sel in backends() {
        let observed = scenario(sel.as_ref());
        results.push((sel.name(), observed));
    }

    let (first_name, first) = &results[0];
    for (name, observed) in &results[1..] {
        assert_eq!(
            first, observed,
            "{what}: backends disagree.\n  {first_name}: {first:?}\n  {name}: {observed:?}"
        );
    }
}

/// The single token a scenario registered, with its flags.
fn sole_entry(observed: &Observed, sel: &dyn Selector, what: &str) -> Flags {
    assert_eq!(
        observed.len(),
        1,
        "{}: expected exactly one ready token after {what}, got {observed:?}",
        sel.name()
    );
    *observed.values().next().expect("len checked above")
}

// ── Scenarios ───────────────────────────────────────────────────────────────

#[test]
fn parity_readable_after_write() {
    assert_parity("a written pipe reports readable", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");
        write_byte(w);
        let observed = drain_until_nonempty(sel, Duration::from_secs(2));

        let flags = sole_entry(&observed, sel, "a write");
        assert!(flags.readable, "{}: a written pipe must be readable", sel.name());
        assert!(!flags.read_closed, "{}: the write end is still open", sel.name());
        assert!(!flags.error, "{}: a plain write is not an error", sel.name());

        sel.deregister(r).ok();
        close(r);
        close(w);
        observed
    });
}

#[test]
fn parity_idle_fd_reports_nothing() {
    assert_parity("an idle pipe reports nothing", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");
        let observed = drain_once(sel, Some(Duration::from_millis(50)));

        assert!(
            observed.is_empty(),
            "{}: an idle pipe reported {observed:?}",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        close(w);
        observed
    });
}

#[test]
fn parity_edge_triggered_does_not_refire_without_new_data() {
    assert_parity("unread data does not re-fire (edge semantics)", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");

        write_byte(w);
        let first = drain_until_nonempty(sel, Duration::from_secs(2));
        assert!(!first.is_empty(), "{}: first write must fire", sel.name());

        // Data is still buffered and deliberately unread. An edge-triggered
        // selector must stay silent; a level-triggered one would re-fire and
        // spin the drain thread at 100% CPU.
        let second = drain_once(sel, Some(Duration::from_millis(100)));

        assert!(
            second.is_empty(),
            "{}: unread data re-fired {second:?}. epoll registers EPOLLET and \
             io_uring's multishot poll re-arms on wakeup, so neither may report \
             readiness again without a new edge",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        close(w);
        second
    });
}

#[test]
fn parity_second_write_refires_after_drain() {
    assert_parity("a fresh edge after draining re-fires", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");

        write_byte(w);
        drain_until_nonempty(sel, Duration::from_secs(2));
        drain_fd(r);

        write_byte(w);
        let observed = drain_until_nonempty(sel, Duration::from_secs(2));

        let flags = sole_entry(&observed, sel, "a second write");
        assert!(
            flags.readable,
            "{}: a cleared registration must re-arm on the next edge",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        close(w);
        observed
    });
}

#[test]
fn parity_write_end_closed_mid_poll() {
    assert_parity("closing the write end mid-poll surfaces hangup", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");

        // Close from another thread while this one is parked in poll().
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            close(w);
        });

        let observed = drain_until_nonempty(sel, Duration::from_secs(2));
        handle.join().expect("closer thread");

        let flags = sole_entry(&observed, sel, "the write end closing");
        assert!(
            flags.read_closed,
            "{}: peer hangup must surface as read_closed, else a task loops on \
             a dead fd instead of seeing EOF; got {flags:?}",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        observed
    });
}

#[test]
fn parity_deregistered_fd_reports_nothing() {
    assert_parity("a deregistered fd stops reporting", |sel| {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");
        sel.deregister(r).expect("deregister");

        write_byte(w);
        let observed = drain_once(sel, Some(Duration::from_millis(100)));

        assert!(
            observed.is_empty(),
            "{}: a deregistered fd still reported {observed:?}",
            sel.name()
        );

        close(r);
        close(w);
        observed
    });
}

#[test]
fn parity_drain_reports_every_ready_fd() {
    assert_parity("all ready fds appear in the drain", |sel| {
        let pipes: Vec<(RawFd, RawFd)> = (0..4).map(|_| nonblocking_pipe()).collect();
        for (i, (r, _)) in pipes.iter().enumerate() {
            sel.register(*r, Token(100 + i), Interest::READABLE).expect("register");
        }
        for (_, w) in &pipes {
            write_byte(*w);
        }

        // Both backends may split the ready set across drains; accumulate until
        // quiescent so the comparison is over the whole set, not one batch.
        let mut observed = Observed::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while observed.len() < pipes.len() && Instant::now() < deadline {
            for (token, flags) in drain_once(sel, Some(Duration::from_millis(20))) {
                let slot = observed.entry(token).or_default();
                *slot = slot.merge(flags);
            }
        }

        assert_eq!(
            observed.len(),
            4,
            "{}: all four written pipes must appear across the drains, got {observed:?}",
            sel.name()
        );
        assert!(
            observed.values().all(|f| f.readable),
            "{}: every written pipe must be readable, got {observed:?}",
            sel.name()
        );

        for (r, w) in pipes {
            sel.deregister(r).ok();
            close(r);
            close(w);
        }
        observed
    });
}

#[test]
fn parity_zero_timeout_never_blocks() {
    for sel in backends() {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");

        let start = Instant::now();
        let observed = drain_once(sel.as_ref(), Some(Duration::ZERO));
        let elapsed = start.elapsed();

        assert!(
            observed.is_empty(),
            "{}: a zero-timeout poll on an idle fd must report nothing",
            sel.name()
        );
        assert!(
            elapsed < Duration::from_millis(50),
            "{}: a zero-timeout poll blocked for {elapsed:?}",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        close(w);
    }
}

#[test]
fn parity_timeout_expires_and_returns() {
    for sel in backends() {
        let (r, w) = nonblocking_pipe();
        sel.register(r, Token(1), Interest::READABLE).expect("register");

        let budget = Duration::from_millis(150);
        let start = Instant::now();
        let observed = drain_once(sel.as_ref(), Some(budget));
        let elapsed = start.elapsed();

        assert!(
            observed.is_empty(),
            "{}: no stimulus, so the drain must be empty",
            sel.name()
        );
        assert!(
            elapsed >= Duration::from_millis(100),
            "{}: poll returned after {elapsed:?}, well before its {budget:?} \
             timeout — the timeout argument is being ignored",
            sel.name()
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "{}: poll overran its {budget:?} timeout, taking {elapsed:?}",
            sel.name()
        );

        sel.deregister(r).ok();
        close(r);
        close(w);
    }
}
