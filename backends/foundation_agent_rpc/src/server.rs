//! `AgentRpcServer` — assembles models, registry and router into a server.
//!
//! WHY: "launch the binary and it serves agents": one call should take loaded
//! models and produce a listening Connect/gRPC/gRPC-Web endpoint.
//!
//! WHAT: [`AgentRpcServerBuilder`] (models, limits, auth choice) →
//! [`AgentRpcServer`] → [`AgentRpcServer::serve`] → [`RunningServer`] (the
//! bound address and graceful shutdown).
//!
//! HOW: one listener serves HTTP/1.1 (Connect, gRPC-Web) and h2c (gRPC,
//! Connect) via `foundation_http`'s `ServerApp::Both`; each procedure path is
//! routed to the connectrpc handler built from [`agent_router`]. The serve loop
//! runs on its own thread; handlers run on the valtron pool, so the caller must
//! have one running (e.g. `#[valtron] fn main`).

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;

use foundation_connectrpc::{ConnectRpcServe, ConnectRpcServeH2};
use foundation_core::synca::OnSignal;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::Serve;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::instance::{InstanceRegistry, RegistryConfig};
use crate::model::ModelHandle;
use crate::proto::v1::agent_service::procedure;
use crate::service::{agent_router, AgentRpcService};

/// Every `AgentService` procedure path the listener routes.
const AGENT_PROCEDURES: [&str; 6] = [
    procedure::HEALTH,
    procedure::CREATE_INSTANCE,
    procedure::LIST_INSTANCES,
    procedure::DESTROY_INSTANCE,
    procedure::CHAT,
    procedure::CHAT_STREAM,
];

/// How clients authenticate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthChoice {
    /// No TLS, no client auth — loopback development and tests only.
    InsecurePlaintext,
}

/// Builder for [`AgentRpcServer`].
pub struct AgentRpcServerBuilder {
    models: Vec<ModelHandle>,
    registry: RegistryConfig,
    http: Option<ServerConfig>,
    auth: Option<AuthChoice>,
}

impl std::fmt::Debug for AgentRpcServerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRpcServerBuilder")
            .field("models", &self.models)
            .field("registry", &self.registry)
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl AgentRpcServerBuilder {
    /// Host `model` (addressed by its name). Call once per model.
    #[must_use]
    pub fn with_model(mut self, model: ModelHandle) -> Self {
        self.models.push(model);
        self
    }

    /// Instance limits.
    #[must_use]
    pub fn with_registry_config(mut self, config: RegistryConfig) -> Self {
        self.registry = config;
        self
    }

    /// HTTP server settings (timeouts, body limits, keep-alive). Defaults to
    /// `ServerConfig::defaults()`.
    #[must_use]
    pub fn with_http_config(mut self, config: ServerConfig) -> Self {
        self.http = Some(config);
        self
    }

    /// Serve without TLS or client authentication. Only for loopback
    /// development and tests: anyone who can reach the port can drive agents.
    #[must_use]
    pub fn insecure_plaintext(mut self) -> Self {
        self.auth = Some(AuthChoice::InsecurePlaintext);
        self
    }

    /// Validate and assemble the server.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when no auth mode was chosen, no model was
    /// added, or two models share a name.
    pub fn build(self) -> AgentRpcResult<AgentRpcServer> {
        let Some(auth) = self.auth else {
            return Err(AgentRpcError::Config(
                "choose client auth explicitly (insecure_plaintext for local use)".into(),
            )
            .into());
        };
        let registry = InstanceRegistry::new(self.models, self.registry)?;
        Ok(AgentRpcServer {
            service: Arc::new(AgentRpcService::new(Arc::new(registry))),
            http: self.http,
            auth,
        })
    }
}

/// An assembled, not yet listening, agent-rpc server.
pub struct AgentRpcServer {
    service: Arc<AgentRpcService>,
    http: Option<ServerConfig>,
    auth: AuthChoice,
}

impl std::fmt::Debug for AgentRpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRpcServer")
            .field("service", &self.service)
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl AgentRpcServer {
    /// Start building a server.
    #[must_use]
    pub fn builder() -> AgentRpcServerBuilder {
        AgentRpcServerBuilder {
            models: Vec::new(),
            registry: RegistryConfig::default(),
            http: None,
            auth: None,
        }
    }

    /// The service (and through it the registry) this server exposes.
    #[must_use]
    pub fn service(&self) -> &Arc<AgentRpcService> {
        &self.service
    }

    /// Bind `addr` (`host:port`; port 0 picks one) and serve on a background
    /// thread until [`RunningServer::graceful_shutdown`].
    ///
    /// # Errors
    /// [`AgentRpcError::Transport`] when the address cannot be bound.
    pub fn serve(self, addr: &str) -> AgentRpcResult<RunningServer> {
        let listener = bind(addr)?;
        let local = listener
            .local_addr()
            .map_err(|e| AgentRpcError::Transport(format!("local address of {addr}: {e}")))?;

        let app = server_app(&self.service);
        let config = self.http.unwrap_or_else(ServerConfig::defaults);
        let server = HttpServer::from_app_with_config(app, &local.to_string(), config);
        let shutdown = Arc::new(OnSignal::new());
        let signal = Arc::clone(&shutdown);
        let thread = std::thread::Builder::new()
            .name(format!("agent-rpc {local}"))
            .spawn(move || match self.auth {
                AuthChoice::InsecurePlaintext => server.serve_with_listener(&listener, &signal),
            })
            .map_err(|e| AgentRpcError::Transport(format!("spawning the serve thread: {e}")))?;

        tracing::info!(addr = %local, "agent-rpc listening (plaintext)");
        Ok(RunningServer {
            addr: local,
            shutdown,
            thread: Some(thread),
        })
    }
}

/// A listening server.
pub struct RunningServer {
    addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for RunningServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningServer")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl RunningServer {
    /// The bound address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://host:port` — the base URL generated clients take.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stop accepting, let in-flight requests drain (the HTTP config's
    /// shutdown grace), and join the serve thread.
    ///
    /// # Errors
    /// [`AgentRpcError::Shutdown`] if the serve thread panicked.
    pub fn graceful_shutdown(mut self) -> AgentRpcResult<()> {
        self.stop()
    }

    fn stop(&mut self) -> AgentRpcResult<()> {
        self.shutdown.turn_on();
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| AgentRpcError::Shutdown)?;
            tracing::info!(addr = %self.addr, "agent-rpc stopped");
        }
        Ok(())
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        if let Err(err) = self.stop() {
            tracing::error!(addr = %self.addr, error = %err.current_context(), "agent-rpc stop failed");
        }
    }
}

fn bind(addr: &str) -> AgentRpcResult<TcpListener> {
    let listener = TcpListener::bind(addr)
        .map_err(|e| AgentRpcError::Transport(format!("binding {addr}: {e}")))?;
    // The serve loop panics if this fails; surface it as an error instead.
    listener
        .set_nonblocking(true)
        .map_err(|e| AgentRpcError::Transport(format!("non-blocking {addr}: {e}")))?;
    Ok(listener)
}

/// HTTP/1.1 (Connect, gRPC-Web) + h2c (gRPC, Connect) on one port.
fn server_app(service: &Arc<AgentRpcService>) -> ServerApp {
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(
        agent_router(Arc::clone(service)).into_handler(),
    ));
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(
        agent_router(Arc::clone(service)).into_handler(),
    ));
    let mut http1 = HttpApp::new_serve();
    let mut http2 = HttpApp::new_h2_serve();
    for path in AGENT_PROCEDURES {
        http1.router.add_route_any(path, &h1);
        http2.route_any_h2(path, Arc::clone(&h2));
    }
    ServerApp::both(http1, http2)
}
