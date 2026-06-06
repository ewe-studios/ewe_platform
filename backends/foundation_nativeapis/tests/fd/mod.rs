//! Integration tests: File descriptor management.
//!
//! Sub-modules:
//! - `registration` — RegisteredFd edge-triggered readiness tracking
//! - `monitor` — FdMonitorTask callback invocation and lifecycle

mod registration;
mod monitor;
