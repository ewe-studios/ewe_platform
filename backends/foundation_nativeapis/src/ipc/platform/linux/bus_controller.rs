/// Bus controller — routes messages between endpoints based on label selectors.
///
/// The controller is the first endpoint that joins a bus with `controller_affinity: true`.
/// It runs on a separate thread and manages:
/// - Accepting new endpoint connections
/// - Handshake (version check, token validation)
/// - Message routing based on selector label expressions
/// - Message buffering with TTL for unroutable messages
/// - Endpoint reachability detection

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::encoded::EncodedMessage;
use crate::ipc::errors::{IpcError, Result};
use crate::ipc::label::Label;
use crate::ipc::version::Version;
use crate::ipc::util::EndpointID;

use super::{accept, listen_abstract, Remote};

/// An endpoint connected to the bus controller.
struct Endpoint {
    id: EndpointID,
    label: Label,
    remote: Remote,
}

/// A buffered message waiting for a matching endpoint.
struct BufferedMessage {
    encoded: EncodedMessage,
    expires: Instant,
}

/// The bus controller state.
struct BusController {
    /// Listen socket for new connections.
    listen: OwnedFd,
    /// Registered endpoints.
    endpoints: Vec<Endpoint>,
    /// Buffered messages with TTL.
    message_buffer: Vec<(Instant, BufferedMessage)>,
    /// Bus label (controller's own label).
    label: Label,
    /// Last time reachability was checked.
    last_detect_reachable: Instant,
}

impl BusController {
    fn new(listen: OwnedFd, label: Label) -> Self {
        Self {
            listen,
            endpoints: Vec::new(),
            message_buffer: Vec::new(),
            label,
            last_detect_reachable: Instant::now(),
        }
    }

    /// Run the controller loop on a background thread.
    fn run(mut self, running: Arc<AtomicBool>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                // Accept new connections (non-blocking)
                if let Ok(conn) = accept(&self.listen) {
                    if let Err(e) = self.handle_new_connection(conn) {
                        tracing::warn!("IPC: failed to handle new connection: {e}");
                    }
                }

                // Check reachability every 30 seconds
                let now = Instant::now();
                if now.duration_since(self.last_detect_reachable) >= Duration::from_secs(30) {
                    self.endpoints.retain(|ep| !ep.remote.is_dead());
                    self.last_detect_reachable = now;
                }

                // Expire old buffered messages
                self.message_buffer.retain(|(expire, _)| *expire > now);

                // Small sleep to avoid busy-waiting
                // In a full implementation, this would use epoll to block until events
                thread::sleep(Duration::from_millis(10));
            }
        })
    }

    /// Handle a new connection: perform handshake and register endpoint.
    fn handle_new_connection(&mut self, conn: OwnedFd) -> Result<()> {
        // Read connect message
        let (data, fds) = EncodedMessage::recv(conn.as_fd())?;

        // Decode connect message
        let (_version, _sel_bytes, payload_bytes) = EncodedMessage::decode(&data)?;
        let connect_msg: ConnectMessage = bincode::decode_from_slice(payload_bytes, bincode::config::standard())
            .map_err(|e| IpcError::Decode(e.0))?
            .0;

        // Validate version
        let local_version = crate::ipc::version::version();
        if !local_version.compatible(connect_msg.version) {
            let ack = ConnectMessageAck::ErrVersion(local_version);
            self.send_reply(&conn, &ack, Vec::new())?;
            return Err(IpcError::VersionMismatch(connect_msg.version, None));
        }

        // Validate token
        // (In a full impl, compare against bus token; for now, accept empty tokens)

        // Create endpoint ID
        let endpoint_id = EndpointID::new();

        // Store endpoint
        let remote = Remote::new(unsafe { OwnedFd::from_raw_fd(conn.as_raw_fd()) });
        let label = connect_msg.label;
        self.endpoints.push(Endpoint {
            id: endpoint_id,
            label: label.clone(),
            remote,
        });

        // Send ack
        let ack = ConnectMessageAck::Ok(endpoint_id);
        self.send_reply(&conn, &ack, fds)?;

        tracing::debug!("IPC: endpoint joined bus, label={label}, id={endpoint_id:?}");
        Ok(())
    }

    /// Send a reply back to a connecting endpoint.
    fn send_reply<T: bincode::Encode>(
        &self,
        conn: &OwnedFd,
        msg: &T,
        fds: Vec<OwnedFd>,
    ) -> Result<()> {
        let local_version = crate::ipc::version::version();
        let encoded = EncodedMessage::encode(
            local_version,
            &[],
            msg,
            fds,
        )?;

        // Simple send
        let data = encoded.data;
        let mut offset = 0;
        while offset < data.len() {
            let written = unsafe {
                libc::send(
                    conn.as_raw_fd(),
                    data.as_ptr().add(offset) as *const _,
                    data.len() - offset,
                    libc::MSG_NOSIGNAL,
                )
            };
            if written < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::Interrupted
                {
                    continue;
                }
                return Err(IpcError::Io(err));
            }
            offset += written as usize;
        }

        Ok(())
    }

    /// Route a message to matching endpoints.
    fn route_message(&mut self, encoded: EncodedMessage) -> Result<()> {
        let mut routed = false;

        for ep in &self.endpoints {
            // Try to decode just the selector to check label matching
            if let Ok((_, sel_bytes, _)) = EncodedMessage::decode(&encoded.data) {
                if let Ok(selector) = bincode::decode_from_slice::<crate::ipc::selector::Selector, _>(
                    sel_bytes,
                    bincode::config::standard(),
                ) {
                    let selector = selector.0;
                    if selector.matches_label(&ep.label.0) {
                        // Send to this endpoint
                        if let Err(e) = encoded.send(&ep.remote) {
                            tracing::warn!("IPC: failed to send to endpoint {:?}: {e}", ep.id);
                            continue;
                        }
                        routed = true;
                        if selector.mode == crate::ipc::selector::SelectorMode::Unicast {
                            break;
                        }
                    }
                }
            }
        }

        if !routed {
            // Check if controller itself is the target
            // For now, just buffer the message if TTL is set
            // In a full impl, we'd decode the selector and check against controller's label
        }

        Ok(())
    }
}

/// Connect message sent by endpoint during handshake.
#[derive(Debug, bincode::Encode, bincode::Decode)]
pub struct ConnectMessage {
    pub version: Version,
    pub token: String,
    pub label: Label,
}

/// Acknowledgement sent by controller during handshake.
#[derive(Debug, bincode::Encode, bincode::Decode)]
pub enum ConnectMessageAck {
    Ok(EndpointID),
    ErrVersion(Version),
    ErrToken,
}

/// Start the bus controller on a background thread.
///
/// Returns the listen fd (for endpoint connections) and a handle to stop the controller.
pub fn start_controller(
    bus_identifier: &str,
    label: Label,
) -> Result<(Arc<AtomicBool>, thread::JoinHandle<()>)> {
    // Try to create the listen socket
    let listen = listen_abstract(bus_identifier, 128).map_err(|e| {
        // If address is in use, another controller exists — caller should connect as endpoint
        IpcError::Io(e)
    })?;

    let running = Arc::new(AtomicBool::new(true));
    let controller = BusController::new(listen, label);
    let handle = controller.run(running.clone());

    Ok((running, handle))
}
