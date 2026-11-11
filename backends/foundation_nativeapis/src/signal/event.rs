// Signal event types.

use std::time::Instant;

/// OS signals that can be received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// Ctrl+C — user interrupt (SIGINT on Unix, `CTRL_C_EVENT` on Windows)
    Interrupt,
    /// Termination request (SIGTERM on Unix, `CTRL_CLOSE_EVENT` on Windows)
    Terminate,
    /// Terminal hangup — reload config (SIGHUP, Unix only)
    Hangup,
    /// Quit with core dump (SIGQUIT, Unix only)
    Quit,
}

// Linux and Windows handlers record a signal in a pending mask, so they need
// one bit per kind. macOS delivers straight from the kqueue event, so it does
// not use the mask.
#[cfg(any(target_os = "linux", target_os = "windows"))]
impl SignalKind {
    /// The bit that names this kind in the pending mask.
    pub(crate) fn bit(self) -> u32 {
        match self {
            Self::Interrupt => 1,
            Self::Terminate => 2,
            Self::Hangup => 4,
            Self::Quit => 8,
        }
    }

    /// Every kind, in the order the pending mask delivers them.
    pub(crate) const ALL: [Self; 4] = [Self::Interrupt, Self::Terminate, Self::Hangup, Self::Quit];
}

impl core::fmt::Display for SignalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalKind::Interrupt => write!(f, "SIGINT"),
            SignalKind::Terminate => write!(f, "SIGTERM"),
            SignalKind::Hangup => write!(f, "SIGHUP"),
            SignalKind::Quit => write!(f, "SIGQUIT"),
        }
    }
}

/// A received signal event.
#[derive(Debug, Clone)]
pub struct SignalEvent {
    pub kind: SignalKind,
    pub timestamp: Instant,
}
