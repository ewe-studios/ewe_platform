/// IPC bus transport for VFS client/daemon communication.
///
/// Wraps the spec-34 IPC bus (`join()`, `Message<T>`, `EndpointSender/Receiver`)
/// to provide `VfsTransport` for the VFS client, and a `listen()` loop for the
/// daemon. Every request carries an id; the daemon copies the id into the
/// response, so the client matches responses to requests by id, not by order.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use foundation_core::valtron::{Ctx, Pipe, PipeSender};
use foundation_errstacks::ErrorTrace;

use crate::ipc::{self, join, Message, Options, Selector};
use crate::shared::ipc::Label;
use crate::shared::vfs::exec_async::exec_async;
use crate::shared::vfs::ipc_client::VfsTransport;
use crate::shared::vfs::ipc_daemon::VfsDaemon;
use crate::shared::vfs::ipc_messages::{VfsRequest, VfsRequestBody, VfsResponse, VfsResponseBody};
use crate::shared::vfs::{AsyncVfsFileSystem, VfsError, VfsResult};

/// The IPC bus identifier for the VFS daemon.
pub const VFS_BUS_IDENTIFIER: &str = "foundation-vfs";

/// Config for the VFS IPC transport.
///
/// The join timeout is the wait for the other side of the bus. The poll
/// timeout is the longest the daemon or client reader blocks on one receive
/// before it checks its stop flag.
#[derive(Debug, Clone)]
pub struct VfsIpcConfig {
    identifier: String,
    join_timeout: Duration,
    poll_timeout: Duration,
}

impl VfsIpcConfig {
    /// Default join timeout: 30 s. nginx's default `proxy_connect_timeout` is
    /// 60 s; 30 s fails a missing daemon in half that time.
    pub const DEFAULT_JOIN_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default poll timeout: 1 s. One stop-flag check per second; shorter than
    /// Go's `http.Server` shutdown poll, which blocks until connections close.
    pub const DEFAULT_POLL_TIMEOUT: Duration = Duration::from_secs(1);

    /// New config with the documented defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identifier: VFS_BUS_IDENTIFIER.to_string(),
            join_timeout: Self::DEFAULT_JOIN_TIMEOUT,
            poll_timeout: Self::DEFAULT_POLL_TIMEOUT,
        }
    }

    /// The bus identifier the transport joins.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// The wait for the other side of the bus.
    #[must_use]
    pub fn join_timeout(&self) -> Duration {
        self.join_timeout
    }

    /// The longest one receive blocks before a stop-flag check.
    #[must_use]
    pub fn poll_timeout(&self) -> Duration {
        self.poll_timeout
    }

    /// Set the bus identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = identifier.into();
        self
    }

    /// Set the join timeout.
    #[must_use]
    pub fn with_join_timeout(mut self, timeout: Duration) -> Self {
        self.join_timeout = timeout;
        self
    }

    /// Set the poll timeout.
    #[must_use]
    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }
}

impl Default for VfsIpcConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // The map is rebuilt after a panic, so a poisoned lock is still usable.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Client-side IPC transport — sends requests, awaits responses by id.
///
/// The bus receive is blocking, so a reader thread owns the receiver and
/// routes each response to the request that waits on its id. Requests send
/// their message and await a pipe; they never block the valtron worker.
pub struct ClientTransport {
    sender: ipc::EndpointSender<VfsRequest>,
    pending: Arc<Mutex<HashMap<u64, PipeSender<VfsResponse>>>>,
    next_id: AtomicU64,
    config: VfsIpcConfig,
    shutdown: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

/// The reader thread: receive responses and hand each one to its waiting pipe.
fn spawn_client_reader(
    receiver: ipc::EndpointReceiver<VfsResponse>,
    pending: Arc<Mutex<HashMap<u64, PipeSender<VfsResponse>>>>,
    shutdown: Arc<AtomicBool>,
    poll_timeout: Duration,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut receiver = receiver;
        while !shutdown.load(Ordering::Relaxed) {
            match receiver.recv(Some(poll_timeout)) {
                Ok(msg) => {
                    let response = msg.payload;
                    // An unknown id is a released request (a close) whose
                    // response nobody waits for: drop it.
                    if let Some(sender) = lock(&pending).remove(&response.id) {
                        let _ = sender.try_send(response);
                    }
                }
                Err(crate::ipc::RecvError::Timeout) => {}
                Err(_) => break,
            }
        }
    })
}

impl ClientTransport {
    /// Connect to an existing VFS daemon on the default bus.
    ///
    /// Returns `Err` if no daemon is running within the join timeout.
    pub fn connect(timeout: Option<Duration>, ctx: Ctx) -> VfsResult<Self> {
        Self::connect_with_config(VfsIpcConfig::default(), timeout, ctx)
    }

    /// Connect with a config. `timeout`, when given, overrides the config join
    /// timeout for this one connect.
    pub fn connect_with_config(
        config: VfsIpcConfig,
        timeout: Option<Duration>,
        ctx: Ctx,
    ) -> VfsResult<Self> {
        let join_timeout = timeout.unwrap_or_else(|| config.join_timeout());
        let opts =
            Options::new(config.identifier(), Label::new("vfs-client")).controller_affinity(false);
        let (sender, receiver) = join::<VfsRequest, VfsResponse>(opts, Some(join_timeout), ctx)
            .map_err(|e| {
                ErrorTrace::new(VfsError::Backend {
                    message: format!("failed to join VFS bus as client: {e}"),
                })
            })?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let reader = spawn_client_reader(
            receiver,
            Arc::clone(&pending),
            Arc::clone(&shutdown),
            config.poll_timeout(),
        );
        Ok(Self {
            sender,
            pending,
            next_id: AtomicU64::new(1),
            config,
            shutdown,
            reader: Some(reader),
        })
    }

    /// The config this transport uses.
    #[must_use]
    pub fn config(&self) -> &VfsIpcConfig {
        &self.config
    }

    fn send(&self, req: VfsRequest) -> Result<(), String> {
        let msg = Message::new(Selector::multicast(ipc::LabelOp::True), req);
        self.sender
            .send(msg)
            .map_err(|e| format!("send failed: {e}"))
    }
}

impl Drop for ClientTransport {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

#[async_trait]
impl VfsTransport for ClientTransport {
    async fn request(&self, body: VfsRequestBody) -> VfsResponseBody {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = Pipe::new();
        lock(&self.pending).insert(id, sender);
        if let Err(e) = self.send(VfsRequest { id, body }) {
            lock(&self.pending).remove(&id);
            return VfsResponseBody::Error(e);
        }
        match receiver.receive().await {
            Some(response) => response.body,
            None => VfsResponseBody::Error("transport closed before a response arrived".into()),
        }
    }

    fn release(&self, body: VfsRequestBody) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        // The response is read and dropped by the reader; nobody waits for it.
        let _ = self.send(VfsRequest { id, body });
    }
}

/// Daemon-side IPC transport — receives requests, sends responses.
///
/// Used by the daemon to receive `VfsRequest` messages and send `VfsResponse`
/// back, matched to the request by id.
pub struct DaemonTransport {
    sender: ipc::EndpointSender<VfsResponse>,
    receiver: ipc::EndpointReceiver<VfsRequest>,
    config: VfsIpcConfig,
}

impl DaemonTransport {
    /// Register as the VFS daemon server on the default bus.
    pub fn serve(ctx: Ctx) -> VfsResult<Self> {
        Self::serve_with_config(VfsIpcConfig::default(), ctx)
    }

    /// Register as the VFS daemon server with a config.
    pub fn serve_with_config(config: VfsIpcConfig, ctx: Ctx) -> VfsResult<Self> {
        let opts =
            Options::new(config.identifier(), Label::new("vfs-daemon")).controller_affinity(true);
        let (sender, receiver) =
            join::<VfsResponse, VfsRequest>(opts, Some(config.join_timeout()), ctx).map_err(
                |e| {
                    ErrorTrace::new(VfsError::Backend {
                        message: format!("failed to join VFS bus as server: {e}"),
                    })
                },
            )?;
        Ok(Self {
            sender,
            receiver,
            config,
        })
    }

    /// The config this transport uses.
    #[must_use]
    pub fn config(&self) -> &VfsIpcConfig {
        &self.config
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

/// Run the daemon listen loop: receive requests, dispatch to filesystem, send
/// responses with the request id. The poll timeout comes from the transport
/// config. It blocks until `running` is set to `false` or the bus disconnects.
pub fn run_daemon_loop<F>(
    transport: &mut DaemonTransport,
    daemon: &Arc<VfsDaemon<F>>,
    running: &AtomicBool,
) -> VfsResult<()>
where
    F: AsyncVfsFileSystem + 'static,
{
    let poll_timeout = transport.config().poll_timeout();
    let response_selector = Selector::multicast(ipc::LabelOp::True);

    while running.load(Ordering::Relaxed) {
        match transport.recv_request_timeout(poll_timeout) {
            Ok(Some(msg)) => {
                let id = msg.payload.id;
                let body = msg.payload.body;
                let daemon = Arc::clone(daemon);
                let result = exec_async(async move { Ok(daemon.dispatch(body).await) })?;
                transport
                    .send_response(VfsResponse { id, body: result }, response_selector.clone())?;
            }
            Ok(None) => {
                // Timeout — check the running flag and loop again.
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(())
}
