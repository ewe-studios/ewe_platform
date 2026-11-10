//! End-to-end tests for ACME certificate provisioning (Decision 18, F15).
//!
//! Stands up a **strict** mock ACME CA over `foundation_http::HttpServer` that
//! implements the RFC 8555 flow (directory → nonce → account → order →
//! authorization → challenge → finalize → certificate) and signs the client's
//! CSR with a test CA. The CA enforces the account flow the client must follow:
//! the newAccount answer carries the account URL in `Location`, every later
//! JWS uses that URL as its `kid` (never the `jwk`), and the `Replay-Nonce`
//! from each answer is single-use and carried to the next request.
//!
//! Then it drives `AcmeCertManager` (and a full `ProxyServer` with the
//! `LetsEncrypt` provider) against the CA and verifies a usable cert+key is
//! produced — proving ACME is actually wired, not stubbed.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{initialize_pool, sleep_async, BlockingFutureExt};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::native::writer::{
    render_message, send_json, send_not_found, ResponseWriter, WriterConfig,
};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, Status,
};
use foundation_netio::{DynNetClient, HttpClientBuilder};

use foundation_proxy::acme::{AcmeClient, Dns01Setter};
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::tls::{AcmeCertManager, CertManager};
use foundation_proxy::{ProxyConfig, ProxyServer};

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

const ACCOUNT_URL_PATH: &str = "/account/0";

// ── Mock ACME CA ─────────────────────────────────────────────────────────

struct MockAcme {
    base: String,
    ca_cert: rcgen::Certificate,
    ca_key: rcgen::KeyPair,
    issued: Mutex<Option<String>>,
    nonce: std::sync::atomic::AtomicU64,
    /// Nonces issued but not yet used (each Replay-Nonce is single-use).
    valid_nonces: Mutex<HashSet<String>>,
    /// How many times the client fetched a fresh nonce from `newNonce`.
    nonce_fetches: AtomicUsize,
}

impl MockAcme {
    fn new(base: String) -> Self {
        let ca_key = rcgen::KeyPair::generate().expect("ca key");
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("ca params");
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");
        Self {
            base,
            ca_cert,
            ca_key,
            issued: Mutex::new(None),
            nonce: std::sync::atomic::AtomicU64::new(1),
            valid_nonces: Mutex::new(HashSet::new()),
            nonce_fetches: AtomicUsize::new(0),
        }
    }

    /// Issue a fresh nonce and record it as valid (single-use).
    fn issue_nonce(&self) -> String {
        let n = self.nonce.fetch_add(1, Ordering::Relaxed);
        let nonce = B64.encode(n.to_be_bytes());
        self.valid_nonces.lock().unwrap().insert(nonce.clone());
        nonce
    }

    /// Use (and invalidate) a nonce carried in a JWS protected header.
    fn consume_nonce(&self, nonce: &str) -> Result<(), String> {
        let mut valid = self.valid_nonces.lock().unwrap();
        if valid.remove(nonce) {
            Ok(())
        } else {
            Err(format!("unknown or reused nonce {nonce}"))
        }
    }

    /// The account URL the newAccount `Location` header must carry.
    fn account_url(&self) -> String {
        format!("{}{ACCOUNT_URL_PATH}", self.base)
    }

    /// Check a JWS protected header. `expect_kid` is false only for the
    /// newAccount request itself (signed with the `jwk`).
    fn validate_jws(&self, body: &[u8], expect_kid: bool) -> Result<(), String> {
        let protected = jws_protected(body)?;
        let nonce = protected
            .get("nonce")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "no nonce in protected header".to_string())?;
        self.consume_nonce(nonce)?;

        if expect_kid {
            let kid = protected
                .get("kid")
                .and_then(|k| k.as_str())
                .ok_or_else(|| "no kid in protected header".to_string())?;
            if kid != self.account_url() {
                return Err(format!("kid {kid:?} is not the account URL"));
            }
            if protected.get("jwk").is_some() {
                return Err("jwk used after account creation".to_string());
            }
        } else {
            if protected.get("jwk").is_none() {
                return Err("no jwk on newAccount".to_string());
            }
            if protected.get("kid").is_some() {
                return Err("kid used before account creation".to_string());
            }
        }
        Ok(())
    }

    /// Sign the CSR embedded in a finalize JWS body and store the issued PEM.
    fn sign_csr_from_finalize(&self, body: &[u8]) -> Result<(), String> {
        let jws: serde_json::Value =
            serde_json::from_slice(body).map_err(|e| format!("jws json: {e}"))?;
        let payload_b64 = jws["payload"].as_str().unwrap_or("");
        let payload = B64
            .decode(payload_b64)
            .map_err(|e| format!("payload b64: {e}"))?;
        let fin: serde_json::Value =
            serde_json::from_slice(&payload).map_err(|e| format!("finalize json: {e}"))?;
        let csr_b64 = fin["csr"].as_str().ok_or("no csr in finalize")?;
        let csr_der = B64.decode(csr_b64).map_err(|e| format!("csr b64: {e}"))?;

        let der = rustls_pki_types::CertificateSigningRequestDer::from(csr_der);
        let csr_params = rcgen::CertificateSigningRequestParams::from_der(&der)
            .map_err(|e| format!("parse csr: {e}"))?;
        let issued = csr_params
            .signed_by(&self.ca_cert, &self.ca_key)
            .map_err(|e| format!("sign csr: {e}"))?;
        *self.issued.lock().unwrap() = Some(issued.pem());
        Ok(())
    }
}

/// Decode the JWS Flattened JSON `protected` field (base64url) into JSON.
fn jws_protected(body: &[u8]) -> Result<serde_json::Value, String> {
    let jws: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("jws json: {e}"))?;
    let protected_b64 = jws["protected"].as_str().unwrap_or("");
    let protected = B64
        .decode(protected_b64)
        .map_err(|e| format!("protected b64: {e}"))?;
    serde_json::from_slice(&protected).map_err(|e| format!("protected json: {e}"))
}

/// Render one ACME answer with a fresh `Replay-Nonce` (and an optional
/// `Location` header for newAccount) through the async response writer.
async fn render_acme(
    conn: &SharedByteBufferStream<RawStream>,
    status: Status,
    nonce: &str,
    content_type: &str,
    body: SendSafeBody,
    location: Option<&str>,
) {
    let mut builder = SimpleOutgoingResponse::builder()
        .with_status(status)
        .add_header(SimpleHeader::custom("replay-nonce"), nonce)
        .add_header(SimpleHeader::CONTENT_TYPE, content_type);
    if let Some(loc) = location {
        builder = builder.add_header(SimpleHeader::LOCATION, loc);
    }
    let Ok(response) = builder.with_body(body).build() else {
        tracing::error!("mock ACME: response build failed");
        return;
    };
    let Ok(head) = render_message(response) else {
        tracing::error!("mock ACME: response render failed");
        return;
    };
    let mut writer = ResponseWriter::new(conn.clone(), WriterConfig::default());
    let result = async {
        writer.write_all(&head).await?;
        writer.flush().await
    }
    .await;
    if let Err(e) = result {
        tracing::warn!("mock ACME: response write failed: {e}");
    }
}

/// Write an already-built async response to completion.
async fn write_response(response: ConnectionResult) {
    if let Ok(write) = response.split() {
        let _ = write.await;
    }
}

/// Render a validated 2xx ACME answer with a fresh `Replay-Nonce`.
// The JSON value is serialized by reference, so the caller can reuse it.
#[allow(clippy::needless_pass_by_value)]
async fn render_acme_ok(
    state: &MockAcme,
    conn: &SharedByteBufferStream<RawStream>,
    status: Status,
    value: serde_json::Value,
    location: Option<&str>,
) {
    let nonce = state.issue_nonce();
    let bytes = serde_json::to_vec(&value).expect("serialize acme body");
    render_acme(
        conn,
        status,
        &nonce,
        "application/json",
        SendSafeBody::Bytes(bytes),
        location,
    )
    .await;
}

/// Answer a JWS POST after validation; on a bad request the client gets a
/// problem document plus a fresh nonce.
async fn serve_acme_post(
    state: &MockAcme,
    conn: &SharedByteBufferStream<RawStream>,
    body: &[u8],
    status: Status,
    value: serde_json::Value,
    location: Option<&str>,
) {
    match state.validate_jws(body, true) {
        Ok(()) => render_acme_ok(state, conn, status, value, location).await,
        Err(e) => {
            let nonce = state.issue_nonce();
            let bytes = serde_json::to_vec(&serde_json::json!({
                "type": "urn:ietf:params:acme:error:malformed",
                "detail": e,
            }))
            .expect("serialize problem");
            render_acme(
                conn,
                Status::BadRequest,
                &nonce,
                "application/problem+json",
                SendSafeBody::Bytes(bytes),
                None,
            )
            .await;
        }
    }
}

struct MockAcmeHandler {
    state: Arc<MockAcme>,
}

impl ServeFactory for MockAcmeHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            state: ctx.get::<MockAcme>().expect("MockAcme in ctx"),
        }
    }
}

impl Serve for MockAcmeHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let state = Arc::clone(&self.state);
        ConnectionResult::Async(Box::pin(handle_acme(state, req, conn)))
    }
}

/// Answer one ACME request. Every answer goes out through the async response
/// writer, so a large certificate body cannot truncate.
#[allow(
    clippy::too_many_lines,
    reason = "the mock ACME server reads best as one ordered match"
)]
async fn handle_acme(
    state: Arc<MockAcme>,
    req: SimpleIncomingRequest,
    conn: SharedByteBufferStream<RawStream>,
) -> ServeOutcome {
    let path = req.request_url.url.clone();
    let base = &state.base;
    let body = req.body.and_then(|b| try_collect_bytes(b).ok());
    let raw = body.as_deref().unwrap_or_default();

    match path.as_str() {
        "/directory" => {
            write_response(send_json(
                conn.clone(),
                200,
                &serde_json::json!({
                    "newNonce": format!("{base}/new-nonce"),
                    "newAccount": format!("{base}/new-account"),
                    "newOrder": format!("{base}/new-order"),
                }),
            ))
            .await;
        }
        "/new-nonce" => {
            // RFC 8555 §7.2: nonce is returned in the Replay-Nonce header.
            state.nonce_fetches.fetch_add(1, Ordering::Relaxed);
            let nonce = state.issue_nonce();
            render_acme(
                &conn,
                Status::OK,
                &nonce,
                "application/octet-stream",
                SendSafeBody::None,
                None,
            )
            .await;
            return ServeOutcome::Close(None);
        }
        "/new-account" => match state.validate_jws(raw, false) {
            Ok(()) => {
                let nonce = state.issue_nonce();
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "status": "valid",
                    "orders": format!("{base}/orders"),
                }))
                .expect("serialize account");
                render_acme(
                    &conn,
                    Status::Created,
                    &nonce,
                    "application/json",
                    SendSafeBody::Bytes(bytes),
                    Some(&state.account_url()),
                )
                .await;
            }
            Err(e) => {
                let nonce = state.issue_nonce();
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "type": "urn:ietf:params:acme:error:malformed",
                    "detail": e,
                }))
                .expect("serialize problem");
                render_acme(
                    &conn,
                    Status::BadRequest,
                    &nonce,
                    "application/problem+json",
                    SendSafeBody::Bytes(bytes),
                    None,
                )
                .await;
            }
        },
        "/new-order" => {
            serve_acme_post(
                &state,
                &conn,
                raw,
                Status::Created,
                serde_json::json!({
                    "status": "pending",
                    "finalize": format!("{base}/finalize"),
                    "authorizations": [format!("{base}/authz/0")],
                }),
                None,
            )
            .await;
        }
        "/authz/0" => {
            serve_acme_post(
                &state,
                &conn,
                raw,
                Status::OK,
                serde_json::json!({
                    "status": "pending",
                    "identifier": { "type": "dns", "value": "test.local" },
                    "challenges": [{
                        "type": "dns-01",
                        "url": format!("{base}/chal/0"),
                        "token": "mock-token",
                        "status": "pending",
                    }],
                }),
                None,
            )
            .await;
        }
        "/chal/0" => {
            serve_acme_post(
                &state,
                &conn,
                raw,
                Status::OK,
                serde_json::json!({ "status": "valid" }),
                None,
            )
            .await;
        }
        "/finalize" => match state.validate_jws(raw, true) {
            Ok(()) => match state.sign_csr_from_finalize(raw) {
                Ok(()) => {
                    render_acme_ok(
                        &state,
                        &conn,
                        Status::OK,
                        serde_json::json!({
                            "status": "valid",
                            "certificate": format!("{base}/cert/0"),
                        }),
                        None,
                    )
                    .await;
                }
                Err(e) => {
                    let nonce = state.issue_nonce();
                    let bytes = serde_json::to_vec(&serde_json::json!({
                        "type": "urn:ietf:params:acme:error:badCSR",
                        "detail": e,
                    }))
                    .expect("serialize problem");
                    render_acme(
                        &conn,
                        Status::BadRequest,
                        &nonce,
                        "application/problem+json",
                        SendSafeBody::Bytes(bytes),
                        None,
                    )
                    .await;
                }
            },
            Err(e) => {
                let nonce = state.issue_nonce();
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "type": "urn:ietf:params:acme:error:malformed",
                    "detail": e,
                }))
                .expect("serialize problem");
                render_acme(
                    &conn,
                    Status::BadRequest,
                    &nonce,
                    "application/problem+json",
                    SendSafeBody::Bytes(bytes),
                    None,
                )
                .await;
            }
        },
        "/cert/0" => match state.validate_jws(raw, true) {
            Ok(()) => {
                let pem = state.issued.lock().unwrap().clone().unwrap_or_default();
                let nonce = state.issue_nonce();
                render_acme(
                    &conn,
                    Status::OK,
                    &nonce,
                    "application/pem-certificate-chain",
                    SendSafeBody::Text(pem),
                    None,
                )
                .await;
            }
            Err(e) => {
                let nonce = state.issue_nonce();
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "type": "urn:ietf:params:acme:error:malformed",
                    "detail": e,
                }))
                .expect("serialize problem");
                render_acme(
                    &conn,
                    Status::BadRequest,
                    &nonce,
                    "application/problem+json",
                    SendSafeBody::Bytes(bytes),
                    None,
                )
                .await;
            }
        },
        _ => {
            write_response(send_not_found(conn.clone())).await;
        }
    }
    ServeOutcome::Close(None)
}

/// Start the mock ACME CA on an ephemeral port. Returns
/// `(directory_url, shutdown, state)`.
fn start_mock_acme() -> (String, Arc<ShutdownSignal>, Arc<MockAcme>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock acme");
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");

    let mut app = HttpApp::new_serve();
    app.context().store(MockAcme::new(base.clone()));
    app.route_any::<MockAcmeHandler>("/*");
    let state = app.context().get::<MockAcme>().expect("stored MockAcme");

    let server =
        HttpServer::with_config(app, &format!("127.0.0.1:{port}"), ServerConfig::defaults());
    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    std::thread::spawn(move || server.serve_with_listener(&listener, &thread_shutdown));

    (format!("{base}/directory"), shutdown, state)
}

fn test_client() -> DynNetClient {
    HttpClientBuilder::new().build()
}

fn noop_dns_setter() -> Dns01Setter {
    Arc::new(|_name: &str, _value: &str| Ok(()))
}

/// An ACME client aimed at the mock CA, with a short DNS wait so the tests run
/// fast. The signer is a dummy: the CA checks the JWS headers, not the signature.
fn test_acme_client(directory: &str) -> AcmeClient {
    AcmeClient::with_directory(
        test_client(),
        directory,
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .with_jwk(serde_json::json!({
        "kty": "EC",
        "crv": "P-256",
        "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "y": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    }))
    .with_signer(|_input: &str| -> Result<String, String> { Ok(B64.encode(b"dummy-signature")) })
    .with_dns_setter(|_name: &str, _value: &str| Ok(()))
    .with_dns_wait_timeout(Duration::from_millis(50))
}

/// A valid DER CSR for `domains`, plus the key (kept alive by the caller only).
fn make_csr(domains: &[&str]) -> (rcgen::KeyPair, Vec<u8>) {
    let key = rcgen::KeyPair::generate().expect("csr key");
    let params = rcgen::CertificateParams::new(
        domains
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>(),
    )
    .expect("csr params");
    let csr = params.serialize_request(&key).expect("serialize csr");
    (key, csr.der().as_ref().to_vec())
}

fn provision_acme(acme: &AcmeClient, csr_der: &[u8]) -> Result<(String, Vec<u8>), String> {
    futures_lite::future::block_on(acme.provision(
        &["test.local".to_string()],
        "admin@test.local",
        csr_der,
    ))
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn acme_cert_manager_provisions_a_usable_cert() {
    let _guard = initialize_pool(42, Some(4));
    let (directory, shutdown, _state) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    let manager = AcmeCertManager::new(
        vec!["test.local".to_string()],
        "admin@test.local",
        directory,
        test_client(),
        noop_dns_setter(),
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    );

    let pair = manager
        .get_cert()
        .expect("ACME provisioning should yield a cert");
    let cert = String::from_utf8_lossy(&pair.cert_chain);
    let key = String::from_utf8_lossy(&pair.private_key);
    assert!(cert.contains("BEGIN CERTIFICATE"), "cert PEM: {cert}");
    assert!(key.contains("PRIVATE KEY"), "key PEM present");

    // The provisioned cert + key must build a real TLS acceptor (they match).
    SSLAcceptor::from_pem(pair.cert_chain.clone(), pair.private_key.clone().into())
        .expect("provisioned cert+key must build a TLS acceptor");

    // Cached: a second call returns the same cert without re-provisioning.
    let again = manager.get_cert().expect("cached cert");
    assert_eq!(
        again.cert_chain, pair.cert_chain,
        "second get_cert is cached"
    );

    shutdown.turn_on();
}

#[test]
fn proxy_server_starts_with_acme_letsencrypt_provider() {
    let _guard = initialize_pool(43, Some(4));
    let (directory, shutdown, _state) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    // A `LetsEncrypt` provider pointed at the mock CA — starting the server
    // provisions the cert and builds the TLS acceptor end-to-end.
    let svc = ServiceConfig::new("echo", "test.local").backend("http://127.0.0.1:19201");
    let config = ProxyConfig::new("test.local", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::lets_encrypt("admin@test.local"))
        .acme(noop_dns_setter(), Some(directory))
        .service(svc);

    let proxy = ProxyServer::start(
        config,
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .block()
    .expect("proxy must start with ACME-provisioned TLS");
    assert!(proxy.local_addr().port() > 0, "TLS front end bound");
    proxy.shutdown();

    shutdown.turn_on();
}

/// The client must sign every post-account request with the account URL as the
/// `kid`, never the `jwk`. The strict CA rejects a wrong or missing `kid`, so a
/// successful provision proves the client used the `Location` answer.
#[test]
fn acme_uses_kid_after_account_creation() {
    let _guard = initialize_pool(44, Some(4));
    let (directory, shutdown, _state) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    let (_key, csr_der) = make_csr(&["test.local"]);
    let acme = test_acme_client(&directory);
    let result = provision_acme(&acme, &csr_der);
    assert!(result.is_ok(), "provision failed: {result:?}");

    shutdown.turn_on();
}

/// The client must carry the `Replay-Nonce` from each answer instead of
/// fetching a fresh one for every request. The CA counts `newNonce` fetches:
/// only the first request may fetch one.
#[test]
fn acme_reuses_replay_nonce() {
    let _guard = initialize_pool(45, Some(4));
    let (directory, shutdown, state) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    let (_key, csr_der) = make_csr(&["test.local"]);
    let acme = test_acme_client(&directory);
    let result = provision_acme(&acme, &csr_der);
    assert!(result.is_ok(), "provision failed: {result:?}");

    assert_eq!(
        state.nonce_fetches.load(Ordering::Relaxed),
        1,
        "newNonce must be fetched only for the first request"
    );

    shutdown.turn_on();
}

/// The DNS wait must sleep asynchronously, not block the thread driving the
/// ACME flow. A 2 s wait and a 2 s heartbeat driven together on one thread
/// finish in about 2 s when the wait yields; a blocking sleep serialises them
/// and takes about 4 s.
#[test]
fn acme_challenge_wait_does_not_block_the_executor() {
    let _guard = initialize_pool(46, Some(4));
    let (directory, shutdown, _state) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    let (_key, csr_der) = make_csr(&["test.local"]);
    let acme = test_acme_client(&directory).with_dns_wait_timeout(Duration::from_secs(2));

    let heartbeat = async {
        for _ in 0..20 {
            sleep_async(Duration::from_millis(100)).await;
        }
    };

    let start = Instant::now();
    let (result, ()) = futures_lite::future::block_on(futures_lite::future::zip(
        acme.provision(&["test.local".to_string()], "admin@test.local", &csr_der),
        heartbeat,
    ));
    let elapsed = start.elapsed();

    result.expect("provision must succeed");
    assert!(
        elapsed < Duration::from_secs(3),
        "the DNS wait blocked the driving thread: {elapsed:?}"
    );

    shutdown.turn_on();
}
