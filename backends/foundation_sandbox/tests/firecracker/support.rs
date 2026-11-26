//! The support of the Firecracker API client tests.

use std::future::Future;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_context::{Clock, Ctx};
use foundation_core::env;
use foundation_core::valtron::SimulatedClock;
use foundation_sandbox::native::firecracker::client::{FirecrackerClient, FirecrackerLimits};
use foundation_sandbox::shared::limits::SandboxLimits;

use super::mock_socket::{self, read_request, Request};

/// Drives a future on the shared pool of the test binary.
pub fn block_on<F: Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// A new, empty directory for one test's socket, and the socket path in it.
pub fn socket_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    // A Unix socket address is short (about 104 bytes on macOS). The default
    // temporary directory is long on macOS, so use `/tmp`, which every Unix
    // host has.
    let base = if Path::new("/tmp").is_dir() {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let dir = base.join(format!("ewe-sandbox-fc-{}-{name}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the socket directory");
    dir.join("api.sock")
}

/// An HTTP response with the given status, content type and body.
pub fn response(status: &str, content_type: Option<&str>, body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len()).into_bytes();
    if let Some(content_type) = content_type {
        out.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

/// A `200 OK` JSON answer.
pub fn json_ok(body: &str) -> Vec<u8> {
    response("200 OK", Some("application/json"), body.as_bytes())
}

/// A `204 No Content` answer.
pub fn no_content() -> Vec<u8> {
    response("204 No Content", None, b"")
}

/// A `400 Bad Request` answer with the given body.
pub fn bad_request(body: &[u8]) -> Vec<u8> {
    response("400 Bad Request", Some("application/json"), body)
}

/// A mock Firecracker API server that answers every request with `respond`.
pub struct MockApi {
    /// The socket the client dials.
    pub socket: PathBuf,
    /// Every request the server received, in order.
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
}

impl MockApi {
    /// Starts the server on a new socket and answers each request with
    /// `respond`.
    pub fn start<F>(name: &str, respond: F) -> Self
    where
        F: Fn(&Request) -> Vec<u8> + Send + Sync + 'static,
    {
        let socket = socket_path(name);
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let respond = Arc::new(respond);
        mock_socket::serve(&socket, Arc::clone(&stop), move |mut stream| {
            if let Some(request) = read_request(&mut stream) {
                let answer = respond(&request);
                recorded
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(request);
                // A client that stops reading closes the socket; the write
                // then fails, and the handler ends.
                let _ = stream.write_all(&answer);
                let _ = stream.flush();
            }
        });
        Self {
            socket,
            requests,
            stop,
        }
    }

    /// Takes the requests the server received so far.
    pub fn take_requests(&self) -> Vec<Request> {
        std::mem::take(
            &mut *self
                .requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl Drop for MockApi {
    fn drop(&mut self) {
        mock_socket::stop(&self.socket, &self.stop);
        if let Some(dir) = self.socket.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// A mock API that accepts one request and never answers until it is released.
///
/// The time-out test uses it: the call stays pending, the test moves the
/// simulated clock past `rpc_timeout`, and the client ends the call.
pub struct HoldingApi {
    /// The socket the client dials.
    pub socket: PathBuf,
    /// Sends `()` to let the held handler return and close the connection.
    pub release: Sender<()>,
    stop: Arc<AtomicBool>,
}

impl HoldingApi {
    /// Starts the server. The returned receiver gives `()` after the server
    /// read the first request.
    pub fn start(name: &str) -> (Self, Receiver<()>) {
        let socket = socket_path(name);
        let stop = Arc::new(AtomicBool::new(false));
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        mock_socket::serve(&socket, Arc::clone(&stop), move |mut stream| {
            if read_request(&mut stream).is_some() {
                let _ = accepted_tx.send(());
                // Hold the connection open until the test releases it. The
                // stream drops at the end of this block and closes it.
                let _ = release_rx.recv();
            }
        });
        (
            Self {
                socket,
                release: release_tx,
                stop,
            },
            accepted_rx,
        )
    }
}

impl Drop for HoldingApi {
    fn drop(&mut self) {
        mock_socket::stop(&self.socket, &self.stop);
        if let Some(dir) = self.socket.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// A context with a simulated clock, a seeded random number generator and the
/// injected timer of the simulation.
#[must_use]
pub fn simulated_ctx(seed: u64) -> (Ctx, Arc<SimulatedClock>) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env = env::simulated(&ctx, seed).expect("the simulated environment builds");
    ctx.store(env);
    (ctx, clock)
}

/// A client with the given call time limit and error-message limit.
#[must_use]
pub fn client(
    ctx: &Ctx,
    socket: &Path,
    rpc_timeout: Duration,
    max_error_message: usize,
) -> FirecrackerClient {
    let limits = FirecrackerLimits::default()
        .with_rpc_timeout(rpc_timeout)
        .with_max_error_message(max_error_message);
    FirecrackerClient::with_limits(ctx, socket, &limits).expect("the client builds")
}

/// A client over the shared sandbox limits.
#[must_use]
pub fn sandbox_client(ctx: &Ctx, socket: &Path) -> FirecrackerClient {
    FirecrackerClient::new(ctx, socket, &SandboxLimits::default()).expect("the client builds")
}
