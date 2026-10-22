/// IPC bus transport for VFS client/daemon communication.
///
/// Wraps the spec-34 IPC bus (`join()`, `Message<T>`, `EndpointSender/Receiver`)
/// to provide `VfsTransport` for the VFS client, and a `listen()` loop for the daemon.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use foundation_errstacks::ErrorTrace;

use crate::ipc::{self, join, Message, Options, Selector};
use crate::shared::ipc::Label;
use crate::shared::vfs::exec_async::exec_async;
use crate::shared::vfs::ipc_client::VfsTransport;
use crate::shared::vfs::ipc_daemon::VfsDaemon;
use crate::shared::vfs::ipc_messages::{VfsRequest, VfsResponse};
use crate::shared::vfs::{AsyncVfsFileSystem, VfsError, VfsResult};

/// The IPC bus identifier for the VFS daemon.
pub const VFS_BUS_IDENTIFIER: &str = "foundation-vfs";

/// Client-side IPC transport — sends requests, receives responses.
///
/// Implements `VfsTransport` for the VFS client to talk to a remote daemon.
/// The bus has no request identifiers: responses come back in request order.
/// So one lock covers a send and its receive, and a released request (a
/// close from `Drop`, sent without a wait) is counted, and its response is
/// skipped before the next request reads its own.
///
/// The bus API is sync: `request` waits for the response on the calling
/// thread (see the bug backlog).
pub struct ClientTransport {
    inner: Mutex<ClientLink>,
}

struct ClientLink {
    sender: ipc::EndpointSender<VfsRequest>,
    receiver: ipc::EndpointReceiver<VfsResponse>,
    /// Responses of released requests that are still to be read and dropped.
    unread_responses: usize,
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
            inner: Mutex::new(ClientLink {
                sender,
                receiver,
                unread_responses: 0,
            }),
        })
    }

    fn link(&self) -> MutexGuard<'_, ClientLink> {
        // Every update keeps the link usable, so a poisoned lock is still used.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl ClientLink {
    fn send(&mut self, req: VfsRequest) -> Result<(), String> {
        let msg = Message::new(Selector::multicast(ipc::LabelOp::True), req);
        self.sender.send(msg).map_err(|e| format!("send failed: {e}"))
    }

    fn recv(&mut self) -> Result<VfsResponse, String> {
        self.receiver
            .recv(None)
            .map(|msg| msg.payload)
            .map_err(|e| format!("recv failed: {e}"))
    }
}

#[async_trait]
impl VfsTransport for ClientTransport {
    async fn request(&self, req: VfsRequest) -> VfsResponse {
        let mut link = self.link();
        while link.unread_responses > 0 {
            if let Err(e) = link.recv() {
                return VfsResponse::Error(e);
            }
            link.unread_responses -= 1;
        }
        if let Err(e) = link.send(req) {
            return VfsResponse::Error(e);
        }
        link.recv().unwrap_or_else(VfsResponse::Error)
    }

    fn release(&self, req: VfsRequest) {
        let mut link = self.link();
        // A failed send has no response to skip; the daemon drops the handle
        // when the client goes away.
        if link.send(req).is_ok() {
            link.unread_responses += 1;
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
/// This is a sync boundary: the bus API is sync, so the loop runs on its own
/// thread, and it waits for each `VfsDaemon::dispatch` through `exec_async`.
/// It blocks until `running` is set to `false` or the bus disconnects.
/// Returns `Ok(())` on clean shutdown, or `Err` on bus failure.
pub fn run_daemon_loop<F>(
    transport: &mut DaemonTransport,
    daemon: &std::sync::Arc<VfsDaemon<F>>,
    running: &AtomicBool,
) -> VfsResult<()>
where
    F: AsyncVfsFileSystem + 'static,
{
    let response_selector = Selector::multicast(ipc::LabelOp::True);

    while running.load(Ordering::Relaxed) {
        match transport.recv_request_timeout(Duration::from_secs(1)) {
            Ok(Some(msg)) => {
                let daemon = daemon.clone();
                let payload = msg.payload;
                let response = exec_async(async move { Ok(daemon.dispatch(payload).await) })?;
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
