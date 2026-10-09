//! TLS certificate provisioning and SSL acceptor building.
//!
//! WHY: Decision 18 — the proxy must terminate TLS for testbed services,
//! zero-downtime deploys, and wildcard certs.  `foundation_http` already
//! supports TLS when a `ServerConfig.tls_acceptor` is provided; this module
//! builds that acceptor from the proxy's `SslConfig`.
//!
//! WHAT: [`CertManager`] trait (pluggable cert provisioning), [`StaticCertManager`]
//! (reads PEM cert+key from disk, stage 2), [`build_acceptor`] (SslConfig →
//! Arc<SSLAcceptor>).
//!
//! HOW: `RustlsAcceptor::from_pem` handles PEM parsing and rustls config.
//! Stage 2 implements only `StaticCertManager`; ACME and Cloudflare are deferred.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foundation_netio::http::NativeHttpClient;
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::shared::client::SystemDnsResolver;

use crate::acme::{AcmeClient, Dns01Setter, ACME_PRODUCTION};
use crate::config::{ProxyConfig, ProxyError, SslConfig, SslProvider};

/// A loaded cert + key pair.
#[derive(Debug, Clone)]
pub struct CertPair {
    /// PEM-encoded certificate chain.
    pub cert_chain: Vec<u8>,
    /// PEM-encoded private key.
    pub private_key: Vec<u8>,
}

/// Pluggable certificate provisioning interface.
pub trait CertManager: Send + Sync {
    /// Return the current cert pair, provisioning if needed.
    fn get_cert(&self) -> Result<CertPair, ProxyError>;
    /// Force renewal. For `StaticCertManager`, re-reads from disk.
    fn renew_cert(&self) -> Result<CertPair, ProxyError>;
}

// ── StaticCertManager ──

/// Cert manager that reads PEM files from disk.
pub struct StaticCertManager {
    cert_path: PathBuf,
    key_path: PathBuf,
    cached: Mutex<Option<CertPair>>,
}

impl StaticCertManager {
    pub fn new(cert: impl Into<PathBuf>, key: impl Into<PathBuf>) -> Self {
        Self { cert_path: cert.into(), key_path: key.into(), cached: Mutex::new(None) }
    }

    fn load_from_disk(&self) -> Result<CertPair, ProxyError> {
        let cert_chain = std::fs::read(&self.cert_path)
            .map_err(|e| ProxyError::Ssl(format!("read cert {}: {e}", self.cert_path.display())))?;
        let private_key = std::fs::read(&self.key_path)
            .map_err(|e| ProxyError::Ssl(format!("read key {}: {e}", self.key_path.display())))?;
        Ok(CertPair { cert_chain, private_key })
    }
}

impl CertManager for StaticCertManager {
    fn get_cert(&self) -> Result<CertPair, ProxyError> {
        let mut cache = self.cached.lock().unwrap();
        if let Some(ref pair) = *cache {
            return Ok(pair.clone());
        }
        let pair = self.load_from_disk()?;
        *cache = Some(pair.clone());
        Ok(pair)
    }

    fn renew_cert(&self) -> Result<CertPair, ProxyError> {
        let mut cache = self.cached.lock().unwrap();
        let pair = self.load_from_disk()?;
        *cache = Some(pair.clone());
        Ok(pair)
    }
}

// ── AcmeCertManager ──

/// Certificate manager that provisions a cert from an ACME CA (Decision 18, F15).
///
/// WHY: `StaticCertManager` reads a pre-issued cert from disk; this one obtains
/// one automatically over ACME (RFC 8555) — the Let's Encrypt path.
///
/// HOW: generates a fresh P-256 account key (for JWS auth) and a certificate
/// key + CSR, then drives the full [`AcmeClient`] flow (account → order →
/// DNS-01 challenge → finalize → download). The provisioned PEM chain plus the
/// generated private key become the [`CertPair`] the acceptor is built from. The
/// DNS-01 TXT records are published through a caller-supplied setter (typically
/// `foundation_deployment_cloudflare`).
pub struct AcmeCertManager {
    domains: Vec<String>,
    email: String,
    directory_url: String,
    client: Arc<NativeHttpClient<SystemDnsResolver>>,
    dns_setter: Dns01Setter,
    cached: Mutex<Option<CertPair>>,
}

impl AcmeCertManager {
    /// Build a manager for `domains`, authenticating to the CA at
    /// `directory_url` (`AcmeClient::PRODUCTION`/`STAGING`, or a private CA).
    pub fn new(
        domains: Vec<String>,
        email: impl Into<String>,
        directory_url: impl Into<String>,
        client: Arc<NativeHttpClient<SystemDnsResolver>>,
        dns_setter: Dns01Setter,
    ) -> Self {
        Self {
            domains,
            email: email.into(),
            directory_url: directory_url.into(),
            client,
            dns_setter,
            cached: Mutex::new(None),
        }
    }

    /// Run one full ACME provisioning round and return the resulting cert+key.
    fn provision(&self) -> Result<CertPair, ProxyError> {
        use base64::Engine;
        use p256::ecdsa::{signature::Signer, Signature, SigningKey};

        // Account key (P-256) → JWK + ES256 JWS signer.
        let account_key = SigningKey::random(&mut rand_core::OsRng);
        let jwk = ec_jwk(account_key.verifying_key());
        let signing_key = account_key.clone();
        let signer = move |input: &str| -> Result<String, String> {
            let sig: Signature = signing_key.sign(input.as_bytes());
            Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes()))
        };

        // Certificate key + CSR for the requested domains.
        let cert_key = rcgen::KeyPair::generate()
            .map_err(|e| ProxyError::Ssl(format!("generate cert key: {e}")))?;
        let params = rcgen::CertificateParams::new(self.domains.clone())
            .map_err(|e| ProxyError::Ssl(format!("build CSR params: {e}")))?;
        let csr = params
            .serialize_request(&cert_key)
            .map_err(|e| ProxyError::Ssl(format!("serialize CSR: {e}")))?;

        // Drive the ACME flow. Its I/O is the shared client's blocking send, so
        // the future resolves in a single poll.
        let dns_setter = Arc::clone(&self.dns_setter);
        let acme = AcmeClient::with_directory(Arc::clone(&self.client), self.directory_url.clone())
            .with_jwk(jwk)
            .with_signer(signer)
            .with_dns_setter(move |name, value| dns_setter(name, value));

        let (cert_pem, _raw) = futures_lite::future::block_on(acme.provision(
            &self.domains,
            &self.email,
            csr.der().as_ref(),
        ))
        .map_err(|e| ProxyError::Ssl(format!("ACME provisioning failed: {e}")))?;

        Ok(CertPair {
            cert_chain: cert_pem.into_bytes(),
            private_key: cert_key.serialize_pem().into_bytes(),
        })
    }
}

impl CertManager for AcmeCertManager {
    fn get_cert(&self) -> Result<CertPair, ProxyError> {
        let mut cache = self.cached.lock().unwrap();
        if let Some(pair) = &*cache {
            return Ok(pair.clone());
        }
        let pair = self.provision()?;
        *cache = Some(pair.clone());
        Ok(pair)
    }

    fn renew_cert(&self) -> Result<CertPair, ProxyError> {
        let pair = self.provision()?;
        *self.cached.lock().unwrap() = Some(pair.clone());
        Ok(pair)
    }
}

/// Build the RFC 7638 EC public JWK for a P-256 verifying key.
fn ec_jwk(vk: &p256::ecdsa::VerifyingKey) -> serde_json::Value {
    use base64::Engine;
    let point = vk.to_encoded_point(false); // uncompressed: 0x04 || x || y
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    serde_json::json!({
        "kty": "EC",
        "crv": "P-256",
        "x": b64(point.x().map(|x| x.as_slice()).unwrap_or_default()),
        "y": b64(point.y().map(|y| y.as_slice()).unwrap_or_default()),
    })
}

// ── Cert-manager selection + acceptor builder ──

/// Select the [`CertManager`] for the proxy's SSL provider (Decision 18).
///
/// `Static` reads PEM from disk; `LetsEncrypt` provisions over ACME (needs the
/// ACME runtime hooks on the config — a DNS-01 setter, optional directory URL).
pub fn build_cert_manager(
    config: &ProxyConfig,
    client: Arc<NativeHttpClient<SystemDnsResolver>>,
) -> Result<Arc<dyn CertManager>, ProxyError> {
    match &config.ssl.provider {
        SslProvider::Static { cert, key } => {
            Ok(Arc::new(StaticCertManager::new(cert.clone(), key.clone())))
        }
        SslProvider::LetsEncrypt { email } => {
            let acme = config.acme.as_ref().ok_or_else(|| {
                ProxyError::Ssl(
                    "Let's Encrypt provider requires ACME runtime hooks — call \
                     ProxyConfig::acme(dns01_setter, directory_url)"
                        .into(),
                )
            })?;
            let directory = acme
                .directory_url
                .clone()
                .unwrap_or_else(|| ACME_PRODUCTION.to_string());
            Ok(Arc::new(AcmeCertManager::new(
                acme_domains(config),
                email.clone(),
                directory,
                client,
                Arc::clone(&acme.dns01_setter),
            )))
        }
        SslProvider::Cloudflare { zone_id } => Err(ProxyError::Ssl(format!(
            "Cloudflare Origin CA (zone {zone_id}) not yet supported; provision \
             an Origin CA cert and configure it via the Static provider."
        ))),
        SslProvider::None => Err(ProxyError::Ssl(
            "TLS requested but no SSL provider configured".into(),
        )),
    }
}

/// Build an `Arc<SSLAcceptor>` from a cert manager's current cert pair.
pub fn acceptor_from_cert_manager(mgr: &dyn CertManager) -> Result<Arc<SSLAcceptor>, ProxyError> {
    acceptor_from_pair(&mgr.get_cert()?)
}

/// Build an `Arc<SSLAcceptor>` from an already-obtained cert pair. Useful when
/// the same cert also feeds the HTTP/3 (QUIC) server.
pub fn acceptor_from_pair(pair: &CertPair) -> Result<Arc<SSLAcceptor>, ProxyError> {
    let acceptor = SSLAcceptor::from_pem(pair.cert_chain.clone(), pair.private_key.clone().into())
        .map_err(|e| ProxyError::Ssl(format!("build TLS acceptor: {e}")))?;
    Ok(Arc::new(acceptor))
}

/// The set of domains a provisioned cert should cover: the proxy domain plus
/// every distinct service host.
fn acme_domains(config: &ProxyConfig) -> Vec<String> {
    let mut domains = vec![config.domain.clone()];
    for svc in &config.services {
        if !domains.contains(&svc.host) {
            domains.push(svc.host.clone());
        }
    }
    domains
}

// ── Acceptor builder ──

/// Build an `Arc<SSLAcceptor>` from the proxy's `SslConfig`.
pub fn build_acceptor(config: &SslConfig) -> Result<Arc<SSLAcceptor>, ProxyError> {
    match &config.provider {
        SslProvider::Static { cert, key } => {
            let manager = StaticCertManager::new(cert.clone(), key.clone());
            let pair = manager.get_cert()?;
            let acceptor = SSLAcceptor::from_pem(pair.cert_chain, pair.private_key.into())
                .map_err(|e| ProxyError::Ssl(format!("build TLS acceptor: {e}")))?;
            Ok(Arc::new(acceptor))
        }
        SslProvider::LetsEncrypt { email } => {
            Err(ProxyError::Ssl(format!(
                "Let's Encrypt ACME configured for {email}. \
                 Set ACME_SIGNER_KEY_PATH to your account private key (PEM) \
                 or ACME_SIGNER_CMD to a JWS-signing subprocess."
            )))
        }
        SslProvider::Cloudflare { zone_id } => {
            Err(ProxyError::Ssl(format!(
                "Cloudflare Origin CA configured for zone {zone_id}. \
                 Use the cloudflare API to provision an Origin CA certificate \
                 and configure it via the Static provider."
            )))
        }
        SslProvider::None => Err(ProxyError::Ssl(
            "TLS requested but no SSL provider configured".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SslConfig;

    #[test]
    fn test_build_acceptor_static_from_fixtures() {
        let cert = include_bytes!("../tests/fixtures/cert.pem");
        let key = include_bytes!("../tests/fixtures/key.pem");
        let dir = std::env::temp_dir().join("proxy_tls_test");
        std::fs::create_dir_all(&dir).ok();
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        std::fs::write(&cert_path, cert).unwrap();
        std::fs::write(&key_path, key).unwrap();

        let config = SslConfig::static_cert(
            &cert_path.display().to_string(),
            &key_path.display().to_string(),
        );
        assert!(build_acceptor(&config).is_ok(), "valid PEM should build acceptor");
    }

    #[test]
    fn test_build_acceptor_none_errs() {
        assert!(build_acceptor(&SslConfig::default()).is_err());
    }
}
