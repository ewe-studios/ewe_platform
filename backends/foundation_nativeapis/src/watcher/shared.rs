/// Thread-safe shared watcher wrapper.
///
/// `SharedNativeWatcher<T>` wraps a concrete `NativeWatcher` implementation
/// in `Arc<RwLock<T>>`, enabling safe concurrent access:
/// - Multiple threads can call `is_ready()` concurrently (shared read lock).
/// - Only one thread can call `poll()` at a time (exclusive write lock).
///
/// On level-triggered platforms (Linux, macOS/BSD), `is_ready()` performs
/// a readiness check with zero-timeout that doesn't consume events.
/// On Windows (IOCP), it peeks the internal event cache.

use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use foundation_core::valtron::EventReadiness;

use crate::error::Result;
use crate::event::WatchEvent;
use crate::watcher::NativeWatcher;

/// A thread-safe shared watcher.
///
/// Wraps a `NativeWatcher` in `Arc<RwLock<T>>` so it can be cloned and
/// shared across threads. Use `read()` / `write()` for fine-grained control,
/// or use the inherent convenience methods that handle locking automatically.
pub struct SharedNativeWatcher<T: NativeWatcher> {
    inner: Arc<RwLock<T>>,
}

impl<T: NativeWatcher> SharedNativeWatcher<T> {
    /// Wrap a watcher in a shared handle.
    pub fn new(watcher: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(watcher)),
        }
    }

    /// Clone this handle — shares the same underlying watcher.
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    /// Acquire a write lock for mutating access (e.g., `poll()`, `has_events()`, `watch()`).
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.inner.write().unwrap()
    }

    /// Check if events are ready WITHOUT consuming them.
    ///
    /// Uses a write lock since scanning and caching is inherently mutating.
    pub fn has_events(&self, timeout: Option<Duration>) -> bool {
        self.write().has_events(timeout)
    }

    /// Poll for events, consuming them.
    ///
    /// Uses an exclusive write lock. Only one thread can poll at a time.
    pub fn poll(&self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        self.write().poll(timeout)
    }

    /// Add a path to watch.
    pub fn watch(&self, path: &Path, recursive: bool) -> Result<()> {
        self.write().watch(path, recursive)
    }

    /// Remove a watched path.
    pub fn unwatch(&self, path: &Path) -> Result<()> {
        self.write().unwatch(path)
    }

    /// Remove all watches and release resources.
    pub fn clear(&self) -> Result<()> {
        self.write().clear()
    }
}

impl<T: NativeWatcher> Clone for SharedNativeWatcher<T> {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}

impl<T: NativeWatcher + 'static> EventReadiness for SharedNativeWatcher<T> {
    fn is_ready(&self, timeout: Option<Duration>) -> bool {
        self.has_events(timeout)
    }
}

/// A type-erased shared watcher for use as `dyn NativeWatcher`.
///
/// Use this when you need a single `dyn NativeWatcher + Send + Sync` that
/// can be cloned and shared. Unlike `SharedNativeWatcher<T>`, this erases
/// the concrete type.
pub struct SharedWatcher {
    inner: Arc<RwLock<Box<dyn NativeWatcher>>>,
}

impl SharedWatcher {
    /// Wrap any `NativeWatcher` in a type-erased shared handle.
    pub fn new<T: NativeWatcher + 'static>(watcher: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Box::new(watcher))),
        }
    }

    /// Wrap an already-boxed `dyn NativeWatcher` in a shared handle.
    pub fn from_boxed(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(watcher)),
        }
    }

    /// Clone this handle — shares the same underlying watcher.
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    /// Acquire a write lock.
    pub fn write(&self) -> RwLockWriteGuard<'_, Box<dyn NativeWatcher>> {
        self.inner.write().unwrap()
    }

    /// Check if events are ready WITHOUT consuming them.
    ///
    /// Uses a write lock since scanning and caching is inherently mutating.
    pub fn has_events(&self, timeout: Option<Duration>) -> bool {
        self.write().has_events(timeout)
    }

    /// Poll for events, consuming them.
    pub fn poll(&self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        self.write().poll(timeout)
    }

    /// Add a path to watch.
    pub fn watch(&self, path: &Path, recursive: bool) -> Result<()> {
        self.write().watch(path, recursive)
    }

    /// Remove a watched path.
    pub fn unwatch(&self, path: &Path) -> Result<()> {
        self.write().unwatch(path)
    }

    /// Remove all watches and release resources.
    pub fn clear(&self) -> Result<()> {
        self.write().clear()
    }
}

impl Clone for SharedWatcher {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl EventReadiness for SharedWatcher {
    fn is_ready(&self, timeout: Option<Duration>) -> bool {
        self.has_events(timeout)
    }
}

impl NativeWatcher for SharedWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        self.inner.write().unwrap().watch(path, recursive)
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.inner.write().unwrap().unwatch(path)
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        self.inner.write().unwrap().poll(timeout)
    }

    fn clear(&mut self) -> Result<()> {
        self.inner.write().unwrap().clear()
    }

    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        self.inner.write().unwrap().has_events(timeout)
    }
}
