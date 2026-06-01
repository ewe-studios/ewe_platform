use std::time::Duration;

use crate::error::{Result, WatchError};
use crate::watcher::NativeWatcher;

/// The native API backend to use for I/O readiness and file watching.
///
/// Available options are feature-gated per platform — attempting to use
/// an unavailable option will result in a compile-time error via `cfg` gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAPI {
    /// io_uring-based completion-driven I/O (Linux 5.1+).
    /// Supports all IORING_OP_* operations, zero-copy, linked ops.
    #[cfg(target_os = "linux")]
    IOUring,

    /// Traditional readiness-driven polling:
    ///   Linux: epoll + inotify
    ///   macOS/BSD: kqueue + EVFILT_VNODE
    ///   Windows: IOCP + ReadDirectoryChangesW
    EPoll,

    /// Stdlib-only metadata polling fallback — works everywhere.
    /// Slow, not real-time. Use as last resort.
    Poll,
}

/// Returns the default preferred API for the current platform.
#[cfg(target_os = "linux")]
pub fn native_default_api() -> NativeAPI {
    NativeAPI::IOUring
}

#[cfg(not(target_os = "linux"))]
pub fn native_default_api() -> NativeAPI {
    NativeAPI::EPoll
}

/// Builder for constructing a `NativeWatcher` with preferred and fallback backends.
///
/// # Example
///
/// ```no_run
/// use foundation_nativeapis::WatcherBuilder;
///
/// // Linux: try io_uring first, fall back to epoll, then poll
/// let watcher = WatcherBuilder::default()
///     .preferred(NativeAPI::IOUring)
///     .fallback(NativeAPI::EPoll)
///     .fallback(NativeAPI::Poll)
///     .build();
/// ```
pub struct WatcherBuilder {
    preferred: Option<NativeAPI>,
    fallbacks: Vec<NativeAPI>,
    poll_timeout: Duration,
}

impl WatcherBuilder {
    /// Create a new builder with no preferred API.
    pub fn new() -> Self {
        Self {
            preferred: None,
            fallbacks: Vec::new(),
            poll_timeout: Duration::from_millis(100),
        }
    }

    /// Set the preferred API backend to try first.
    pub fn preferred(mut self, api: NativeAPI) -> Self {
        self.preferred = Some(api);
        self
    }

    /// Add a fallback API to try if the preferred (and earlier fallbacks) fail.
    pub fn fallback(mut self, api: NativeAPI) -> Self {
        self.fallbacks.push(api);
        self
    }

    /// Set the poll timeout used by the watcher.
    pub fn poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

    /// Build a `NativeWatcher` by trying the preferred API, then each fallback in order.
    ///
    /// Returns the first watcher that successfully initializes.
    /// If all backends fail, returns an error.
    pub fn build(self) -> Result<Box<dyn NativeWatcher>> {
        let apis = self.preferred.into_iter().chain(self.fallbacks);

        let mut last_err = None;
        for api in apis {
            match Self::try_build_api(api, self.poll_timeout) {
                Ok(watcher) => return Ok(watcher),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or(WatchError::AllBackendsFailed))
    }

    #[cfg(target_os = "linux")]
    fn try_build_api(api: NativeAPI, _timeout: Duration) -> Result<Box<dyn NativeWatcher>> {
        match api {
            #[cfg(feature = "uring")]
            NativeAPI::IOUring => {
                // TODO: implement uring-based watcher
                Err(WatchError::UnsupportedPlatform)
            }
            #[cfg(not(feature = "uring"))]
            NativeAPI::IOUring => {
                Err(WatchError::UnsupportedPlatform)
            }
            #[cfg(feature = "watcher-linux")]
            NativeAPI::EPoll => Self::build_inotify_watcher(),
            #[cfg(not(feature = "watcher-linux"))]
            NativeAPI::EPoll => {
                Err(WatchError::UnsupportedPlatform)
            }
            NativeAPI::Poll => Self::build_poll_watcher(),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn try_build_api(api: NativeAPI, _timeout: Duration) -> Result<Box<dyn NativeWatcher>> {
        match api {
            NativeAPI::EPoll => {
                // TODO: implement kqueue/IOCP watcher
                Err(WatchError::UnsupportedPlatform)
            }
            NativeAPI::Poll => Self::build_poll_watcher(),
        }
    }

    fn build_poll_watcher() -> Result<Box<dyn NativeWatcher>> {
        Ok(Box::new(crate::watcher::poll::PollWatcher::new()))
    }

    #[cfg(feature = "watcher-linux")]
    fn build_inotify_watcher() -> Result<Box<dyn NativeWatcher>> {
        match crate::watcher::linux::InotifyWatcher::new() {
            Ok(w) => Ok(Box::new(w)),
            Err(e) => Err(e),
        }
    }
}

impl Default for WatcherBuilder {
    fn default() -> Self {
        Self {
            preferred: Some(native_default_api()),
            fallbacks: vec![NativeAPI::Poll],
            poll_timeout: Duration::from_millis(100),
        }
    }
}

/// Create the best native watcher for the current platform.
///
/// Equivalent to `WatcherBuilder::default().build()`.
///
/// Platform defaults:
///   Linux: IOUring → Poll fallback
///   macOS/BSD: EPoll (kqueue) → Poll fallback
///   Windows: EPoll (IOCP) → Poll fallback
pub fn native_watcher() -> Result<Box<dyn NativeWatcher>> {
    WatcherBuilder::default().build()
}
