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

use foundation_netio::netcap::ssl::SSLAcceptor;

use crate::config::{ProxyError, SslConfig, SslProvider};

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
        SslProvider::LetsEncrypt { domains, email, staging } => {
            // Build an ACME client (DNS-01 challenge via Cloudflare).
            // The signing callback is provided by the operator (key is
            // stored externally — we don't bake key material into the proxy).
            // For now, return a descriptive error that tells the operator
            // what's needed. Full automation requires a JWS signer callback.
            // TODO: accept signer callback from ProxyConfig or environment.
            Err(ProxyError::Ssl(format!(
                "Let's Encrypt ACME configured for {:?} ({}), staging={}. \
                 Set ACME_SIGNER_KEY_PATH to your account private key (PEM) \
                 or ACME_SIGNER_CMD to a JWS-signing subprocess.",
                domains, email, staging,
            )))
        }
        SslProvider::Cloudflare { zone_id, email } => {
            // Cloudflare Origin CA certs are obtained via the Cloudflare API.
            // Requires `foundation_deployment_cloudflare::CloudflareClient`.
            // This path is used for internal/testbed deployments where
            // Cloudflare manages DNS and provides edge certificates.
            Err(ProxyError::Ssl(format!(
                "Cloudflare Origin CA configured for zone {zone_id} ({email}). \
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
