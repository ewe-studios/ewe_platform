//! The certificate authority: issuance, CSR validation, reload and
//! deterministic signing.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::pki::{
    generate_csr, CaConfig, CaKey, CaStore, CertificateAuthority, CertificateRequest, KeyAlgorithm,
};
use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;

/// A simulated clock pinned to `secs` after the UNIX epoch.
fn ctx_at(secs: u64) -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(secs));
    let as_clock: Arc<dyn foundation_context::Clock> = clock.clone();
    (Ctx::fixed(as_clock), clock)
}

const NOW: u64 = 1_700_000_000;

#[test]
fn ca_issues_certificate() {
    let (ctx, _clock) = ctx_at(NOW);
    let (ca, _pem) = CertificateAuthority::generate(&CaConfig::new("test CA"), &ctx).expect("CA");
    let (_key_pem, csr_pem) = generate_csr("laptop").expect("CSR");

    let lifetime = Duration::from_secs(30 * 24 * 60 * 60);
    let issued = ca
        .sign_client_csr(&csr_pem, "laptop", lifetime)
        .expect("sign");

    let der = pem_to_der(&issued.cert_pem);
    let (_, cert) = x509_parser::parse_x509_certificate(&der).expect("parse issued");
    let ca_der = pem_to_der(ca.ca_cert_pem());
    let (_, ca_cert) = x509_parser::parse_x509_certificate(&ca_der).expect("parse CA");

    cert.verify_signature(Some(ca_cert.public_key()))
        .expect("the certificate chains to the CA");
    assert_eq!(
        cert.subject()
            .iter_common_name()
            .next()
            .and_then(|n| n.as_str().ok()),
        Some("laptop")
    );
    assert_eq!(
        u64::try_from(cert.validity().not_before.timestamp()).expect("before"),
        NOW - 5 * 60,
        "the backdate is 5 minutes"
    );
    assert_eq!(
        u64::try_from(cert.validity().not_after.timestamp()).expect("after"),
        NOW + lifetime.as_secs()
    );
    assert_eq!(issued.not_after, NOW + lifetime.as_secs());
    assert_eq!(issued.fingerprint_sha256.len(), 64);
}

#[test]
fn csr_wrong_signature_refused() {
    let (ctx, _clock) = ctx_at(NOW);
    let (ca, _pem) = CertificateAuthority::generate(&CaConfig::new("test CA"), &ctx).expect("CA");
    let (_key_pem, csr_pem) = generate_csr("laptop").expect("CSR");
    let tampered = tamper_csr_pem(&csr_pem);

    let err = ca
        .sign_client_csr(&tampered, "laptop", Duration::from_secs(60))
        .expect_err("a bad CSR signature is refused");
    assert!(
        matches!(err, foundation_auth::pki::PkiError::BadCsr(_)),
        "{err:?}"
    );
}

#[test]
fn ca_round_trip() {
    let (ctx, _clock) = ctx_at(NOW);
    let dir = tempfile::tempdir().expect("tempdir");
    let config = CaConfig::new("round-trip CA").with_key_algorithm(KeyAlgorithm::Ed25519);

    let (first, first_pem) = CaStore::file(dir.path())
        .load_or_generate(&config, &ctx)
        .expect("generate");
    assert!(first_pem.is_some(), "the store reports the new certificate");

    let (second, second_pem) = CaStore::file(dir.path())
        .load_or_generate(&config, &ctx)
        .expect("reload");
    assert!(second_pem.is_none(), "a reload makes no new certificate");
    assert_eq!(first.ca_cert_pem(), second.ca_cert_pem());

    let key = CaKey::generate(KeyAlgorithm::Ed25519, &ctx).expect("client key");
    let csr = key.csr_pem("laptop").expect("CSR");
    let a = first
        .sign_client_csr(&csr, "laptop", Duration::from_secs(3600))
        .expect("sign with the first CA");
    let b = second
        .sign_client_csr(&csr, "laptop", Duration::from_secs(3600))
        .expect("sign with the reloaded CA");
    assert_eq!(a, b, "the reloaded CA signs the same bytes");

    // The memory store also round-trips.
    let store = CaStore::memory();
    let (memory_first, generated) = store.load_or_generate(&config, &ctx).expect("memory CA");
    assert!(generated.is_some());
    let (memory_second, _) = store
        .load_or_generate(&config, &ctx)
        .expect("memory reload");
    assert_eq!(memory_first.ca_cert_pem(), memory_second.ca_cert_pem());
}

#[test]
fn ed25519_deterministic() {
    let (ctx_a, _clock_a) = ctx_at(NOW);
    let (ctx_b, _clock_b) = ctx_at(NOW);
    let config = CaConfig::new("deterministic CA").with_key_algorithm(KeyAlgorithm::Ed25519);

    let (ca_a, _) = CaStore::memory()
        .load_or_generate(&config, &ctx_a)
        .expect("CA a");
    let (ca_b, _) = CaStore::memory()
        .load_or_generate(&config, &ctx_b)
        .expect("CA b");
    // The seeded RNGs are independent instances with seed 0; the same sequence
    // gives the same key.
    assert_eq!(ca_a.ca_cert_pem(), ca_b.ca_cert_pem());

    let client_key = CaKey::ed25519_from_seed(&[7_u8; 32]).expect("client key");
    let csr = client_key.csr_pem("laptop").expect("CSR");
    let a = ca_a
        .sign_client_csr(&csr, "laptop", Duration::from_secs(3600))
        .expect("sign a");
    let b = ca_b
        .sign_client_csr(&csr, "laptop", Duration::from_secs(3600))
        .expect("sign b");
    assert_eq!(a, b, "two runs make the same certificate bytes");
    assert_eq!(a.fingerprint_sha256, b.fingerprint_sha256);
}

#[test]
fn certificate_within_ca_lifetime() {
    let (ctx, _clock) = ctx_at(NOW);
    let (ca, _pem) = CertificateAuthority::generate(&CaConfig::default(), &ctx).expect("CA");
    let key = CaKey::generate(KeyAlgorithm::EcdsaP256, &ctx).expect("key");
    let identity = ca
        .issue_server_identity(
            &key,
            &["localhost".to_string()],
            Duration::from_secs(90 * 24 * 60 * 60),
        )
        .expect("server identity");

    let ca_der = pem_to_der(ca.ca_cert_pem());
    let (_, ca_cert) = x509_parser::parse_x509_certificate(&ca_der).expect("parse CA");
    let (_, server) =
        x509_parser::parse_x509_certificate(identity.cert_der().as_ref()).expect("parse server");
    assert!(
        server.validity().not_after <= ca_cert.validity().not_after,
        "a leaf certificate ends before its CA"
    );
}

#[test]
fn keys_are_redacted() {
    let (ctx, _clock) = ctx_at(NOW);
    let key = CaKey::generate(KeyAlgorithm::Ed25519, &ctx).expect("key");
    let body = key
        .to_pem()
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();

    let debug = format!("{key:?}");
    let display = format!("{key}");
    assert!(!debug.contains(&body), "Debug shows no key byte");
    assert!(!display.contains(&body), "Display shows no key byte");
    assert!(debug.contains("Ed25519"));
}

#[test]
fn issue_client_uses_the_request() {
    let (ctx, _clock) = ctx_at(NOW);
    let (ca, _pem) = CertificateAuthority::generate(
        &CaConfig::default().with_key_algorithm(KeyAlgorithm::Ed25519),
        &ctx,
    )
    .expect("CA");
    let key = CaKey::generate(KeyAlgorithm::Ed25519, &ctx).expect("key");
    let request = CertificateRequest::new(key.csr_pem("phone").expect("CSR"), "phone")
        .with_lifetime(Duration::from_secs(600));
    let issued = ca.issue_client(&request).expect("issue");
    assert_eq!(issued.not_after, NOW + 600);

    let renewed = ca
        .renew(&request.csr_pem, "phone", Duration::from_secs(600))
        .expect("renew");
    assert_eq!(renewed, issued);
}

/// Base64 decode a PEM body.
fn pem_body(pem: &str) -> Vec<u8> {
    use base64::Engine as _;

    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body)
        .expect("PEM body")
}

/// Build a PEM with a standard body.
fn pem_with(label: &str, der: &[u8]) -> String {
    use base64::Engine as _;

    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in body.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    pem_body(pem)
}

/// Flip one byte inside the signature of a CSR, so its own signature no longer
/// matches its public key.
fn tamper_csr_pem(csr_pem: &str) -> String {
    let mut der = pem_body(csr_pem);
    let last = der.len() - 1;
    der[last] ^= 0xff;
    pem_with("CERTIFICATE REQUEST", &der)
}
