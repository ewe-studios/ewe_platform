//! Shared helper for the io_uring test suites (spec 70 feature 78 E2).
//!
//! WHY: the completion tests skip when the kernel lacks the completion tier.
//! A skip is correct on a developer machine, but CI must not lose the coverage
//! without anyone noticing. The same rule as `foundation_iogate` applies: with
//! `EWE_REQUIRE_IO_URING=1`, a missing io_uring fails the test with the reason.
//!
//! WHAT: [`io_uring_missing`] probes the kernel and returns whether the caller
//! must skip.
//!
//! HOW: each test binary declares `mod common;`. Cargo does not build
//! `tests/common/mod.rs` as a test target of its own.

use foundation_nativeapis::native::poll::probe;

/// What the caller needs from this kernel.
///
/// The four test binaries share this module. Only the syscall suite uses
/// `IoUring`, so the variant is dead code in the other three binaries; allow it
/// here instead of copying the helper into each one.
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
