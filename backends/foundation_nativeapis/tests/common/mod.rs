//! Shared helpers for the kernel-gated test suites (spec 70 features 78 and 83).
//!
//! WHY: the completion tests skip when the kernel lacks the completion tier,
//! and the ptrace spawn tests need a real kernel. A skip is correct on a
//! developer machine, but CI must not lose the coverage without anyone
//! noticing. The same rule as `foundation_iogate` applies: with
//! `EWE_REQUIRE_IO_URING=1` or `EWE_REQUIRE_PTRACE=1`, a missing kernel
//! feature fails the test with the reason.
//!
//! WHAT: [`io_uring_missing`] and [`ptrace_missing`] probe the kernel and
//! return whether the caller must skip. [`ptrace_available`] is the raw
//! probe, shared with `tests/ptrace_probe.rs`.
//!
//! HOW: each test binary declares `mod common;`. Cargo does not build
//! `tests/common/mod.rs` as a test target of its own.

// The io_uring helpers exist only when that feature is on. The ptrace probe
// target includes this module without `uring`, so the import must be gated too.
#[cfg(feature = "uring")]
use foundation_nativeapis::native::poll::probe;

/// What the caller needs from this kernel.
///
/// The four test binaries share this module. Only the syscall suite uses
/// `IoUring`, so the variant is dead code in the other three binaries; allow it
/// here instead of copying the helper into each one.
#[cfg(feature = "uring")]
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Requires {
    /// Any io_uring support. The readiness tier does not need completion mode.
    IoUring,
    /// The completion tier (`IORING_OP_RECV`, `IORING_OP_SEND`, ...).
    Completion,
}

/// Whether the running kernel lacks what `requires` asks for, so the caller
/// must skip its test.
///
/// A local machine without io_uring skips with one line. An environment that
/// sets `EWE_REQUIRE_IO_URING=1` expects io_uring, so the test fails with the
/// reason instead.
#[cfg(feature = "uring")]
#[must_use]
pub fn io_uring_missing(what: &str, requires: Requires) -> bool {
    let promised = std::env::var("EWE_REQUIRE_IO_URING").as_deref() == Ok("1");
    match probe::probe() {
        Ok(caps) if matches!(requires, Requires::IoUring) || caps.supports_completion() => false,
        Ok(caps) => {
            assert!(
                !promised,
                "EWE_REQUIRE_IO_URING=1 but the kernel lacks the completion tier ({what}: {caps})"
            );
            eprintln!("skipping {what}: the kernel lacks the completion tier ({caps})");
            true
        }
        Err(e) => {
            assert!(
                !promised,
                "EWE_REQUIRE_IO_URING=1 but io_uring is unavailable ({what}: {e})"
            );
            eprintln!("skipping {what}: io_uring is unavailable ({e})");
            true
        }
    }
}

/// Probe whether this kernel can trace a child and read its registers.
///
/// WHY: an emulated amd64 kernel (OrbStack, qemu) returns `EIO` from
/// `PTRACE_GETREGS` for every traced process. A native amd64 Linux kernel
/// returns the registers. The caller must know which kernel it runs on before
/// it reports a tracer failure.
///
/// WHAT: fork. The child calls `PTRACE_TRACEME` and stops itself with
/// `SIGSTOP`. The parent waits, then calls `PTRACE_GETREGS`, and always
/// resumes and reaps the child.
///
/// HOW: the result is `Ok(())` when the registers are readable, and `Err(errno)`
/// otherwise. `EIO` means an emulated kernel, `EPERM` a seccomp or `Yama`
/// policy, and `ESRCH` that the tracee is gone.
#[cfg(target_os = "linux")]
#[allow(dead_code)] // shared with `tests/ptrace_probe.rs`; uring suites do not call it
pub fn ptrace_available() -> Result<(), i32> {
    use std::ptr;

    #[inline]
    fn last_errno() -> i32 {
        std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO)
    }

    // SAFETY: `fork` duplicates the process. The child calls only
    // async-signal-safe libc functions before `_exit`, as the POSIX rules for
    // the window between `fork` and `exec` require.
    let child = unsafe { libc::fork() };
    if child < 0 {
        return Err(last_errno());
    }
    if child == 0 {
        // Child: ask the kernel to make the parent the tracer, then stop so the
        // parent can read the registers. If `PTRACE_TRACEME` fails, exit at
        // once: an untraced `SIGSTOP` would block the parent forever.
        let traced = unsafe {
            libc::ptrace(
                libc::PTRACE_TRACEME,
                0 as libc::pid_t,
                ptr::null_mut::<libc::c_void>(),
                ptr::null_mut::<libc::c_void>(),
            )
        };
        if traced == -1 {
            unsafe { libc::_exit(1) };
        }
        unsafe { libc::raise(libc::SIGSTOP) };
        unsafe { libc::_exit(0) };
    }

    let mut status = 0;
    // A traced child's stop is reported without `WUNTRACED`.
    let waited = unsafe { libc::waitpid(child, &mut status, 0) };
    if waited < 0 {
        let errno = last_errno();
        unsafe {
            libc::kill(child, libc::SIGKILL);
            libc::waitpid(child, &mut status, 0);
        }
        return Err(errno);
    }
    if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
        // `PTRACE_TRACEME` failed in the child, so the tracee is gone.
        return Err(libc::ESRCH);
    }
    if !libc::WIFSTOPPED(status) {
        unsafe {
            libc::kill(child, libc::SIGKILL);
            libc::waitpid(child, &mut status, 0);
        }
        return Err(libc::EIO);
    }

    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let read = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            child,
            ptr::null_mut::<libc::c_void>(),
            &mut regs as *mut _ as *mut libc::c_void,
        )
    };
    let result = if read == -1 {
        Err(last_errno())
    } else {
        Ok(())
    };

    // Resume the stopped tracee (detach with signal 0) and reap it. If detach
    // fails, kill it, so this never leaves a stopped process or a zombie.
    let detached = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            child,
            ptr::null_mut::<libc::c_void>(),
            ptr::null_mut::<libc::c_void>(),
        )
    };
    if detached == -1 {
        unsafe { libc::kill(child, libc::SIGKILL) };
    }
    unsafe { libc::waitpid(child, &mut status, 0) };
    result
}

/// Whether this kernel cannot trace a child, so the caller must skip its test.
///
/// A local machine without a usable ptrace skips with one line. An environment
/// that sets `EWE_REQUIRE_PTRACE=1` expects ptrace, so the test fails with the
/// errno instead.
#[cfg(target_os = "linux")]
#[allow(dead_code)] // no suite calls it yet; the ptrace spawn suite must fail in CI, not skip
#[must_use]
pub fn ptrace_missing(what: &str) -> bool {
    let promised = std::env::var("EWE_REQUIRE_PTRACE").as_deref() == Ok("1");
    match ptrace_available() {
        Ok(()) => false,
        Err(errno) => {
            assert!(
                !promised,
                "EWE_REQUIRE_PTRACE=1 but ptrace is unavailable ({what}: errno {errno})"
            );
            eprintln!("skipping {what}: ptrace is unavailable (errno {errno})");
            true
        }
    }
}
