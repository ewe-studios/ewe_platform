//! End-to-end test for ACME certificate provisioning (Decision 18, F15).
//!
//! Stands up a **real** mock ACME CA over `foundation_http::HttpServer` that
//! implements the RFC 8555 flow (directory → nonce → account → order →
//! authorization → challenge → finalize → certificate) and signs the client's
//! CSR with a test CA. Then it drives `AcmeCertManager` (and a full
//! `ProxyServer` with the `LetsEncrypt` provider) against it and verifies a
//! usable cert+key is produced — proving ACME is actually wired, not stubbed.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};
use foundation_netio::http::{HttpConnectionPool, NativeHttpClient};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::client::{ClientConfig, SystemDnsResolver};
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};

use foundation_proxy::acme::Dns01Setter;
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::tls::{AcmeCertManager, CertManager};
use foundation_proxy::{ProxyConfig, ProxyServer};

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

// ── Mock ACME CA ─────────────────────────────────────────────────────────

struct MockAcme {
    base: String,
    ca_cert: rcgen::Certificate,
    ca_key: rcgen::KeyPair,
    issued: Mutex<Option<String>>,
    nonce: std::sync::atomic::AtomicU64,
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
        }
    }

    fn next_nonce(&self) -> String {
        let n = self.nonce.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        B64.encode(n.to_be_bytes())
    }

    /// Sign the CSR embedded in a finalize JWS body and store the issued PEM.
    fn sign_csr_from_finalize(&self, body: &[u8]) -> Result<(), String> {
        let jws: serde_json::Value =
            serde_json::from_slice(body).map_err(|e| format!("jws json: {e}"))?;
        let payload_b64 = jws["payload"].as_str().unwrap_or("");
        let payload = B64.decode(payload_b64).map_err(|e| format!("payload b64: {e}"))?;
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

struct MockAcmeHandler {
    state: Arc<MockAcme>,
}

impl ServeFactory for MockAcmeHandler {
    fn create(bag: &ContextBag) -> Self {
        Self { state: bag.get::<MockAcme>().expect("MockAcme in bag") }
    }
}

impl Serve for MockAcmeHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let path = req.request_url.url.clone();
        let base = &self.state.base;

        let result = match path.as_str() {
            "/directory" => respond::json(
                &mut conn,
                200,
                &serde_json::json!({
                    "newNonce": format!("{base}/new-nonce"),
                    "newAccount": format!("{base}/new-account"),
                    "newOrder": format!("{base}/new-order"),
                }),
            ),
            "/new-nonce" => {
                // RFC 8555 §7.2: nonce is returned in the Replay-Nonce header.
                let nonce = self.state.next_nonce();
                if let Ok(r) = SimpleOutgoingResponse::builder()
                    .with_status(Status::OK)
                    .add_header(SimpleHeader::custom("replay-nonce"), nonce.as_str())
                    .with_body(SendSafeBody::None)
                    .build()
                {
                    let _ = Http11::response(r).http_render_to_writer(&mut conn);
                }
                let _ = conn.flush();
                return ConnectionResult::Close(None);
            }
            "/new-account" => respond::json(
                &mut conn,
                200,
                &serde_json::json!({ "status": "valid", "orders": format!("{base}/orders") }),
            ),
            "/new-order" => respond::json(
                &mut conn,
                201,
                &serde_json::json!({
                    "status": "pending",
                    "finalize": format!("{base}/finalize"),
                    "authorizations": [format!("{base}/authz/0")],
                }),
            ),
            "/authz/0" => respond::json(
                &mut conn,
                200,
                &serde_json::json!({
                    "status": "pending",
                    "identifier": { "type": "dns", "value": "test.local" },
                    "challenges": [{
                        "type": "dns-01",
                        "url": format!("{base}/chal/0"),
                        "token": "mock-token",
                        "status": "pending",
                    }],
                }),
            ),
            "/chal/0" => respond::json(&mut conn, 200, &serde_json::json!({ "status": "valid" })),
            "/finalize" => {
                let body = req.body.and_then(|b| try_collect_bytes(b).ok()).unwrap_or_default();
                match self.state.sign_csr_from_finalize(&body) {
                    Ok(()) => respond::json(
                        &mut conn,
                        200,
                        &serde_json::json!({
                            "status": "valid",
                            "certificate": format!("{base}/cert/0"),
                        }),
                    ),
                    Err(e) => respond::text(&mut conn, 400, &format!("finalize: {e}")),
                }
            }
            "/cert/0" => {
                let pem = self.state.issued.lock().unwrap().clone().unwrap_or_default();
                respond::text(&mut conn, 200, &pem)
            }
            _ => respond::not_found(&mut conn),
        };
        let _ = result;
        let _ = conn.flush();
        ConnectionResult::Close(None)
    }
}

/// Start the mock ACME CA on an ephemeral port. Returns `(directory_url, shutdown)`.
fn start_mock_acme() -> (String, Arc<OnSignal>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock acme");
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");

    let mut app = HttpApp::new_serve();
    app.context().store(MockAcme::new(base.clone()));
    app.route_any::<MockAcmeHandler>("/*");

    let server = HttpServer::with_config(app, &format!("127.0.0.1:{port}"), ServerConfig::defaults());
    let shutdown = Arc::new(OnSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    std::thread::spawn(move || server.serve_with_listener(&listener, &thread_shutdown));

    (format!("{base}/directory"), shutdown)
}

fn test_client() -> Arc<NativeHttpClient<SystemDnsResolver>> {
    let pool = Arc::new(HttpConnectionPool::default());
    Arc::new(NativeHttpClient::with_config_and_pool(
        ClientConfig::default(),
        pool,
        SystemDnsResolver::default(),
    ))
}

fn noop_dns_setter() -> Dns01Setter {
    Arc::new(|_name: &str, _value: &str| Ok(()))
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn acme_cert_manager_provisions_a_usable_cert() {
    let _guard = initialize_pool(42, Some(4));
    let (directory, shutdown) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    let manager = AcmeCertManager::new(
        vec!["test.local".to_string()],
        "admin@test.local",
        directory,
        test_client(),
        noop_dns_setter(),
    );

    let pair = manager.get_cert().expect("ACME provisioning should yield a cert");
    let cert = String::from_utf8_lossy(&pair.cert_chain);
    let key = String::from_utf8_lossy(&pair.private_key);
    assert!(cert.contains("BEGIN CERTIFICATE"), "cert PEM: {cert}");
    assert!(key.contains("PRIVATE KEY"), "key PEM present");

    // The provisioned cert + key must build a real TLS acceptor (they match).
    use foundation_netio::netcap::ssl::SSLAcceptor;
    SSLAcceptor::from_pem(pair.cert_chain.clone(), pair.private_key.clone().into())
        .expect("provisioned cert+key must build a TLS acceptor");

    // Cached: a second call returns the same cert without re-provisioning.
    let again = manager.get_cert().expect("cached cert");
    assert_eq!(again.cert_chain, pair.cert_chain, "second get_cert is cached");

    shutdown.turn_on();
}

#[test]
fn proxy_server_starts_with_acme_letsencrypt_provider() {
    let _guard = initialize_pool(43, Some(4));
    let (directory, shutdown) = start_mock_acme();
    std::thread::sleep(Duration::from_millis(200));

    // A `LetsEncrypt` provider pointed at the mock CA — starting the server
    // provisions the cert and builds the TLS acceptor end-to-end.
    let svc = ServiceConfig::new("echo", "test.local").backend("http://127.0.0.1:19201");
    let config = ProxyConfig::new("test.local", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::lets_encrypt("admin@test.local"))
        .acme(noop_dns_setter(), Some(directory))
        .service(svc);

    let proxy = ProxyServer::start(config).expect("proxy must start with ACME-provisioned TLS");
    assert!(proxy.local_addr().port() > 0, "TLS front end bound");
    proxy.shutdown();

    shutdown.turn_on();
}
