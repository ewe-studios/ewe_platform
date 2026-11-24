//! T16 — enrollment and refresh over a real in-memory trust store.
//!
//! A client key enrolls, receives an access secret and a server proof, and
//! refreshes with its private key. A CSR enrolls through the issuer seam.

use std::sync::Arc;

use ed25519_compact::{KeyPair, Seed};
use foundation_agent_rpc::core::config::EnrollConfig;
use foundation_agent_rpc::core::enroll::{ClientCertIssuer, EnrollmentService, IssuedClientCert};
use foundation_agent_rpc::proto::v1 as pb;
use foundation_agent_rpc::AgentRpcError;
use foundation_auth::secret::{
    DecodeLimits, SecretDomain, SecretIssuer, SecretPurpose, SecretRequest, SecretVerifier,
    SignedSecret, SigningKey, VerifyPolicy,
};
use foundation_auth::trust::{MemoryTrustStore, TrustConfig, TrustStore, TrustedClients};
use foundation_compact::time::unix_millis;
use foundation_compact::Duration;
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;

const URL: &str = "https://203.0.113.7:7443";
const SECRET_DOMAIN: SecretDomain = SecretDomain::AGENT_RPC;

/// A test context with the real effect bundle installed.
fn test_ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

fn test_key() -> SigningKey {
    SigningKey::from_seed([9_u8; 32], None)
}

async fn service() -> EnrollmentService {
    let ctx = test_ctx();
    let key = test_key();
    let mut policy = VerifyPolicy::enroll();
    policy.audience = Some(URL.to_string());
    let verifier = SecretVerifier::new(
        SECRET_DOMAIN,
        vec![key.verifying_key().clone()],
        ctx.clone(),
        policy,
    );
    let signer = SecretIssuer::new(SECRET_DOMAIN, key, ctx.clone(), Duration::from_hours(24));
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trust = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    EnrollmentService::new(verifier, signer, None, trust, ctx, EnrollConfig::default())
        .expect("service")
}

fn enroll_token() -> String {
    let ctx = test_ctx();
    let issuer = SecretIssuer::new(
        SECRET_DOMAIN,
        test_key(),
        ctx.clone(),
        Duration::from_hours(24),
    );
    let mut request = SecretRequest::enroll(Duration::from_secs(600));
    request.audience = Some(URL.to_string());
    issuer.issue(&request).expect("issue").encode().to_string()
}

fn client_keypair() -> KeyPair {
    KeyPair::from_seed(Seed::new([7_u8; 32]))
}

fn enroll_request(token: String, key: &KeyPair) -> pb::EnrollRequest {
    pb::EnrollRequest {
        token,
        client_name: "browser".into(),
        credential: Some(pb::__buffa::oneof::enroll_request::Credential::ClientKey(
            Box::new(pb::ClientKey {
                alg: "ed25519".into(),
                public_key: key.pk[..].to_vec(),
                ..Default::default()
            }),
        )),
        nonce: vec![9_u8; 32],
        ..Default::default()
    }
}

fn refresh_message(client_id: &str, unix_ms: u64, nonce: &[u8]) -> Vec<u8> {
    let mut message = b"ewe.agent_rpc.refresh.v1\0".to_vec();
    message.extend_from_slice(client_id.as_bytes());
    message.extend_from_slice(&unix_ms.to_be_bytes());
    message.extend_from_slice(nonce);
    message
}

#[valtron_test]
async fn a_client_key_enrolls_and_refreshes() {
    let service = service().await;
    let key = client_keypair();
    let response = service
        .enroll_client(&enroll_request(enroll_token(), &key))
        .await
        .expect("enroll");
    assert_ne!(response.client_id, "");
    assert_ne!(response.server_proof, Vec::<u8>::new());
    let access = SignedSecret::decode(&response.access_secret, &DecodeLimits::default())
        .expect("access secret");
    assert_eq!(access.purpose(), SecretPurpose::Access);
    assert_eq!(access.audience(), Some(URL));
    assert_eq!(access.subject(), Some(response.client_id.as_str()));

    let ctx = test_ctx();
    let unix_ms = u64::try_from(unix_millis(ctx.clock().wall_now())).expect("time");
    let nonce = vec![3_u8; 32];
    let message = refresh_message(&response.client_id, unix_ms, &nonce);
    let signature = key.sk.sign(&message, None);
    let refresh = pb::RefreshAccessRequest {
        client_id: response.client_id.clone(),
        unix_ms,
        nonce: nonce.clone(),
        signature: signature.to_vec(),
        ..Default::default()
    };
    let refreshed = service.refresh_access(&refresh).await.expect("refresh");
    let new_access = SignedSecret::decode(&refreshed.access_secret, &DecodeLimits::default())
        .expect("new access secret");
    assert_eq!(new_access.subject(), Some(response.client_id.as_str()));

    let replay = service.refresh_access(&refresh).await.expect_err("replay");
    assert!(matches!(replay.current_context(), AgentRpcError::Auth(_)));
}

#[valtron_test]
async fn a_bad_refresh_signature_is_refused() {
    let service = service().await;
    let key = client_keypair();
    let response = service
        .enroll_client(&enroll_request(enroll_token(), &key))
        .await
        .expect("enroll");
    let ctx = test_ctx();
    let unix_ms = u64::try_from(unix_millis(ctx.clock().wall_now())).expect("time");
    let request = pb::RefreshAccessRequest {
        client_id: response.client_id,
        unix_ms,
        nonce: vec![4_u8; 32],
        signature: vec![0_u8; 64],
        ..Default::default()
    };
    let error = service
        .refresh_access(&request)
        .await
        .expect_err("bad signature");
    assert!(matches!(error.current_context(), AgentRpcError::Auth(_)));
}

#[valtron_test]
async fn an_old_refresh_time_is_refused() {
    let service = service().await;
    let key = client_keypair();
    let response = service
        .enroll_client(&enroll_request(enroll_token(), &key))
        .await
        .expect("enroll");
    let unix_ms = 1_000_000_u64;
    let nonce = vec![5_u8; 32];
    let message = refresh_message(&response.client_id, unix_ms, &nonce);
    let signature = key.sk.sign(&message, None);
    let request = pb::RefreshAccessRequest {
        client_id: response.client_id,
        unix_ms,
        nonce,
        signature: signature.to_vec(),
        ..Default::default()
    };
    let error = service.refresh_access(&request).await.expect_err("stale");
    assert!(matches!(error.current_context(), AgentRpcError::Auth(_)));
}

#[valtron_test]
async fn a_bad_nonce_is_invalid_argument() {
    let service = service().await;
    let key = client_keypair();
    let mut request = enroll_request(enroll_token(), &key);
    request.nonce = vec![1_u8; 4];
    let error = service
        .enroll_client(&request)
        .await
        .expect_err("bad nonce");
    assert!(matches!(
        error.current_context(),
        AgentRpcError::InvalidArgument(_)
    ));
}

#[valtron_test]
async fn an_unknown_client_key_algorithm_is_invalid_argument() {
    let service = service().await;
    let key = client_keypair();
    let mut request = enroll_request(enroll_token(), &key);
    if let Some(pb::__buffa::oneof::enroll_request::Credential::ClientKey(client)) =
        &mut request.credential
    {
        client.alg = "mystery".into();
    }
    let error = service.enroll_client(&request).await.expect_err("bad alg");
    assert!(matches!(
        error.current_context(),
        AgentRpcError::InvalidArgument(_)
    ));
}

struct TestIssuer {
    ca: String,
}

impl TestIssuer {
    fn new() -> Self {
        Self {
            ca: "ca".to_string(),
        }
    }
}

impl ClientCertIssuer for TestIssuer {
    fn sign_client_csr(
        &self,
        _csr_pem: &str,
        _client_name: &str,
        _lifetime: Duration,
    ) -> foundation_agent_rpc::AgentRpcResult<IssuedClientCert> {
        Ok(IssuedClientCert {
            cert_pem: "cert".into(),
            fingerprint_sha256: "ab".repeat(32),
            not_after: 2_000_000_000,
        })
    }

    fn ca_cert_pem(&self) -> &str {
        &self.ca
    }
}

async fn service_with_issuer() -> EnrollmentService {
    let ctx = test_ctx();
    let key = test_key();
    let mut policy = VerifyPolicy::enroll();
    policy.audience = Some(URL.to_string());
    let verifier = SecretVerifier::new(
        SECRET_DOMAIN,
        vec![key.verifying_key().clone()],
        ctx.clone(),
        policy,
    );
    let signer = SecretIssuer::new(SECRET_DOMAIN, key, ctx.clone(), Duration::from_hours(24));
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trust = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("trust store");
    EnrollmentService::new(
        verifier,
        signer,
        Some(Arc::new(TestIssuer::new())),
        trust,
        ctx,
        EnrollConfig::default(),
    )
    .expect("service")
}

#[valtron_test]
async fn a_csr_enrolls_through_the_issuer() {
    let service = service_with_issuer().await;
    let request = pb::EnrollRequest {
        token: enroll_token(),
        client_name: "native".into(),
        credential: Some(pb::__buffa::oneof::enroll_request::Credential::CsrPem(
            "csr".into(),
        )),
        nonce: vec![2_u8; 32],
        ..Default::default()
    };
    let response = service.enroll_client(&request).await.expect("enroll");
    assert_eq!(response.client_cert_pem, "cert");
    assert_eq!(response.ca_cert_pem, "ca");
    assert_ne!(response.server_proof, Vec::<u8>::new());
    assert_eq!(response.access_secret, "");
}

#[valtron_test]
async fn a_csr_without_an_issuer_is_unimplemented() {
    let service = service().await;
    let request = pb::EnrollRequest {
        token: enroll_token(),
        client_name: "native".into(),
        credential: Some(pb::__buffa::oneof::enroll_request::Credential::CsrPem(
            "csr".into(),
        )),
        nonce: vec![2_u8; 32],
        ..Default::default()
    };
    let error = service.enroll_client(&request).await.expect_err("no CA");
    assert!(matches!(
        error.current_context(),
        AgentRpcError::Unsupported(_)
    ));
}
