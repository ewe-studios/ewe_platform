//! Reactor backend selection (F42 — Decision 14 OQ#14.3 / Scope §2).
//!
//! WHY: an explicitly requested backend that is silently swapped for another is
//! the no-silent-defaults anti-pattern, and its ops failure mode is the worst
//! one — a perf-critical deploy quietly running on epoll, discovered months
//! later from latency graphs. So the preference semantics are asymmetric on
//! purpose: `Auto` surfaces a documented choice, an explicit backend is a
//! requirement.
//!
//! WHAT: [`Backend`] (what we are running on), [`BackendPreference`] (what the
//! caller asked for), and [`select`], which resolves one to the other or fails
//! with the probe's concrete reason.
//!
//! HOW: the ladder is uring-completion → uring-readiness → epoll. Each demotion
//! is logged with the reason it demoted.
//!
//! ```text
//!   Auto     completion tier? -> UringCompletion
//!            readiness tier?  -> UringReadiness   (log: why not completion)
//!            otherwise        -> Epoll            (log: probe failure)
//!
//!   Uring    readiness tier?  -> UringReadiness / UringCompletion
//!            otherwise        -> hard error carrying the probe detail
//!
//!   Epoll    -> Epoll, no probe run
//! ```

use std::io;

#[cfg(all(target_os = "linux", feature = "uring"))]
use super::probe;

/// Whether the completion-mode selector exists in this build.
///
/// F42 shipped the probe and the ladder; F43 shipped the completion selector
/// (`uring_completion.rs`) the `UringCompletion` rung needs. With both in place
/// the ladder may name that rung, and does so whenever the kernel's completion
/// tier probes green.
#[cfg(all(target_os = "linux", feature = "uring"))]
pub const COMPLETION_IMPLEMENTED: bool = true;

/// The selector backend actually in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Linux `epoll`.
    Epoll,
    /// Linux `io_uring`, readiness mode (multishot poll).
    UringReadiness,
    /// Linux `io_uring`, completion mode (kernel-filled buffer rings).
    UringCompletion,
    /// BSD/macOS `kqueue`.
    Kqueue,
}

impl Backend {
    /// Whether this backend is one of the io_uring modes.
    pub fn is_uring(self) -> bool {
        matches!(self, Backend::UringReadiness | Backend::UringCompletion)
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Backend::Epoll => "epoll",
            Backend::UringReadiness => "io_uring(readiness)",
            Backend::UringCompletion => "io_uring(completion)",
            Backend::Kqueue => "kqueue",
        })
    }
}

/// What the caller wants, which is not always what they get.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendPreference {
    /// Walk the ladder and surface the choice. Never fails on a supported OS.
    #[default]
    Auto,
    /// io_uring is a **requirement**. Probe failure is a hard error carrying
    /// the probe detail — no silent demotion to epoll. Demotion *within*
    /// io_uring (completion → readiness on a 5.13–5.18 kernel) is fine: the
    /// caller asked for uring and got uring.
    Uring,
    /// epoll, with no probe run.
    Epoll,
}

/// Why a requested backend could not be provided.
#[derive(Debug)]
pub struct SelectionError {
    /// The backend that was explicitly requested.
    pub requested: BackendPreference,
    /// Operator-facing detail, e.g. the concrete probe failure.
    pub detail: String,
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot use requested reactor backend {:?}: {}", self.requested, self.detail)
    }
}

impl std::error::Error for SelectionError {}

impl From<SelectionError> for io::Error {
    fn from(e: SelectionError) -> io::Error {
        io::Error::new(io::ErrorKind::Unsupported, e.to_string())
    }
}

/// WHY: `Poll::new` must decide which selector to construct, once, from facts
/// about the running kernel rather than from compile-time guesses.
///
/// WHAT: resolve a [`BackendPreference`] into the [`Backend`] to construct.
///
/// HOW: see the ladder in the module docs. On non-Linux this is `Kqueue`; with
/// the `uring` feature off, Linux is always `Epoll` and an explicit
/// [`BackendPreference::Uring`] is a hard error (the code is not in the binary).
///
/// # Errors
/// [`SelectionError`] when an explicitly requested backend is unavailable.
/// [`BackendPreference::Auto`] never errors.
///
/// # Panics
/// Never panics.
#[cfg(all(target_os = "linux", feature = "uring"))]
pub fn select(preference: BackendPreference) -> Result<Backend, SelectionError> {
    if preference == BackendPreference::Epoll {
        tracing::info!(backend = %Backend::Epoll, "reactor backend selected (epoll requested; no probe run)");
        return Ok(Backend::Epoll);
    }
    resolve(probe::probe(), preference)
}

/// WHY: the interesting half of the ladder — demote on `Auto`, hard-error on an
/// explicit request — only runs when the probe *fails*, which cannot be
/// provoked on a healthy kernel. Splitting the decision from the syscall makes
/// both branches directly testable with a synthetic probe result.
///
/// WHAT: resolve a probe outcome and a preference into a backend.
///
/// HOW: on success, take the highest implemented tier the kernel supports. On
/// failure, `Auto` demotes to epoll with the reason logged, and `Uring` returns
/// a [`SelectionError`] carrying that same reason.
///
/// # Errors
/// [`SelectionError`] when `preference` is [`BackendPreference::Uring`] and the
/// probe failed.
///
/// # Panics
/// Panics if called with [`BackendPreference::Epoll`], which [`select`] handles
/// before the probe runs.
#[cfg(all(target_os = "linux", feature = "uring"))]
pub fn resolve(
    probed: Result<probe::UringCapabilities, probe::ProbeError>,
    preference: BackendPreference,
) -> Result<Backend, SelectionError> {
    match probed {
        Ok(caps) => {
            let backend = if caps.supports_completion() && COMPLETION_IMPLEMENTED {
                Backend::UringCompletion
            } else {
                if caps.supports_completion() {
                    tracing::info!(
                        "io_uring completion tier available but not implemented in this build \
                         (feature 43); using readiness mode"
                    );
                } else {
                    tracing::info!(
                        caps = %caps,
                        "io_uring completion tier unavailable (needs buffer rings + multishot \
                         recv); using readiness mode"
                    );
                }
                Backend::UringReadiness
            };
            tracing::info!(backend = %backend, caps = %caps, "reactor backend selected");
            Ok(backend)
        }
        Err(e) => match preference {
            // Explicit request: no silent demotion.
            BackendPreference::Uring => Err(SelectionError {
                requested: preference,
                detail: e.to_string(),
            }),
            // Auto: demote, and say exactly why.
            BackendPreference::Auto => {
                tracing::info!(
                    backend = %Backend::Epoll,
                    reason = %e,
                    "reactor backend demoted to epoll"
                );
                Ok(Backend::Epoll)
            }
            BackendPreference::Epoll => {
                unreachable!("BackendPreference::Epoll short-circuits before the probe")
            }
        },
    }
}

/// Selection when io_uring is not compiled into this build.
///
/// # Errors
/// [`SelectionError`] if io_uring is explicitly requested, since the code is
/// absent from the binary — reporting epoll instead would be a silent default.
///
/// # Panics
/// Never panics.
#[cfg(all(target_os = "linux", not(feature = "uring")))]
pub fn select(preference: BackendPreference) -> Result<Backend, SelectionError> {
    match preference {
        BackendPreference::Uring => Err(SelectionError {
            requested: preference,
            detail: "io_uring support is not compiled into this build (enable the `uring` feature)"
                .to_string(),
        }),
        _ => {
            tracing::info!(backend = %Backend::Epoll, "reactor backend selected");
            Ok(Backend::Epoll)
        }
    }
}

/// Selection on platforms whose reactor is kqueue.
///
/// # Errors
/// [`SelectionError`] if a Linux-only backend is explicitly requested.
///
/// # Panics
/// Never panics.
#[cfg(not(target_os = "linux"))]
pub fn select(preference: BackendPreference) -> Result<Backend, SelectionError> {
    match preference {
        BackendPreference::Uring | BackendPreference::Epoll => Err(SelectionError {
            requested: preference,
            detail: "io_uring and epoll are Linux-only; this platform uses kqueue".to_string(),
        }),
        BackendPreference::Auto => Ok(Backend::Kqueue),
    }
}
