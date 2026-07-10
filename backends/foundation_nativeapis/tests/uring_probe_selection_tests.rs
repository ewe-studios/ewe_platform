//! Runtime backend selection + functional probe (F42 — Decision 14 OQ#14.3).
//!
//! WHY: the rule under test is a *policy*: `Auto` surfaces a documented choice,
//! an explicit backend is a requirement that fails hard rather than being
//! silently swapped. Decision 14 calls the silent swap "the worst" ops failure
//! mode — a perf-critical deploy running on epoll, discovered from latency
//! graphs months later. Policy that isn't tested isn't policy.
//!
//! WHAT: the probe reports this kernel's real capabilities; the ladder resolves
//! (probe outcome × preference) into the right backend or the right error.
//!
//! HOW: probe assertions run against the live kernel. The demotion and
//! hard-error branches only trigger when the probe *fails*, which a healthy
//! kernel never does, so they are driven through `backend::resolve` with a
//! synthetic `ProbeError` — the same function `select` calls after probing.

#![cfg(target_os = "linux")]

use foundation_nativeapis::native::poll::{Backend, BackendPreference};

#[test]
fn epoll_is_always_constructible() {
    let poll = foundation_nativeapis::Poll::with_preference(BackendPreference::Epoll)
        .expect("epoll must always be available on Linux");
    assert_eq!(
        poll.backend(),
        Backend::Epoll,
        "an explicit epoll request must yield epoll"
    );
}

#[test]
fn auto_never_fails_on_linux() {
    let poll = foundation_nativeapis::Poll::with_preference(BackendPreference::Auto)
        .expect("Auto must always resolve on Linux");
    let backend = poll.backend();
    assert!(
        matches!(backend, Backend::Epoll | Backend::UringReadiness | Backend::UringCompletion),
        "Auto resolved to a non-Linux backend: {backend}"
    );
}

/// Before F42, `native_default_api()` returned `IOUring` on Linux, and the
/// watcher's `IOUring` arm built a stdlib `PollWatcher` — so the Linux default
/// file watcher was never inotify, and nothing said so.
#[test]
fn native_default_api_is_auto_not_a_silent_backend() {
    use foundation_nativeapis::shared::api::{native_default_api, NativeAPI};

    assert_eq!(
        native_default_api(),
        NativeAPI::Auto,
        "the default must be the surfaced ladder, not a specific backend"
    );
}

#[test]
fn default_poll_matches_auto() {
    let default = foundation_nativeapis::Poll::new().expect("Poll::new");
    let auto = foundation_nativeapis::Poll::with_preference(BackendPreference::Auto)
        .expect("Auto");
    assert_eq!(
        default.backend(),
        auto.backend(),
        "Poll::new() must be the Auto ladder, not a separate default"
    );
}

// ── Builds without the `uring` feature ──────────────────────────────────────

#[cfg(not(feature = "uring"))]
mod without_uring {
    use super::*;

    #[test]
    fn explicit_uring_fails_hard_when_not_compiled() {
        let err = foundation_nativeapis::Poll::with_preference(BackendPreference::Uring)
            .expect_err("io_uring is not in this build; the request must fail, not demote");

        assert_eq!(
            err.kind(),
            std::io::ErrorKind::Unsupported,
            "a missing backend is Unsupported"
        );
        assert!(
            err.to_string().contains("uring"),
            "the error must name the backend it could not provide: {err}"
        );
    }

    #[test]
    fn auto_lands_on_epoll_without_uring() {
        let poll = foundation_nativeapis::Poll::with_preference(BackendPreference::Auto)
            .expect("Auto");
        assert_eq!(poll.backend(), Backend::Epoll);
    }
}

// ── Builds with the `uring` feature ─────────────────────────────────────────

#[cfg(feature = "uring")]
mod with_uring {
    use super::*;

    use std::io;

    use foundation_nativeapis::native::poll::backend::{self, COMPLETION_IMPLEMENTED};
    use foundation_nativeapis::native::poll::probe::{self, ProbeError, UringCapabilities};

    /// The probe must reflect the kernel this test is running on. Every kernel
    /// that can run this suite at all supports multishot poll, because `probe`
    /// refuses to return `Ok` without it.
    #[test]
    fn probe_reports_readiness_tier_on_a_supporting_kernel() {
        match probe::probe() {
            Ok(caps) => {
                assert!(caps.poll_add, "POLL_ADD must be in the opcode bitmap: {caps}");
                assert!(
                    caps.poll_add_multi,
                    "probe returned Ok without multishot poll: {caps}"
                );
                assert!(
                    caps.supports_readiness(),
                    "readiness tier must hold whenever probe succeeds: {caps}"
                );
            }
            // A hardened host (kernel.io_uring_disabled=2, CONFIG_IO_URING=n)
            // is a legitimate environment for this suite. What must never
            // happen is a silent success.
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("io_uring"),
                    "a probe failure must name io_uring and its reason: {msg}"
                );
            }
        }
    }

    #[test]
    fn completion_tier_implies_readiness_tier() {
        if let Ok(caps) = probe::probe() {
            if caps.supports_completion() {
                assert!(
                    caps.supports_readiness(),
                    "completion tier without readiness tier is incoherent: {caps}"
                );
                assert!(caps.buffer_ring && caps.recv_multishot, "{caps}");
            }
        }
    }

    #[test]
    fn explicit_uring_yields_a_uring_backend_on_a_supporting_kernel() {
        match foundation_nativeapis::Poll::with_preference(BackendPreference::Uring) {
            Ok(poll) => assert!(
                poll.backend().is_uring(),
                "an explicit uring request resolved to {} — silent demotion is exactly \
                 what OQ#14.3 forbids",
                poll.backend()
            ),
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Unsupported);
                assert!(
                    e.to_string().contains("io_uring"),
                    "a hard failure must carry the probe detail: {e}"
                );
            }
        }
    }

    /// Until F43 lands the completion selector, the ladder must stop at
    /// readiness rather than naming a backend it cannot construct.
    #[test]
    fn ladder_does_not_select_completion_before_it_is_implemented() {
        if COMPLETION_IMPLEMENTED {
            return;
        }
        if let Ok(poll) = foundation_nativeapis::Poll::with_preference(BackendPreference::Auto) {
            assert_ne!(
                poll.backend(),
                Backend::UringCompletion,
                "the ladder selected completion mode, but this build has no completion selector"
            );
        }
    }

    // ── The failure branches, driven with a synthetic probe result ──────────

    fn probe_failure() -> ProbeError {
        ProbeError::Setup(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "kernel.io_uring_disabled=2",
        ))
    }

    #[test]
    fn auto_demotes_to_epoll_when_the_probe_fails() {
        let chosen = backend::resolve(Err(probe_failure()), BackendPreference::Auto)
            .expect("Auto must never fail; it demotes");
        assert_eq!(
            chosen,
            Backend::Epoll,
            "Auto must fall back to epoll when io_uring is unusable"
        );
    }

    #[test]
    fn explicit_uring_hard_errors_when_the_probe_fails() {
        let err = backend::resolve(Err(probe_failure()), BackendPreference::Uring)
            .expect_err("an explicit uring request must not be silently demoted");

        assert_eq!(err.requested, BackendPreference::Uring);
        assert!(
            err.detail.contains("kernel.io_uring_disabled=2"),
            "the hard error must carry the concrete probe detail so an operator can \
             act on it; got: {}",
            err.detail
        );

        let as_io: io::Error = err.into();
        assert_eq!(as_io.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn resolve_picks_readiness_when_completion_is_unavailable() {
        let caps = UringCapabilities {
            poll_add: true,
            poll_add_multi: true,
            buffer_ring: false,
            recv_multishot: false,
        };
        assert!(!caps.supports_completion());

        let chosen = backend::resolve(Ok(caps), BackendPreference::Auto).expect("resolve");
        assert_eq!(
            chosen,
            Backend::UringReadiness,
            "a 5.13–5.18 kernel supports readiness mode but not buffer rings"
        );
    }

    #[test]
    fn resolve_keeps_uring_when_only_completion_is_missing() {
        // The caller asked for uring and gets uring: demotion *within* io_uring
        // is explicitly allowed by OQ#14.3.
        let caps = UringCapabilities {
            poll_add: true,
            poll_add_multi: true,
            buffer_ring: false,
            recv_multishot: false,
        };
        let chosen = backend::resolve(Ok(caps), BackendPreference::Uring).expect("resolve");
        assert!(
            chosen.is_uring(),
            "completion→readiness demotion must stay on io_uring, got {chosen}"
        );
    }

    #[test]
    fn constructing_completion_mode_directly_is_refused_before_f43() {
        use foundation_nativeapis::native::poll::sys::unix::selector::dispatch::Selector;

        if COMPLETION_IMPLEMENTED {
            return;
        }
        let err = Selector::with_backend(Backend::UringCompletion)
            .expect_err("completion mode has no selector until F43");
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }
}
