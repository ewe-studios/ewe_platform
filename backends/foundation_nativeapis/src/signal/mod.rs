// Cross-platform signal handling — SIGINT, SIGTERM, SIGHUP, SIGQUIT.
//
// Linux:    eventfd + sigaction + epoll
// macOS:    kqueue EVFILT_SIGNAL
// Windows:  SetConsoleCtrlHandler + WaitForSingleObject

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

static GLOBAL_BUS_PTR: AtomicU64 = AtomicU64::new(0);

pub(crate) fn set_global_bus(bus: Arc<SignalBus>) {
    GLOBAL_BUS_PTR.store(Arc::into_raw(bus) as u64, Ordering::SeqCst);
}

pub(crate) fn deliver_signal(kind: SignalKind) {
    let ptr = GLOBAL_BUS_PTR.load(Ordering::SeqCst);
    if ptr != 0 {
        // SAFETY: we stored an Arc<SignalBus> via into_raw, never freed
        let bus: &SignalBus = unsafe { &*(ptr as *const SignalBus) };
        bus.deliver(SignalEvent {
            kind,
            timestamp: std::time::Instant::now(),
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
/// # Errors
/// Returns an error if signal registration fails (unsupported platform, OS error).
pub fn signal_task() -> Result<(SignalTask, std::sync::Arc<SignalBus>), SignalError> {
    let bus = std::sync::Arc::new(SignalBus::new());
    set_global_bus(bus.clone());
    let handle = register_signals()?;
    let task = SignalTask::new(bus.clone(), handle);
    Ok((task, bus))
}
