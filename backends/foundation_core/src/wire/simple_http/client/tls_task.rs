//! TLS handshake task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides non-blocking TLS handshake execution that integrates with valtron
//! executors. Enables async-like TLS upgrades without async/await.
//!
//! WHAT: Implements `TlsHandshakeTask` which performs TLS handshakes through a
//! state machine (Init → Handshaking → Complete). Works with netcap TLS backends
//! (rustls, openssl, native-tls).
//!
//! HOW: State machine pattern where each `next()` call advances through handshake
//! states. Uses netcap connectors to perform actual TLS handshake. Sends result
//! via channel on completion.
//!
//! NOTE: This entire module is `#[cfg(not(target_arch = "wasm32"))]` at the module level,
//! so no additional WASM guards are needed within.

use crate::netcap::{Connection, RawStream};
use crate::synca::mpp::Sender;
use crate::valtron::{NoAction, TaskIterator, TaskStatus};

use crate::netcap::ssl::SSLConnector;

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
/// Performs actual TLS handshake using netcap connectors. Sends result via channel
/// on completion or error.
pub struct TlsHandshakeTask {
    /// SSL connector
    connector: SSLConnector,
    /// Current state of the handshake
    state: TlsHandshakeState,
    /// TCP connection to upgrade to TLS
    connection: Option<Connection>,
    /// Server Name Indication (hostname for TLS)
    sni: String,
    /// Channel to send the result
    sender: Option<Sender<Result<RawStream, String>>>,
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
            sni,
            sender: Some(sender),
        }
    }

    /// Performs the TLS handshake using the appropriate backend.
    ///
    /// WHY: Different TLS backends (rustls, openssl, native-tls) have different APIs.
    /// This method provides a unified interface.
    ///
    /// WHAT: Performs the actual TLS handshake and returns the upgraded stream.
    ///
    /// HOW: Uses feature flags to select the appropriate TLS backend connector.
    fn perform_handshake(&mut self) -> Result<RawStream, String> {
        let connection = self
            .connection
            .take()
            .ok_or_else(|| "No connection to upgrade".to_string())?;

        self.perform_rustls_handshake(connection)
    }

    fn perform_rustls_handshake(&self, connection: Connection) -> Result<RawStream, String> {
        let (tls_stream, _addr) = self
            .connector
            .from_tcp_stream(self.sni.clone(), connection)
            .map_err(|e: Box<dyn std::error::Error + Send + Sync>| e.to_string())?;

        RawStream::from_client_tls(tls_stream).map_err(|e| e.to_string())
    }
}

impl TaskIterator for TlsHandshakeTask {
    type Pending = TlsHandshakeState;
    type Ready = ();
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
                // Perform the actual TLS handshake
                match self.perform_handshake() {
                    Ok(tls_stream) => {
                        tracing::debug!("TLS handshake successful for {}", self.sni);

                        // Send result through channel
                        if let Some(sender) = self.sender.take() {
                            if let Err(e) = sender.send(Ok(tls_stream)) {
                                tracing::error!("Failed to send TLS handshake result: {:?}", e);
                            }
                        }

                        self.state = TlsHandshakeState::Complete;
                        Some(TaskStatus::Ready(()))
                    }
                    Err(e) => {
                        tracing::error!("TLS handshake failed for {}: {}", self.sni, e);

                        // Send error through channel
                        if let Some(sender) = self.sender.take() {
                            if let Err(send_err) = sender.send(Err(e.clone())) {
                                tracing::error!(
                                    "Failed to send TLS handshake error: {:?}",
                                    send_err
                                );
                            }
                        }

                        self.state = TlsHandshakeState::Error;
                        None
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_handshake_state_variants() {
        // Verify all state variants exist
        let _init = TlsHandshakeState::Init;
        let _handshaking = TlsHandshakeState::Handshaking;
        let _complete = TlsHandshakeState::Complete;
        let _error = TlsHandshakeState::Error;
    }

    #[test]
    fn test_tls_handshake_state_debug() {
        let state = TlsHandshakeState::Init;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Init"));
    }

    #[test]
    fn test_tls_handshake_task_is_task_iterator() {
        // Compile-time check that TlsHandshakeTask implements TaskIterator
        fn assert_task_iterator<T: TaskIterator>() {}
        assert_task_iterator::<TlsHandshakeTask>();
    }

    #[test]
    fn test_tls_handshake_task_associated_types() {
        // Verify associated types are correct
        use crate::valtron::TaskIterator;

        fn check_types<T>()
        where
            T: TaskIterator<Pending = TlsHandshakeState, Ready = (), Spawner = NoAction>,
        {
        }

        check_types::<TlsHandshakeTask>();
    }
}
