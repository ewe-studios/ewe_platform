//! The in-process test environment.
//!
//! One server hosts the HTTP Durable handler and the Durable WebSocket handler
//! over one store. A second server hosts the `ConnectRPC` service over its own
//! store. Each driver runs the scenario against a fresh store, so a scenario is
//! independent on every transport.

use std::net::SocketAddr;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use foundation_connectrpc::shared::durable::{durable_stream_router, ConnectRpcAdapter};
use foundation_connectrpc::{ConnectRpcServe, ConnectRpcServeH2};
use foundation_context::Ctx;
use foundation_core::durable::client::{ClientConfig, DurableClient};
use foundation_core::durable::{DurableService, DurableStream, MemoryVfs};
use foundation_http::native::app::ServerApp;
use foundation_http::native::durable::DurableHandler;
use foundation_http::native::durable_ws::{DurableWsConfig, DurableWsHandler};
use foundation_http::native::serve::{H2Serve, Serve};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::native::durable::NetioTransport;
use foundation_netio::shared::http::SimpleMethod;
use foundation_netio::HttpClientBuilder;

use crate::http::HttpDriver;
use crate::rpc::RpcDriver;
use crate::ws::WsDriver;

/// Every procedure path the RPC server routes.
const PROCEDURES: [&str; 8] = [
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::CREATE,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::APPEND,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::READ,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::SUBSCRIBE,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::HEAD,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::CLOSE,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::DELETE,
    foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::procedure::FORK,
];

/// One running server and its stop signal.
struct RunningServer {
    shutdown: Arc<ShutdownSignal>,
    done: mpsc::Receiver<()>,
}

impl RunningServer {
    /// Stop the server and wait for its thread.
    fn stop(self) {
        self.shutdown.turn_on();
        let _ = self.done.recv_timeout(Duration::from_secs(5));
    }
}

/// Start the HTTP server over a fresh store.
fn start_http() -> (SocketAddr, RunningServer) {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let mut app = HttpApp::with_context(ctx);
    app.fallback::<DurableHandler>();
    start_native_server(app)
}

/// Start the WebSocket server over a fresh store.
fn start_ws() -> (SocketAddr, RunningServer) {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let _ = ctx.get_or_insert(DurableWsConfig::default());
    let mut app = HttpApp::with_context(ctx);
    app.route::<DurableWsHandler>(SimpleMethod::GET, "/ws");
    start_native_server(app)
}

/// Start one native server for an HTTP/1 application.
fn start_native_server(app: HttpApp<Arc<dyn Serve>>) -> (SocketAddr, RunningServer) {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local address");
    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), config);

    let thread_shutdown = Arc::clone(&shutdown);
    let (done_tx, done) = mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &thread_shutdown);
        let _ = done_tx.send(());
    });
    (addr, RunningServer { shutdown, done })
}

/// Start the `ConnectRPC` server over a fresh store.
fn start_rpc() -> (String, RunningServer) {
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host());
    let adapter = Arc::new(ConnectRpcAdapter::new(Arc::new(DurableService::new(
        stream,
    ))));
    let http1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(
        durable_stream_router(Arc::clone(&adapter)).into_handler(),
    ));
    let http2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(
        durable_stream_router(adapter).into_handler(),
    ));
    let mut http1_app = HttpApp::new_serve();
    let mut http2_app = HttpApp::new_h2_serve();
    for path in PROCEDURES {
        http1_app.router.add_route_any(path, &http1);
        http2_app.route_any_h2(path, Arc::clone(&http2));
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener
        .set_nonblocking(true)
        .expect("set the listener non-blocking");
    let addr = listener.local_addr().expect("local address");
    let server = HttpServer::from_app_with_config(
        ServerApp::both(http1_app, http2_app),
        &addr.to_string(),
        ServerConfig::defaults(),
    );

    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    let (done_tx, done) = mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &thread_shutdown);
        let _ = done_tx.send(());
    });
    (format!("http://{addr}"), RunningServer { shutdown, done })
}

/// The servers and driver factories of one scenario run.
pub struct Environment {
    http_addr: SocketAddr,
    ws_addr: SocketAddr,
    rpc_base_url: String,
    http_server: Option<RunningServer>,
    ws_server: Option<RunningServer>,
    rpc_server: Option<RunningServer>,
}

impl Environment {
    /// Start one HTTP server, one WebSocket server and one `ConnectRPC` server.
    ///
    /// Each server owns a fresh store, so every driver runs the scenario from
    /// the same initial state.
    #[must_use]
    pub fn start() -> Self {
        let (http_addr, http_server) = start_http();
        let (ws_addr, ws_server) = start_ws();
        let (rpc_base_url, rpc_server) = start_rpc();
        Self {
            http_addr,
            ws_addr,
            rpc_base_url,
            http_server: Some(http_server),
            ws_server: Some(ws_server),
            rpc_server: Some(rpc_server),
        }
    }

    /// Build a driver over the HTTP transport.
    #[must_use]
    pub fn http_driver(&self) -> HttpDriver {
        let client = HttpClientBuilder::new()
            .resolve_to(self.http_addr)
            .max_retries(10)
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(20))
            .write_timeout(Duration::from_secs(5))
            .build();
        let transport = Arc::new(NetioTransport::from_client(client));
        let client = DurableClient::new(
            transport,
            ClientConfig {
                base_url: "http://testserver".to_string(),
                default_headers: Vec::new(),
            },
        );
        HttpDriver::new(client)
    }

    /// Build a driver over the WebSocket transport.
    #[must_use]
    pub fn ws_driver(&self) -> WsDriver {
        WsDriver::connect(self.ws_addr)
    }

    /// Build a driver over the `ConnectRPC` transport.
    #[must_use]
    pub fn rpc_driver(&self) -> RpcDriver {
        RpcDriver::new(&self.rpc_base_url)
    }

    /// Stop every server.
    pub fn stop(mut self) {
        if let Some(server) = self.http_server.take() {
            server.stop();
        }
        if let Some(server) = self.ws_server.take() {
            server.stop();
        }
        if let Some(server) = self.rpc_server.take() {
            server.stop();
        }
    }
}
