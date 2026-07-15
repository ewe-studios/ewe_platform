use std::time::Duration;

use super::error::{Result, WatchError};
use super::watcher::NativeWatcher;

/// The native API backend to use for I/O readiness and file watching.
///
/// Two concerns share this enum, and they are not the same thing:
///
/// - **The reactor backend** (`Auto` / `IOUring` / `EPoll`) — how
///   [`crate::Poll`] waits for fd readiness. Selected at runtime by the
///   Decision 14 OQ#14.3 probe ladder; see [`crate::native::poll::backend`].
/// - **The file watcher** — how directory changes are observed. On Linux that
///   is inotify regardless of the reactor backend, because io_uring is not a
///   file-watching mechanism. `Poll` selects the stdlib metadata-polling
///   watcher instead.
///
/// Requesting `IOUring` therefore asserts that io_uring is usable on this host
/// (probe failure is a hard error, never a demotion) and still watches files
/// with inotify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeAPI {
    /// Walk the backend ladder and surface the choice. The documented default.
    #[default]
    Auto,

    /// io_uring-based I/O readiness. A **requirement**: if the functional probe
    /// says io_uring is unusable on this host, building fails with the probe's
    /// reason rather than quietly falling back (Decision 14 OQ#14.3).
    #[cfg(target_os = "linux")]
    IOUring,

    /// Traditional readiness-driven polling (epoll on Linux, kqueue elsewhere).
    EPoll,

    /// Stdlib-only metadata polling fallback.
    Poll,
}

#[cfg(target_os = "linux")]
impl NativeAPI {
    /// Map a watcher-facing API choice onto a reactor backend preference.
    fn backend_preference(self) -> Option<crate::native::poll::BackendPreference> {
        use crate::native::poll::BackendPreference;
        match self {
            NativeAPI::Auto => Some(BackendPreference::Auto),
            NativeAPI::IOUring => Some(BackendPreference::Uring),
            NativeAPI::EPoll => Some(BackendPreference::Epoll),
            // The stdlib watcher does not use the reactor at all.
            NativeAPI::Poll => None,
        }
    }
}

/// WHY: Decision 14 OQ#14.3 — `Auto` is a *documented selection policy* that
/// surfaces its choice, not a silent default. Before F42 this returned
/// `IOUring` on Linux, whose watcher branch then built a stdlib `PollWatcher`,
/// so the Linux default file watcher was never inotify and nothing said so.
///
/// WHAT: the default preferred API for the current platform.
///
/// HOW: `Auto` everywhere; the ladder resolves it and logs what it picked.
///
/// # Panics
/// Never panics.
pub fn native_default_api() -> NativeAPI {
    NativeAPI::Auto
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

    /// Build the watcher for `api`, first validating that the reactor backend it
    /// names is actually usable.
    ///
    /// The validation is what makes `NativeAPI::IOUring` honest: it runs the
    /// functional probe and propagates the concrete failure (e.g.
    /// "kernel.io_uring_disabled=2") instead of silently producing some other
    /// watcher. The watcher itself is inotify for every reactor backend —
    /// io_uring does not watch files.
    #[cfg(target_os = "linux")]
    fn try_build_api(api: NativeAPI, _timeout: Duration) -> Result<Box<dyn NativeWatcher>> {
        // A named reactor backend must be available before we claim to honour it.
        if let Some(preference) = api.backend_preference() {
            crate::native::poll::backend::select(preference)
                .map_err(|e| WatchError::Io(std::io::Error::from(e)))?;
        }

        match api {
            NativeAPI::Poll => Self::build_poll_watcher(),

            #[cfg(feature = "watcher-linux")]
            _ => {
                let w = crate::native::watcher::linux::InotifyWatcher::new()?;
                Ok(Box::new(w))
            }

            // Without the inotify watcher compiled in there is no native Linux
            // file watcher; the caller must ask for `NativeAPI::Poll` explicitly.
            #[cfg(not(feature = "watcher-linux"))]
            _ => Err(WatchError::UnsupportedPlatform),
        }
    }

    /// `Auto` and `EPoll` both mean "this platform's native watcher" off Linux.
    #[cfg(not(target_os = "linux"))]
    fn try_build_api(api: NativeAPI, _timeout: Duration) -> Result<Box<dyn NativeWatcher>> {
        match api {
            NativeAPI::Poll => Self::build_poll_watcher(),

            #[cfg(all(
                any(
                    target_os = "macos",
                    target_os = "ios",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "dragonfly",
                ),
                feature = "watcher-macos"
            ))]
            NativeAPI::Auto | NativeAPI::EPoll => {
                let w = crate::native::watcher::unix::KqueueWatcher::new()?;
                Ok(Box::new(w))
            }

            #[cfg(all(target_os = "windows", feature = "watcher-windows"))]
            NativeAPI::Auto | NativeAPI::EPoll => {
                let w = crate::native::watcher::windows::WinWatcher::new()?;
                Ok(Box::new(w))
            }

            #[cfg(not(any(
                all(
                    any(
                        target_os = "macos",
                        target_os = "ios",
                        target_os = "freebsd",
                        target_os = "netbsd",
                        target_os = "openbsd",
                        target_os = "dragonfly",
                    ),
                    feature = "watcher-macos"
                ),
                all(target_os = "windows", feature = "watcher-windows"),
            )))]
            NativeAPI::Auto | NativeAPI::EPoll => Err(WatchError::UnsupportedPlatform),
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
