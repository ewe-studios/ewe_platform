//! The certificates that the authority issues.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use zeroize::Zeroizing;

/// A server certificate and its private key, ready for a TLS listener.
pub struct ServerIdentity {
    cert_der: CertificateDer<'static>,
    cert_pem: String,
    key_der: Zeroizing<Vec<u8>>,
    fingerprint_sha256: String,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerIdentity")
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .finish_non_exhaustive()
    }
}

impl ServerIdentity {
    pub(crate) fn new(
        cert_der: CertificateDer<'static>,
        cert_pem: String,
        key_der: Zeroizing<Vec<u8>>,
        fingerprint_sha256: String,
    ) -> Self {
        Self {
            cert_der,
            cert_pem,
            key_der,
            fingerprint_sha256,
        }
    }

    /// Lowercase hexadecimal SHA-256 of the certificate. A client pins this
    /// value before it has the CA.
    #[must_use]
    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    /// The certificate in Distinguished Encoding Rules (DER) form.
    #[must_use]
    pub fn cert_der(&self) -> &CertificateDer<'static> {
        &self.cert_der
    }

    /// The certificate in Privacy-Enhanced Mail (PEM) form.
    #[must_use]
    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    /// The private key in PKCS#8 DER form.
    #[must_use]
    pub fn key_der(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.to_vec()))
    }
}

/// A client certificate that the authority issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedCertificate {
    /// The certificate in Privacy-Enhanced Mail (PEM) form.
    pub cert_pem: String,
    /// Lowercase hexadecimal SHA-256 of the certificate. This is its identifier
    /// in the trust store.
    pub fingerprint_sha256: String,
    /// The end of the validity window (Unix seconds).
    pub not_after: u64,
}
