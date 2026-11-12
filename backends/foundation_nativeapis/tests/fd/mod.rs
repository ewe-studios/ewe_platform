//! Integration tests: File descriptor management.
//!
//! Sub-modules:
//! - `registration` — `RegisteredFd` edge-triggered readiness tracking
//! - `monitor` — `FdMonitorTask` callback invocation and lifecycle
// This suite uses Unix file descriptors, so it builds on Unix only.
#![cfg(unix)]

mod monitor;
mod registration;
