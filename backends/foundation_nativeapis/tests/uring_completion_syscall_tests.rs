//! The F43 acceptance criterion: zero `read()` syscalls on the hot path.
//!
//! WHY: "the kernel does the read" is the entire claim of completion mode. It
//! cannot be checked by reading the code — a stray `read(2)` anywhere under
//! `poll()` would silently reintroduce the syscall this feature exists to
//! remove. The feature's acceptance criterion says so directly: *"Hot-path reads
//! show zero read() syscalls under strace on a supporting kernel."*
//!
//! WHAT: run the completion read path under a syscall tracer and assert that no
//! read-family syscall is ever issued against the socket, while the bytes still
//! arrive. For contrast, the same harness runs the readiness path, which *must*
//! read the socket — proving the tracer can see what it is looking for.
//!
//! HOW: `strace` is not a dependency, so the test is its own tracer. It forks,
//! the child calls `PTRACE_TRACEME` and runs the hot path, and the parent single
//! steps it with `PTRACE_SYSCALL`, decoding `orig_rax` (syscall number) and `rdi`
//! (first argument, the fd) at each syscall-entry stop.
//!
//! x86_64 only: `PTRACE_GETREGS` and `user_regs_struct` are architecture
//! specific, and aarch64 needs `PTRACE_GETREGSET` instead.

#![cfg(all(target_os = "linux", target_arch = "x86_64", feature = "uring"))]

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::poll::probe;
use foundation_nativeapis::native::poll::sys::unix::selector::{uring, uring_completion};
use foundation_nativeapis::{Events, Interest, Token};

/// x86_64 syscall numbers for every way a process can read from an fd.
const SYS_READ: u64 = 0;
const SYS_PREAD64: u64 = 17;
const SYS_READV: u64 = 19;
const SYS_RECVFROM: u64 = 45;
const SYS_RECVMSG: u64 = 47;
const SYS_PREADV: u64 = 295;
const SYS_RECVMMSG: u64 = 299;
const SYS_PREADV2: u64 = 327;

/// The syscall completion mode should be using instead.
const SYS_IO_URING_ENTER: u64 = 426;

fn is_read_family(nr: u64) -> bool {
    matches!(
        nr,
        SYS_READ
            | SYS_PREAD64
            | SYS_READV
            | SYS_RECVFROM
            | SYS_RECVMSG
            | SYS_PREADV
            | SYS_RECVMMSG
            | SYS_PREADV2
    )
}

/// What the tracer observed while the child ran the hot path.
#[derive(Debug, Default, PartialEq, Eq)]
struct SyscallCounts {
    /// Read-family syscalls issued against the traced socket fd.
    reads_on_socket: usize,
    /// Read-family syscalls against any fd (waker eventfds, /proc, …).
    reads_total: usize,
    /// `io_uring_enter` calls — how completion mode reaches the kernel.
    io_uring_enters: usize,
    /// The child's exit status; 0 means it saw the bytes it expected.
    child_exit: i32,
}

fn socketpair() -> (RawFd, RawFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for socketpair to fill.
    let rc = unsafe {
        libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr())
    };
    assert!(rc == 0, "socketpair: {}", io::Error::last_os_error());
    (fds[0], fds[1])
}

fn write_all(fd: RawFd, data: &[u8]) {
    // SAFETY: writing `data.len()` bytes from a valid slice to an owned fd.
    let n = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
    assert_eq!(n, data.len() as isize, "write: {}", io::Error::last_os_error());
}

fn close(fd: RawFd) {
    // SAFETY: `fd` is owned by the caller and not used again.
    unsafe { libc::close(fd) };
}

/// Fork, run `hot_path` in the traced child, and count its syscalls on `watch_fd`.
///
/// `hot_path` returns `true` when it observed the bytes it expected. It runs
/// after the child is already under trace, so every syscall it makes is counted.
///
/// # Panics
/// Panics if `fork`, `ptrace`, or `waitpid` fails.
fn trace_hot_path(watch_fd: RawFd, hot_path: impl FnOnce() -> bool) -> SyscallCounts {
    // SAFETY: fork(2). The child touches only async-signal-safe calls plus
    // glibc malloc, which installs pthread_atfork handlers making it fork-safe.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork: {}", io::Error::last_os_error());

    if pid == 0 {
        // ── Child ──
        // SAFETY: standard PTRACE_TRACEME handshake; the parent is already in
        // waitpid. `_exit` avoids running the test harness's atexit handlers.
        unsafe {
            libc::ptrace(libc::PTRACE_TRACEME, 0, std::ptr::null_mut::<libc::c_void>(), std::ptr::null_mut::<libc::c_void>());
            libc::raise(libc::SIGSTOP);
        }
        let ok = hot_path();
        // SAFETY: terminating the forked child without unwinding.
        unsafe { libc::_exit(i32::from(!ok)) };
    }

    // ── Parent: the tracer ──
    let mut counts = SyscallCounts::default();
    let mut status: libc::c_int = 0;

    // Wait for the child's SIGSTOP.
    // SAFETY: `status` is a valid out-param; `pid` is our child.
    unsafe { libc::waitpid(pid, &mut status, 0) };

    // SAFETY: the child is stopped and traced.
    unsafe {
        libc::ptrace(
            libc::PTRACE_SETOPTIONS,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            (libc::PTRACE_O_TRACESYSGOOD | libc::PTRACE_O_EXITKILL) as *mut libc::c_void,
        );
    }

    // Syscall stops alternate entry, exit, entry, exit… Count on entry only,
    // where the arguments are still in the registers.
    let mut at_entry = true;
    loop {
        // SAFETY: resume the stopped, traced child until its next syscall stop.
        unsafe {
            libc::ptrace(libc::PTRACE_SYSCALL, pid, std::ptr::null_mut::<libc::c_void>(), std::ptr::null_mut::<libc::c_void>());
            libc::waitpid(pid, &mut status, 0);
        }

        if libc::WIFEXITED(status) {
            counts.child_exit = libc::WEXITSTATUS(status);
            break;
        }
        if libc::WIFSIGNALED(status) {
            panic!("traced child died from signal {}", libc::WTERMSIG(status));
        }
        if !libc::WIFSTOPPED(status) {
            continue;
        }

        // Only `SIGTRAP | 0x80` stops are syscall stops (PTRACE_O_TRACESYSGOOD).
        if libc::WSTOPSIG(status) != libc::SIGTRAP | 0x80 {
            continue;
        }

        if at_entry {
            let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
            // SAFETY: `regs` is a valid out-param of the expected type, and the
            // child is stopped at a syscall entry.
            unsafe {
                libc::ptrace(
                    libc::PTRACE_GETREGS,
                    pid,
                    std::ptr::null_mut::<libc::c_void>(),
                    std::ptr::from_mut(&mut regs).cast::<libc::c_void>(),
                );
            }
            let nr = regs.orig_rax;
            let arg0 = regs.rdi;

            if is_read_family(nr) {
                counts.reads_total += 1;
                if arg0 == watch_fd as u64 {
                    counts.reads_on_socket += 1;
                }
            } else if nr == SYS_IO_URING_ENTER {
                counts.io_uring_enters += 1;
            }
        }
        at_entry = !at_entry;
    }

    counts
}

/// Wait until `done` reports success, or the budget expires.
fn spin_until(budget: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
    }
    false
}

#[test]
fn completion_mode_reads_the_socket_without_a_read_syscall() {
    match probe::probe() {
        Ok(caps) if caps.supports_completion() => {}
        Ok(caps) => {
            eprintln!("skipping: kernel lacks the completion tier ({caps})");
            return;
        }
        Err(e) => {
            eprintln!("skipping: io_uring unavailable ({e})");
            return;
        }
    }

    let (a, b) = socketpair();
    let token = Token(1);

    // Build and arm before forking, so the traced region is only the hot path.
    let sel = uring_completion::Selector::new().expect("completion selector");
    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    write_all(b, b"kernel read this");

    let counts = trace_hot_path(a, move || {
        let mut events = Events::with_capacity(16);
        spin_until(Duration::from_secs(3), || {
            events.clear();
            if sel.poll(&mut events, Some(Duration::from_millis(20))).is_err() {
                return false;
            }
            sel.take_completions(token).iter().any(|c| match c {
                uring_completion::Completion::Data(buf) => &buf[..] == b"kernel read this",
                _ => false,
            })
        })
    });

    close(a);
    close(b);

    assert_eq!(
        counts.child_exit, 0,
        "the child never saw the bytes, so the syscall counts below prove nothing: {counts:?}"
    );
    assert_eq!(
        counts.reads_on_socket, 0,
        "completion mode issued {} read-family syscall(s) on the socket. The kernel \
         is supposed to have already read those bytes into a provided buffer; a read() \
         here means the syscall this feature removes has crept back in. {counts:?}",
        counts.reads_on_socket
    );
    assert!(
        counts.io_uring_enters > 0,
        "the bytes must have arrived through io_uring_enter; {counts:?}"
    );
}

/// The transport-facing opt-in — `RegisteredFd::read_bytes` — must be just as
/// syscall-free as the zero-copy `take_completions` path. A transport that
/// swapped `read(2)` for `read_bytes` and still paid a syscall would have gained
/// nothing.
///
/// The reactor's drain thread does not survive `fork`, so the bytes are
/// delivered into the inbox *before* forking. What the child then runs is
/// exactly the hot path: pop the inbox and copy out.
#[cfg(feature = "fd")]
#[test]
fn read_bytes_copies_from_the_inbox_without_a_read_syscall() {
    use foundation_nativeapis::native::fd::{CompletionSource, Reactor, RegisteredFd};
    use foundation_nativeapis::native::poll::Backend;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    let reactor = Reactor::get().expect("reactor");
    if reactor.backend() != Backend::UringCompletion {
        eprintln!("skipping: reactor is on {} — completion mode unavailable", reactor.backend());
        return;
    }

    let (a, b) = socketpair();
    // SAFETY: `a` is a fresh, valid, owned descriptor.
    let owned_a = unsafe { OwnedFd::from_raw_fd(a) };
    let raw_a = owned_a.as_raw_fd();

    let sock = RegisteredFd::with_interest(owned_a, reactor.registry(), Token(0xF43_9001), Interest::READABLE)
        .expect("register socket");
    write_all(b, b"popped from the inbox");

    // Let the drain thread deliver into the inbox before we fork away from it.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && !sock.has_completions() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(sock.has_completions(), "the kernel never delivered into the inbox");

    let counts = trace_hot_path(raw_a, move || {
        let mut buf = [0u8; 64];
        let ok = match sock.read_bytes(&mut buf) {
            Ok(n) => &buf[..n] == b"popped from the inbox",
            Err(_) => false,
        };
        // Dropping the RegisteredFd would deregister, which submits an
        // ASYNC_CANCEL and so issues an `io_uring_enter` — teardown, not the
        // hot path. The child is about to `_exit`, so leak it and keep the
        // measurement to the read itself.
        std::mem::forget(sock);
        ok
    });

    close(b);

    assert_eq!(
        counts.child_exit, 0,
        "read_bytes did not return the staged bytes: {counts:?}"
    );
    assert_eq!(
        counts.reads_on_socket, 0,
        "read_bytes issued {} read-family syscall(s) on the socket. In completion \
         mode it must be a memcpy out of the inbox: {counts:?}",
        counts.reads_on_socket
    );
    assert_eq!(
        counts.io_uring_enters, 0,
        "popping an already-filled inbox must not enter the kernel at all: {counts:?}"
    );
}

/// The control: readiness mode *must* read the socket. Without this, a tracer
/// that silently counts nothing would make the test above pass vacuously.
#[test]
fn readiness_mode_does_issue_a_read_syscall() {
    if probe::probe().is_err() {
        eprintln!("skipping: io_uring unavailable");
        return;
    }

    let (a, b) = socketpair();
    let token = Token(1);

    let sel = uring::Selector::new().expect("readiness selector");
    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    write_all(b, b"read me");

    let counts = trace_hot_path(a, move || {
        let mut events = Events::with_capacity(16);
        spin_until(Duration::from_secs(3), || {
            events.clear();
            if sel.poll(&mut events, Some(Duration::from_millis(20))).is_err() {
                return false;
            }
            if !events.iter().any(|e| e.token() == token && e.is_readable()) {
                return false;
            }
            // Readiness mode only says "there are bytes"; we must fetch them.
            let mut buf = [0u8; 64];
            // SAFETY: reading into a valid local buffer from the socket.
            let n = unsafe { libc::read(a, buf.as_mut_ptr().cast(), buf.len()) };
            n > 0 && &buf[..n as usize] == b"read me"
        })
    });

    close(a);
    close(b);

    assert_eq!(counts.child_exit, 0, "readiness child failed: {counts:?}");
    assert!(
        counts.reads_on_socket > 0,
        "readiness mode must read the socket — if the tracer sees zero here it is \
         blind, and the completion-mode assertion is meaningless: {counts:?}"
    );
}
