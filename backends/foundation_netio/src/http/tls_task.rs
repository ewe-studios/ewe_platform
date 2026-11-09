//! TLS handshake task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides non-blocking TLS handshake execution that integrates with valtron
//! executors. Enables async-like TLS upgrades without async/await.
//!
//! WHAT: Implements `TlsHandshakeTask` which performs TLS handshakes through a
//! state machine (Init → Handshaking → Complete).
//!
//! HOW: The task sets the socket non-blocking, starts the handshake, and drives
//! it until it finishes. While the peer must send more handshake data, the task
//! parks with `TaskStatus::Depends` on the socket readiness, so no worker waits
//! in a blocking handshake (issues #277 and #279).
//!
//! NOTE: This entire module is `#[cfg(not(target_family = "wasm"))]` at the module level,
//! so no additional WASM guards are needed within.

#![cfg(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]

use crate::native::readiness::{io_or_deadline, IoInterest};
use crate::netcap::ssl::SSLConnector;
use crate::netcap::{Connection, RawStream};
use foundation_core::synca::mpp::Sender;
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
use std::time::{Duration, Instant};

/// TLS handshake processing states.
///
/// WHY: TLS handshakes involve multiple steps that should not block.
/// Each state represents a distinct phase of the handshake lifecycle.
///
/// WHAT: Enum representing all possible states during TLS handshake.
///
/// HOW: State transitions occur in `TlsHandshakeTask::next()`. Each state
/// determines the next action or state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsHandshakeState {
    /// Initial state - preparing to start handshake
    Init,
    /// Performing TLS handshake
    Handshaking,
    /// Handshake completed successfully
    Complete,
    /// Handshake failed with error
    Error,
}

/// TLS handshake task implementing `TaskIterator`.
///
/// WHY: Provides non-blocking TLS handshake execution using iterator pattern.
/// Integrates with valtron executor for concurrent TLS operations.
///
/// WHAT: Stateful task that performs TLS handshakes through multiple phases.
/// Yields `TaskStatus` variants to indicate progress or completion.
///
/// HOW: Maintains internal state and advances through states on each `next()` call.
/// The pending handshake is kept across calls and resumed on socket readiness.
pub struct TlsHandshakeTask {
    /// SSL connector
    connector: SSLConnector,
    /// Current state of the handshake
    state: TlsHandshakeState,
    /// TCP connection to upgrade to TLS
    connection: Option<Connection>,
    /// The handshake in flight, once it started.
    pending: Option<crate::netcap::ssl::ClientSSLStream>,
    /// Server Name Indication (hostname for TLS)
    sni: String,
    /// Channel to send the result
    sender: Option<Sender<Result<RawStream, String>>>,
    /// Handshake deadline, counted from the start of the handshake.
    timeout: Option<Duration>,
    /// The instant the handshake must finish by.
    deadline: Option<Instant>,
}

impl TlsHandshakeTask {
    /// Creates a new TLS handshake task.
    ///
    /// # Arguments
    ///
    /// * `connection` - TCP connection to upgrade to TLS
    /// * `sni` - Server Name Indication (hostname for TLS certificate validation)
    /// * `sender` - Channel to send the result when handshake completes
    ///
    /// # Returns
    ///
    /// A new `TlsHandshakeTask` in the `Init` state.
    #[must_use]
    pub fn new(
        connection: Connection,
        sni: String,
        sender: Sender<Result<RawStream, String>>,
    ) -> Self {
        Self {
            state: TlsHandshakeState::Init,
            connector: SSLConnector::new(),
            connection: Some(connection),
            pending: None,
            sni,
            sender: Some(sender),
            timeout: None,
            deadline: None,
        }
    }

    /// Bounds the handshake with a timeout.
    ///
    /// WHY: limits are configuration, not constants (project rule 8). A caller
    /// can use its own configured connect timeout here.
    ///
    /// WHAT: sets the maximum time the handshake may take. `None` means no
    /// deadline.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Start the handshake on a non-blocking socket.
    ///
    /// The TLS backend returns a pending stream. The task drives the handshake
    /// from this pending state on each later call.
    fn start_handshake(&mut self) -> Result<crate::netcap::ssl::ClientSSLStream, String> {
        let connection = self
            .connection
            .take()
            .ok_or_else(|| "No connection to upgrade".to_string())?;

        // A non-blocking socket makes the handshake return `WouldBlock` instead
        // of waiting in the kernel.
        #[cfg(unix)]
        connection
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;

        let (tls_stream, _addr) = self
            .connector
            .from_tcp_stream(self.sni.clone(), connection)
            .map_err(|err: Box<dyn std::error::Error + Send + Sync>| err.to_string())?;

        Ok(tls_stream)
    }

    /// Build a `RawStream` from the finished handshake and send it.
    fn finish_handshake(
        &mut self,
        stream: crate::netcap::ssl::ClientSSLStream,
    ) -> Option<TaskStatus<(), TlsHandshakeState>> {
        if let Err(err) = stream.set_nonblocking(false) {
            return self.fail(err.to_string());
        }
        match RawStream::from_client_tls(stream) {
            Ok(raw) => {
                tracing::debug!("TLS handshake successful for {}", self.sni);
                if let Some(sender) = self.sender.take() {
                    if let Err(err) = sender.send(Ok(raw)) {
                        tracing::error!("Failed to send TLS handshake result: {:?}", err);
                    }
                }
                self.state = TlsHandshakeState::Complete;
                Some(TaskStatus::Ready(()))
            }
            Err(err) => self.fail(err.to_string()),
        }
    }

    /// Send the error to the waiting caller and end the task.
    fn fail(&mut self, message: String) -> Option<TaskStatus<(), TlsHandshakeState>> {
        tracing::error!("TLS handshake failed for {}: {}", self.sni, message);
        if let Some(sender) = self.sender.take() {
            if let Err(err) = sender.send(Err(message)) {
                tracing::error!("Failed to send TLS handshake error: {:?}", err);
            }
        }
        self.state = TlsHandshakeState::Error;
        None
    }
}

impl TaskIterator for TlsHandshakeTask {
    type Pending = TlsHandshakeState;
    type Ready = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.state {
            TlsHandshakeState::Init => {
                // Validate we have a connection
                if self.connection.is_none() {
                    tracing::error!("No connection to upgrade to TLS");
                    self.state = TlsHandshakeState::Error;
                    return None;
                }

                // Transition to handshaking
                self.state = TlsHandshakeState::Handshaking;
                Some(TaskStatus::Pending(TlsHandshakeState::Init))
            }
            TlsHandshakeState::Handshaking => {
                if self.pending.is_none() {
                    match self.start_handshake() {
                        Ok(stream) => {
                            self.pending = Some(stream);
                            if let Some(timeout) = self.timeout {
                                self.deadline = Some(ctx.clock().now() + timeout);
                            }
                        }
                        Err(err) => return self.fail(err),
                    }
                }

                let Some(stream) = self.pending.as_ref() else {
                    return self.fail("No TLS handshake in flight".to_string());
                };

                match stream.drive_handshake() {
                    Ok(()) => {
                        let stream = self
                            .pending
                            .take()
                            .expect("the pending handshake was checked above");
                        self.finish_handshake(stream)
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        if self
                            .deadline
                            .is_some_and(|deadline| ctx.clock().now() >= deadline)
                        {
                            return self.fail(format!("TLS handshake with {} timed out", self.sni));
                        }

                        #[cfg(unix)]
                        {
                            let Some(readiness) = stream.readiness(IoInterest::Readable) else {
                                return self.fail(
                                    "the TLS backend cannot report handshake readiness".to_string(),
                                );
                            };
                            let readiness = match self.deadline {
                                Some(deadline) => io_or_deadline(readiness, deadline, ctx.clock()),
                                None => readiness,
                            };
                            Some(TaskStatus::Depends(readiness))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = err;
                            self.fail(
                                "the TLS backend cannot report handshake readiness".to_string(),
                            )
                        }
                    }
                    Err(err) => self.fail(err.to_string()),
                }
            }
            TlsHandshakeState::Complete => {
                // Task is done
                None
            }
            TlsHandshakeState::Error => {
                // Task failed
                None
            }
        }
    }
}
