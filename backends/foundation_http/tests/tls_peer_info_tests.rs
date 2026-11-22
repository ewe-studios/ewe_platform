// Tests build the effect sources on purpose (spec 84, section 6, item 2).
#![allow(clippy::disallowed_methods)]

//! The TLS peer chain reaches the handler on `ConnectionContext::tls`.
//!
//! Spec 80 feature 00 task T4. A mutual-TLS server accepts a client
//! certificate; the protocol detector completes the handshake, captures the
//! peer chain, and rebuilds the connection context before the handler runs.

#![cfg(all(feature = "ssl-rustls", not(target_family = "wasm")))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::native::writer::send_text;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::ConnectionResult;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::{ConnectionContext, RawStream};
use foundation_netio::shared::http::SimpleIncomingRequest;
use rcgen::{BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig as RustlsServerConfig};

struct Capture(Arc<Mutex<Option<Arc<ConnectionContext>>>>);

impl Serve for Capture {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        *self.0.lock().expect("capture lock") = Some(Arc::clone(&req.connection));
        send_text(conn, 200, "ok", &ctx)
    }
}

fn pkcs8(key: &KeyPair) -> PrivateKeyDer<'static> {
    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()))
}

#[test]
#[serial_test::serial]
fn tls_peer_certificates_reach_the_handler() {
    let _pool = initialize_pool(301, Some(2));

    // A CA, a server certificate and a client certificate with the right EKUs.
    let ca_key = KeyPair::generate().expect("CA key");
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "peer-info test CA");
    let ca_cert = ca_params.self_signed(&ca_key).expect("CA cert");

    let server_key = KeyPair::generate().expect("server key");
    let mut server_params =
        CertificateParams::new(vec!["localhost".to_string()]).expect("server params");
    server_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .expect("server cert");

    let client_key = KeyPair::generate().expect("client key");
    let mut client_params = CertificateParams::new(Vec::<String>::new()).expect("client params");
    client_params
        .distinguished_name
        .push(DnType::CommonName, "peer-info test client");
    client_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ClientAuth);
    let client_cert = client_params
        .signed_by(&client_key, &ca_cert, &ca_key)
        .expect("client cert");

    let ca_der: CertificateDer<'static> = ca_cert.der().clone();
    let client_der: CertificateDer<'static> = client_cert.der().clone();

    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let mut server_roots = RootCertStore::empty();
    server_roots.add(ca_der.clone()).expect("server root");
    let verifier =
        WebPkiClientVerifier::builder_with_provider(Arc::new(server_roots), Arc::clone(&provider))
            .build()
            .expect("client verifier");
    let rustls_server = RustlsServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .expect("server versions")
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![server_cert.der().clone()], pkcs8(&server_key))
        .expect("server config");
    let acceptor = Arc::new(foundation_netio::netcap::ssl::SSLAcceptor::with_config(
        Arc::new(rustls_server),
    ));

    let captured: Arc<Mutex<Option<Arc<ConnectionContext>>>> = Arc::new(Mutex::new(None));
    let handler: Arc<dyn Serve> = Arc::new(Capture(Arc::clone(&captured)));
    let mut app = HttpApp::new_serve();
    app.router.add_route_any("/*", &handler);

    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server_config = ServerConfig::defaults().with_tls(acceptor);
    let server = HttpServer::with_config(app, &addr.to_string(), server_config);
    let shutdown_thread = Arc::clone(&shutdown);
    let handle = thread::spawn(move || server.serve_tls_with_listener(&listener, &shutdown_thread));

    // The client trusts the CA and presents its own certificate.
    let mut client_roots = RootCertStore::empty();
    client_roots.add(ca_der).expect("client root");
    let client_config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("client versions")
        .with_root_certificates(client_roots)
        .with_client_auth_cert(vec![client_der.clone()], pkcs8(&client_key))
        .expect("client config");
    let connection = ClientConnection::new(
        Arc::new(client_config),
        ServerName::try_from("localhost").expect("server name"),
    )
    .expect("client connection");
    let tcp = TcpStream::connect(addr).expect("connect");
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut tls = rustls::StreamOwned::new(connection, tcp);
    tls.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .expect("write request");
    let mut response = Vec::new();
    let _ = tls.read_to_end(&mut response);
    assert!(
        response.starts_with(b"HTTP/1.1 200"),
        "response: {:?}",
        String::from_utf8_lossy(&response)
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let connection = loop {
        if let Some(connection) = captured.lock().expect("capture lock").clone() {
            break connection;
        }
        assert!(Instant::now() < deadline, "the handler never ran");
        thread::sleep(Duration::from_millis(10));
    };

    let tls_info = connection.tls.as_ref().expect("the handler sees TLS info");
    assert_eq!(
        tls_info.peer_certificates.first(),
        Some(&client_der.to_vec()),
        "the peer chain starts with the client certificate"
    );

    shutdown.turn_on();
    let _ = handle.join();
}
