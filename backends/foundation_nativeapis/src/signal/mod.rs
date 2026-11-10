//! Cross-platform signal handling — SIGINT, SIGTERM, SIGHUP, SIGQUIT.
//!
//! Linux:    eventfd + sigaction + epoll
//! macOS:    kqueue EVFILT_SIGNAL
//! Windows:  SetConsoleCtrlHandler + WaitForSingleObject
//!
//! # Delivery contract
//!
//! A signal is delivered to [`SignalBus`] when something polls the handle's
//! readiness. The Linux and Windows handlers only record the signal and wake
//! the poll. A handler that takes no lock and allocates no memory cannot
//! deadlock on a bus lock or on the allocator. [`SignalTask`] does this poll:
//! it parks on the handle, and the executor calls
//! [`foundation_core::valtron::EventReadiness::is_ready`] on the thread that
//! drives the task. A host that drives the handle itself also gets delivery.

mod bus;
mod error;
mod event;
mod handle;
mod task;

pub use bus::SignalBus;
pub use error::SignalError;
pub use event::{SignalEvent, SignalKind};
pub use handle::SignalHandle;
pub use task::SignalTask;

// Internal: global bus for signal handler delivery
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_context::Ctx;

#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::sync::atomic::AtomicU32;

static GLOBAL_BUS_PTR: AtomicU64 = AtomicU64::new(0);

/// Signals that a handler recorded and no poll has delivered yet.
///
/// Only the Linux and Windows handlers write it. macOS reads the signal number
/// from the kqueue event, so it does not need a mask.
#[cfg(any(target_os = "linux", target_os = "windows"))]
static PENDING: AtomicU32 = AtomicU32::new(0);

pub(crate) fn set_global_bus(bus: Arc<SignalBus>) {
    GLOBAL_BUS_PTR.store(Arc::into_raw(bus) as u64, Ordering::SeqCst);
}

/// Record a signal in the pending mask.
///
/// This is async-signal-safe: one lock-free atomic OR, with no lock and no
/// allocation.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn note_signal(kind: SignalKind) {
    PENDING.fetch_or(kind.bit(), Ordering::Relaxed);
}

/// Deliver every recorded signal to the bus.
///
/// Call this from an executor thread only, after the readiness poll woke on the
/// platform handle.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn deliver_pending_signals() {
    let mask = PENDING.swap(0, Ordering::AcqRel);
    for kind in SignalKind::ALL {
        if mask & kind.bit() != 0 {
            deliver_signal(kind);
        }
    }
}

pub(crate) fn deliver_signal(kind: SignalKind) {
    let ptr = GLOBAL_BUS_PTR.load(Ordering::SeqCst);
    if ptr != 0 {
        // SAFETY: we stored an Arc<SignalBus> via into_raw, never freed
        let bus: &SignalBus = unsafe { &*(ptr as *const SignalBus) };
        bus.deliver(&SignalEvent {
            kind,
            timestamp: bus.ctx().clock().now(),
        });
    }
}

// Platform-specific registration (called once at process startup)
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::register_signals;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::register_signals;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::register_signals;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn register_signals() -> Result<SignalHandle, SignalError> {
    Err(SignalError::UnsupportedPlatform)
}

/// Create a signal task and bus pair.
///
/// The task delivers a signal when something polls the handle's readiness. It
/// parks on the handle, so the executor's readiness poll delivers the recorded
/// signal (the eventfd on Linux, the kqueue event on macOS, the event on
/// Windows), and the task then returns it from its queue.
///
/// # Errors
/// Returns an error if signal registration fails (unsupported platform, OS error).
pub fn signal_task(ctx: Ctx) -> Result<(SignalTask, std::sync::Arc<SignalBus>), SignalError> {
    let bus = std::sync::Arc::new(SignalBus::new(ctx));
    set_global_bus(bus.clone());
    let handle = register_signals()?;
    let task = SignalTask::new(bus.clone(), handle);
    Ok((task, bus))
}
