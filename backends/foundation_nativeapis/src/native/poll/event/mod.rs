/// Platform-specific `Event` types.
///
/// An `Event` is returned by [`Poll::poll()`](super::Poll::poll) when a
/// registered file descriptor becomes readable, writable, closed, or errors.
/// The struct is platform-specific internally but exposes a uniform interface.

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::Event;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
mod kqueue;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
pub use kqueue::Event;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Event;

pub mod source;
pub use source::Source;

/// Container for multiple events returned by `poll()`.
pub mod events;
pub use events::Events;
