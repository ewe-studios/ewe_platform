//! The registration authority: listeners, enrollment and the registry.
//!
//! WHY: one place must assemble the registration authority from its parts —
//! the certificate authority (CA), the trust store, the token signer, the
//! mutual-TLS (mTLS) and enrollment listeners, the optional Unix socket, the
//! backend registry and the lease sweeper (spec 82 feature 00, sections 5 and
//! 6.5).
//!
//! WHAT: [`RegistrationAuthority`], [`start`](RegistrationAuthority::start) and
//! the bound addresses for an operator view or a test.
//!
//! HOW: the CA, the trust store and the enrollment-token key seed live in
//! `RegistrationConfig::state_dir`, so a token and a certificate survive a
//! proxy restart. The
//! registration listener requires a client certificate; the enrollment listener
//! uses server-auth Transport Layer Security (TLS) and serves only `Enroll`. A
//! call without a client certificate on the registration listener fails in the
//! handshake, so no request reaches a handler. All serve threads stop on the
//! proxy's shutdown signal.

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_auth::mtls::{server_config_mutual, server_config_server_auth};
use foundation_auth::pki::{CaConfig, CaKey, CaStore, KeyAlgorithm, PkiError};
use foundation_auth::secret::{SecretDomain, SecretIssuer, SigningKey};
use foundation_auth::trust::{SqlTrustStore, TrustConfig, TrustError, TrustedClients};
use foundation_connectrpc::{ConnectRpcServe, ConnectRpcServeH2, Router as RpcRouter};
use foundation_context::Ctx;
use foundation_http::native::app::ServerApp;
use foundation_http::native::serve::{H2Serve, Serve};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::shared::client::{DnsResolver, SystemDnsResolver};

use super::config::RegistrationConfig;
use super::enroll::EnrollmentAuthority;
use super::proto::v1::registrar_enrollment_service::{
    procedure as enroll_procedure, register_registrar_enrollment_service,
};
use super::proto::v1::registration_service::{
    procedure as registration_procedure, register_registration_service,
};
use super::service::{AccessMode, EnrollmentHandler, RegistrationHandler};
#[cfg(unix)]
use super::unix::UnixSocketAcceptor;
use super::{spawn_sweeper, RegistrationAuthorityError};
use crate::router::RouterHandle;
use crate::runtime::ServiceRuntime;

/// The file name of the trusted-client store inside the state directory.
const TRUST_DB_FILE: &str = "registrar_trust.db";
/// The sub-directory of the state directory that holds the registration CA.
const CA_DIR: &str = "ca";
/// The file that holds the 32-byte seed of the enrollment-token signing key.
/// A token that the operator issued must still work after a proxy restart, so
/// the seed is a file, not a fresh value at each start.
const TOKEN_KEY_FILE: &str = "registrar_token.key";

/// The registration authority of one proxy.
pub struct RegistrationAuthority {
    registry: Arc<super::BackendRegistry>,
    enrollment: Arc<EnrollmentAuthority>,
    server_fingerprint: Option<String>,
    registration_addr: Option<SocketAddr>,
    enrollment_addr: Option<SocketAddr>,
    #[cfg(unix)]
    unix: Option<Arc<UnixSocketAcceptor>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl std::fmt::Debug for RegistrationAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("RegistrationAuthority");
        debug
            .field("registration_addr", &self.registration_addr)
            .field("enrollment_addr", &self.enrollment_addr);
        #[cfg(unix)]
        debug.field("unix", &self.unix.as_ref().map(|u| u.path()));
        debug.finish_non_exhaustive()
    }
}

impl RegistrationAuthority {
    /// Assemble and start the authority from `config`.
    ///
    /// Every listener binds before any serve thread starts, so an error leaves
    /// nothing running.
    ///
    /// # Errors
    /// [`RegistrationAuthorityError`] when the CA, the trust store, the token
    /// signer, a listener or the Unix socket cannot be set up.
    #[allow(
        clippy::too_many_lines,
        reason = "the start-up order reads best as one flow"
    )]
    pub async fn start(
        config: &RegistrationConfig,
        base_services: Vec<Arc<ServiceRuntime>>,
        router_handle: RouterHandle,
        ctx: Ctx,
        shutdown: Arc<ShutdownSignal>,
    ) -> Result<Arc<Self>, RegistrationAuthorityError> {
        let state_dir = config
            .state_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("ewe_proxy_registration"));

        // 1. The registration certificate authority.
        let (ca, _fresh) = CaStore::file(state_dir.join(CA_DIR))
            .load_or_generate(&CaConfig::new("ewe proxy registration CA"), &ctx)
            .map_err(|error| authority_pki_error(&error))?;
        let ca = Arc::new(ca);

        // 2. The trusted-client store.
        let store = SqlTrustStore::open_sqlite(state_dir.join(TRUST_DB_FILE), ctx.clone())
            .await
            .map_err(|e| RegistrationAuthorityError::Storage(e.to_string()))?;
        let trusted = TrustedClients::open(Arc::new(store), &ctx, TrustConfig::default())
            .await
            .map_err(|error| authority_trust_error(&error))?;

        // 3. The token signer and its verifier. The seed is made from the
        //    host's injected entropy on the first start and kept in the state
        //    directory, so an issued token still works after a restart. A
        //    context without the `Env` is a typed error, never a silent
        //    fallback to the real source.
        let seed = load_or_create_token_seed(&state_dir, &ctx)?;
        let signing_key = SigningKey::from_seed(seed, None);
        let issuer = Arc::new(SecretIssuer::new(
            SecretDomain::PROXY_REGISTRATION,
            signing_key,
            ctx.clone(),
            Duration::from_hours(24),
        ));
        let enrollment = Arc::new(EnrollmentAuthority::new(
            issuer,
            Arc::clone(&trusted),
            Arc::clone(&ca),
            config.public_url.clone(),
            config.client_cert_lifetime,
            config.registrar_token_validity,
            config.registrar_token_uses,
            ctx.clone(),
        ));

        // 4. The backend registry.
        let registry = Arc::new(super::BackendRegistry::new(
            base_services,
            router_handle,
            ctx.clone(),
            config.clone(),
            Arc::new(SystemDnsResolver) as Arc<dyn DnsResolver>,
        ));

        // 5. Bind every requested listener before starting any thread.
        let needs_server_identity = config.bind.is_some() || config.enrollment_bind.is_some();
        let mut server_fingerprint = None;
        let mut mutual_acceptor: Option<Arc<SSLAcceptor>> = None;
        let mut server_auth_acceptor: Option<Arc<SSLAcceptor>> = None;
        if needs_server_identity {
            let server_key = CaKey::generate(KeyAlgorithm::EcdsaP256, &ctx)
                .map_err(|error| authority_pki_error(&error))?;
            let identity = ca
                .issue_server_identity(
                    &server_key,
                    &config.server_names,
                    config.server_cert_lifetime,
                )
                .map_err(|error| authority_pki_error(&error))?;
            server_fingerprint = Some(identity.fingerprint_sha256().to_string());
            let mutual_config = server_config_mutual(&identity, &ca, trusted.index(), &ctx)
                .map_err(|error| authority_pki_error(&error))?;
            let server_auth_config = server_config_server_auth(&identity, &ctx)
                .map_err(|error| authority_pki_error(&error))?;
            mutual_acceptor = Some(Arc::new(SSLAcceptor::with_config(mutual_config)));
            server_auth_acceptor = Some(Arc::new(SSLAcceptor::with_config(server_auth_config)));
        }

        let mut registration_listener: Option<(TcpListener, SocketAddr)> = None;
        if let Some(bind) = &config.bind {
            registration_listener = Some(bind_tcp(bind)?);
        }
        let mut enrollment_listener: Option<(TcpListener, SocketAddr)> = None;
        let enrollment_bind = config.enrollment_bind.clone().or_else(|| {
            registration_listener
                .as_ref()
                .map(|(_, addr)| format!("{}:0", host_of(*addr)))
        });
        if let Some(bind) = enrollment_bind {
            enrollment_listener = Some(bind_tcp(&bind)?);
        }

        #[cfg(unix)]
        let unix = match &config.unix_socket {
            Some(path) => Some(Arc::new(UnixSocketAcceptor::bind(path)?)),
            None => None,
        };
        #[cfg(not(unix))]
        if config.unix_socket.is_some() {
            return Err(RegistrationAuthorityError::Socket(
                "the Unix socket transport is not available on this operating system".to_string(),
            ));
        }

        // 6. Build the apps. Each listener gets its own handler instance, so the
        //    access mode is fixed per listener.
        let registration_paths = [
            registration_procedure::REGISTER,
            registration_procedure::REPORT,
            registration_procedure::DEREGISTER,
            registration_procedure::RENEW_CERTIFICATE,
        ];
        let registration_app = {
            let handler = Arc::new(RegistrationHandler::new(
                Arc::clone(&registry),
                Arc::clone(&enrollment),
                AccessMode::MutualTls,
            ));
            routed_app(&registration_paths, move || {
                let mut router = RpcRouter::new();
                register_registration_service(&mut router, Arc::clone(&handler));
                router
            })
        };
        let enrollment_app = {
            let handler = Arc::new(EnrollmentHandler::new(Arc::clone(&enrollment)));
            routed_app(&[enroll_procedure::ENROLL], move || {
                let mut router = RpcRouter::new();
                register_registrar_enrollment_service(&mut router, Arc::clone(&handler));
                router
            })
        };
        let unix_app = {
            let handler = Arc::new(RegistrationHandler::new(
                Arc::clone(&registry),
                Arc::clone(&enrollment),
                AccessMode::Local,
            ));
            routed_app(&registration_paths, move || {
                let mut router = RpcRouter::new();
                register_registration_service(&mut router, Arc::clone(&handler));
                router
            })
        };

        // 7. Start the sweeper and the serve threads.
        spawn_sweeper(Arc::clone(&registry), Arc::clone(&shutdown), ctx.clone())
            .map_err(|e| RegistrationAuthorityError::Runtime(format!("{e:?}")))?;

        let mut threads = Vec::new();
        let mut registration_addr = None;
        if let (Some((listener, addr)), Some(acceptor)) =
            (registration_listener, mutual_acceptor.clone())
        {
            registration_addr = Some(addr);
            threads.push(spawn_tls(
                registration_app,
                listener,
                addr,
                acceptor,
                &shutdown,
                "registrar-mtls",
            )?);
        }
        let mut enrollment_addr = None;
        if let (Some((listener, addr)), Some(acceptor)) =
            (enrollment_listener, server_auth_acceptor)
        {
            enrollment_addr = Some(addr);
            threads.push(spawn_tls(
                enrollment_app,
                listener,
                addr,
                acceptor,
                &shutdown,
                "registrar-enroll",
            )?);
        }
        #[cfg(unix)]
        if let Some(acceptor) = &unix {
            let server = HttpServer::from_app_with_config(
                unix_app,
                &acceptor.path().to_string_lossy(),
                ServerConfig::defaults(),
            );
            let acceptor = Arc::clone(acceptor);
            let shutdown = Arc::clone(&shutdown);
            threads.push(spawn_thread("registrar-unix", move || {
                server.serve_with_acceptor(acceptor.as_ref(), &shutdown);
            })?);
        }

        #[cfg(unix)]
        let unix_path = unix.as_ref().map(|u| u.path().to_path_buf());
        #[cfg(not(unix))]
        let unix_path: Option<PathBuf> = None;
        tracing::info!(
            registration = ?registration_addr,
            enrollment = ?enrollment_addr,
            unix = ?unix_path,
            "registration authority started"
        );

        Ok(Arc::new(Self {
            registry,
            enrollment,
            server_fingerprint,
            registration_addr,
            enrollment_addr,
            #[cfg(unix)]
            unix,
            threads: Mutex::new(threads),
        }))
    }

    /// The backend registry that the request path reads.
    #[must_use]
    pub fn registry(&self) -> &Arc<super::BackendRegistry> {
        &self.registry
    }

    /// The enrollment, renewal and revocation operations.
    #[must_use]
    pub fn enrollment(&self) -> &Arc<EnrollmentAuthority> {
        &self.enrollment
    }

    /// The bound mutual-TLS registration address, when the listener is on.
    #[must_use]
    pub fn registration_addr(&self) -> Option<SocketAddr> {
        self.registration_addr
    }

    /// The bound enrollment address, when the listener is on.
    #[must_use]
    pub fn enrollment_addr(&self) -> Option<SocketAddr> {
        self.enrollment_addr
    }

    /// The Unix socket's path, when the transport is on.
    #[must_use]
    #[cfg(unix)]
    pub fn unix_path(&self) -> Option<PathBuf> {
        self.unix.as_ref().map(|u| u.path().to_path_buf())
    }

    /// Lowercase hexadecimal SHA-256 of the registration listener certificate,
    /// for a test client to pin.
    #[must_use]
    pub fn server_fingerprint(&self) -> Option<&str> {
        self.server_fingerprint.as_deref()
    }

    /// The Unix socket's path is never set on this platform.
    #[must_use]
    #[cfg(not(unix))]
    pub fn unix_path(&self) -> Option<PathBuf> {
        None
    }

    /// The registration CA certificate in Privacy-Enhanced Mail (PEM) form.
    #[must_use]
    pub fn ca_cert_pem(&self) -> &str {
        self.enrollment.ca_cert_pem()
    }

    /// Stop accepting and join the serve threads. The caller has set the
    /// shutdown signal.
    pub fn stop(&self) {
        let mut threads = self
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for thread in threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn authority_pki_error(error: &PkiError) -> RegistrationAuthorityError {
    RegistrationAuthorityError::Tls(error.to_string())
}

fn authority_trust_error(error: &TrustError) -> RegistrationAuthorityError {
    RegistrationAuthorityError::Storage(error.to_string())
}

/// Read the enrollment-token signing seed, or make one and keep it.
///
/// The seed is 32 bytes in `registrar_token.key`, with mode 0600 on Unix. A
/// token that the operator issued must still verify after a proxy restart, so
/// the key stays the same for the life of the state directory.
fn load_or_create_token_seed(
    state_dir: &std::path::Path,
    ctx: &Ctx,
) -> Result<[u8; 32], RegistrationAuthorityError> {
    let path = state_dir.join(TOKEN_KEY_FILE);
    if let Some(seed) = read_token_seed(&path)? {
        return Ok(seed);
    }
    let Some(env) = ctx.env() else {
        return Err(RegistrationAuthorityError::Config(
            "the registration authority needs the host Env on the first start \
             (entropy for the enrollment-token key); install one with \
             foundation_core::env::install"
                .to_string(),
        ));
    };
    let mut seed = [0u8; 32];
    env.entropy()
        .fill(&mut seed)
        .map_err(|e| RegistrationAuthorityError::Config(e.to_string()))?;
    std::fs::create_dir_all(state_dir).map_err(|e| storage_error("create", state_dir, &e))?;
    match write_secret_file(&path, &seed) {
        Ok(()) => Ok(seed),
        // Two proxies started together: the other one wrote the file first.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => read_token_seed(&path)?
            .ok_or_else(|| {
                RegistrationAuthorityError::Storage(format!(
                    "{} vanished after another process made it",
                    path.display()
                ))
            }),
        Err(error) => Err(storage_error("write", &path, &error)),
    }
}

/// The stored seed, or `None` when the file does not exist yet.
fn read_token_seed(path: &std::path::Path) -> Result<Option<[u8; 32]>, RegistrationAuthorityError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.len() != 32 {
                return Err(RegistrationAuthorityError::Storage(format!(
                    "{} must hold 32 bytes, not {}",
                    path.display(),
                    bytes.len()
                )));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            Ok(Some(seed))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_error("read", path, &error)),
    }
}

fn storage_error(
    verb: &str,
    path: &std::path::Path,
    error: &std::io::Error,
) -> RegistrationAuthorityError {
    RegistrationAuthorityError::Storage(format!("{verb} {}: {error}", path.display()))
}

/// Write the seed with mode 0600 and fail when the file already exists.
#[cfg(unix)]
fn write_secret_file(path: &std::path::Path, seed: &[u8; 32]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(seed)?;
    file.sync_all()
}

/// Write the seed and fail when the file already exists. Windows keeps its
/// inherited access control entries; key protection at rest is a later spec.
#[cfg(not(unix))]
fn write_secret_file(path: &std::path::Path, seed: &[u8; 32]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(seed)?;
    file.sync_all()
}

/// Bind `addr`, with a non-blocking listener.
fn bind_tcp(addr: &str) -> Result<(TcpListener, SocketAddr), RegistrationAuthorityError> {
    let listener = TcpListener::bind(addr)
        .map_err(|e| RegistrationAuthorityError::Runtime(format!("bind {addr}: {e}")))?;
    let local = listener
        .local_addr()
        .map_err(|e| RegistrationAuthorityError::Runtime(format!("local_addr {addr}: {e}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| RegistrationAuthorityError::Runtime(format!("non-blocking {addr}: {e}")))?;
    Ok((listener, local))
}

/// `host:port` of a bound address, for the enrollment listener's default.
fn host_of(addr: SocketAddr) -> String {
    match addr {
        SocketAddr::V4(v4) => v4.ip().to_string(),
        SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
    }
}

/// One app over HTTP/1.1 and HTTP/2 for the given procedure paths.
fn routed_app(paths: &[&str], make: impl Fn() -> RpcRouter) -> ServerApp {
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(make().into_handler()));
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(make().into_handler()));
    let mut http1 = HttpApp::new_serve();
    let mut http2 = HttpApp::new_h2_serve();
    for path in paths {
        http1.router.add_route_any(path, &h1);
        http2.route_any_h2(path, Arc::clone(&h2));
    }
    ServerApp::both(http1, http2)
}

fn spawn_tls(
    app: ServerApp,
    listener: TcpListener,
    addr: SocketAddr,
    acceptor: Arc<SSLAcceptor>,
    shutdown: &Arc<ShutdownSignal>,
    name: &str,
) -> Result<JoinHandle<()>, RegistrationAuthorityError> {
    let server = HttpServer::from_app_with_config(
        app,
        &addr.to_string(),
        ServerConfig::defaults().with_tls(acceptor),
    );
    let shutdown = Arc::clone(shutdown);
    spawn_thread(name, move || {
        server.serve_tls_with_listener(&listener, &shutdown);
    })
}

fn spawn_thread(
    name: &str,
    run: impl FnOnce() + Send + 'static,
) -> Result<JoinHandle<()>, RegistrationAuthorityError> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(run)
        .map_err(|e| RegistrationAuthorityError::Runtime(format!("spawn {name}: {e}")))
}
