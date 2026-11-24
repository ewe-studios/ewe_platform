//! Registration client tests against the real registration authority (T7).
//!
//! WHY: the sidecar's one authenticated link must enroll with a one-time
//! token, verify the server proof, use mutual TLS (mTLS), renew at two thirds
//! of the certificate life, and use a Unix socket when it shares the machine
//! with the proxy.
//!
//! WHAT: enrollment, the token file deletion, the stored file modes, a second
//! process over the stored identity, renewal, the Unix socket, a superseded
//! session and an expired certificate.
//!
//! HOW: the proxy's `RegistrationAuthority` is the real test server. The
//! clock is simulated, so a certificate lifetime runs without a real wait.

mod support;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::initialize_pool;
use foundation_http::ShutdownSignal;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::registration::{RegistrationAuthority, RegistrationConfig};
use foundation_proxy::router::{Router, RouterHandle};
use foundation_proxy::runtime::ServiceRuntime;
use foundation_registrar::proto::v1::{BackendHealth, BackendReport, BackendSpec, Protocol, Route};
use foundation_registrar::{
    RegistrarConfig, RegistrarError, RegistrarMode, RegistrationApi, RegistrationClient,
};
use support::Sim;

const HOST: &str = "agents.example.com";
const SERVICE: &str = "agents";
const SIDECAR: &str = "host-17";

fn start_authority(
    dir: &Path,
    ctx: Ctx,
    lifetime: Option<Duration>,
    unix_socket: bool,
) -> (Arc<RegistrationAuthority>, Arc<ShutdownSignal>) {
    let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
        SERVICE, HOST,
    )))];
    let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
    let mut config = RegistrationConfig::new()
        .with_state_dir(dir)
        .with_server_names(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    if unix_socket {
        config = config.with_unix_socket(dir.join("registration.sock"));
    } else {
        config = config.with_bind("127.0.0.1:0");
    }
    if let Some(lifetime) = lifetime {
        config.client_cert_lifetime = lifetime;
    }
    let shutdown = Arc::new(ShutdownSignal::new());
    let authority = futures_lite::future::block_on(RegistrationAuthority::start(
        &config,
        base,
        router,
        ctx,
        Arc::clone(&shutdown),
    ))
    .expect("start the registration authority");
    (authority, shutdown)
}

fn client_config(authority: &RegistrationAuthority, dir: &Path) -> RegistrarConfig {
    let token_dir = dir.join("state");
    RegistrarConfig::new(RegistrarMode::Container, SIDECAR)
        .with_proxy(format!(
            "https://{}",
            authority.registration_addr().expect("registration address")
        ))
        .with_enrollment_url(format!(
            "https://{}",
            authority.enrollment_addr().expect("enrollment address")
        ))
        .with_state_dir(&token_dir)
        .with_enrollment_token_path(dir.join("token"))
}

fn write_token(authority: &RegistrationAuthority, dir: &Path) {
    let token = authority
        .enrollment()
        .issue_token(SIDECAR, None, None)
        .expect("issue token");
    std::fs::write(dir.join("token"), token).expect("write token");
}

fn backend_spec(port: u16) -> BackendSpec {
    BackendSpec {
        backend_id: format!("{SIDECAR}/app"),
        service: SERVICE.to_string(),
        route: buffa::MessageField::some(Route {
            host: HOST.to_string(),
            path_prefix: "/".to_string(),
            ..Default::default()
        }),
        protocol: Protocol::PROTOCOL_HTTP.into(),
        address: "127.0.0.1".to_string(),
        port: u32::from(port),
        weight: 1,
        version: "1".to_string(),
        ..Default::default()
    }
}

fn ready_report() -> BackendReport {
    BackendReport {
        backend_id: format!("{SIDECAR}/app"),
        health: BackendHealth::BACKEND_HEALTH_READY.into(),
        ..Default::default()
    }
}

#[test]
fn enrollment_writes_0600_files_deletes_the_token_and_registers() {
    let _guard = initialize_pool(301, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, false);
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());

    let client = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("enroll and connect");

    assert!(
        !dir.path().join("token").exists(),
        "the token file is deleted"
    );
    for file in ["client_cert.pem", "client_key.pem", "ca.pem"] {
        let path = config.state_dir.join(file);
        assert!(path.is_file(), "{file} is stored");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{file} is mode 0600");
        }
    }

    let response =
        futures_lite::future::block_on(client.register(&[backend_spec(9000)])).expect("register");
    assert_eq!(response.results[0].rejected_reason, "");
    futures_lite::future::block_on(client.report(&[ready_report()])).expect("report");
    let snapshot = authority.registry().snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].owner_host_id, SIDECAR);
    assert!(snapshot[0].is_in_rotation());

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_second_process_uses_the_stored_identity_and_a_new_session() {
    let _guard = initialize_pool(302, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, false);
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());

    let first = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("first process");
    futures_lite::future::block_on(first.register(&[backend_spec(9000)])).expect("register");
    futures_lite::future::block_on(first.report(&[ready_report()])).expect("report");
    let first_session = first.session().session_id.clone();
    drop(first);

    let second = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("the stored identity is enough");
    assert_ne!(
        second.session().session_id,
        first_session,
        "a new process makes a new session"
    );
    let response = futures_lite::future::block_on(second.register(&[backend_spec(9000)]))
        .expect("register after restart");
    assert_eq!(response.results[0].rejected_reason, "");

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn an_old_session_is_superseded_by_the_new_one() {
    let _guard = initialize_pool(303, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, false);
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());

    let first = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("first process");
    futures_lite::future::block_on(first.register(&[backend_spec(9000)])).expect("register");
    let second = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("second process");
    futures_lite::future::block_on(second.register(&[backend_spec(9000)])).expect("register");

    let error = futures_lite::future::block_on(first.report(&[ready_report()]))
        .expect_err("the old session is superseded");
    assert_eq!(error, RegistrarError::Superseded(SIDECAR.to_string()));

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn the_certificate_renews_at_two_thirds_of_its_life() {
    let _guard = initialize_pool(304, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(
        proxy_dir.path(),
        sim.ctx.clone(),
        Some(Duration::from_mins(15)),
        false,
    );
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());

    let client = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("enroll");
    let before = client.identity().validity().expect("validity before");
    assert!(!client.identity().renewal_due(now_ms(&sim)), "not due yet");

    sim.advance_wall(Duration::from_secs(501));
    assert!(
        client.identity().renewal_due(now_ms(&sim)),
        "two thirds passed"
    );
    futures_lite::future::block_on(client.renew_if_due()).expect("renew");
    let after = client.identity().validity().expect("validity after");
    assert!(
        after.1 > before.1 + 400,
        "the new certificate is later: {before:?} -> {after:?}"
    );

    // The renewed certificate still registers.
    let response = futures_lite::future::block_on(client.register(&[backend_spec(9000)]))
        .expect("register after renewal");
    assert_eq!(response.results[0].rejected_reason, "");

    shutdown.turn_on();
    authority.stop();
}

#[test]
#[cfg(unix)]
fn a_unix_socket_proxy_skips_enrollment() {
    let _guard = initialize_pool(305, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, true);
    let socket = authority.unix_path().expect("unix path");
    let config = RegistrarConfig::new(RegistrarMode::Container, SIDECAR)
        .with_proxy(format!("unix://{}", socket.display()))
        .with_state_dir(dir.path());

    let client = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("connect over the Unix socket");
    assert!(
        !config.state_dir.join("client_cert.pem").exists(),
        "no certificate is written for a Unix socket"
    );
    let response =
        futures_lite::future::block_on(client.register(&[backend_spec(9000)])).expect("register");
    assert_eq!(response.results[0].rejected_reason, "");
    futures_lite::future::block_on(client.report(&[ready_report()])).expect("report");
    assert_eq!(authority.registry().snapshot().len(), 1);

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn an_expired_certificate_without_a_token_needs_an_operator() {
    let _guard = initialize_pool(306, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(
        proxy_dir.path(),
        sim.ctx.clone(),
        Some(Duration::from_secs(5)),
        false,
    );
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());

    let client = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect("enroll");
    drop(client);
    sim.advance_wall(Duration::from_secs(6));

    let error = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect_err("an expired certificate cannot connect");
    match error {
        RegistrarError::Enroll(message) => {
            assert!(message.contains("operator"), "{message}");
            assert!(message.contains("token"), "{message}");
        }
        other => panic!("expected an enrollment error, got {other:?}"),
    }

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_missing_token_without_a_certificate_is_an_enrollment_error() {
    let _guard = initialize_pool(307, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, false);
    let config = client_config(&authority, dir.path());

    let error = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect_err("no token");
    match error {
        RegistrarError::Enroll(message) => assert!(message.contains("token"), "{message}"),
        other => panic!("expected an enrollment error, got {other:?}"),
    }

    shutdown.turn_on();
    authority.stop();
}

#[test]
fn a_wrong_certificate_pin_refuses_enrollment() {
    let _guard = initialize_pool(308, Some(8));
    let sim = Sim::new();
    let proxy_dir = tempfile::TempDir::new().expect("proxy dir");
    let dir = tempfile::TempDir::new().expect("test dir");
    let (authority, shutdown) = start_authority(proxy_dir.path(), sim.ctx.clone(), None, false);
    write_token(&authority, dir.path());
    let config = client_config(&authority, dir.path());
    let config = RegistrarConfig {
        proxy_fingerprint_sha256: Some("0".repeat(64)),
        ..config
    };

    let error = futures_lite::future::block_on(RegistrationClient::connect(&config, &sim.ctx))
        .expect_err("the pin does not match");
    assert!(matches!(error, RegistrarError::Enroll(_)), "{error:?}");

    shutdown.turn_on();
    authority.stop();
}

fn now_ms(sim: &Sim) -> u64 {
    u64::try_from(foundation_compact::time::unix_millis(
        sim.ctx.clock().wall_now(),
    ))
    .unwrap_or(0)
}
