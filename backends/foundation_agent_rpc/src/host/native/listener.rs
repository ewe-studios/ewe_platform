//! The two native listeners and their TLS configurations.
//!
//! WHY: a native host serves two entry points: the agent listener (mutual
//! Transport Layer Security, mTLS) and the enrollment listener (server-auth
//! TLS). Each runs on its own serve thread, as today.
//!
//! WHAT: `serve`, the [`RunningServer`] handle, and the two TLS configurations.
//!
//! HOW: `serve` binds both addresses, builds the two `rustls` configurations,
//! starts one serve thread for each and returns. `HttpServer` runs each
//! connection as a valtron task. The verifier reads the in-memory trust index
//! only: it does no Input/Output (I/O) inside the handshake.

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;

use foundation_auth::mtls::{
    server_config_mutual, server_config_optional_mutual, server_config_server_auth,
};
use foundation_compact::Duration;
use foundation_connectrpc::shared::auth::ClientAuthInterceptor;
use foundation_connectrpc::{ConnectRpcServe, ConnectRpcServeH2, Interceptor, Router};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_http::native::app::ServerApp;
use foundation_http::native::serve::{H2Serve, Serve};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::ssl::SSLAcceptor;

use crate::core::enroll::EnrollmentService;
use crate::core::registry::LocalInstanceRegistry;
use crate::core::service::AgentRpcService;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::native::config::NativeHostConfig;
use crate::host::native::security::NativeSecurityParts;
use crate::proto::v1::agent_service::{procedure as agent_procedure, register_agent_service};
use crate::proto::v1::bootstrap_service::{
    procedure as bootstrap_procedure, register_bootstrap_service,
};
use crate::proto::v1::{BootstrapPayload, Capabilities};
use foundation_auth::secret::SignedSecret;

/// Every `AgentService` procedure path that the agent listener routes.
const AGENT_PROCEDURES: [&str; 7] = [
    agent_procedure::HEALTH,
    agent_procedure::GET_CAPABILITIES,
    agent_procedure::CREATE_INSTANCE,
    agent_procedure::LIST_INSTANCES,
    agent_procedure::DESTROY_INSTANCE,
    agent_procedure::CHAT,
    agent_procedure::CHAT_STREAM,
];

/// The operator key material of a secure host, or the development mode.
pub(crate) enum HostKeys {
    /// No TLS and no auth.
    Insecure,
    /// Mutual TLS (mTLS), with enrollment.
    Mtls {
        /// The bootstrap signing key. Boxed: the variant is much larger than
        /// `Insecure`, and the key is built once.
        signing: Box<foundation_auth::secret::SigningKey>,
        /// The certificate authority.
        ca: Arc<foundation_auth::pki::CertificateAuthority>,
        /// The server certificate.
        identity: foundation_auth::pki::ServerIdentity,
        /// The trusted clients.
        trust: Arc<foundation_auth::trust::TrustedClients>,
    },
}

/// Bind the listeners, build the authentication parts and serve.
///
/// The agent listener binds first, because the enrollment audience and the
/// bootstrap payload name the bound address. Port 0 therefore works.
///
/// # Errors
/// [`AgentRpcError::Transport`] when an address cannot be bound;
/// [`AgentRpcError::Config`] when the TLS material or the enrollment service
/// cannot be built.
#[allow(clippy::too_many_lines)] // one ordered start-up: bind, keys, apps, threads
pub(crate) fn serve(
    config: &NativeHostConfig,
    service: Arc<AgentRpcService>,
    keys: HostKeys,
    ctx: &Ctx,
    registry: Arc<LocalInstanceRegistry>,
) -> AgentRpcResult<RunningServer> {
    let shutdown = Arc::new(ShutdownSignal::new());
    let agent_listener = bind(&config.bind)?;
    let agent_addr = local_addr(&agent_listener, &config.bind)?;
    let agent_url = config.agent_public_url(&agent_addr.to_string());

    let security = match keys {
        HostKeys::Insecure => NativeSecurityParts::Insecure,
        HostKeys::Mtls {
            signing,
            ca,
            identity,
            trust,
        } => crate::host::native::security::build_mtls(
            config,
            *signing,
            Arc::clone(&ca),
            identity,
            Arc::clone(&trust),
            ctx,
            &agent_url,
        )?,
    };

    let models: Vec<String> = service
        .catalog()
        .capabilities()
        .models
        .into_iter()
        .map(|model| model.name)
        .collect();

    match security {
        NativeSecurityParts::Insecure => {
            let thread = spawn_plain(
                config,
                &service,
                agent_listener,
                agent_addr,
                &(Arc::new(foundation_auth::client_auth::InsecureNoAuth::new())
                    as Arc<dyn foundation_auth::client_auth::ClientAuth>),
                &shutdown,
            )?;
            tracing::info!(addr = %agent_addr, "agent-rpc listening (plaintext)");
            Ok(RunningServer {
                addr: agent_addr,
                bootstrap_addr: None,
                tls_fingerprint: None,
                ca_cert_pem: None,
                enrollment: None,
                agent_url,
                enroll_url: None,
                models,
                shutdown,
                threads: vec![thread],
                registry,
                turn_grace: config.shutdown_grace,
            })
        }
        NativeSecurityParts::Mtls(parts) => {
            let bootstrap_bind = config.bootstrap_bind_or_default(&host_of(agent_addr));
            let bootstrap_listener = bind(&bootstrap_bind)?;
            let bootstrap_addr = local_addr(&bootstrap_listener, &bootstrap_bind)?;
            let enroll_url = config.enroll_public_url(&bootstrap_addr.to_string());

            // A bearer access secret needs a listener that accepts a client
            // with no certificate. The verifier still checks a presented one.
            let agent_tls = match config.security {
                crate::host::native::config::NativeSecurity::Mtls {
                    accept_access_secrets: true,
                } => server_config_optional_mutual(
                    &parts.identity,
                    &parts.ca,
                    parts.trust.index(),
                    ctx,
                ),
                _ => server_config_mutual(&parts.identity, &parts.ca, parts.trust.index(), ctx),
            }
            .map_err(|error| pki_error(&error))?;
            let bootstrap_tls = server_config_server_auth(&parts.identity, ctx)
                .map_err(|error| pki_error(&error))?;

            let agent_auth = Arc::clone(&parts.client_auth);
            let agent_app = routed_app(&AGENT_PROCEDURES, move || {
                let interceptor: Arc<dyn Interceptor> =
                    Arc::new(ClientAuthInterceptor::new(Arc::clone(&agent_auth)));
                // The interceptors must be set before registration: the router
                // composes them into each handler at registration time.
                let mut router = Router::new().with_global_interceptors(vec![interceptor]);
                register_agent_service(&mut router, Arc::clone(&service));
                router
            });
            let enrollment = Arc::clone(&parts.enrollment);
            let bootstrap_app = routed_app(&[bootstrap_procedure::ENROLL], move || {
                let mut router = Router::new();
                register_bootstrap_service(&mut router, Arc::clone(&enrollment));
                router
            });

            let mut threads = vec![spawn_tls(
                agent_app,
                agent_listener,
                agent_addr,
                config
                    .http
                    .clone()
                    .with_tls(Arc::new(SSLAcceptor::with_config(agent_tls))),
                &shutdown,
            )?];
            threads.push(spawn_tls(
                bootstrap_app,
                bootstrap_listener,
                bootstrap_addr,
                config
                    .http
                    .clone()
                    .with_tls(Arc::new(SSLAcceptor::with_config(bootstrap_tls))),
                &shutdown,
            )?);

            tracing::info!(
                addr = %agent_addr,
                bootstrap = %bootstrap_addr,
                fingerprint = %parts.fingerprint_sha256,
                "agent-rpc listening (mTLS)"
            );
            Ok(RunningServer {
                addr: agent_addr,
                bootstrap_addr: Some(bootstrap_addr),
                tls_fingerprint: Some(parts.fingerprint_sha256.clone()),
                ca_cert_pem: Some(parts.ca.ca_cert_pem().to_string()),
                enrollment: Some(parts.enrollment),
                agent_url,
                enroll_url: Some(enroll_url),
                models,
                shutdown,
                threads,
                registry,
                turn_grace: config.shutdown_grace,
            })
        }
    }
}

fn pki_error(error: &foundation_auth::pki::PkiError) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Config(format!("TLS configuration: {error}")).into()
}

/// A listening native host.
pub struct RunningServer {
    addr: SocketAddr,
    bootstrap_addr: Option<SocketAddr>,
    tls_fingerprint: Option<String>,
    ca_cert_pem: Option<String>,
    enrollment: Option<Arc<EnrollmentService>>,
    agent_url: String,
    enroll_url: Option<String>,
    models: Vec<String>,
    shutdown: Arc<ShutdownSignal>,
    threads: Vec<JoinHandle<()>>,
    registry: Arc<LocalInstanceRegistry>,
    turn_grace: Duration,
}

impl std::fmt::Debug for RunningServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningServer")
            .field("addr", &self.addr)
            .field("bootstrap_addr", &self.bootstrap_addr)
            .field("tls_fingerprint", &self.tls_fingerprint)
            .finish_non_exhaustive()
    }
}

impl RunningServer {
    /// The agent listener's address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The `AgentService` base URL, as a bootstrap payload names it.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.agent_url
    }

    /// The enrollment listener's address (secure mode only).
    #[must_use]
    pub fn bootstrap_addr(&self) -> Option<SocketAddr> {
        self.bootstrap_addr
    }

    /// The `BootstrapService` base URL (secure mode only).
    #[must_use]
    pub fn bootstrap_url(&self) -> Option<&str> {
        self.enroll_url.as_deref()
    }

    /// Lowercase hexadecimal SHA-256 of the server certificate (secure mode).
    #[must_use]
    pub fn tls_fingerprint(&self) -> Option<&str> {
        self.tls_fingerprint.as_deref()
    }

    /// The certificate authority (CA) certificate that clients trust after
    /// enrollment (secure mode).
    #[must_use]
    pub fn ca_cert_pem(&self) -> Option<&str> {
        self.ca_cert_pem.as_deref()
    }

    /// The enrollment service (secure mode only).
    #[must_use]
    pub fn enrollment(&self) -> Option<&Arc<EnrollmentService>> {
        self.enrollment.as_ref()
    }

    /// How many instances the registry still holds.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.registry.instance_count()
    }

    /// The instance registry, for a test that checks the sessions after
    /// shutdown.
    #[must_use]
    pub fn registry(&self) -> &Arc<LocalInstanceRegistry> {
        &self.registry
    }

    /// Issue one enrollment secret, valid for `validity`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a plaintext server or when the secret
    /// cannot be made.
    pub fn issue_enroll_secret(&self, validity: Duration) -> AgentRpcResult<SignedSecret> {
        let enrollment = self.enrollment.as_ref().ok_or_else(|| {
            ErrorTrace::new(AgentRpcError::Config(
                "a plaintext server has no enrollment secret".to_string(),
            ))
        })?;
        enrollment.issue_enroll_secret(validity)
    }

    /// The payload that a client needs to connect: addresses, the server
    /// certificate fingerprint, `secret` and the capabilities. Hand it to the
    /// operator only: it contains the secret.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a plaintext server.
    pub fn payload_with(&self, secret: &SignedSecret) -> AgentRpcResult<BootstrapPayload> {
        let (Some(fingerprint), Some(enroll_url)) = (&self.tls_fingerprint, &self.enroll_url)
        else {
            return Err(AgentRpcError::Config(
                "a plaintext server has no bootstrap payload".to_string(),
            )
            .into());
        };
        Ok(BootstrapPayload {
            url: self.agent_url.clone(),
            enroll_url: enroll_url.clone(),
            tls_fingerprint_sha256: fingerprint.clone(),
            token: secret.encode().to_string(),
            capabilities: Some(Capabilities {
                protocols: vec!["connect".into(), "grpc".into(), "grpc-web".into()],
                models: self.models.clone(),
                auth_modes: vec!["mtls".into()],
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    /// A payload with a new enrollment secret of the default validity.
    ///
    /// # Errors
    /// As [`issue_enroll_secret`](Self::issue_enroll_secret).
    pub fn bootstrap_payload(&self) -> AgentRpcResult<BootstrapPayload> {
        let secret =
            self.issue_enroll_secret(crate::host::native::security::ENROLL_SECRET_VALIDITY)?;
        self.payload_with(&secret)
    }

    /// Stop accepting, let in-flight requests drain, and join the serve
    /// threads.
    ///
    /// # Errors
    /// [`AgentRpcError::Shutdown`] if a serve thread panicked.
    pub fn graceful_shutdown(mut self) -> AgentRpcResult<()> {
        self.stop()
    }

    fn stop(&mut self) -> AgentRpcResult<()> {
        self.shutdown.turn_on();
        // Wait up to the shutdown grace for the running turns to end. The ones
        // that still run get the terminal `TurnFailed("server shutting down")`.
        self.registry.shutdown(self.turn_grace);
        if self.registry.instance_count() > 0 {
            let registry = Arc::clone(&self.registry);
            if let Err(error) =
                foundation_core::valtron::block_on_future(async move { registry.end_all().await })
            {
                tracing::warn!(
                    error = %error.current_context(),
                    "a session did not end cleanly during shutdown"
                );
            }
        }
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
        if let Err(error) = self.stop() {
            tracing::error!(
                addr = %self.addr,
                error = %error.current_context(),
                "agent-rpc stop failed"
            );
        }
    }
}

fn bind(addr: &str) -> AgentRpcResult<TcpListener> {
    let listener = TcpListener::bind(addr)
        .map_err(|error| AgentRpcError::Transport(format!("binding {addr}: {error}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| AgentRpcError::Transport(format!("non-blocking {addr}: {error}")))?;
    Ok(listener)
}

fn local_addr(listener: &TcpListener, addr: &str) -> AgentRpcResult<SocketAddr> {
    listener.local_addr().map_err(|error| {
        AgentRpcError::Transport(format!("local address of {addr}: {error}")).into()
    })
}

fn host_of(addr: SocketAddr) -> String {
    match addr {
        SocketAddr::V4(v4) => v4.ip().to_string(),
        SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
    }
}

fn spawn_plain(
    config: &NativeHostConfig,
    service: &Arc<AgentRpcService>,
    listener: TcpListener,
    addr: SocketAddr,
    auth: &Arc<dyn foundation_auth::client_auth::ClientAuth>,
    shutdown: &Arc<ShutdownSignal>,
) -> AgentRpcResult<JoinHandle<()>> {
    let app = routed_app(&AGENT_PROCEDURES, || {
        let interceptor: Arc<dyn Interceptor> =
            Arc::new(ClientAuthInterceptor::new(Arc::clone(auth)));
        let mut router = Router::new().with_global_interceptors(vec![interceptor]);
        register_agent_service(&mut router, Arc::clone(service));
        router
    });
    let server = HttpServer::from_app_with_config(app, &addr.to_string(), config.http.clone());
    let signal = Arc::clone(shutdown);
    spawn(addr, move || server.serve_with_listener(&listener, &signal))
}

fn spawn_tls(
    app: ServerApp,
    listener: TcpListener,
    addr: SocketAddr,
    config: ServerConfig,
    shutdown: &Arc<ShutdownSignal>,
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
        .map_err(|error| {
            AgentRpcError::Transport(format!("spawning the serve thread for {addr}: {error}"))
                .into()
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
