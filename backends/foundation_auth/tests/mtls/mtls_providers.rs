//! The native mutual-TLS (mTLS) provider, over the trust store and the clock.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::client_auth::MtlsAuth;
use foundation_auth::pki::sha256_hex;
use foundation_auth::server_credential::{AuthError, AuthMethod, ClientAuth, Principal};
use foundation_auth::trust::{
    MemoryTrustStore, TrustConfig, TrustStore, TrustedClient, TrustedClients,
};
use foundation_context::Ctx;
use foundation_core::valtron::{valtron_test, SimulatedClock};
use foundation_netio::shared::context::{ConnectionContext, TlsInfo};
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

fn ctx_at(secs: u64) -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(secs));
    let as_clock: Arc<dyn foundation_context::Clock> = clock.clone();
    (Ctx::fixed(as_clock), clock)
}

fn connection_with(leaf: &[u8]) -> ConnectionContext {
    ConnectionContext {
        tls: Some(TlsInfo {
            peer_certificates: vec![leaf.to_vec()],
            server_name: None,
        }),
        ..Default::default()
    }
}

async fn enrolled_client(trusted: &TrustedClients, leaf: &[u8], expires_at: u64) {
    trusted
        .record_enrollment(
            TrustedClient {
                fingerprint_sha256: sha256_hex(leaf),
                client_id: None,
                client_name: "laptop".to_string(),
                enrolled_at: 1_700_000_000,
                expires_at,
                last_seen_at: None,
                revoked_at: None,
            },
            "cert",
        )
        .await
        .expect("enroll");
}

#[valtron_test]
async fn chain_alone_refused() {
    let (ctx, _clock) = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    let auth = MtlsAuth::new(trusted, &ctx);

    let headers = SimpleHeaders::new();
    let extensions = Extensions::new();
    let leaf = b"a-certificate-that-chains-to-the-ca";
    let connection = connection_with(leaf);
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    let error = auth.authenticate(call).await.expect_err("not enrolled");
    match error {
        AuthError::Invalid(reason) => {
            assert!(reason.contains("trust store"), "{reason}");
        }
        other => panic!("expected Invalid, got {other:?}"),
    }

    // A cleartext connection has no certificate of this kind.
    let cleartext = ConnectionContext::default();
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &cleartext,
        extensions: &extensions,
    };
    assert!(auth
        .authenticate(call)
        .await
        .expect("no credential")
        .is_none());
}

#[valtron_test]
async fn revocation_stops_next_call() {
    let (ctx, _clock) = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    let leaf = b"an-enrolled-certificate";
    enrolled_client(&trusted, leaf, u64::MAX).await;
    let auth = MtlsAuth::new(Arc::clone(&trusted), &ctx);

    let headers = SimpleHeaders::new();
    let extensions = Extensions::new();
    let connection = connection_with(leaf);
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    let info = auth
        .authenticate(call)
        .await
        .expect("first call")
        .expect("a known client");
    let principal = info.artifacts.get::<Principal>().expect("principal");
    assert_eq!(principal.subject, "laptop");
    assert_eq!(
        principal.method,
        AuthMethod::ClientCertificate {
            fingerprint_sha256: sha256_hex(leaf)
        }
    );

    assert!(trusted
        .revoke_client(&sha256_hex(leaf))
        .await
        .expect("revoke"));
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    assert_eq!(
        auth.authenticate(call).await.expect_err("revoked"),
        AuthError::Revoked
    );
}

#[valtron_test]
async fn expired_at_simulated_clock() {
    let (ctx, clock) = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    let leaf = b"a-certificate-that-expires";
    enrolled_client(&trusted, leaf, 1_700_000_060).await;
    let auth = MtlsAuth::new(trusted, &ctx);

    let headers = SimpleHeaders::new();
    let extensions = Extensions::new();
    let connection = connection_with(leaf);

    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    assert!(auth
        .authenticate(call)
        .await
        .expect("before expiry")
        .is_some());

    clock.advance_wall(Duration::from_secs(120));
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    assert_eq!(
        auth.authenticate(call).await.expect_err("expired"),
        AuthError::Expired
    );
}

// ---------------------------------------------------------------------------
// The Cloudflare provider (spec 87 T5).

use foundation_auth::client_auth::{CfMtlsAuth, CfTlsClientAuth};

fn cf_facts(fingerprint: &str) -> CfTlsClientAuth {
    CfTlsClientAuth {
        cert_presented: true,
        cert_verified: "SUCCESS".to_string(),
        cert_fingerprint_sha256: fingerprint.to_string(),
        cert_subject_dn: "CN=laptop".to_string(),
        cert_not_after: "2030-01-01T00:00:00Z".to_string(),
        cert_serial: "01".to_string(),
    }
}

async fn cf_setup(leaf: &[u8]) -> Arc<CfMtlsAuth> {
    let (ctx, _clock) = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    enrolled_client(&trusted, leaf, u64::MAX).await;
    Arc::new(CfMtlsAuth::new(trusted, &ctx))
}

#[valtron_test]
async fn cf_mtls_success() {
    let leaf = b"a-worker-client-certificate";
    let auth = cf_setup(leaf).await;
    let mut extensions = Extensions::new();
    extensions.insert(cf_facts(&sha256_hex(leaf)));
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    let info = auth
        .authenticate(call)
        .await
        .expect("Cloudflare facts")
        .expect("a known client");
    let principal = info.artifacts.get::<Principal>().expect("principal");
    assert_eq!(principal.subject, "laptop");
    assert_eq!(
        principal.method,
        AuthMethod::CloudflareMtls {
            fingerprint_sha256: sha256_hex(leaf)
        }
    );
}

#[valtron_test]
async fn cf_mtls_not_presented() {
    let leaf = b"a-worker-client-certificate";
    let auth = cf_setup(leaf).await;
    let mut facts = cf_facts(&sha256_hex(leaf));
    facts.cert_presented = false;
    let mut extensions = Extensions::new();
    extensions.insert(facts);
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    assert!(auth
        .authenticate(call)
        .await
        .expect("no certificate")
        .is_none());
}

#[valtron_test]
async fn cf_mtls_failed() {
    let leaf = b"a-worker-client-certificate";
    let auth = cf_setup(leaf).await;
    let mut facts = cf_facts(&sha256_hex(leaf));
    facts.cert_verified = "FAILED:expired".to_string();
    let mut extensions = Extensions::new();
    extensions.insert(facts);
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let call = foundation_auth::server_credential::CallCredentials {
        procedure: "/ewe.test.v1.Service/Call",
        headers: &headers,
        connection: &connection,
        extensions: &extensions,
    };
    assert!(matches!(
        auth.authenticate(call).await.expect_err("not verified"),
        AuthError::Invalid(reason) if reason.contains("FAILED")
    ));
}
