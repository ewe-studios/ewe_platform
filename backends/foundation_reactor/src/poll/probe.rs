//! Functional io_uring capability probe (F42 — Decision 14 OQ#14.3).
//!
//! WHY: kernel version strings are the wrong gate. Distros backport (an
//! enterprise "5.14" carries far more than mainline 5.14) and they also
//! restrict (`kernel.io_uring_disabled=2` on hardened hosts, `CONFIG_IO_URING=n`
//! in some builds), so a version comparison produces both false negatives and
//! false positives. The only trustworthy question is "does *this* kernel accept
//! *this* opcode right now", asked by trying it.
//!
//! WHAT: [`probe`] answers that once, returning the [`UringCapabilities`] the
//! selection ladder needs, or a [`ProbeError`] naming the concrete reason
//! io_uring is unusable on this host.
//!
//! HOW: three tiers, in order of cost.
//!
//! 1. `io_uring_setup` on a small ring — catches `CONFIG_IO_URING=n` and
//!    `kernel.io_uring_disabled` regardless of version.
//! 2. `IORING_REGISTER_PROBE` — the kernel's own supported-opcode bitmap.
//! 3. Functional checks for the capabilities whose presence the bitmap cannot
//!    express, because they are *flags* on an opcode rather than opcodes:
//!    `IORING_POLL_ADD_MULTI` (readiness tier) and `IORING_RECV_MULTISHOT`
//!    (completion tier), plus an actual `PBUF_RING` registration attempt.
//!
//! Version numbers appear in log lines for humans, never in a branch.

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

use io_uring::types::{SubmitArgs, Timespec};
use io_uring::{opcode, types, IoUring, Probe};

/// Buffer-group id used for the probe's throwaway buffer ring.
const PROBE_BGID: u16 = 0;

/// Entries in the probe's buffer ring. Must be a power of two.
const PROBE_RING_ENTRIES: u16 = 1;

/// Kernel ABI: one entry of a provided-buffer ring. 16 bytes, stable layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct IoUringBuf {
    addr: u64,
    len: u32,
    bid: u16,
    resv: u16,
}

/// What this kernel can actually do with io_uring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UringCapabilities {
    /// `IORING_OP_POLL_ADD` is a supported opcode.
    pub poll_add: bool,
    /// `IORING_POLL_ADD_MULTI` is accepted (multishot poll; mainline ≥ 5.13).
    pub poll_add_multi: bool,
    /// A provided-buffer ring registers successfully (mainline ≥ 5.19).
    pub buffer_ring: bool,
    /// `IORING_RECV_MULTISHOT` is accepted (mainline ≥ 6.0).
    pub recv_multishot: bool,
}

impl UringCapabilities {
    /// WHY: the readiness selector needs multishot poll and nothing else.
    ///
    /// WHAT: whether the F41 readiness backend can run here.
    ///
    /// HOW: `POLL_ADD` present and its multishot flag accepted.
    ///
    /// # Panics
    /// Never panics.
    pub fn supports_readiness(self) -> bool {
        self.poll_add && self.poll_add_multi
    }

    /// WHY: the F43 completion selector reads through kernel-filled buffer
    /// rings, so both the ring and multishot recv must work.
    ///
    /// WHAT: whether the completion backend can run here.
    ///
    /// HOW: readiness tier, plus a registerable buffer ring and multishot recv.
    ///
    /// # Panics
    /// Never panics.
    pub fn supports_completion(self) -> bool {
        self.supports_readiness() && self.buffer_ring && self.recv_multishot
    }

    /// A one-line summary for the init log.
    fn summary(self) -> String {
        format!(
            "poll_add={} poll_add_multi={} buffer_ring={} recv_multishot={}",
            self.poll_add, self.poll_add_multi, self.buffer_ring, self.recv_multishot
        )
    }
}

impl std::fmt::Display for UringCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.summary())
    }
}

/// Why io_uring cannot be used on this host.
///
/// Carried verbatim into the hard error an explicit `NativeAPI::IOUring`
/// produces, so operators see the cause rather than a silent demotion to epoll.
#[derive(Debug)]
pub enum ProbeError {
    /// `io_uring_setup` failed. `CONFIG_IO_URING=n`, or
    /// `kernel.io_uring_disabled` is set (sysctl, 6.6+, common on hardened
    /// hosts), or the ring could not be allocated.
    Setup(io::Error),
    /// `IORING_REGISTER_PROBE` failed, so the opcode bitmap is unknown.
    RegisterProbe(io::Error),
    /// The ring works but lacks multishot poll, which the readiness selector
    /// requires.
    NoMultishotPoll(UringCapabilities),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::Setup(e) => {
                write!(f, "io_uring unavailable: io_uring_setup failed: {e}")
            }
            ProbeError::RegisterProbe(e) => {
                write!(f, "io_uring unavailable: IORING_REGISTER_PROBE failed: {e}")
            }
            ProbeError::NoMultishotPoll(caps) => write!(
                f,
                "io_uring unusable: kernel lacks multishot poll (IORING_POLL_ADD_MULTI); {caps}"
            ),
        }
    }
}

impl std::error::Error for ProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProbeError::Setup(e) | ProbeError::RegisterProbe(e) => Some(e),
            ProbeError::NoMultishotPoll(_) => None,
        }
    }
}

/// WHY: the selection ladder must know what this kernel supports before it
/// picks a backend, and must be able to say *why* when it demotes.
///
/// WHAT: run the functional probe and report the capabilities.
///
/// HOW: `io_uring_setup` → `IORING_REGISTER_PROBE` → functional checks for the
/// two multishot flags and buffer-ring registration. Runs once, at shared
/// reactor init; the result is cached by the caller.
///
/// # Errors
/// [`ProbeError::Setup`] if io_uring is absent or administratively disabled;
/// [`ProbeError::RegisterProbe`] if the opcode bitmap cannot be read;
/// [`ProbeError::NoMultishotPoll`] if the ring works but cannot drive the
/// readiness selector.
///
/// # Panics
/// Never panics.
pub fn probe() -> Result<UringCapabilities, ProbeError> {
    // Tier 1: does this kernel give us a ring at all?
    let ring = IoUring::new(8).map_err(ProbeError::Setup)?;

    // Tier 2: the kernel's supported-opcode bitmap.
    let mut opcodes = Probe::new();
    ring.submitter()
        .register_probe(&mut opcodes)
        .map_err(ProbeError::RegisterProbe)?;

    let poll_add = opcodes.is_supported(opcode::PollAdd::CODE);

    // Tier 3: flags the bitmap cannot describe, checked by submitting them.
    let poll_add_multi = poll_add && probe_multishot_poll(&ring);
    let buffer_ring = probe_buffer_ring(&ring);
    let recv_multishot = buffer_ring && probe_recv_multishot(&ring);

    let caps = UringCapabilities { poll_add, poll_add_multi, buffer_ring, recv_multishot };

    if !caps.supports_readiness() {
        return Err(ProbeError::NoMultishotPoll(caps));
    }

    Ok(caps)
}

/// Wait briefly for one CQE and return its raw result, or `None` on timeout.
///
/// A kernel that rejects an SQE's flags posts an error CQE immediately, so a
/// timeout means "accepted and pending" — which is exactly what "supported"
/// looks like for a multishot op with no data to deliver.
fn await_one_cqe(ring: &IoUring, budget: Duration) -> Option<i32> {
    let ts = Timespec::new().sec(budget.as_secs()).nsec(budget.subsec_nanos());
    let args = SubmitArgs::new().timespec(&ts);
    // ETIME here means no CQE arrived, which the caller reads as "accepted".
    let _ = ring.submitter().submit_with_args(1, &args);

    // SAFETY: the probe ring is local to this function's call tree and never
    // shared, so this is the only accessor of its completion queue.
    let mut cq = unsafe { ring.completion_shared() };
    cq.sync();
    cq.next().map(|cqe| cqe.result())
}

/// Is `IORING_POLL_ADD_MULTI` accepted?
///
/// Arms a multishot poll on an eventfd that is *already* readable, so a
/// supporting kernel completes with a positive revents mask. A kernel without
/// the flag rejects the SQE with `-EINVAL`.
fn probe_multishot_poll(ring: &IoUring) -> bool {
    let Some(efd) = probe_eventfd(1) else {
        return false;
    };

    let sqe = opcode::PollAdd::new(types::Fd(efd.as_raw_fd()), libc::POLLIN as u32)
        .multi(true)
        .build()
        .user_data(1);

    // SAFETY: sole owner of this local ring's submission queue; `sqe` is fully
    // initialised and `efd` outlives the submission.
    unsafe {
        let mut sq = ring.submission_shared();
        if sq.push(&sqe).is_err() {
            return false;
        }
        sq.sync();
    }

    match await_one_cqe(ring, Duration::from_millis(50)) {
        // Positive result: the poll completed with a revents mask.
        Some(result) if result >= 0 => true,
        // -EINVAL / -EOPNOTSUPP: the multishot flag was rejected.
        Some(_) => false,
        // Accepted but no completion — cannot happen for an already-readable
        // eventfd, so treat as unsupported rather than guessing.
        None => false,
    }
}

/// Does a provided-buffer ring register? (`IORING_REGISTER_PBUF_RING`, ≥ 5.19)
///
/// Registers a minimal ring and immediately unregisters it. Nothing is read
/// through it; the registration itself is the probe D14 OQ#14.3 calls for.
fn probe_buffer_ring(ring: &IoUring) -> bool {
    let Some((ptr, layout)) = alloc_buf_ring() else {
        return false;
    };

    // SAFETY: `ptr` is a page-aligned, zeroed allocation of `layout.size()`
    // bytes, large enough for `PROBE_RING_ENTRIES` entries, and it stays live
    // until `unregister_buf_ring` returns below.
    let registered = unsafe {
        ring.submitter()
            .register_buf_ring_with_flags(ptr as u64, PROBE_RING_ENTRIES, PROBE_BGID, 0)
            .is_ok()
    };

    if registered {
        let _ = ring.submitter().unregister_buf_ring(PROBE_BGID);
    }

    // SAFETY: `ptr` came from `alloc_zeroed` with this exact `layout`, the
    // kernel no longer references it, and it is not used again.
    unsafe { dealloc(ptr, layout) };

    registered
}

/// Is `IORING_RECV_MULTISHOT` accepted? (≥ 6.0)
///
/// Submits a multishot recv on a socketpair with an empty buffer group. A
/// kernel without the flag rejects it with `-EINVAL`/`-EOPNOTSUPP`; a
/// supporting kernel either waits (no CQE) or reports `-ENOBUFS` because the
/// group is empty. Both of the latter mean the flag exists.
///
/// Requires the buffer ring, so the caller only reaches this once
/// [`probe_buffer_ring`] returned true.
fn probe_recv_multishot(ring: &IoUring) -> bool {
    let Some((ptr, layout)) = alloc_buf_ring() else {
        return false;
    };

    // SAFETY: see `probe_buffer_ring` — same allocation contract.
    let registered = unsafe {
        ring.submitter()
            .register_buf_ring_with_flags(ptr as u64, PROBE_RING_ENTRIES, PROBE_BGID, 0)
            .is_ok()
    };
    if !registered {
        // SAFETY: allocation owned here, kernel never took it.
        unsafe { dealloc(ptr, layout) };
        return false;
    }

    let supported = probe_recv_multishot_inner(ring);

    let _ = ring.submitter().unregister_buf_ring(PROBE_BGID);
    // SAFETY: kernel released the ring above; allocation not used again.
    unsafe { dealloc(ptr, layout) };

    supported
}

fn probe_recv_multishot_inner(ring: &IoUring) -> bool {
    let Some((a, _b)) = probe_socketpair() else {
        return false;
    };

    let sqe = opcode::RecvMulti::new(types::Fd(a.as_raw_fd()), PROBE_BGID)
        .build()
        .user_data(2);

    // SAFETY: sole owner of this local ring's submission queue; `sqe` is fully
    // initialised and the sockets outlive the submission.
    unsafe {
        let mut sq = ring.submission_shared();
        if sq.push(&sqe).is_err() {
            return false;
        }
        sq.sync();
    }

    match await_one_cqe(ring, Duration::from_millis(50)) {
        // Rejected: the multishot flag is not understood.
        Some(result) if result == -libc::EINVAL || result == -libc::EOPNOTSUPP => false,
        // Any other completion (e.g. -ENOBUFS, the group is empty) means the
        // flag was accepted and the op ran.
        Some(_) => true,
        // No completion: accepted and waiting for data.
        None => true,
    }
}

// ── Small fixtures ──────────────────────────────────────────────────────────

/// An eventfd pre-loaded with `initial`, so it is immediately readable.
fn probe_eventfd(initial: u32) -> Option<OwnedFd> {
    // SAFETY: eventfd(2) with valid flags; the fd is immediately owned.
    let fd = unsafe { libc::eventfd(initial, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    if fd < 0 {
        return None;
    }
    // SAFETY: `fd` is a fresh, valid, owned descriptor.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// A connected `AF_UNIX` socket pair for the recv probe.
fn probe_socketpair() -> Option<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for socketpair to fill.
    let rc = unsafe {
        libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr())
    };
    if rc < 0 {
        return None;
    }
    // SAFETY: both are fresh, valid, owned descriptors.
    unsafe { Some((OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))) }
}

/// Allocate page-aligned zeroed memory for a provided-buffer ring.
///
/// The kernel maps the pages containing the ring, so the address must be
/// page-aligned; one page comfortably holds `PROBE_RING_ENTRIES` entries.
fn alloc_buf_ring() -> Option<(*mut u8, Layout)> {
    const PAGE: usize = 4096;
    let size = PAGE.max(PROBE_RING_ENTRIES as usize * std::mem::size_of::<IoUringBuf>());
    let layout = Layout::from_size_align(size, PAGE).ok()?;

    // SAFETY: `layout` has non-zero size.
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() {
        return None;
    }
    Some((ptr, layout))
}
