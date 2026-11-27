//! Turn OpenSSH, PKCS#8 and SEC1 PEM into one PKCS#8 DER form.

use ed25519_compact::{KeyPair, Seed};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use p256::SecretKey;
use ssh_key::private::KeypairData;
use ssh_key::EcdsaCurve;

use super::{KeyLoadError, PrivateKeyAlgorithm};
use crate::secret::keys::pem_der;

/// The 16-byte prefix of an Ed25519 PKCS#8 version 1 key (RFC 8410), 48 bytes
/// in total.
const V1_HEADER: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The 16-byte prefix of an Ed25519 PKCS#8 version 2 key, 83 bytes in total.
const V2_HEADER: [u8; 16] = [
    0x30, 0x51, 0x02, 0x01, 0x01, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The 3-byte separator before the stored public key in a version 2 key.
const V2_TRAILER: [u8; 3] = [0x81, 0x21, 0x00];

/// A parsed key: the algorithm, the canonical PKCS#8 DER, and the public key.
pub(super) struct ParsedKey {
    pub(super) algorithm: PrivateKeyAlgorithm,
    pub(super) pkcs8_der: Vec<u8>,
    pub(super) public_key: Vec<u8>,
}

/// Parse key text in one of the formats of the design.
pub(super) fn parse(text: &[u8]) -> Result<ParsedKey, KeyLoadError> {
    let text = core::str::from_utf8(text)
        .map_err(|_| KeyLoadError::UnsupportedFormat("the key text is not UTF-8".to_string()))?;
    let text = text.trim();
    if text.contains("-----BEGIN ENCRYPTED PRIVATE KEY-----") {
        return Err(KeyLoadError::Encrypted);
    }
    if text.contains("-----BEGIN OPENSSH PRIVATE KEY-----") {
        return parse_openssh(text, None);
    }
    if text.contains("-----BEGIN EC PRIVATE KEY-----") {
        return parse_sec1(text);
    }
    if text.contains("-----BEGIN PRIVATE KEY-----") {
        return parse_pkcs8(text);
    }
    Err(KeyLoadError::UnsupportedFormat(
        "expected an OpenSSH, PKCS#8 or SEC1 PEM key".to_string(),
    ))
}

/// Parse key text with its passphrase. An unencrypted key ignores it.
pub(super) fn parse_with_passphrase(
    text: &[u8],
    passphrase: &str,
) -> Result<ParsedKey, KeyLoadError> {
    let text = core::str::from_utf8(text)
        .map_err(|_| KeyLoadError::UnsupportedFormat("the key text is not UTF-8".to_string()))?;
    let text = text.trim();
    if text.contains("-----BEGIN ENCRYPTED PRIVATE KEY-----") {
        return parse_encrypted_pkcs8(text, passphrase);
    }
    if text.contains("-----BEGIN OPENSSH PRIVATE KEY-----") {
        return parse_openssh(text, Some(passphrase));
    }
    parse(text.as_bytes())
}

fn parse_openssh(text: &str, passphrase: Option<&str>) -> Result<ParsedKey, KeyLoadError> {
    let mut key = ssh_key::PrivateKey::from_openssh(text).map_err(|error| {
        if error.to_string().to_ascii_lowercase().contains("encrypt") {
            KeyLoadError::Encrypted
        } else {
            KeyLoadError::UnsupportedFormat(format!("OpenSSH key: {error}"))
        }
    })?;
    if key.is_encrypted() {
        let passphrase = passphrase.ok_or(KeyLoadError::Encrypted)?;
        key = key
            .decrypt(passphrase)
            .map_err(|_| KeyLoadError::BadPassphrase)?;
    }
    match key.key_data() {
        KeypairData::Ed25519(keypair) => {
            let seed = keypair.private.to_bytes();
            Ok(ed25519_from_seed(&seed))
        }
        KeypairData::Ecdsa(keypair) if keypair.curve() == EcdsaCurve::NistP256 => {
            let secret = SecretKey::from_slice(keypair.private_key_bytes()).map_err(|error| {
                KeyLoadError::UnsupportedFormat(format!("OpenSSH P-256 key: {error}"))
            })?;
            p256_parts(&secret)
        }
        _ => Err(KeyLoadError::UnsupportedFormat(format!(
            "unsupported OpenSSH key algorithm {:?}",
            key.algorithm()
        ))),
    }
}

fn parse_encrypted_pkcs8(text: &str, passphrase: &str) -> Result<ParsedKey, KeyLoadError> {
    let der = pem_der(text, "ENCRYPTED PRIVATE KEY").ok_or_else(|| {
        KeyLoadError::UnsupportedFormat("the encrypted PKCS#8 PEM body is not base64".to_string())
    })?;
    let encrypted = pkcs8::EncryptedPrivateKeyInfo::try_from(der.as_slice())
        .map_err(|error| KeyLoadError::UnsupportedFormat(format!("encrypted PKCS#8: {error}")))?;
    let document = encrypted
        .decrypt(passphrase)
        .map_err(|_| KeyLoadError::BadPassphrase)?;
    parse_pkcs8_der(document.as_bytes())
}

fn parse_pkcs8(text: &str) -> Result<ParsedKey, KeyLoadError> {
    let der = pem_der(text, "PRIVATE KEY").ok_or_else(|| {
        KeyLoadError::UnsupportedFormat("the PKCS#8 PEM body is not base64".to_string())
    })?;
    parse_pkcs8_der(&der)
}

fn parse_pkcs8_der(der: &[u8]) -> Result<ParsedKey, KeyLoadError> {
    if der.len() == 48 && der[..16] == V1_HEADER {
        return Ok(ed25519_from_seed(&der[16..48]));
    }
    if der.len() == 83 && der[..16] == V2_HEADER && der[48..51] == V2_TRAILER {
        let seed = &der[16..48];
        let stored = &der[51..83];
        let pair = KeyPair::from_seed(Seed::new(seed_from(seed)));
        if pair.pk.as_slice() != stored {
            return Err(KeyLoadError::UnsupportedFormat(
                "the stored Ed25519 public key does not match the seed".to_string(),
            ));
        }
        return Ok(ed25519_from_seed(seed));
    }
    if let Ok(secret) = SecretKey::from_pkcs8_der(der) {
        return p256_parts(&secret);
    }
    #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
    {
        use rsa::pkcs1::EncodeRsaPublicKey as _;
        use rsa::pkcs8::DecodePrivateKey as _;
        use rsa::traits::PublicKeyParts as _;

        if let Ok(private) = rsa::RsaPrivateKey::from_pkcs8_der(&der) {
            if (256..=1024).contains(&private.size()) {
                let public = private.to_public_key().to_pkcs1_der().map_err(|error| {
                    KeyLoadError::UnsupportedFormat(format!("RSA public key: {error}"))
                })?;
                return Ok(ParsedKey {
                    algorithm: PrivateKeyAlgorithm::Rsa,
                    pkcs8_der: der.to_vec(),
                    public_key: public.as_bytes().to_vec(),
                });
            }
        }
    }
    Err(KeyLoadError::UnsupportedFormat(
        "expected an Ed25519, P-256 or RSA PKCS#8 key; an RSA key needs native + feature secret-rsa"
            .to_string(),
    ))
}

fn parse_sec1(text: &str) -> Result<ParsedKey, KeyLoadError> {
    let block = pem_block(text, "EC PRIVATE KEY").ok_or_else(|| {
        KeyLoadError::UnsupportedFormat("the SEC1 PEM block is malformed".to_string())
    })?;
    let secret = SecretKey::from_sec1_pem(block)
        .map_err(|error| KeyLoadError::UnsupportedFormat(format!("SEC1 P-256 key: {error}")))?;
    p256_parts(&secret)
}

/// The whole PEM block for `label`, with its original line breaks.
fn pem_block<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = text.find(&begin)?;
    let end_start = text[start..].find(&end)? + start + end.len();
    Some(&text[start..end_start])
}

fn p256_parts(secret: &SecretKey) -> Result<ParsedKey, KeyLoadError> {
    let der = secret
        .to_pkcs8_der()
        .map_err(|error| KeyLoadError::UnsupportedFormat(format!("P-256 key: {error}")))?;
    let public_key = secret
        .public_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec();
    Ok(ParsedKey {
        algorithm: PrivateKeyAlgorithm::EcdsaP256,
        pkcs8_der: der.as_bytes().to_vec(),
        public_key,
    })
}

fn ed25519_from_seed(seed: &[u8]) -> ParsedKey {
    let seed = seed_from(seed);
    let pair = KeyPair::from_seed(Seed::new(seed));
    let mut pkcs8_der = Vec::with_capacity(48);
    pkcs8_der.extend_from_slice(&V1_HEADER);
    pkcs8_der.extend_from_slice(&seed);
    ParsedKey {
        algorithm: PrivateKeyAlgorithm::Ed25519,
        pkcs8_der,
        public_key: pair.pk.as_slice().to_vec(),
    }
}

fn seed_from(bytes: &[u8]) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed.copy_from_slice(bytes);
    seed
}
