//! A real `rustls` handshake through the trust verifier, and the `Ctx` clock.

use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use foundation_auth::mtls::{
    client_config_mutual, server_config_mutual, MtlsConfig, ServerTlsConfig,
};
use foundation_auth::pki::{
    generate_csr, CaConfig, CaKey, CaStore, CertificateAuthority, ServerIdentity,
};
use foundation_auth::trust::{
    MemoryTrustStore, TrustConfig, TrustStore, TrustedClient, TrustedClients,
};
use foundation_context::Ctx;
use foundation_core::valtron::{valtron_test, SimulatedClock};

const NOW: u64 = 1_700_000_000;

fn ctx_at(secs: u64) -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(secs));
    let as_clock: Arc<dyn foundation_context::Clock> = clock.clone();
    (Ctx::fixed(as_clock), clock)
}

/// One server and one enrolled client.
struct Fixture {
    ctx: Ctx,
    ca: CertificateAuthority,
    identity: ServerIdentity,
    client_cert_pem: String,
    client_key_pem: String,
    trusted: Arc<TrustedClients>,
}

async fn build_fixture(ctx: Ctx, client_lifetime: Duration, trust_expires_at: u64) -> Fixture {
    let config = CaConfig::default();
    let (ca, _) = CaStore::memory()
        .load_or_generate(&config, &ctx)
        .expect("CA");
    let server_key = CaKey::generate(config.key_algorithm, &ctx).expect("server key");
    let identity = ca
        .issue_server_identity(
            &server_key,
            &["localhost".to_string()],
            Duration::from_hours(90 * 24),
        )
        .expect("server identity");

    let (client_key_pem, csr_pem) = generate_csr("laptop").expect("CSR");
    let issued = ca
        .sign_client_csr(&csr_pem, "laptop", client_lifetime)
        .expect("client certificate");

    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(Arc::clone(&store), &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    trusted
        .record_enrollment(
            TrustedClient {
                fingerprint_sha256: issued.fingerprint_sha256.clone(),
                client_name: "laptop".to_string(),
                enrolled_at: NOW,
                expires_at: trust_expires_at,
                last_seen_at: None,
                revoked_at: None,
            },
            &issued.cert_pem,
        )
        .await
        .expect("enroll");

    Fixture {
        ctx,
        ca,
        identity,
        client_cert_pem: issued.cert_pem,
        client_key_pem: client_key_pem.to_string(),
        trusted,
    }
}

impl Fixture {
    fn server_config(&self) -> Arc<rustls::ServerConfig> {
        server_config_mutual(&self.identity, &self.ca, self.trusted.index(), &self.ctx)
            .expect("server config")
    }

    fn client_config(&self) -> Arc<rustls::ClientConfig> {
        client_config_mutual(
            self.ca.ca_cert_pem(),
            &self.client_cert_pem,
            &self.client_key_pem,
            &self.ctx,
        )
        .expect("client config")
    }
}

/// Run one complete handshake over a loopback Transmission Control Protocol
/// (TCP) connection. The server sends two bytes when the handshake succeeds.
fn handshake(
    server: Arc<rustls::ServerConfig>,
    client: Arc<rustls::ClientConfig>,
) -> Result<(), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let server_thread = std::thread::spawn(move || -> Result<(), String> {
        let (socket, _) = listener.accept().map_err(|e| e.to_string())?;
        let connection = rustls::ServerConnection::new(server).map_err(|e| e.to_string())?;
        let mut stream = rustls::StreamOwned::new(connection, socket);
        stream.write_all(b"ok").map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())
    });

    let result = (|| -> Result<(), String> {
        let socket = std::net::TcpStream::connect(addr).map_err(|e| e.to_string())?;
        let name = rustls::pki_types::ServerName::try_from("localhost")
            .map_err(|e| e.to_string())?
            .to_owned();
        let connection = rustls::ClientConnection::new(client, name).map_err(|e| e.to_string())?;
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut buffer = [0_u8; 2];
        stream.read_exact(&mut buffer).map_err(|e| e.to_string())
    })();

    let outcome = server_thread
        .join()
        .map_err(|_| "the server thread panicked".to_string())?;
    result.and(outcome)
}

#[valtron_test]
async fn rustls_handshake_succeeds() {
    let (ctx, _clock) = ctx_at(NOW);
    let fixture = build_fixture(ctx, Duration::from_hours(1), NOW + 3_600).await;
    handshake(fixture.server_config(), fixture.client_config())
        .expect("an enrolled client completes the handshake");
}

#[valtron_test]
async fn verifier_reads_ctx_clock() {
    // Case A: the certificate is valid, but the trust entry expires.
    let (ctx, clock) = ctx_at(NOW);
    let fixture = build_fixture(ctx, Duration::from_hours(1), NOW + 60).await;
    clock.advance_wall(Duration::from_secs(30));
    handshake(fixture.server_config(), fixture.client_config())
        .expect("inside the trust-store window");
    clock.advance_wall(Duration::from_secs(90));
    assert!(
        handshake(fixture.server_config(), fixture.client_config()).is_err(),
        "the verifier's now comes from the Ctx clock"
    );

    // Case B: the trust entry never expires, but the certificate does.
    let (ctx, clock) = ctx_at(NOW);
    let fixture = build_fixture(ctx, Duration::from_secs(60), NOW + 100_000).await;
    clock.advance_wall(Duration::from_secs(30));
    handshake(fixture.server_config(), fixture.client_config())
        .expect("inside the certificate window");
    clock.advance_wall(Duration::from_secs(90));
    assert!(
        handshake(fixture.server_config(), fixture.client_config()).is_err(),
        "the WebPKI chain check also reads the Ctx clock"
    );
}

#[valtron_test]
async fn unknown_client_refused() {
    let (ctx, _clock) = ctx_at(NOW);
    let fixture = build_fixture(ctx, Duration::from_hours(1), NOW + 3_600).await;

    // A certificate from the same CA, but never enrolled.
    let (key_pem, csr_pem) = generate_csr("sideloaded").expect("CSR");
    let issued = fixture
        .ca
        .sign_client_csr(&csr_pem, "sideloaded", Duration::from_hours(1))
        .expect("sideloaded certificate");
    let client = client_config_mutual(
        fixture.ca.ca_cert_pem(),
        &issued.cert_pem,
        &key_pem,
        &fixture.ctx,
    )
    .expect("client config");
    assert!(
        handshake(fixture.server_config(), client).is_err(),
        "a certificate that is not in the trust store is refused"
    );
}

#[test]
fn limits_are_configuration() {
    let defaults = MtlsConfig::new();
    assert_eq!(defaults.client_cert_lifetime, Duration::from_hours(30 * 24));
    assert_eq!(defaults.max_enrollments, 1);
    let server = ServerTlsConfig::default().with_lifetime(Duration::from_hours(90));
    assert_eq!(server.renew_before, Duration::from_hours(30));
}
