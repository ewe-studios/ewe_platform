//! `AgentRpcServer` — assembles models, registry, auth and router into a server.
//!
//! WHY: "launch the binary and it serves agents": one call should take loaded
//! models and produce a listening Connect/gRPC/gRPC-Web endpoint.
//!
//! WHAT: [`AgentRpcServerBuilder`] (models, limits, auth) → [`AgentRpcServer`]
//! → [`AgentRpcServer::serve`] → [`RunningServer`] (bound addresses, the TLS
//! fingerprint, graceful shutdown).
//!
//! HOW: the agent listener serves HTTP/1.1 (Connect, gRPC-Web) and h2c (gRPC,
//! Connect) via `foundation_http`'s `ServerApp::Both`, every `AgentService`
//! procedure routed to the connectrpc handler from [`agent_router`]. With auth
//! (`with_bootstrap_token` + `with_mtls`) it is a mutual-TLS listener, and a
//! second, server-auth TLS listener serves only `BootstrapService/Enroll`.
//! Without auth (`insecure_plaintext`) there is one plaintext listener.
//! Each serve loop runs on its own thread; handlers run on the valtron pool, so
//! the caller must have one running (≥ 8 workers — see LEARNINGS 01 T6).

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;

use foundation_connectrpc::{ConnectRpcServe, ConnectRpcServeH2, Router};
use foundation_core::synca::OnSignal;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::Serve;

use crate::auth::{tls_listeners, BootstrapToken, EnrollmentService, MtlsConfig};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::instance::{InstanceRegistry, RegistryConfig};
use crate::model::ModelHandle;
use crate::proto::v1::agent_service::procedure as agent_procedure;
use crate::proto::v1::bootstrap_service::{
    procedure as bootstrap_procedure, register_bootstrap_service,
};
use crate::service::{agent_router, AgentRpcService};

/// Every `AgentService` procedure path the agent listener routes.
const AGENT_PROCEDURES: [&str; 6] = [
    agent_procedure::HEALTH,
    agent_procedure::CREATE_INSTANCE,
    agent_procedure::LIST_INSTANCES,
    agent_procedure::DESTROY_INSTANCE,
    agent_procedure::CHAT,
    agent_procedure::CHAT_STREAM,
];

/// How clients authenticate.
#[derive(Debug)]
enum Auth {
    /// No TLS, no client auth — loopback development and tests only.
    InsecurePlaintext,
    /// Bootstrap token → client certificate → mutual TLS.
    Mtls {
        config: MtlsConfig,
        enrollment: Arc<EnrollmentService>,
    },
}

/// Builder for [`AgentRpcServer`].
pub struct AgentRpcServerBuilder {
    models: Vec<ModelHandle>,
    registry: RegistryConfig,
    http: Option<ServerConfig>,
    insecure: bool,
    token: Option<BootstrapToken>,
    mtls: Option<MtlsConfig>,
}

impl std::fmt::Debug for AgentRpcServerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRpcServerBuilder")
            .field("models", &self.models)
            .field("registry", &self.registry)
            .field("insecure", &self.insecure)
            .field("token", &self.token)
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

    /// HTTP settings (timeouts, body limits, keep-alive) for the agent
    /// listener. Defaults to `ServerConfig::defaults()`; TLS is set by the
    /// server.
    #[must_use]
    pub fn with_http_config(mut self, config: ServerConfig) -> Self {
        self.http = Some(config);
        self
    }

    /// The secret clients present once to enroll. Requires [`with_mtls`](Self::with_mtls).
    #[must_use]
    pub fn with_bootstrap_token(mut self, token: BootstrapToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Mutual TLS on the agent listener. Requires a bootstrap token.
    #[must_use]
    pub fn with_mtls(mut self, config: MtlsConfig) -> Self {
        self.mtls = Some(config);
        self
    }

    /// Serve without TLS or client authentication. Only for loopback
    /// development and tests: anyone who can reach the port can drive agents.
    #[must_use]
    pub fn insecure_plaintext(mut self) -> Self {
        self.insecure = true;
        self
    }

    /// Validate and assemble the server.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when auth is not chosen (or half-chosen, or
    /// both secure and insecure), no model was added, or two models share a
    /// name.
    pub fn build(self) -> AgentRpcResult<AgentRpcServer> {
        let auth = match (self.insecure, self.token, self.mtls) {
            (true, None, None) => Auth::InsecurePlaintext,
            (false, Some(token), Some(config)) => Auth::Mtls {
                enrollment: Arc::new(EnrollmentService::new(token, &config)),
                config,
            },
            (true, _, _) => {
                return Err(AgentRpcError::Config(
                    "insecure_plaintext cannot be combined with a token or mTLS".into(),
                )
                .into())
            }
            (false, Some(_), None) | (false, None, Some(_)) => {
                return Err(AgentRpcError::Config(
                    "secure mode needs both with_bootstrap_token and with_mtls".into(),
                )
                .into())
            }
            (false, None, None) => {
                return Err(AgentRpcError::Config(
                    "choose client auth explicitly (with_bootstrap_token + with_mtls, or insecure_plaintext for local use)"
                        .into(),
                )
                .into())
            }
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
    auth: Auth,
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
            insecure: false,
            token: None,
            mtls: None,
        }
    }

    /// The service (and through it the registry) this server exposes.
    #[must_use]
    pub fn service(&self) -> &Arc<AgentRpcService> {
        &self.service
    }

    /// The enrollment service (secure mode only) — e.g. to rotate the token.
    #[must_use]
    pub fn enrollment(&self) -> Option<&Arc<EnrollmentService>> {
        match &self.auth {
            Auth::Mtls { enrollment, .. } => Some(enrollment),
            Auth::InsecurePlaintext => None,
        }
    }

    /// Bind `addr` (`host:port`; port 0 picks one) and serve on background
    /// threads until [`RunningServer::graceful_shutdown`]. In secure mode the
    /// bootstrap listener binds too (`MtlsConfig::with_bootstrap_bind`, default
    /// the same host on an ephemeral port).
    ///
    /// # Errors
    /// [`AgentRpcError::Transport`] when an address cannot be bound;
    /// [`AgentRpcError::Config`] when TLS cannot be set up.
    pub fn serve(self, addr: &str) -> AgentRpcResult<RunningServer> {
        let shutdown = Arc::new(OnSignal::new());
        let agent_listener = bind(addr)?;
        let agent_addr = local_addr(&agent_listener, addr)?;
        let agent_app = agent_app(&self.service);
        let http = self.http.unwrap_or_else(ServerConfig::defaults);

        match self.auth {
            Auth::InsecurePlaintext => {
                let thread = spawn_plain(agent_app, agent_listener, agent_addr, http, &shutdown)?;
                tracing::info!(addr = %agent_addr, "agent-rpc listening (plaintext)");
                Ok(RunningServer {
                    addr: agent_addr,
                    bootstrap_addr: None,
                    tls: None,
                    shutdown,
                    threads: vec![thread],
                })
            }
            Auth::Mtls { config, enrollment } => {
                let tls = tls_listeners(&config)?;
                let bootstrap_bind = config
                    .bootstrap_bind()
                    .map_or_else(|| format!("{}:0", host_of(agent_addr)), str::to_string);
                let bootstrap_listener = bind(&bootstrap_bind)?;
                let bootstrap_addr = local_addr(&bootstrap_listener, &bootstrap_bind)?;

                let mut threads = vec![spawn_tls(
                    agent_app,
                    agent_listener,
                    agent_addr,
                    http.with_tls(Arc::clone(&tls.agent)),
                    &shutdown,
                )?];
                threads.push(spawn_tls(
                    bootstrap_app(&enrollment),
                    bootstrap_listener,
                    bootstrap_addr,
                    ServerConfig::defaults().with_tls(Arc::clone(&tls.bootstrap)),
                    &shutdown,
                )?);
                tracing::info!(
                    addr = %agent_addr,
                    bootstrap = %bootstrap_addr,
                    fingerprint = %tls.fingerprint_sha256,
                    "agent-rpc listening (mTLS)"
                );
                Ok(RunningServer {
                    addr: agent_addr,
                    bootstrap_addr: Some(bootstrap_addr),
                    tls: Some(TlsInfo {
                        fingerprint_sha256: tls.fingerprint_sha256,
                        ca_cert_pem: config.ca().ca_cert_pem().to_string(),
                    }),
                    shutdown,
                    threads,
                })
            }
        }
    }
}

#[derive(Debug)]
struct TlsInfo {
    fingerprint_sha256: String,
    ca_cert_pem: String,
}

/// A listening server.
pub struct RunningServer {
    addr: SocketAddr,
    bootstrap_addr: Option<SocketAddr>,
    tls: Option<TlsInfo>,
    shutdown: Arc<OnSignal>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for RunningServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningServer")
            .field("addr", &self.addr)
            .field("bootstrap_addr", &self.bootstrap_addr)
            .field("tls", &self.tls)
            .finish_non_exhaustive()
    }
}

impl RunningServer {
    /// The agent listener's address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http(s)://host:port` of the agent listener — the base URL generated
    /// clients take.
    #[must_use]
    pub fn base_url(&self) -> String {
        let scheme = if self.tls.is_some() { "https" } else { "http" };
        format!("{scheme}://{}", self.addr)
    }

    /// The bootstrap listener's address (secure mode).
    #[must_use]
    pub fn bootstrap_addr(&self) -> Option<SocketAddr> {
        self.bootstrap_addr
    }

    /// `https://host:port` of the bootstrap listener (secure mode).
    #[must_use]
    pub fn bootstrap_url(&self) -> Option<String> {
        self.bootstrap_addr.map(|a| format!("https://{a}"))
    }

    /// Lowercase hex SHA-256 of the server certificate (secure mode).
    #[must_use]
    pub fn tls_fingerprint(&self) -> Option<&str> {
        self.tls.as_ref().map(|t| t.fingerprint_sha256.as_str())
    }

    /// The CA certificate clients trust after enrollment (secure mode).
    #[must_use]
    pub fn ca_cert_pem(&self) -> Option<&str> {
        self.tls.as_ref().map(|t| t.ca_cert_pem.as_str())
    }

    /// Stop accepting, let in-flight requests drain (the HTTP config's
    /// shutdown grace), and join the serve threads.
    ///
    /// # Errors
    /// [`AgentRpcError::Shutdown`] if a serve thread panicked.
    pub fn graceful_shutdown(mut self) -> AgentRpcResult<()> {
        self.stop()
    }

    fn stop(&mut self) -> AgentRpcResult<()> {
        self.shutdown.turn_on();
        let mut panicked = false;
        for thread in self.threads.drain(..) {
            panicked |= thread.join().is_err();
        }
        if panicked {
            return Err(AgentRpcError::Shutdown.into());
        }
        tracing::debug!(addr = %self.addr, "agent-rpc stopped");
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

fn local_addr(listener: &TcpListener, addr: &str) -> AgentRpcResult<SocketAddr> {
    listener
        .local_addr()
        .map_err(|e| AgentRpcError::Transport(format!("local address of {addr}: {e}")).into())
}

fn host_of(addr: SocketAddr) -> String {
    match addr {
        SocketAddr::V4(v4) => v4.ip().to_string(),
        SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
    }
}

fn spawn_plain(
    app: ServerApp,
    listener: TcpListener,
    addr: SocketAddr,
    config: ServerConfig,
    shutdown: &Arc<OnSignal>,
) -> AgentRpcResult<JoinHandle<()>> {
    let server = HttpServer::from_app_with_config(app, &addr.to_string(), config);
    let signal = Arc::clone(shutdown);
    spawn(addr, move || server.serve_with_listener(&listener, &signal))
}

fn spawn_tls(
    app: ServerApp,
    listener: TcpListener,
    addr: SocketAddr,
    config: ServerConfig,
    shutdown: &Arc<OnSignal>,
) -> AgentRpcResult<JoinHandle<()>> {
    let server = HttpServer::from_app_with_config(app, &addr.to_string(), config);
    let signal = Arc::clone(shutdown);
    spawn(addr, move || {
        server.serve_tls_with_listener(&listener, &signal);
    })
}

fn spawn(addr: SocketAddr, run: impl FnOnce() + Send + 'static) -> AgentRpcResult<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("agent-rpc {addr}"))
        .spawn(run)
        .map_err(|e| {
            AgentRpcError::Transport(format!("spawning the serve thread for {addr}: {e}")).into()
        })
}

/// HTTP/1.1 (Connect, gRPC-Web) + h2c (gRPC, Connect) routes for `paths`,
/// each app getting its own router from `router`.
fn routed_app(paths: &[&str], router: impl Fn() -> Router) -> ServerApp {
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router().into_handler()));
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router().into_handler()));
    let mut http1 = HttpApp::new_serve();
    let mut http2 = HttpApp::new_h2_serve();
    for path in paths {
        http1.router.add_route_any(path, &h1);
        http2.route_any_h2(path, Arc::clone(&h2));
    }
    ServerApp::both(http1, http2)
}

fn agent_app(service: &Arc<AgentRpcService>) -> ServerApp {
    routed_app(&AGENT_PROCEDURES, || agent_router(Arc::clone(service)))
}

fn bootstrap_app(enrollment: &Arc<EnrollmentService>) -> ServerApp {
    routed_app(&[bootstrap_procedure::ENROLL], || {
        let mut router = Router::new();
        register_bootstrap_service(&mut router, Arc::clone(enrollment));
        router
    })
}
