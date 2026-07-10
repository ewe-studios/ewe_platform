/// Unix platform selector and utilities.

/// Selector implementations for this platform.
///
/// On Linux, `epoll` is **always** compiled, and `uring` additionally when the
/// `uring` feature is on. Both must coexist in one build for two reasons:
///
/// 1. Decision 14 OQ#14.3 requires a *runtime* selection ladder
///    (uring-completion → uring-readiness → epoll) with epoll as the automatic
///    fallback. A backend that isn't compiled cannot be fallen back to.
/// 2. The F41 uring↔epoll parity suite drives both selectors in a single test
///    process, feeding identical stimuli to each and comparing the events.
///
/// `sys::Selector` names the type this build dispatches through.
#[cfg(target_os = "linux")]
pub mod selector {
    pub mod epoll;

    #[cfg(feature = "uring")]
    pub mod uring;
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
pub mod selector {
    pub mod kqueue;
}

#[cfg(target_os = "linux")]
pub mod waker {
    /// Linux waker using eventfd.
    mod eventfd;
}

/// SourceFd — register any raw file descriptor with the poll selector.
pub mod sourcefd;
pub use sourcefd::SourceFd;
