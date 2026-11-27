//! The libkrun driver and its runner (feature 09).
//!
//! The driver never calls libkrun (invariant I1). It writes
//! `<run_dir>/krun.json` and starts the runner program `sandbox-krun`. The
//! runner loads the library and calls `krun_start_enter`, which turns the
//! runner process into the virtual machine.
//!
//! The module is Unix only. The driver needs the process and file side effects
//! of the native control plane, and libkrun runs on Linux and on macOS with
//! Apple silicon.

/// The foreign function interface over `libloading`, for the runner only.
#[cfg(feature = "libkrun-runner")]
pub mod ffi;

/// The driver: one `sandbox-krun` process for one sandbox.
#[cfg(feature = "libkrun")]
pub mod driver;

#[cfg(feature = "libkrun")]
pub use driver::{capabilities, krun_exit_error, KrunLaunch, KrunPlan, LibkrunDriver};
