//! Registration end to end over real TCP: enroll, register a loopback backend,
//! route a client request to it, redeploy to a new address without a failed
//! request, and recover from a proxy restart through `REREGISTER` (spec 82
//! feature 00, T11).
//!
//! WHY: the parts must work together with a real proxy, a real backend and real
//! Transport Layer Security (TLS), not only in unit tests.
//!
//! WHAT: a loopback HTTP backend, a `ProxyServer` with the registration
//! authority, an enrolled sidecar over mutual TLS (mTLS), and raw client
//! requests through the public listener.
//!
//! HOW: the sidecar's enrollment and registration use generated `ConnectRPC`
//! clients. The in-memory registry is what a proxy restart loses, so the last
//! part builds a fresh `BackendRegistry` over the same base service and proves
//! that a `Report` asks the sidecar to register again.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_auth::pki;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, Transport};
use foundation_context::Ctx;
use foundation_core::valtron::initialize_pool;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::shared::client::{DnsResolver, StaticSocketAddr};
use foundation_netio::HttpClientBuilder;
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::registration::proto::v1::registrar_enrollment_service::RegistrarEnrollmentServiceClient;
use foundation_proxy::registration::proto::v1::registration_service::RegistrationServiceClient;
use foundation_proxy::registration::proto::v1::{
    BackendHealth, BackendReport, BackendSpec, EnrollRequest, Protocol, RegisterRequest,
    ReportAction, ReportRequest, Route, SessionRef,
};
use foundation_proxy::registration::{BackendRegistry, PeerIdentity, RegistrationConfig};
use foundation_proxy::router::{Router, RouterHandle};
use foundation_proxy::runtime::ServiceRuntime;
use foundation_proxy::{ProxyConfig, ProxyServer};
use futures_lite::future::block_on;
use tempfile::TempDir;

const HOST: &str = "agents.example.com";
const SERVICE: &str = "agents";
const SIDECAR: &str = "host-17";
const BACKEND_ID: &str = "host-17/agent";

struct Backend {
    stop: Arc<AtomicBool>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A loopback HTTP/1.1 responder that returns `body` for every request.
fn start_backend(body: &str) -> (u16, Backend) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind backend");
    listener.set_nonblocking(true).ok();
    let port = listener.local_addr().expect("backend addr").port();
    let stop = Arc::new(AtomicBool::new(false));
    let body = body.to_string();
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    let body = body.clone();
                    std::thread::spawn(move || {
                        sock.set_nonblocking(false).ok();
                        sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        let mut buf = [0u8; 4096];
                        while let Ok(n) = sock.read(&mut buf) {
                            if n == 0 {
                                break;
                            }
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
                                body.len()
                            );
                            if sock.write_all(response.as_bytes()).is_err() {
                                break;
                            }
                        }
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    });
    (port, Backend { stop })
}

fn http_get(addr: SocketAddr, host: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect proxy");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .expect("write request");
    stream.flush().ok();
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let body = response
        .split_once("\r\n\r\n")
        .map_or(response.as_str(), |(_, body)| body);
    body.to_string()
}

fn spec(port: u16) -> BackendSpec {
    BackendSpec {
        backend_id: BACKEND_ID.to_string(),
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

fn session(session_id: &str) -> SessionRef {
    SessionRef {
        session_id: session_id.to_string(),
        host_id: SIDECAR.to_string(),
        ..Default::default()
    }
}

fn registration_client(
    base: &str,
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
    RegistrationServiceClient::new(transport, base, ClientOptions::new()).expect("client")
}

fn enroll(
    enrollment_base: &str,
    fingerprint: &str,
    token: &str,
) -> (zeroize::Zeroizing<String>, String, String) {
    let (key_pem, csr_pem) = pki::generate_csr(SIDECAR).expect("client CSR");
    let connector = SSLConnector::with_config(
        foundation_auth::mtls::client_config_pinned_default(fingerprint).expect("pinned config"),
    );
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    let client =
        RegistrarEnrollmentServiceClient::new(transport, enrollment_base, ClientOptions::new())
            .expect("enrollment client");
    let reply = block_on(client.enroll(
        RpcCtx::background(),
        Request::new(EnrollRequest {
            token: token.to_string(),
            host_id: SIDECAR.to_string(),
            csr_pem,
            nonce: vec![3u8; 32],
            ..Default::default()
        }),
    ))
    .expect("enroll")
    .msg;
    (key_pem, reply.client_cert_pem, reply.ca_cert_pem)
}

fn register(client: &RegistrationServiceClient, port: u16) {
    let response = block_on(client.register(
        RpcCtx::background(),
        Request::new(RegisterRequest {
            session: Some(session("session-1")).into(),
            backends: vec![spec(port)],
            ..Default::default()
        }),
    ))
    .expect("register");
    assert_eq!(response.msg.results[0].rejected_reason, "");
}

fn report_ready(client: &RegistrationServiceClient) {
    block_on(client.report(
        RpcCtx::background(),
        Request::new(ReportRequest {
            session: Some(session("session-1")).into(),
            backends: vec![BackendReport {
                backend_id: BACKEND_ID.to_string(),
                health: BackendHealth::BACKEND_HEALTH_READY.into(),
                ..Default::default()
            }],
            ..Default::default()
        }),
    ))
    .expect("report");
}

#[test]
#[allow(clippy::too_many_lines, reason = "one end-to-end story")]
fn register_route_redeploy_and_restart() {
    let _guard = initialize_pool(201, Some(12));
    let (port_one, _backend_one) = start_backend("one");
    let (port_two, _backend_two) = start_backend("two");
    let dir = TempDir::new().expect("temp dir");

    let registration = RegistrationConfig::new()
        .with_bind("127.0.0.1:0")
        .with_state_dir(dir.path())
        .with_server_names(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    let config = ProxyConfig::new(HOST, "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::default())
        .service(ServiceConfig::new(SERVICE, HOST))
        .registration(Some(registration));
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("host env");
    let proxy = block_on(ProxyServer::start(config, ctx)).expect("start proxy");
    let proxy_addr = proxy.local_addr();
    let authority = proxy
        .registration_authority()
        .cloned()
        .expect("registration authority");

    // Enroll the sidecar, then register and report the first backend.
    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    let enrollment_base = format!(
        "https://{}",
        authority.enrollment_addr().expect("enrollment address")
    );
    let fingerprint = authority
        .server_fingerprint()
        .expect("server fingerprint")
        .to_string();
    let (key_pem, cert_pem, ca_pem) = enroll(&enrollment_base, &fingerprint, &token);
    let base = format!(
        "https://{}",
        authority.registration_addr().expect("registration address")
    );
    let client = registration_client(&base, &cert_pem, &key_pem, &ca_pem);
    register(&client, port_one);
    // The sidecar's first Register does not put the backend in rotation.
    assert_eq!(http_get(proxy_addr, HOST), "Service Unavailable");
    report_ready(&client);
    assert_eq!(http_get(proxy_addr, HOST), "one");

    // Redeploy to a new address: the old address keeps serving until the new
    // one reports READY, so no request fails.
    register(&client, port_two);
    assert_eq!(http_get(proxy_addr, HOST), "one");
    report_ready(&client);
    assert_eq!(http_get(proxy_addr, HOST), "two");

    // A proxy restart loses the in-memory registry. The fresh registry answers
    // the sidecar's first Report with REREGISTER, then accepts the Register.
    let base_services = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
        SERVICE, HOST,
    )))];
    let handle = RouterHandle::new(Arc::new(Router::new(base_services.clone())));
    let restarted = Arc::new(BackendRegistry::new(
        base_services,
        handle,
        Ctx::host(),
        RegistrationConfig::new(),
        Arc::new(StaticSocketAddr::new(SocketAddr::from((
            [127, 0, 0, 1],
            9000,
        )))) as Arc<dyn DnsResolver>,
    ));
    let peer = PeerIdentity::from_certificate(SIDECAR, "fingerprint");
    let report = restarted
        .report(
            &peer,
            &ReportRequest {
                session: Some(session("session-1")).into(),
                backends: vec![BackendReport {
                    backend_id: BACKEND_ID.to_string(),
                    health: BackendHealth::BACKEND_HEALTH_READY.into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .expect("report after restart");
    assert_eq!(
        report.action.as_known(),
        Some(ReportAction::REPORT_ACTION_REREGISTER)
    );
    let register = block_on(restarted.register(
        &peer,
        &RegisterRequest {
            session: Some(session("session-1")).into(),
            backends: vec![spec(port_two)],
            ..Default::default()
        },
    ))
    .expect("register after restart");
    assert_eq!(
        register.results[0].generation, 1,
        "the restarted proxy accepts the backend again"
    );
    assert_eq!(
        restarted
            .report(
                &peer,
                &ReportRequest {
                    session: Some(session("session-1")).into(),
                    backends: vec![BackendReport {
                        backend_id: BACKEND_ID.to_string(),
                        health: BackendHealth::BACKEND_HEALTH_READY.into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }
            )
            .expect("report")
            .action
            .as_known(),
        Some(ReportAction::REPORT_ACTION_CONTINUE)
    );
    assert_eq!(
        restarted
            .router()
            .route(HOST, "/")
            .expect("service")
            .backends()
            .len(),
        1
    );

    proxy.shutdown();
}
