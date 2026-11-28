//! Key material for the certificate authority (CA) and its certificates.

// The no_std poison guard needs a closure; its function item does not unify.
#![allow(clippy::redundant_closure, clippy::redundant_closure_for_method_calls)]
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use zeroize::Zeroizing;

use super::error::PkiError;

/// The key algorithm of the CA and of the certificates it makes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyAlgorithm {
    /// Elliptic Curve Digital Signature Algorithm (ECDSA) over P-256. This is
    /// the default: every TLS client accepts it.
    #[default]
    EcdsaP256,
    /// Ed25519. The signature is deterministic (RFC 8032), so a deterministic
    /// simulation test (DST) run produces the same bytes from the same seed.
    /// Browsers do not accept Ed25519 server certificates, so use it only for a
    /// test and a DST run.
    Ed25519,
}

impl KeyAlgorithm {
    /// The `rcgen` signature algorithm for this key algorithm.
    #[must_use]
    pub fn signature_algorithm(self) -> &'static rcgen::SignatureAlgorithm {
        match self {
            Self::EcdsaP256 => &rcgen::PKCS_ECDSA_P256_SHA256,
            Self::Ed25519 => &rcgen::PKCS_ED25519,
        }
    }
}

/// A private key that the CA, a server or a client uses.
///
/// The key bytes never reach `Debug`. The type wraps `rcgen`'s key pair, so it
/// can sign a certificate and it can be written as Privacy-Enhanced Mail (PEM).
pub struct CaKey {
    pair: rcgen::KeyPair,
}

impl std::fmt::Debug for CaKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaKey")
            .field("algorithm", &self.algorithm())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for CaKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CaKey(<redacted>)")
    }
}

impl CaKey {
    /// Generate a key for `algorithm`.
    ///
    /// An Ed25519 key comes from the `Ctx` random number generator (RNG), so a
    /// deterministic simulation test (DST) with a seeded `Ctx` makes the same
    /// key on every run. A P-256 key comes from the operating system entropy
    /// pool, because its signatures use a random nonce and are not
    /// deterministic anyway.
    ///
    /// # Errors
    /// [`PkiError::Key`] when the operating system entropy or the key format
    /// fails.
    pub fn generate(algorithm: KeyAlgorithm, ctx: &Ctx) -> Result<Self, PkiError> {
        match algorithm {
            KeyAlgorithm::EcdsaP256 => {
                let pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
                    .map_err(|e| PkiError::Key(e.to_string()))?;
                Ok(Self { pair })
            }
            KeyAlgorithm::Ed25519 => {
                let mut seed = [0_u8; 32];
                ctx.rng()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .fill_bytes(&mut seed);
                Self::ed25519_from_seed(&seed)
            }
        }
    }

    /// An Ed25519 key from a 32-byte seed.
    ///
    /// The method builds the PKCS#8 version 1 structure by hand. The structure
    /// is 48 bytes and the first 16 bytes are fixed.
    ///
    /// # Errors
    /// [`PkiError::Key`] when the seed is not a valid Ed25519 seed.
    pub fn ed25519_from_seed(seed: &[u8; 32]) -> Result<Self, PkiError> {
        const PKCS8_ED25519_V1_PREFIX: [u8; 16] = [
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
        ];
        let mut der = Vec::with_capacity(PKCS8_ED25519_V1_PREFIX.len() + seed.len());
        der.extend_from_slice(&PKCS8_ED25519_V1_PREFIX);
        der.extend_from_slice(seed);
        let pkcs8 = rustls::pki_types::PrivatePkcs8KeyDer::from(der);
        let pair = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8, &rcgen::PKCS_ED25519)
            .map_err(|e| PkiError::Key(e.to_string()))?;
        Ok(Self { pair })
    }

    /// Parse a PKCS#8 private key in PEM form.
    ///
    /// # Errors
    /// [`PkiError::Key`] when the text is not a supported private key.
    pub fn from_pem(pem: &str) -> Result<Self, PkiError> {
        let pair = rcgen::KeyPair::from_pem(pem).map_err(|e| PkiError::Key(e.to_string()))?;
        Ok(Self { pair })
    }

    /// The algorithm of this key.
    #[must_use]
    pub fn algorithm(&self) -> KeyAlgorithm {
        if self.pair.algorithm() == &rcgen::PKCS_ED25519 {
            KeyAlgorithm::Ed25519
        } else {
            KeyAlgorithm::EcdsaP256
        }
    }

    /// The public key in Distinguished Encoding Rules (DER) form.
    #[must_use]
    pub fn public_key_der(&self) -> Vec<u8> {
        rcgen::PublicKeyData::subject_public_key_info(&self.pair)
    }

    /// The private key in PKCS#8 PEM form. Treat the result as a secret.
    #[must_use]
    pub fn to_pem(&self) -> Zeroizing<String> {
        Zeroizing::new(self.pair.serialize_pem())
    }

    /// A certificate signing request (CSR) in Privacy-Enhanced Mail (PEM) form
    /// for `client_name`, signed by this key. The key never leaves the caller.
    ///
    /// # Errors
    /// [`PkiError::BadClientName`] for an invalid name, and [`PkiError::Ca`]
    /// when the CSR cannot be made.
    pub fn csr_pem(&self, client_name: &str) -> Result<String, PkiError> {
        super::validate_client_name(client_name)?;
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| PkiError::Ca(format!("CSR parameters: {e}")))?;
        let mut name = rcgen::DistinguishedName::new();
        name.push(rcgen::DnType::CommonName, client_name);
        params.distinguished_name = name;
        let csr = params
            .serialize_request(&self.pair)
            .map_err(|e| PkiError::Ca(format!("CSR: {e}")))?;
        csr.pem().map_err(|e| PkiError::Ca(format!("CSR PEM: {e}")))
    }

    /// The `rcgen` key pair.
    pub(crate) fn pair(&self) -> &rcgen::KeyPair {
        &self.pair
    }
}
