//! Integration tests: File watching.
//!
//! Sub-modules:
//! - `native` — InotifyWatcher + PollWatcher direct tests
//! - `direct` — FileWatcherTask + EventBroadcaster via direct TaskIterator::next_status()

mod native;
mod direct;
