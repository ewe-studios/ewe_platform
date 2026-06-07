/// IPC bus transport for VFS client/daemon communication.
///
/// Wraps the spec-34 IPC bus (`join()`, `Message<T>`, `EndpointSender/Receiver`)
/// to provide `VfsTransport` for the VFS client, and a `listen()` loop for the daemon.

use std::sync::Mutex;
use std::time::Duration;

use foundation_errstacks::ErrorTrace;

use crate::ipc::{self, join, Message, Options, Selector};
use crate::shared::ipc::Label;
use crate::shared::vfs::ipc_messages::{VfsRequest, VfsResponse};
use crate::shared::vfs::{VfsError, VfsResult};

/// The IPC bus identifier for the VFS daemon.
pub const VFS_BUS_IDENTIFIER: &str = "foundation-vfs";

/// Client-side IPC transport — sends requests, receives responses.
///
/// Implements `VfsTransport` for the VFS client to communicate with a remote daemon.
/// Sender and receiver are wrapped in Mutex because `VfsTransport::request` takes `&self`
/// but the receiver needs `&mut` to read.
pub struct ClientTransport {
    sender: Mutex<ipc::EndpointSender<VfsRequest>>,
    receiver: Mutex<ipc::EndpointReceiver<VfsResponse>>,
}

impl ClientTransport {
    /// Connect to an existing VFS daemon on the bus.
    ///
    /// Returns `Err` if no daemon is running (timeout).
    pub fn connect(timeout: Option<Duration>) -> VfsResult<Self> {
        let opts = Options::new(VFS_BUS_IDENTIFIER, Label::new("vfs-client"))
            .controller_affinity(false);
        let (sender, receiver) =
            join::<VfsRequest, VfsResponse>(opts, timeout).map_err(|e| {
                ErrorTrace::new(VfsError::Backend {
                    message: format!("failed to join VFS bus as client: {e}"),
                })
            })?;
        Ok(Self {
            sender: Mutex::new(sender),
            receiver: Mutex::new(receiver),
        })
    }
}

/// Implement `VfsTransport` for the client-side transport.
impl crate::shared::vfs::ipc_client::VfsTransport for ClientTransport {
    fn request(&self, req: VfsRequest) -> VfsResponse {
        let msg = Message::new(
            Selector::multicast(ipc::LabelOp::True),
            req,
        );
        if let Err(e) = self.sender.lock().unwrap().send(msg) {
            return VfsResponse::Error(format!("send failed: {e}"));
        }
        match self.receiver.lock().unwrap().recv(None) {
            Ok(msg) => msg.payload,
            Err(e) => VfsResponse::Error(format!("recv failed: {e}")),
        }
    }
}

/// Daemon-side IPC transport — receives requests, sends responses.
///
/// Used by the daemon to receive `VfsRequest` messages and send `VfsResponse` back.
pub struct DaemonTransport {
    sender: ipc::EndpointSender<VfsResponse>,
    receiver: ipc::EndpointReceiver<VfsRequest>,
}

impl DaemonTransport {
    /// Register as the VFS daemon server on the bus.
    pub fn serve() -> VfsResult<Self> {
        let opts = Options::new(VFS_BUS_IDENTIFIER, Label::new("vfs-daemon"))
            .controller_affinity(true);
        let (sender, receiver) =
            join::<VfsResponse, VfsRequest>(opts, Some(Duration::from_secs(30)))
                .map_err(|e| {
                    ErrorTrace::new(VfsError::Backend {
                        message: format!("failed to join VFS bus as server: {e}"),
                    })
                })?;
        Ok(Self { sender, receiver })
    }

    /// Receive a single request from the bus. Blocks until a request arrives.
    pub fn recv_request(&mut self) -> VfsResult<Message<VfsRequest>> {
        self.receiver.recv(None).map_err(|e| {
            ErrorTrace::new(VfsError::Backend {
                message: format!("failed to receive request: {e}"),
            })
        })
    }

    /// Receive a request with a timeout. Returns `None` on timeout.
    pub fn recv_request_timeout(
        &mut self,
        timeout: Duration,
    ) -> VfsResult<Option<Message<VfsRequest>>> {
        match self.receiver.recv(Some(timeout)) {
            Ok(msg) => Ok(Some(msg)),
            Err(crate::ipc::RecvError::Timeout) => Ok(None),
            Err(e) => Err(ErrorTrace::new(VfsError::Backend {
                message: format!("failed to receive request: {e}"),
            })),
        }
    }

    /// Send a response back through the bus.
    pub fn send_response(&self, response: VfsResponse, selector: Selector) -> VfsResult<()> {
        let msg = Message::new(selector, response);
        self.sender.send(msg).map_err(|e| {
            ErrorTrace::new(VfsError::Backend {
                message: format!("failed to send response: {e}"),
            })
        })
    }
}

/// Run the daemon listen loop: receive requests, dispatch to filesystem, send responses.
///
/// Blocks until `running` is set to `false` or the bus disconnects.
/// Returns `Ok(())` on clean shutdown, or `Err` on bus failure.
pub fn run_daemon_loop<F>(
    transport: &mut DaemonTransport,
    daemon: &crate::shared::vfs::ipc_daemon::VfsDaemon<F>,
    running: &std::sync::atomic::AtomicBool,
) -> VfsResult<()>
where
    F: crate::shared::vfs::VfsFileSystem + 'static,
    <F as crate::shared::vfs::VfsFileSystem>::File: Send + Sync + 'static,
    <F as crate::shared::vfs::VfsFileSystem>::SeekableFile: Send + Sync + 'static,
    <F as crate::shared::vfs::VfsFileSystem>::Directory: Send + Sync + 'static,
{
    let response_selector = Selector::multicast(ipc::LabelOp::True);

    while running.load(std::sync::atomic::Ordering::Relaxed) {
        match transport.recv_request_timeout(Duration::from_secs(1)) {
            Ok(Some(msg)) => {
                let response = daemon.dispatch(msg.payload);
                transport.send_response(response, response_selector.clone())?;
            }
            Ok(None) => {
                // Timeout — check running flag and continue.
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(())
}
