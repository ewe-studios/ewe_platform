//! Integration tests: Valtron executor lifecycle.
//!
//! All tests require `initialize_pool()` and exercise tasks through
//! `execute()` + `collect_one()`/`collect_result()`.
//!
//! Sub-modules:
//! - `executor` — FileWatcherTask, FdMonitorTask, EventBroadcaster through valtron
//! - `multi` — Multiple concurrent watchers through valtron multi-threaded pool

mod executor;
mod multi;
