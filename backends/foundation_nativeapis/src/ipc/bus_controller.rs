/// Top-level bus controller — manages the controller thread and endpoint lifecycle.

use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use bincode::config::standard;

use crate::ipc::errors::{IpcError, JoinError, RecvError, Result, SendError};
use crate::ipc::label::Label;
use crate::ipc::message::{BytesMessage, MessageBox, Message, ConnectMessage, ConnectMessageAck};
use crate::ipc::options::Options;
use crate::ipc::platform::linux::{connect_abstract, EncodedMessage, Remote};
use crate::ipc::platform::linux::bus_controller::start_controller;
use crate::ipc::selector::{Selector, SelectorMode};
use crate::ipc::util::EndpointID;
use crate::ipc::version::Version;

/// Global registry of active bus controllers.
struct ControllerRegistry {
    controllers: HashMap<String, ControllerHandle>,
}

impl ControllerRegistry {
    fn new() -> Self {
        Self {
            controllers: HashMap::new(),
        }
    }
}

fn global_controllers() -> &'static Mutex<ControllerRegistry> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Mutex<ControllerRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ControllerRegistry::new()))
}

struct ControllerHandle {
    running: Arc<AtomicBool>,
    thread: thread::JoinHandle<()>,
}

impl Drop for ControllerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Try to start or get an existing controller for a bus.
fn ensure_controller(
    bus_identifier: &str,
    label: Label,
) -> std::result::Result<bool, IpcError> {
    let mut registry = global_controllers().lock().unwrap();

    if registry.controllers.contains_key(bus_identifier) {
        return Ok(false);
    }

    match start_controller(bus_identifier, label) {
        Ok((running, handle)) => {
            registry.controllers.insert(
                bus_identifier.to_string(),
                ControllerHandle { running, thread: handle },
            );
            Ok(true)
        }
        Err(IpcError::Io(ref e)) if e.kind() == std::io::ErrorKind::AddrInUse => {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Join a message bus.
pub fn join<T: MessageBox, R: MessageBox>(
    options: Options,
    _timeout: Option<Duration>,
) -> std::result::Result<(EndpointSender<T>, EndpointReceiver<R>), JoinError> {
    let bus_id = options.identifier.clone();

    if options.controller_affinity {
        match ensure_controller(&bus_id, options.label.clone()) {
            Ok(owned) => {
                if owned {
                    tracing::info!("IPC: became bus controller for '{bus_id}'");
                }
            }
            Err(e) => {
                tracing::warn!("IPC: failed to start controller: {e}");
            }
        }
    }

    let local_version = Version::from_str_parts(
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
    );

    let conn = connect_with_retry(&bus_id, Duration::from_secs(2))?;

    let connect_msg = ConnectMessage {
        version: local_version,
        token: options.token.clone(),
        label: options.label.clone(),
    };

    let selector_bytes = bincode::encode_to_vec(&Selector::broadcast(), standard())
        .map_err(|e| JoinError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to encode selector: {e:?}"),
        )))?;

    let encoded = EncodedMessage::encode(local_version, &selector_bytes, &connect_msg, Vec::new())?;

    // Send connect message
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
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::WouldBlock
                || err.kind() == std::io::ErrorKind::Interrupted
            {
                continue;
            }
            return Err(JoinError::Io(err));
        }
        offset += written as usize;
    }

    // Receive ack
    let (ack_data, _fds) = EncodedMessage::recv(conn.as_fd())?;
    let (_ver_u32, _sel, payload_bytes) = EncodedMessage::decode(&ack_data)?;
    let ack: ConnectMessageAck = bincode::decode_from_slice(payload_bytes, standard())
        .map_err(|e| JoinError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to decode ack: {e:?}"),
        )))?
        .0;

    let endpoint_id = match ack {
        ConnectMessageAck::Ok(id) => id,
        ConnectMessageAck::ErrVersion(v) => return Err(JoinError::VersionMismatch(v)),
        ConnectMessageAck::ErrToken => return Err(JoinError::TokenMismatch),
    };

    // Leak the OwnedFd into a Remote so it lives independently
    let raw_fd = conn.as_raw_fd();
    std::mem::forget(conn);
    let remote = Remote::new(unsafe {
        crate::ipc::platform::linux::Fd::from_raw(raw_fd)
    });

    Ok((
        EndpointSender {
            remote: Arc::new(remote),
            endpoint_id,
            version: local_version,
            label: options.label.clone(),
            _phantom: std::marker::PhantomData,
        },
        EndpointReceiver {
            endpoint_id,
            version: local_version,
            label: options.label.clone(),
            _phantom: std::marker::PhantomData,
        },
    ))
}

fn connect_with_retry(address: &str, timeout: Duration) -> std::result::Result<OwnedFd, JoinError> {
    let deadline = std::time::Instant::now() + timeout;
    let mut last_err = None;

    while std::time::Instant::now() < deadline {
        match connect_abstract(address) {
            Ok(fd) => return Ok(fd),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    last_err = Some(e);
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                return Err(JoinError::Io(e));
            }
        }
    }

    if let Some(e) = last_err {
        Err(JoinError::Io(e))
    } else {
        Err(JoinError::Timeout)
    }
}

/// Sender side of an IPC bus endpoint. Cloneable.
pub struct EndpointSender<T: MessageBox> {
    remote: Arc<Remote>,
    endpoint_id: EndpointID,
    version: Version,
    label: Label,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: MessageBox> Clone for EndpointSender<T> {
    fn clone(&self) -> Self {
        Self {
            remote: self.remote.clone(),
            endpoint_id: self.endpoint_id,
            version: self.version,
            label: self.label.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: MessageBox> EndpointSender<T> {
    /// Send a message to the bus.
    pub fn send(&self, message: Message<T>) -> std::result::Result<(), SendError> {
        use crate::ipc::util::Align4;

        let selector_bytes = bincode::encode_to_vec(&message.selector, standard())
            .map_err(|e| SendError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to encode selector: {e:?}"),
            )))?;

        let payload_bytes = bincode::encode_to_vec(&message.payload, standard())
            .map_err(|e| SendError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to encode payload: {e:?}"),
            )))?;

        let sel_len = selector_bytes.len();
        let sel_aligned = sel_len.align4();
        let pay_len = payload_bytes.len();
        let pay_aligned = pay_len.align4();
        let total = 4 + 4 + sel_aligned + 4 + pay_aligned;
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(&self.version.to_u32().to_le_bytes());
        buf.extend_from_slice(&(sel_len as u32).to_le_bytes());
        buf.extend_from_slice(&selector_bytes);
        buf.resize(buf.len() + (sel_aligned - sel_len), 0);
        buf.extend_from_slice(&(pay_len as u32).to_le_bytes());
        buf.extend_from_slice(&payload_bytes);
        buf.resize(buf.len() + (pay_aligned - pay_len), 0);

        let fd_guard = self.remote.lock();
        let sock_fd = fd_guard.as_raw_fd();
        let mut offset = 0;
        while offset < buf.len() {
            let written = unsafe {
                libc::send(sock_fd, buf.as_ptr().add(offset) as *const _, buf.len() - offset, libc::MSG_NOSIGNAL)
            };
            if written < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                if err.kind() == std::io::ErrorKind::ConnectionReset || err.kind() == std::io::ErrorKind::BrokenPipe {
                    return Err(SendError::Disconnect);
                }
                return Err(SendError::Io(err));
            }
            offset += written as usize;
        }

        Ok(())
    }

    pub fn endpoint_id(&self) -> EndpointID {
        self.endpoint_id
    }
}

/// Receiver side of an IPC bus endpoint. Not cloneable.
pub struct EndpointReceiver<R: MessageBox> {
    endpoint_id: EndpointID,
    version: Version,
    label: Label,
    _phantom: std::marker::PhantomData<R>,
}

impl<R: MessageBox> EndpointReceiver<R> {
    pub fn try_recv(&self) -> std::result::Result<Option<R>, RecvError> {
        Ok(None)
    }

    pub fn endpoint_id(&self) -> EndpointID {
        self.endpoint_id
    }
}
