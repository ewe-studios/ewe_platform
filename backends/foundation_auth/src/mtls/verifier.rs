//! The client-certificate verifier.

use std::sync::Arc;

use foundation_netio::shared::context::{ConnectionContext, TlsInfo};
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier as RustlsClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

use crate::pki::sha256_hex;
use crate::trust::TrustIndex;

/// The certificate of one peer connection, and its fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCertificate {
    /// Lowercase hexadecimal SHA-256 of the certificate in Distinguished
    /// Encoding Rules (DER) form.
    pub fingerprint_sha256: String,
}

/// The peer certificate facts of a connection.
///
/// A native listener fills [`ConnectionContext::tls`] after the handshake. The
/// mutual-TLS (mTLS) provider reads the leaf certificate from there.
#[must_use]
pub fn peer_identity(connection: &ConnectionContext) -> Option<PeerCertificate> {
    let leaf = connection
        .tls
        .as_ref()
        .and_then(|tls| tls.peer_certificates.first())?;
    Some(PeerCertificate {
        fingerprint_sha256: sha256_hex(leaf),
    })
}

/// The peer certificate facts of a `rustls` server connection.
///
/// A host builds this after the handshake and puts it in
/// [`ConnectionContext::tls`].
#[must_use]
pub fn tls_info_of(connection: &rustls::ServerConnection) -> TlsInfo {
    TlsInfo {
        peer_certificates: connection
            .peer_certificates()
            .map_or_default(|certs| certs.iter().map(|cert| cert.as_ref().to_vec()).collect()),
        server_name: connection.server_name().map(str::to_string),
    }
}

/// A `WebPKI` chain check to the certificate authority (CA), then the trusted-
/// client index.
///
/// The check is pure: no Input/Output (I/O), no task and no write. `rustls`
/// calls it in the middle of the handshake, so it must not wait. The last-seen
/// write belongs to the mTLS provider, on the call path.
#[derive(Debug)]
pub struct ClientCertVerifier {
    chain: Arc<dyn RustlsClientCertVerifier>,
    index: TrustIndex,
}

impl ClientCertVerifier {
    /// A verifier over `chain` and `index`.
    #[must_use]
    pub fn new(chain: Arc<dyn RustlsClientCertVerifier>, index: TrustIndex) -> Self {
        Self { chain, index }
    }
}

/// The earlier name of [`ClientCertVerifier`].
pub type TrustedClientVerifier = ClientCertVerifier;

impl RustlsClientCertVerifier for ClientCertVerifier {
    fn offer_client_auth(&self) -> bool {
        self.chain.offer_client_auth()
    }

    fn client_auth_mandatory(&self) -> bool {
        self.chain.client_auth_mandatory()
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.chain.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        self.chain
            .verify_client_cert(end_entity, intermediates, now)?;
        let fingerprint = sha256_hex(end_entity.as_ref());
        match self.index.get(&fingerprint) {
            Some(client) if client.is_active_at(now.as_secs()) => {
                Ok(ClientCertVerified::assertion())
            }
            Some(client) if client.revoked_at.is_some() => {
                tracing::warn!(client = %client.client_name, "mTLS refused: client revoked");
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::Revoked,
                ))
            }
            Some(client) => {
                tracing::warn!(client = %client.client_name, "mTLS refused: client expired");
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::Expired,
                ))
            }
            None => {
                tracing::warn!(%fingerprint, "mTLS refused: certificate not in the trust store");
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ))
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.chain.supported_verify_schemes()
    }
}
