//! Registration authentication: enrollment, mutual TLS, renewal, revocation and
//! the Unix socket permission check (spec 82 feature 00, T7, T8, T9).
//!
//! WHY: only enrolled sidecars may change where client traffic goes. These
//! tests run real Transport Layer Security (TLS) listeners and real clients.
//!
//! WHAT: enrollment with a one-time token, the server proof, an mTLS
//! `Register`/`Report`, a host mismatch, a spent token, a revoked client and the
//! Unix-socket permission check.
//!
//! HOW: the enrollment client pins the registration listener certificate; the
//! mTLS client trusts the registration certificate authority (CA) and presents
//! its client certificate.

use std::path::Path;
use std::sync::Arc;

use foundation_auth::pki;
use foundation_auth::secret::{DecodeLimits, SignedSecret};
use foundation_connectrpc::{ClientOptions, Code, H1Transport, Request, RpcCtx, Transport};
use foundation_context::Ctx;
use foundation_core::valtron::initialize_pool;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::HttpClientBuilder;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::registration::proto::v1::registrar_enrollment_service::RegistrarEnrollmentServiceClient;
use foundation_proxy::registration::proto::v1::registration_service::{
    RegistrationService, RegistrationServiceClient,
};
use foundation_proxy::registration::proto::v1::{
    BackendHealth, BackendReport, BackendSpec, EnrollRequest, Protocol, RegisterRequest,
    ReportRequest, Route, SessionRef,
};
use foundation_proxy::registration::{
    AccessMode, EnrollmentError, RegistrationAuthority, RegistrationConfig, RegistrationHandler,
};
use foundation_proxy::router::{Router, RouterHandle};
use foundation_proxy::runtime::ServiceRuntime;
use futures_lite::future::block_on;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const HOST: &str = "agents.example.com";
const SERVICE: &str = "agents";
const SIDECAR: &str = "host-17";

fn start_authority(dir: &Path) -> (Arc<RegistrationAuthority>, Arc<ShutdownSignal>) {
    let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
        SERVICE, HOST,
    )))];
    let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
    let config = RegistrationConfig::new()
        .with_bind("127.0.0.1:0")
        .with_state_dir(dir)
        .with_server_names(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    let shutdown = Arc::new(ShutdownSignal::new());
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("host env");
    let authority = block_on(RegistrationAuthority::start(
        &config,
        base,
        router,
        ctx,
        Arc::clone(&shutdown),
    ))
    .expect("start the registration authority");
    (authority, shutdown)
}

fn spec(backend_id: &str, port: u16) -> BackendSpec {
    BackendSpec {
        backend_id: backend_id.to_string(),
        service: SERVICE.to_string(),
        route: Some(Route {
            host: HOST.to_string(),
            path_prefix: String::new(),
            ..Default::default()
        })
        .into(),
        protocol: Protocol::PROTOCOL_HTTP.into(),
        address: "127.0.0.1".to_string(),
        port: u32::from(port),
        weight: 0,
        version: "1".to_string(),
        ..Default::default()
    }
}

fn session(session_id: &str, host_id: &str) -> SessionRef {
    SessionRef {
        session_id: session_id.to_string(),
        host_id: host_id.to_string(),
        ..Default::default()
    }
}

fn enrollment_client(authority: &RegistrationAuthority) -> RegistrarEnrollmentServiceClient {
    let fingerprint = authority.server_fingerprint().expect("server fingerprint");
    let connector = SSLConnector::with_config(
        foundation_auth::mtls::client_config_pinned_default(fingerprint)
            .expect("pinned client config"),
    );
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    let addr = authority.enrollment_addr().expect("enrollment address");
    RegistrarEnrollmentServiceClient::new(
        transport,
        &format!("https://{addr}"),
        ClientOptions::new(),
    )
    .expect("enrollment client")
}

fn registration_client(
    authority: &RegistrationAuthority,
    cert_pem: &str,
    key_pem: &str,
    ca_pem: &str,
) -> RegistrationServiceClient {
    let connector = SSLConnector::from_client_mutual_pem(
        Some(ca_pem.as_bytes()),
        cert_pem.as_bytes(),
        key_pem.as_bytes(),
        true,
    )
    .expect("mutual TLS connector");
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    let addr = authority.registration_addr().expect("registration address");
    RegistrationServiceClient::new(transport, &format!("https://{addr}"), ClientOptions::new())
        .expect("registration client")
}

/// Enroll `host` and return its key, certificate and the registration CA.
fn enroll(
    authority: &RegistrationAuthority,
    token: &str,
    host: &str,
) -> (zeroize::Zeroizing<String>, String, String, EnrollRequest) {
    let (key_pem, csr_pem) = pki::generate_csr(host).expect("client CSR");
    let nonce = vec![9u8; 32];
    let request = EnrollRequest {
        token: token.to_string(),
        host_id: host.to_string(),
        csr_pem: csr_pem.clone(),
        nonce: nonce.clone(),
        ..Default::default()
    };
    let client = enrollment_client(authority);
    let reply = block_on(client.enroll(RpcCtx::background(), Request::new(request.clone())))
        .expect("enroll")
        .msg;

    // The server proof proves that the right proxy answered (auth.md 9.1.1).
    let secret = SignedSecret::decode(token, &DecodeLimits::default()).expect("decode token");
    let csr_hash = Sha256::digest(csr_pem.as_bytes());
    assert!(
        secret
            .verifying_key()
            .verify_enroll_proof(&secret.id(), &nonce, &csr_hash, host, &reply.server_proof)
            .expect("verify proof"),
        "the server proof must check with the token key"
    );
    assert_ne0(&reply.client_cert_pem);
    assert_ne0(&reply.ca_cert_pem);
    (key_pem, reply.client_cert_pem, reply.ca_cert_pem, request)
}

fn assert_ne0(value: &str) {
    assert_ne!(value.len(), 0);
}

fn register(
    client: &RegistrationServiceClient,
    session_id: &str,
    host_id: &str,
    specs: Vec<BackendSpec>,
) -> foundation_connectrpc::ConnectResult<foundation_proxy::registration::proto::v1::RegisterResponse>
{
    block_on(client.register(
        RpcCtx::background(),
        Request::new(RegisterRequest {
            session: Some(session(session_id, host_id)).into(),
            backends: specs,
            ..Default::default()
        }),
    ))
    .map(|response| response.msg)
}

fn report(client: &RegistrationServiceClient, session_id: &str, host_id: &str, id: &str) {
    block_on(client.report(
        RpcCtx::background(),
        Request::new(ReportRequest {
            session: Some(session(session_id, host_id)).into(),
            backends: vec![BackendReport {
                backend_id: id.to_string(),
                health: BackendHealth::BACKEND_HEALTH_READY.into(),
                ..Default::default()
            }],
            ..Default::default()
        }),
    ))
    .expect("report");
}

#[test]
fn enrollment_then_mutual_tls_register_and_report() {
    let _guard = initialize_pool(101, Some(8));
    let dir = TempDir::new().expect("temp dir");
    let (authority, shutdown) = start_authority(dir.path());

    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    let (key_pem, cert_pem, ca_pem, _request) = enroll(&authority, &token, SIDECAR);
    let client = registration_client(&authority, &cert_pem, &key_pem, &ca_pem);

    let response = register(
        &client,
        "session-1",
        SIDECAR,
        vec![spec("host-17/agent", 7443)],
    )
    .expect("register");
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].rejected_reason, "");
    assert_eq!(response.results[0].generation, 1);

    report(&client, "session-1", SIDECAR, "host-17/agent");
    let snapshot = authority.registry().snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].owner_host_id, SIDECAR);
    assert_eq!(snapshot[0].spec.url(), "http://127.0.0.1:7443");

    let router = authority.registry().router();
    let service = router.route(HOST, "/").expect("service");
    assert_eq!(service.backends().len(), 1);

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_host_that_differs_from_the_certificate_is_unauthenticated() {
    let _guard = initialize_pool(102, Some(8));
    let dir = TempDir::new().expect("temp dir");
    let (authority, shutdown) = start_authority(dir.path());

    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    let (key_pem, cert_pem, ca_pem, _request) = enroll(&authority, &token, SIDECAR);
    let client = registration_client(&authority, &cert_pem, &key_pem, &ca_pem);

    let error = register(
        &client,
        "session-1",
        "host-other",
        vec![spec("host-other/agent", 7443)],
    )
    .expect_err("a mismatched host is refused");
    assert_eq!(error.current_context().code(), Code::Unauthenticated);
    assert_eq!(authority.registry().snapshot().len(), 0);

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn an_unauthenticated_register_changes_nothing() {
    let _guard = initialize_pool(103, Some(8));
    let dir = TempDir::new().expect("temp dir");
    let (authority, shutdown) = start_authority(dir.path());

    // No client certificate: the handler in mTLS mode refuses the call.
    let handler = RegistrationHandler::new(
        Arc::clone(authority.registry()),
        Arc::clone(authority.enrollment()),
        AccessMode::MutualTls,
    );
    let error = block_on(handler.register(
        RpcCtx::background(),
        Request::new(RegisterRequest {
            session: Some(session("session-1", SIDECAR)).into(),
            backends: vec![spec("host-17/agent", 7443)],
            ..Default::default()
        }),
    ))
    .expect_err("unauthenticated");
    assert_eq!(error.current_context().code(), Code::Unauthenticated);
    assert_eq!(authority.registry().snapshot().len(), 0);

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_used_enrollment_token_is_refused() {
    let _guard = initialize_pool(104, Some(8));
    let dir = TempDir::new().expect("temp dir");
    let (authority, shutdown) = start_authority(dir.path());

    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, Some(1))
        .expect("issue token");
    let (_key, _cert, _ca, request) = enroll(&authority, &token, SIDECAR);
    let second = block_on(authority.enrollment().enroll(&request));
    assert!(
        matches!(second, Err(EnrollmentError::Unauthenticated(_))),
        "a spent token must be refused"
    );

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_revoked_sidecar_is_refused_and_its_backends_drain() {
    let _guard = initialize_pool(105, Some(8));
    let dir = TempDir::new().expect("temp dir");
    let (authority, shutdown) = start_authority(dir.path());

    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    let (key_pem, cert_pem, ca_pem, _request) = enroll(&authority, &token, SIDECAR);
    let client = registration_client(&authority, &cert_pem, &key_pem, &ca_pem);
    register(
        &client,
        "session-1",
        SIDECAR,
        vec![spec("host-17/agent", 7443)],
    )
    .expect("register");
    report(&client, "session-1", SIDECAR, "host-17/agent");
    assert_eq!(authority.registry().snapshot().len(), 1);

    let revoked = block_on(authority.enrollment().revoke_host(SIDECAR)).expect("revoke");
    assert_eq!(revoked, 1);
    let drained = authority.registry().drain_host(SIDECAR);
    assert_eq!(drained, 1);
    assert_eq!(authority.registry().snapshot().len(), 0);

    // A new connection with the revoked certificate fails the handshake.
    let fresh = registration_client(&authority, &cert_pem, &key_pem, &ca_pem);
    let refused = register(
        &fresh,
        "session-2",
        SIDECAR,
        vec![spec("host-17/agent", 7443)],
    );
    assert!(refused.is_err(), "a revoked certificate must not register");

    shutdown.turn_on();
    authority.stop();
}

#[test]
#[cfg(unix)]
fn a_unix_socket_in_an_unwritable_directory_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;

    // Root ignores the directory mode, so the bind cannot fail that way. The
    // Linux integration job runs the tests as root.
    // SAFETY: `geteuid` reads process state and takes no pointer.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skip: the test runs as root, which ignores the directory mode");
        return;
    }

    let dir = TempDir::new().expect("temp dir");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500))
        .expect("make the directory read-only");
    let result =
        foundation_proxy::registration::UnixSocketAcceptor::bind(dir.path().join("reg.sock"));
    // Restore write permission so the temporary directory can be removed.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("restore the directory");
    assert!(
        matches!(
            result,
            Err(foundation_proxy::registration::RegistrationAuthorityError::Socket(_))
        ),
        "an unwritable directory must refuse the socket"
    );
}

#[test]
#[cfg(unix)]
fn a_unix_socket_has_mode_660() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("reg.sock");
    let _acceptor =
        foundation_proxy::registration::UnixSocketAcceptor::bind(&path).expect("bind socket");
    let mode = std::fs::metadata(&path)
        .expect("socket metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o660);
}

#[test]
fn a_token_issued_before_a_restart_still_enrolls() {
    let _guard = initialize_pool(110, Some(8));
    let dir = TempDir::new().expect("temp dir");

    let (first, first_shutdown) = start_authority(dir.path());
    let token = first
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    first_shutdown.turn_on();
    first.stop();
    drop(first);

    // The second authority starts from the same state directory. Its token key
    // must be the same, or this token fails with UNAUTHENTICATED.
    let (second, second_shutdown) = start_authority(dir.path());
    let (_key_pem, cert_pem, _ca_pem, _request) = enroll(&second, &token, SIDECAR);
    assert_ne0(&cert_pem);

    second_shutdown.turn_on();
    second.stop();
}

#[test]
#[cfg(unix)]
fn a_unix_socket_uses_the_configured_group() {
    use std::os::unix::fs::MetadataExt as _;

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("reg.sock");
    // SAFETY: `getegid` reads process state and takes no pointer.
    let gid = unsafe { libc::getegid() };
    let _acceptor =
        foundation_proxy::registration::UnixSocketAcceptor::bind_with_group(&path, Some(gid))
            .expect("bind socket");
    assert_eq!(
        std::fs::metadata(&path).expect("socket metadata").gid(),
        gid
    );
}

#[test]
#[cfg(unix)]
fn root_can_give_the_socket_another_group() {
    use std::os::unix::fs::MetadataExt as _;

    // SAFETY: `geteuid` reads process state and takes no pointer.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skip: not root, so the test cannot move the file to another group");
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("reg.sock");
    let _acceptor =
        foundation_proxy::registration::UnixSocketAcceptor::bind_with_group(&path, Some(65534))
            .expect("bind socket");
    assert_eq!(
        std::fs::metadata(&path).expect("socket metadata").gid(),
        65534
    );
}

#[test]
fn the_registration_state_directory_is_required() {
    let _guard = initialize_pool(111, Some(8));
    let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
        SERVICE, HOST,
    )))];
    let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
    let config = RegistrationConfig::new();
    let shutdown = Arc::new(ShutdownSignal::new());
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("host env");
    let result = block_on(RegistrationAuthority::start(
        &config,
        base,
        router,
        ctx,
        Arc::clone(&shutdown),
    ));
    assert!(
        matches!(
            result,
            Err(foundation_proxy::registration::RegistrationAuthorityError::Config(_))
        ),
        "a missing state directory must be a typed config error"
    );
    shutdown.turn_on();
}

#[test]
fn an_ephemeral_state_directory_needs_the_flag() {
    let _guard = initialize_pool(112, Some(8));
    let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
        SERVICE, HOST,
    )))];
    let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
    let config = RegistrationConfig::new().with_allow_ephemeral_state(true);
    let shutdown = Arc::new(ShutdownSignal::new());
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("host env");
    let authority = block_on(RegistrationAuthority::start(
        &config,
        base,
        router,
        ctx,
        Arc::clone(&shutdown),
    ))
    .expect("an ephemeral state directory is accepted when the flag is set");
    shutdown.turn_on();
    authority.stop();
}
