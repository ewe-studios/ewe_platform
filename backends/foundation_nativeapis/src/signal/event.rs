// Signal event types.

use std::time::Instant;

/// OS signals that can be received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// Ctrl+C — user interrupt (SIGINT on Unix, CTRL_C_EVENT on Windows)
    Interrupt,
    /// Termination request (SIGTERM on Unix, CTRL_CLOSE_EVENT on Windows)
    Terminate,
    /// Terminal hangup — reload config (SIGHUP, Unix only)
    Hangup,
    /// Quit with core dump (SIGQUIT, Unix only)
    Quit,
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
