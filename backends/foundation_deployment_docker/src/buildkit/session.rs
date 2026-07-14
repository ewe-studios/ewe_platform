//! BuildKit build-context session — serves FileSync back to buildkitd.
//!
//! WHY: A `dockerfile.v0` Solve needs the Dockerfile + build context, which
//! buildkitd pulls from the *client* over a callback gRPC channel (the
//! `Control/Session` bidi stream). buildkitd multiplexes an HTTP/2 connection
//! over that stream and calls the client's `FileSync/DiffCopy`.
//!
//! WHAT: [`SessionServer`] serves the generated FileSync service (backed by a
//! local directory) on a loopback HTTP/2 socket, then bridges that socket to the
//! `Control/Session` bidi stream — so buildkitd's session traffic reaches our
//! own `foundation_connectrpc` server. No bespoke transport: the server is the
//! ordinary `grpc_echo` serving path; only a byte pump sits between it and the
//! session stream.
//!
//! HOW: `SessionServer::start` (1) serves a `Router` with `register_file_sync`
//! on `127.0.0.1:0`, (2) opens `Control/Session` with the required
//! `x-docker-expose-session-*` headers, (3) dials the loopback server and pumps
//! bytes both ways between that socket and the session bidi. Callers then run a
//! Solve carrying the returned session id.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, ProcedureCodecs, Router, Transport,
};
use foundation_core::synca::OnSignal;
use foundation_core::valtron;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use tracing::error;

use crate::buildkit::generated::fsutil::types::packet::PacketType;
use crate::buildkit::generated::fsutil::types::{Packet, Stat};
use crate::buildkit::generated::grpc::health::v1::health_check_response::ServingStatus;
use crate::buildkit::generated::grpc::health::v1::{HealthCheckRequest, HealthCheckResponse};
use crate::buildkit::generated::moby::filesync::v1::BytesMessage as FsBytesMessage;
use crate::buildkit::services::filesend::{self, register_file_send, FileSend};
use crate::buildkit::services::filesync::{self, register_file_sync, FileSync};
use crate::buildkit::services::health::{self, register_health, Health};
use crate::buildkit::types::BytesMessage;

use foundation_connectrpc::{ConnectError, ConnectResult, Request, Response};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

// ── FileSync implementation — serves a local directory ──────────────────────

/// A [`FileSync`] backed by a local build-context directory. Every `DiffCopy`
/// streams the same directory (matching the common `--local context=. --local
/// dockerfile=.` case).
#[derive(Clone)]
pub struct DirFileSync {
    root: Arc<PathBuf>,
}

/// One walked entry: its fsutil [`Stat`] and, for regular files, the bytes to
/// stream on request.
struct Entry {
    stat: Stat,
    content: Option<Vec<u8>>,
}

/// Go `os.FileMode` type bit for directories (`1 << 31`).
const GO_MODE_DIR: u32 = 0x8000_0000;

impl DirFileSync {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: Arc::new(root.into()) }
    }

    /// Walk the context directory into a deterministic (lexical) list of entries,
    /// each with an fsutil `Stat`; regular files also carry their bytes.
    fn walk(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        walk_dir(&self.root, &self.root, &mut out);
        out.sort_by(|a, b| a.stat.path.cmp(&b.stat.path));
        out
    }
}

fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<Entry>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        // fsutil paths are slash-separated and relative to the context root.
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if meta.is_dir() {
            out.push(Entry {
                stat: Stat { path: rel, mode: GO_MODE_DIR | 0o755, ..Default::default() },
                content: None,
            });
            walk_dir(root, &path, out);
        } else if meta.is_file() {
            let content = std::fs::read(&path).unwrap_or_default();
            out.push(Entry {
                stat: Stat {
                    path: rel,
                    mode: 0o644,
                    size: content.len() as i64,
                    ..Default::default()
                },
                content: Some(content),
            });
        }
    }
}

/// State machine driving the fsutil sender side of `DiffCopy` as an output
/// stream that also consumes the incoming request stream.
enum Phase {
    /// Emitting stat packet `i` of the walk.
    Stats(usize),
    /// Emitting the terminating empty-stat packet.
    EndStats,
    /// Serving `PACKET_REQ` → `PACKET_DATA` until the peer sends `PACKET_FIN`.
    Serving,
    Done,
}

struct DiffState<R> {
    entries: Vec<Entry>,
    phase: Phase,
    /// The incoming request stream (`PACKET_REQ` / `PACKET_FIN` from buildkitd).
    reqs: R,
    /// Data/EOF packets queued for the file currently being served.
    pending: std::collections::VecDeque<Packet>,
}

fn stat_packet(stat: Stat) -> Packet {
    Packet {
        r#type: PacketType::PACKET_STAT.into(),
        stat: buffa::MessageField::some(stat),
        ..Default::default()
    }
}

fn data_packet(id: u32, data: Vec<u8>) -> Packet {
    Packet { r#type: PacketType::PACKET_DATA.into(), ID: id, data, ..Default::default() }
}

impl DirFileSync {
    /// The shared `DiffCopy` driver used for both `diff_copy` and `tar_stream`.
    fn diff_copy_stream(
        &self,
        requests: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send + 'static,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send>>
    {
        use futures::StreamExt;

        let entries = self.walk();
        let state = DiffState {
            entries,
            phase: Phase::Stats(0),
            reqs: Box::pin(requests),
            pending: std::collections::VecDeque::new(),
        };

        let stream = futures::stream::unfold(state, |mut st| async move {
            loop {
                if let Some(pkt) = st.pending.pop_front() {
                    return Some((Ok(pkt), st));
                }
                match st.phase {
                    Phase::Stats(i) => {
                        if i < st.entries.len() {
                            let pkt = stat_packet(st.entries[i].stat.clone());
                            st.phase = Phase::Stats(i + 1);
                            return Some((Ok(pkt), st));
                        }
                        st.phase = Phase::EndStats;
                    }
                    Phase::EndStats => {
                        // Empty stat signals the end of the walk.
                        let pkt = stat_packet(Stat::default());
                        st.phase = Phase::Serving;
                        return Some((Ok(pkt), st));
                    }
                    Phase::Serving => {
                        match st.reqs.next().await {
                            Some(Ok(pkt)) => {
                                let kind = pkt.r#type.to_i32();
                                if kind == PacketType::PACKET_REQ as i32 {
                                    let id = pkt.ID;
                                    if let Some(entry) = st.entries.get(id as usize) {
                                        if let Some(content) = &entry.content {
                                            for chunk in content.chunks(32 * 1024) {
                                                st.pending.push_back(data_packet(id, chunk.to_vec()));
                                            }
                                        }
                                    }
                                    // Empty data packet = EOF for this id.
                                    st.pending.push_back(data_packet(id, Vec::new()));
                                } else if kind == PacketType::PACKET_FIN as i32 {
                                    st.pending.push_back(Packet {
                                        r#type: PacketType::PACKET_FIN.into(),
                                        ..Default::default()
                                    });
                                    st.phase = Phase::Done;
                                }
                                // continue the loop to flush pending
                            }
                            _ => {
                                st.phase = Phase::Done;
                            }
                        }
                    }
                    Phase::Done => return None,
                }
            }
        });

        stream.boxed()
    }
}

impl FileSync for DirFileSync {
    fn diff_copy(
        &self,
        _ctx: Ctx,
        requests: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send + 'static,
    ) -> impl core::future::Future<
        Output = foundation_connectrpc::ConnectResult<
            std::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send>>,
        >,
    > + Send {
        let this = self.clone();
        async move { Ok(this.diff_copy_stream(requests)) }
    }

    fn tar_stream(
        &self,
        _ctx: Ctx,
        requests: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send + 'static,
    ) -> impl core::future::Future<
        Output = foundation_connectrpc::ConnectResult<
            std::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<Packet>> + Send>>,
        >,
    > + Send {
        let this = self.clone();
        async move { Ok(this.diff_copy_stream(requests)) }
    }
}

// ── Health service: always SERVING — satisfies buildkit's monitorHealth ──────

/// Always returns `SERVING` — satisfies buildkit's `monitorHealth` so the
/// session connection stays alive past the HTTP/2 handshake. Without this,
/// buildkit tears down the session ~20ms after connecting.
pub struct HealthService;

impl Health for HealthService {
    fn check(
        &self,
        _ctx: Ctx,
        _request: Request<HealthCheckRequest>,
    ) -> impl core::future::Future<Output = ConnectResult<Response<HealthCheckResponse>>> + Send
    {
        async move {
            Ok(Response::new(HealthCheckResponse {
                status: ServingStatus::SERVING.into(),
                ..Default::default()
            }))
        }
    }
    // watch defaults to unimplemented — buildkit only calls Check.
}

// ── FileSend sink: receive exported artifacts from buildkitd ─────────────────

/// A [`FileSend`] sink that writes exported build artifacts to a file.
///
/// buildkitd's `oci` / `docker` / `tar` exporters call
/// `moby.filesync.v1.FileSend/DiffCopy` on the session and stream the archive
/// as `BytesMessage` chunks; the receiver writes each chunk and acknowledges
/// by ending its (empty) response stream once the input reaches EOF — the
/// same shape as buildkit's own `writeTargetFile`.
#[derive(Clone)]
pub struct FileExportSink {
    path: Arc<PathBuf>,
}

impl FileExportSink {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: Arc::new(path.into()) }
    }
}

impl FileSend for FileExportSink {
    fn diff_copy(
        &self,
        _ctx: Ctx,
        requests: impl futures::Stream<Item = ConnectResult<FsBytesMessage>> + Send + 'static,
    ) -> impl core::future::Future<
        Output = ConnectResult<
            std::pin::Pin<Box<dyn futures::Stream<Item = ConnectResult<FsBytesMessage>> + Send>>,
        >,
    > + Send {
        let path = self.path.clone();
        async move {
            use futures::StreamExt;

            let file = std::fs::File::create(path.as_ref())
                .map_err(|e| ConnectError::internal(format!("create export file: {e}")))?;

            // Drive the input inside the response stream so writing is pull-based:
            // each poll of our (never-yielding) output consumes input chunks. The
            // stream ends — sending clean trailers — when the input reaches EOF.
            let state = (Box::pin(requests), file);
            let out = futures::stream::unfold(state, |(mut reqs, mut file)| async move {
                use std::io::Write;
                loop {
                    match reqs.next().await {
                        Some(Ok(msg)) => {
                            if let Err(e) = file.write_all(&msg.data) {
                                error!(err = %e, "filesend: export write failed");
                                return Some((
                                    Err(ConnectError::internal(format!("export write: {e}"))
                                        .into()),
                                    (reqs, file),
                                ));
                            }
                        }
                        Some(Err(e)) => {
                            error!(err = %e, "filesend: request stream error");
                            return None;
                        }
                        None => {
                            if let Err(e) = file.flush() {
                                error!(err = %e, "filesend: export flush failed");
                            }
                            return None;
                        }
                    }
                }
            });
            let boxed: std::pin::Pin<
                Box<dyn futures::Stream<Item = ConnectResult<FsBytesMessage>> + Send>,
            > = Box::pin(out);
            Ok(boxed)
        }
    }
}

// ── Session server: serve FileSync on loopback + pump to the Session bidi ───

/// A running build-context session. Holds the session id (to pass in
/// `SolveRequest.Session`) and the resources kept alive for the build's
/// duration; dropping it tears the session down.
pub struct SessionServer {
    /// The BuildKit session id — set `SolveRequest.Session` to this.
    pub id: String,
    shutdown: Arc<OnSignal>,
    _server: Option<JoinHandle<()>>,
    _pump_down: Option<JoinHandle<()>>,
    _pump_up: Option<JoinHandle<()>>,
    _pump_send: Option<JoinHandle<()>>,
}

/// Configures the sidecar services a [`SessionServer`] exposes to buildkitd.
///
/// The context directory (FileSync) and Health are always served; everything
/// else is opt-in. Finish with [`start`](Self::start) (fresh `H2Transport`
/// per the control authority) or [`start_with`](Self::start_with) (caller-
/// supplied transport — e.g. to share `BuildKitClient`'s Unix-socket or
/// pooled transport).
pub struct SessionBuilder {
    context_dir: PathBuf,
    export_file: Option<PathBuf>,
}

impl SessionBuilder {
    /// Receive exported build artifacts into `path`. Registers the
    /// `FileSend/DiffCopy` sidecar service — required for the `oci`, `docker`
    /// and `tar` exporters, which stream the archive back over the session.
    #[must_use]
    pub fn export_to_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.export_file = Some(path.into());
        self
    }

    /// Start the session against `control_authority` on a fresh
    /// [`H2Transport`] (one TCP connection per call).
    ///
    /// # Errors
    ///
    /// Returns an error if the loopback server cannot bind, the Session
    /// stream cannot be opened, or the loopback dial fails.
    pub async fn start(self, control_authority: &str) -> Result<SessionServer, BoxErr> {
        let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
        self.start_with(transport, control_authority).await
    }

    /// Start the session on a **caller-supplied** [`Transport`] — e.g.
    /// [`BuildKitClient::transport`], so the Session bidi dials the same
    /// Unix socket / pooled connection as the control-plane RPCs.
    ///
    /// # Errors
    ///
    /// Returns an error if the loopback server cannot bind, the Session
    /// stream cannot be opened, or the loopback dial fails.
    pub async fn start_with(
        self,
        control_transport: Arc<dyn Transport>,
        control_authority: &str,
    ) -> Result<SessionServer, BoxErr> {
        let id = new_session_id();

        // 1. Session gRPC server on a loopback h2c socket. Every registered
        //    method is routed AND advertised via a repeated
        //    `x-docker-expose-session-grpc-method` header — buildkitd only
        //    calls methods the session declares.
        let mut methods: Vec<&'static str> = Vec::new();
        let mut router = Router::new();
        register_file_sync(&mut router, Arc::new(DirFileSync::new(self.context_dir)));
        methods.push(filesync::procedure::DIFF_COPY);
        methods.push(filesync::procedure::TAR_STREAM);
        register_health(&mut router, Arc::new(HealthService));
        methods.push(health::procedure::CHECK);
        methods.push(health::procedure::WATCH);
        if let Some(path) = &self.export_file {
            register_file_send(&mut router, Arc::new(FileExportSink::new(path)));
            methods.push(filesend::procedure::DIFF_COPY);
        }

        let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
        let mut app = HttpApp::new_h2_serve();
        for method in &methods {
            app.route_any_h2(method, rpc.clone());
        }

        let listener = TcpListener::bind("127.0.0.1:0")?;
        let local_addr = listener.local_addr()?;
        let shutdown = Arc::new(OnSignal::new());

        let server_shutdown = shutdown.clone();
        let server = HttpServer::from_app(ServerApp::http2(app), &local_addr.to_string());
        let server_handle = std::thread::spawn(move || {
            server.serve_with_listener(&listener, &server_shutdown);
        });

        // 2. Open Control/Session with the required session metadata headers.
        let mut options = ClientOptions::new()
            .with_grpc()
            .with_header("x-docker-expose-session-uuid".to_string(), id.clone())
            .with_header("x-docker-expose-session-name".to_string(), "ewe");
        for method in &methods {
            options = options.with_header(
                "x-docker-expose-session-grpc-method".to_string(),
                *method,
            );
        }
        let session_client = Client::<BytesMessage, BytesMessage>::new(
            control_transport,
            &format!("http://{control_authority}/moby.buildkit.v1.Control/Session"),
            ProcedureCodecs::defaults(),
            options,
        )?;

        let mut bidi = session_client
            .bidi_stream(Ctx::background(), futures::stream::pending::<BytesMessage>())
            .await?;

        // Check the gRPC response headers — do we have grpc-status?
        match bidi.response_headers().await {
            Ok(h) => {
                let mut s = String::new();
                for (k, vs) in h.iter() { s.push_str(&format!("{k}={vs:?} ")); }
                eprintln!("[sess] bidi response headers: {s}");
            }
            Err(e) => eprintln!("[sess] bidi response headers FAILED: {e}"),
        }
        // NOTE: never send anything on the bidi before the loopback server's
        // SETTINGS. RFC 7540 §3.5: the first frame the server sends MUST be
        // SETTINGS — grpc-go treats any other first frame (e.g. a PING) as a
        // connection error and tears the Level 2 transport down.
        let (mut sender, mut receiver) = bidi.split();
        // 3. Dial our own loopback server.
        let tcp = TcpStream::connect(local_addr)?;
        let mut tcp_read = tcp.try_clone()?;
        let mut tcp_write = tcp;

        // buildkitd → loopback: async receive → std channel → blocking write.
        let (down_tx, down_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let read_pool = valtron::from_future(Box::pin(async move {
            loop {
                match receiver.receive().await {
                    Ok(Some(msg)) => {
                        let tag = h2_frame_tag(&msg.data);
                        let hex: String = msg.data.iter()
                            .map(|b| format!("{b:02x}")).collect();
                        eprintln!("[sess] bk->me {:>3}b {tag:<14} {hex}", msg.data.len());
                        if let Err(e) = down_tx.send(msg.data) {
                            error!(err = %e, "[sess] down_tx send failed");
                            break;
                        }
                    }
                    Ok(None) => {
                        eprintln!("[sess] bk->me: stream ended (Ok None)");
                        break;
                    }
                    Err(e) => {
                        error!(err = %e, "[sess] bk->me: receive error");
                        break;
                    }
                }
            }
        }));
        valtron::send(read_pool).map_err(|e| format!("spawn session receive task: {e}"))?;
        let write_thread = std::thread::spawn(move || {
            while let Ok(data) = down_rx.recv() {
                if let Err(e) = tcp_write.write_all(&data) {
                    error!(err = %e, "[sess] tcp_write failed");
                    break;
                }
                if let Err(e) = tcp_write.flush() {
                    error!(err = %e, "[sess] tcp_write flush failed");
                    break;
                }
            }
            if let Err(e) = tcp_write.shutdown(std::net::Shutdown::Both) {
                eprintln!("[sess] tcp shutdown: {e}");
            }
        });

        // loopback → buildkitd: blocking read → std::sync::mpsc → DEDICATED
        // OS thread that calls sender.send() via futures_lite::block_on. This
        // eliminates valtron pool-polling latency — the thread sleeps on recv()
        // and wakes instantly when data arrives, keeping the tunnel's added
        // round-trip latency to a minimum.
        let (up_tx, up_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let start = std::time::Instant::now();
        let read_thread = std::thread::spawn(move || {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                match tcp_read.read(&mut buf) {
                    Ok(0) => { eprintln!("[sess] loopback read 0 @ {:?}", start.elapsed()); break; }
                    Err(e) => { eprintln!("[sess] loopback read err @ {:?}: {e}", start.elapsed()); break; }
                    Ok(n) => {
                        let tag = h2_frame_tag(&buf[..n]);
                        eprintln!("[sess] me<-lb {:>3}b @{:>6.1?}ms {tag:<14}", n, start.elapsed().as_secs_f64() * 1000.0);
                        if up_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        // Dedicated thread: recv() blocks the OS thread, wakes instantly,
        // block_on drives the async send future to completion synchronously.
        let send_thread = std::thread::spawn(move || {
            // sender is moved in via `move` closure — it is now owned here.
            while let Ok(data) = up_rx.recv() {
                let tag = h2_frame_tag(&data);
                eprintln!("[sess] me->bk {:>3}b {tag:<14}", data.len());
                let msg = BytesMessage { data, ..Default::default() };
                if let Err(e) = futures_lite::future::block_on(sender.send(&msg)) {
                    error!(err = %e, "[sess] me->buildkit send failed");
                    break;
                }
            }
            eprintln!("[sess] me->bk DONE — closing sender");
            let _ = futures_lite::future::block_on(sender.close_request());
        });

        Ok(SessionServer {
            id,
            shutdown,
            _server: Some(server_handle),
            _pump_down: Some(write_thread),
            _pump_up: Some(read_thread),
            _pump_send: Some(send_thread),
        })
    }
}

impl SessionServer {
    /// Start configuring a session for `context_dir` — see [`SessionBuilder`]
    /// for the optional sidecar services (artifact export, …).
    pub fn builder(context_dir: impl Into<PathBuf>) -> SessionBuilder {
        SessionBuilder { context_dir: context_dir.into(), export_file: None }
    }

    /// Serve a FileSync session for `context_dir` and attach it to the
    /// Control/Session stream at `control_authority` on a fresh
    /// [`H2Transport`]. Shorthand for `builder(context_dir).start(..)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the loopback server cannot bind, the Session
    /// stream cannot be opened, or the loopback dial fails.
    pub async fn start(
        control_authority: &str,
        context_dir: impl Into<PathBuf>,
    ) -> Result<Self, BoxErr> {
        Self::builder(context_dir).start(control_authority).await
    }

    /// Like [`start`](Self::start), but on a **caller-supplied**
    /// [`Transport`] (e.g. [`BuildKitClient::transport`] for Unix-socket or
    /// pooled connections). Shorthand for `builder(..).start_with(..)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the loopback server cannot bind, the Session
    /// stream cannot be opened, or the loopback dial fails.
    pub async fn start_with(
        transport: Arc<dyn Transport>,
        control_authority: &str,
        context_dir: impl Into<PathBuf>,
    ) -> Result<Self, BoxErr> {
        Self::builder(context_dir).start_with(transport, control_authority).await
    }
}

impl Drop for SessionServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// A random BuildKit identifier (32 lowercase-hex chars, like buildx's
/// `identity.NewID()`). Used for session ids and build refs alike.
pub(crate) fn new_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let mut x = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut s = String::with_capacity(32);
    for _ in 0..4 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push_str(&format!("{x:016x}"));
    }
    s.truncate(32);
    s
}

/// Quick H2 frame type tag for debug hex dumps.
fn h2_frame_tag(data: &[u8]) -> &'static str {
    if data == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" {
        return "[H2 PREFACE]";
    }
    if data.len() < 9 {
        return "[?short]";
    }
    let ty = data[3];
    let flags = data[4];
    match ty {
        0x00 => "[DATA]",
        0x01 => "[HEADERS]",
        0x04 if flags & 0x01 != 0 => "[SETTINGS ACK]",
        0x04 => "[SETTINGS]",
        0x07 => "[GOAWAY]",
        0x08 => "[WINDOW_UPDATE]",
        _ => "[?]",
    }
}
