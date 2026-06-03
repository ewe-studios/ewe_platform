/// Bus controller — routes messages between endpoints based on label selectors.

use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
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

/// The bus controller state.
struct BusController {
    listen: OwnedFd,
    endpoints: Vec<Endpoint>,
    label: Label,
    last_detect_reachable: Instant,
}

impl BusController {
    fn new(listen: OwnedFd, label: Label) -> Self {
        Self {
            listen,
            endpoints: Vec::new(),
            label,
            last_detect_reachable: Instant::now(),
        }
    }

    fn run(mut self, running: Arc<AtomicBool>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                if let Ok(conn) = accept(&self.listen) {
                    if let Err(e) = self.handle_new_connection(conn) {
                        tracing::warn!("IPC: failed to handle new connection: {e}");
                    }
                }

                let now = Instant::now();
                if now.duration_since(self.last_detect_reachable) >= Duration::from_secs(30) {
                    self.endpoints.retain(|ep| !ep.remote.is_dead());
                    self.last_detect_reachable = now;
                }

                thread::sleep(Duration::from_millis(10));
            }
        })
    }

    fn handle_new_connection(&mut self, conn: OwnedFd) -> Result<()> {
        let (data, _fds) = EncodedMessage::recv(conn.as_fd())?;

        let (_version, _sel_bytes, payload_bytes) = EncodedMessage::decode(&data)?;
        let connect_msg: ConnectMessage = bincode::decode_from_slice(payload_bytes, bincode::config::standard())
            .map_err(|e| IpcError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to decode connect message: {e:?}"),
            )))?
            .0;

        let local_version = crate::ipc::version::version();
        if !local_version.compatible(connect_msg.version) {
            let ack = ConnectMessageAck::ErrVersion(local_version);
            self.send_reply(&conn, &ack)?;
            return Err(IpcError::VersionMismatch(connect_msg.version, None));
        }

        let endpoint_id = EndpointID::new();

        let raw_fd = conn.as_raw_fd();
        // Transfer ownership: the Remote now owns the fd
        std::mem::forget(conn);
        let remote = Remote::new(unsafe { crate::ipc::platform::linux::Fd::from_raw(raw_fd) });
        let label = connect_msg.label.clone();
        self.endpoints.push(Endpoint {
            id: endpoint_id,
            label: label.clone(),
            remote,
        });

        let ack = ConnectMessageAck::Ok(endpoint_id);
        self.send_reply_raw(&raw_fd, &ack)?;

        tracing::debug!("IPC: endpoint joined bus, label={label}, id={endpoint_id:?}");
        Ok(())
    }

    fn send_reply<T: bincode::Encode>(&self, conn: &OwnedFd, msg: &T) -> Result<()> {
        self.send_reply_raw(&conn.as_raw_fd(), msg)
    }

    fn send_reply_raw<T: bincode::Encode>(&self, fd: &i32, msg: &T) -> Result<()> {
        let local_version = crate::ipc::version::version();
        let selector_bytes = bincode::encode_to_vec(
            &crate::ipc::selector::Selector::broadcast(),
            bincode::config::standard(),
        ).map_err(IpcError::Encode)?;

        let payload_bytes = bincode::encode_to_vec(msg, bincode::config::standard())
            .map_err(IpcError::Encode)?;

        use crate::ipc::util::Align4;
        let sel_len = selector_bytes.len();
        let sel_aligned = sel_len.align4();
        let pay_len = payload_bytes.len();
        let pay_aligned = pay_len.align4();
        let total = 4 + 4 + sel_aligned + 4 + pay_aligned;
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(&local_version.to_u32().to_le_bytes());
        buf.extend_from_slice(&(sel_len as u32).to_le_bytes());
        buf.extend_from_slice(&selector_bytes);
        buf.resize(buf.len() + (sel_aligned - sel_len), 0);
        buf.extend_from_slice(&(pay_len as u32).to_le_bytes());
        buf.extend_from_slice(&payload_bytes);
        buf.resize(buf.len() + (pay_aligned - pay_len), 0);

        let mut offset = 0;
        while offset < buf.len() {
            let written = unsafe {
                libc::send(*fd, buf.as_ptr().add(offset) as *const _, buf.len() - offset, libc::MSG_NOSIGNAL)
            };
            if written < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock || err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(IpcError::Io(err));
            }
            offset += written as usize;
        }

        Ok(())
    }
}

/// Connect message sent by endpoint during handshake.
#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct ConnectMessage {
    pub version: Version,
    pub token: String,
    pub label: Label,
}

/// Acknowledgement sent by controller during handshake.
#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub enum ConnectMessageAck {
    Ok(EndpointID),
    ErrVersion(Version),
    ErrToken,
}

/// Start the bus controller on a background thread.
pub fn start_controller(
    bus_identifier: &str,
    label: Label,
) -> Result<(Arc<AtomicBool>, thread::JoinHandle<()>)> {
    let listen = listen_abstract(bus_identifier, 128).map_err(IpcError::Io)?;

    let running = Arc::new(AtomicBool::new(true));
    let controller = BusController::new(listen, label);
    let handle = controller.run(running.clone());

    Ok((running, handle))
}
