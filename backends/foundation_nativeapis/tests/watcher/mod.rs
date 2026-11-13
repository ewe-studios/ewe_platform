//! Integration tests: File watching.
//!
//! Sub-modules:
//! - `native` — InotifyWatcher + PollWatcher direct tests
//! - `windows` — WinWatcher (I/O completion port) tests, Windows only
//! - `direct` — FileWatcherTask + EventBroadcaster via direct TaskIterator::next_status()

mod direct;
mod native;

#[cfg(all(windows, feature = "watcher-windows"))]
mod windows;
