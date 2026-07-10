use std::time::Duration;

use super::error::{Result, WatchError};
use super::watcher::NativeWatcher;

/// The native API backend to use for I/O readiness and file watching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAPI {
    /// io_uring-based completion-driven I/O (Linux 5.1+).
    #[cfg(target_os = "linux")]
    IOUring,

    /// Traditional readiness-driven polling.
    EPoll,

    /// Stdlib-only metadata polling fallback.
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
pub struct WatcherBuilder {
    preferred: Option<NativeAPI>,
    fallbacks: Vec<NativeAPI>,
    poll_timeout: Duration,
}

impl WatcherBuilder {
    pub fn new() -> Self {
        Self {
            preferred: None,
            fallbacks: Vec::new(),
            poll_timeout: Duration::from_millis(100),
        }
    }

    pub fn preferred(mut self, api: NativeAPI) -> Self {
        self.preferred = Some(api);
        self
    }

    pub fn fallback(mut self, api: NativeAPI) -> Self {
        self.fallbacks.push(api);
        self
    }

    pub fn poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

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
                // F42: probe — the uring Selector constructor tries IoUring::new()
                // which fails on unsupported kernels. If it succeeds, the uring
                // backend is operational.
                match crate::native::poll::sys::Selector::new() {
                    Ok(_) => Self::build_poll_watcher(),
                    Err(e) => Err(e.into()),
                }
            }
            #[cfg(not(feature = "uring"))]
            NativeAPI::IOUring => Err(WatchError::UnsupportedPlatform),
            #[cfg(all(target_os = "linux", feature = "watcher-linux"))]
            NativeAPI::EPoll => {
                let w = crate::native::watcher::linux::InotifyWatcher::new()?;
                Ok(Box::new(w))
            }
            #[cfg(not(all(target_os = "linux", feature = "watcher-linux")))]
            NativeAPI::EPoll => Err(WatchError::UnsupportedPlatform),
            NativeAPI::Poll => Self::build_poll_watcher(),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn try_build_api(api: NativeAPI, _timeout: Duration) -> Result<Box<dyn NativeWatcher>> {
        match api {
            #[cfg(all(target_os = "macos", feature = "watcher-macos"))]
            NativeAPI::EPoll => {
                let w = crate::native::watcher::unix::KqueueWatcher::new()?;
                Ok(Box::new(w))
            }
            #[cfg(all(
                any(
                    target_os = "ios",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "dragonfly",
                ),
                feature = "watcher-macos"
            ))]
            NativeAPI::EPoll => {
                let w = crate::native::watcher::unix::KqueueWatcher::new()?;
                Ok(Box::new(w))
            }
            #[cfg(not(feature = "watcher-macos"))]
            NativeAPI::EPoll => Err(WatchError::UnsupportedPlatform),
            #[cfg(target_os = "windows")]
            NativeAPI::EPoll => {
                #[cfg(feature = "watcher-windows")]
                {
                    let w = crate::native::watcher::windows::WinWatcher::new()?;
                    return Ok(Box::new(w));
                }
                #[cfg(not(feature = "watcher-windows"))]
                Err(WatchError::UnsupportedPlatform)
            }
            NativeAPI::Poll => Self::build_poll_watcher(),
        }
    }

    fn build_poll_watcher() -> Result<Box<dyn NativeWatcher>> {
        Ok(Box::new(super::watcher::poll_watcher::PollWatcher::new()))
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
pub fn native_watcher() -> Result<Box<dyn NativeWatcher>> {
    WatcherBuilder::default().build()
}
