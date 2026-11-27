//! The signing key and the verifying key.
//!
//! WHY: the issuer holds a private key; the verifier holds only public keys. A
//! Worker gets public keys from its configuration and never sees a private one.
//!
//! WHAT: [`SigningKey`] signs a message with Ed25519 (RFC 8032), and
//! [`VerifyingKey`] checks it.
//!
//! HOW: Ed25519 signatures are deterministic, so no randomness enters the sign
//! path and a deterministic simulation test (DST) run replays exactly. A new
//! key takes its 32-byte seed from the `Env` entropy source.

use base64::Engine;
use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};
use foundation_context::env::EntropySource;
use foundation_context::Ctx;
use zeroize::{Zeroize, Zeroizing};

#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
use rsa::pkcs1::EncodeRsaPublicKey;
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
use rsa::pkcs8::DecodePrivateKey;
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
use rsa::signature::{RandomizedSigner, SignatureEncoding, Verifier as _};

use super::{SecretError, SigningAlgorithm};

/// The 16-byte prefix of an Ed25519 PKCS#8 version 1 key (RFC 8410), 48 bytes
/// in total.
const V1_HEADER: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The 16-byte prefix of an Ed25519 PKCS#8 version 2 key, 83 bytes in total,
/// the layout that `ring` writes.
const V2_HEADER: [u8; 16] = [
    0x30, 0x51, 0x02, 0x01, 0x01, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The 3-byte separator before the stored public key in a version 2 key.
const V2_TRAILER: [u8; 3] = [0x81, 0x21, 0x00];

/// Decode a PEM body whose label is `label` in standard base64.
///
/// The block may sit after another block, as `openssl ecparam` writes it.
pub(crate) fn pem_body(text: &str, label: &str) -> Option<Zeroizing<String>> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let after = text.split(&begin).nth(1)?;
    let body = after.split(&end).next()?;
    Some(Zeroizing::new(
        body.chars().filter(|c| !c.is_whitespace()).collect(),
    ))
}

/// Decode a PEM body to DER bytes in standard base64.
pub(crate) fn pem_der(text: &str, label: &str) -> Option<Zeroizing<Vec<u8>>> {
    let body = pem_body(text, label)?;
    base64::engine::general_purpose::STANDARD
        .decode(body.as_bytes())
        .ok()
        .map(Zeroizing::new)
}

/// The private key that signs secrets. Never printed; its secret bytes are
/// zeroed on drop.
pub struct SigningKey {
    alg: SigningAlgorithm,
    inner: SigningKeyInner,
    verifying: VerifyingKey,
}

enum SigningKeyInner {
    Ed25519(KeyPair),
    #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
    RsaPss {
        signing: Box<rsa::pss::SigningKey<sha2::Sha256>>,
        pkcs8: Zeroizing<Vec<u8>>,
    },
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey")
            .field("alg", &self.alg)
            .field("kid", &self.verifying.kid)
            .field("fingerprint", &self.verifying.fingerprint())
            .finish_non_exhaustive()
    }
}

impl SigningKey {
    /// A new Ed25519 key from 32 bytes of `ctx` entropy.
    ///
    /// # Errors
    /// [`SecretError::Entropy`] when the context holds no `Env` or the entropy
    /// source fails.
    pub fn generate_ed25519(ctx: &Ctx, kid: Option<String>) -> Result<Self, SecretError> {
        let env = ctx
            .require_env()
            .map_err(|e| SecretError::Entropy(e.to_string()))?;
        let mut seed = Zeroizing::new([0u8; 32]);
        env.entropy()
            .fill(&mut *seed)
            .map_err(|e| SecretError::Entropy(e.to_string()))?;
        Ok(Self::from_seed(*seed, kid))
    }

    /// An Ed25519 key from a 32-byte seed.
    #[must_use]
    pub fn from_seed(seed: [u8; 32], kid: Option<String>) -> Self {
        let pair = KeyPair::from_seed(Seed::new(seed));
        let verifying = VerifyingKey {
            alg: SigningAlgorithm::Ed25519,
            bytes: pair.pk.as_slice().to_vec(),
            kid,
        };
        Self {
            alg: SigningAlgorithm::Ed25519,
            inner: SigningKeyInner::Ed25519(pair),
            verifying,
        }
    }

    /// Parse a PKCS#8 private key in DER form.
    ///
    /// Accepts an Ed25519 PKCS#8 version 1 key (48 bytes, RFC 8410) and a
    /// version 2 key (83 bytes, the `ring` layout). With feature `secret-rsa`
    /// on a native target it also accepts an RSA key of 2048 to 8192 bits. A
    /// version 2 key whose stored public key does not match its seed is an
    /// error; the code never overwrites it.
    ///
    /// # Errors
    /// [`SecretError::BadKey`] for any other input. A passphrase-protected key
    /// file needs the `PRIVATE KEY` form of [`from_pkcs8_pem`](Self::from_pkcs8_pem).
    pub fn from_pkcs8_der(der: &[u8], kid: Option<String>) -> Result<Self, SecretError> {
        match der.len() {
            48 if der[..16] == V1_HEADER => {
                let seed = seed_from(&der[16..48]);
                return Ok(Self::from_seed(seed, kid));
            }
            83 if der[..16] == V2_HEADER && der[48..51] == V2_TRAILER => {
                let seed = seed_from(&der[16..48]);
                let stored = &der[51..83];
                let pair = KeyPair::from_seed(Seed::new(seed));
                if pair.pk.as_slice() != stored {
                    return Err(SecretError::BadKey(
                        "the stored Ed25519 public key does not match the seed".to_string(),
                    ));
                }
                return Ok(Self::from_seed(seed, kid));
            }
            _ => {}
        }
        #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
        if let Ok(private) = rsa::RsaPrivateKey::from_pkcs8_der(der) {
            use rsa::traits::PublicKeyParts as _;

            // The design accepts RSA keys of 2048 to 8192 bits.
            if !(256..=1024).contains(&private.size()) {
                return Err(SecretError::BadKey(format!(
                    "an RSA key must be 2048 to 8192 bits, not {}",
                    private.size() * 8
                )));
            }
            let public = private
                .to_public_key()
                .to_pkcs1_der()
                .map_err(|e| SecretError::BadKey(format!("RSA public key: {e}")))?;
            let verifying = VerifyingKey {
                alg: SigningAlgorithm::RsaPssSha256,
                bytes: public.as_bytes().to_vec(),
                kid,
            };
            return Ok(Self {
                alg: SigningAlgorithm::RsaPssSha256,
                inner: SigningKeyInner::RsaPss {
                    signing: Box::new(rsa::pss::SigningKey::new(private)),
                    pkcs8: Zeroizing::new(der.to_vec()),
                },
                verifying,
            });
        }
        Err(SecretError::BadKey(
            "expected an Ed25519 PKCS#8 key (48 or 83 bytes), or an RSA key with native + feature secret-rsa"
                .to_string(),
        ))
    }

    /// Parse a `-----BEGIN PRIVATE KEY-----` PEM.
    ///
    /// # Errors
    /// [`SecretError::BadKey`] for another PEM label, bad base64, or an
    /// unsupported key.
    pub fn from_pkcs8_pem(pem: &str, kid: Option<String>) -> Result<Self, SecretError> {
        if pem.contains("-----BEGIN ENCRYPTED PRIVATE KEY-----") {
            return Err(SecretError::BadKey(
                "passphrase-protected keys are not supported; make one with -N \"\"".to_string(),
            ));
        }
        let der = pem_der(pem, "PRIVATE KEY").ok_or_else(|| {
            SecretError::BadKey("the key must be a PKCS#8 \"PRIVATE KEY\" PEM block".to_string())
        })?;
        Self::from_pkcs8_der(&der, kid)
    }

    /// The key as PKCS#8 PEM (Ed25519: the version 1 layout). Treat as secret.
    #[must_use]
    pub fn to_pkcs8_pem(&self) -> Zeroizing<String> {
        match &self.inner {
            SigningKeyInner::Ed25519(pair) => Zeroizing::new(pair.sk.to_pem()),
            #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
            SigningKeyInner::RsaPss { pkcs8, .. } => encode_pkcs8_pem(pkcs8),
        }
    }

    /// The signing algorithm.
    #[must_use]
    pub fn algorithm(&self) -> SigningAlgorithm {
        self.alg
    }

    /// The public half, for verifiers.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying
    }

    /// Sign `message`. Ed25519 is deterministic: the same message and key give
    /// the same signature. RSA-PSS is randomized, so it draws its salt from
    /// `entropy`, the injected source of `Ctx`; it never reads a system global.
    // Without the RSA arm every path is infallible. The signature keeps one
    // shape on every target.
    #[cfg_attr(
        not(all(feature = "secret-rsa", not(target_family = "wasm"))),
        allow(clippy::unnecessary_wraps)
    )]
    pub(crate) fn sign(
        &self,
        message: &[u8],
        #[cfg_attr(
            not(all(feature = "secret-rsa", not(target_family = "wasm"))),
            allow(unused_variables)
        )]
        entropy: &dyn EntropySource,
    ) -> Result<Vec<u8>, SecretError> {
        match &self.inner {
            SigningKeyInner::Ed25519(pair) => Ok(pair.sk.sign(message, None).to_vec()),
            #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
            SigningKeyInner::RsaPss { signing, .. } => sign_rsa_pss(signing, message, entropy),
        }
    }
}

/// Encode PKCS#8 DER bytes as a `PRIVATE KEY` PEM block, 64 characters a line.
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
fn encode_pkcs8_pem(der: &[u8]) -> Zeroizing<String> {
    let encoded = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(der));
    let mut pem = Zeroizing::new(String::with_capacity(encoded.len() + 64));
    pem.push_str("-----BEGIN PRIVATE KEY-----\n");
    for line in encoded.as_bytes().chunks(64) {
        // The base64 text is ASCII.
        pem.push_str(std::str::from_utf8(line).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END PRIVATE KEY-----\n");
    pem
}

/// A random source over the injected [`EntropySource`], for RSA-PSS.
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
struct EntropyRng(rand_chacha::ChaCha12Rng);

#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
impl EntropyRng {
    /// Seed a `ChaCha12` stream from 32 bytes of the injected entropy. The seed
    /// fill is the one fallible step; the stream then serves every byte that
    /// the signing code asks for.
    fn new(entropy: &dyn EntropySource) -> Result<Self, SecretError> {
        use rsa::rand_core::SeedableRng as _;
        let mut seed = Zeroizing::new([0u8; 32]);
        entropy
            .fill(&mut *seed)
            .map_err(|e| SecretError::Entropy(e.to_string()))?;
        Ok(Self(rand_chacha::ChaCha12Rng::from_seed(*seed)))
    }
}

#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
impl rsa::rand_core::RngCore for EntropyRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
        self.0.try_fill_bytes(dest)
    }
}

#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
impl rsa::rand_core::CryptoRng for EntropyRng {}

/// Sign `message` with RSA-PSS and SHA-256. The salt comes from `entropy`.
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
fn sign_rsa_pss(
    signing: &rsa::pss::SigningKey<sha2::Sha256>,
    message: &[u8],
    entropy: &dyn EntropySource,
) -> Result<Vec<u8>, SecretError> {
    let mut rng = EntropyRng::new(entropy)?;
    let signature = signing
        .try_sign_with_rng(&mut rng, message)
        .map_err(|e| SecretError::BadRequest(format!("RSA-PSS signing failed: {e}")))?;
    Ok(signature.to_vec())
}

fn seed_from(bytes: &[u8]) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed.copy_from_slice(bytes);
    seed
}

impl Drop for SigningKey {
    fn drop(&mut self) {
        // `KeyPair` zeroes its secret key on drop. Drop the explicit copy of
        // the public bytes too, so no part of the key lingers.
        self.verifying.bytes.zeroize();
    }
}

/// A public key that verifies secrets. A Worker gets it from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyingKey {
    pub(crate) alg: SigningAlgorithm,
    /// Raw Ed25519 key (32 bytes).
    pub(crate) bytes: Vec<u8>,
    pub(crate) kid: Option<String>,
}

impl VerifyingKey {
    /// From the base64url form that a `SignedSecret` and the configuration use.
    ///
    /// # Errors
    /// [`SecretError::BadKey`] when the text is not base64url.
    pub fn from_base64url(
        alg: SigningAlgorithm,
        b64: &str,
        kid: Option<String>,
    ) -> Result<Self, SecretError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64.trim())
            .map_err(|e| SecretError::BadKey(format!("public key is not base64url: {e}")))?;
        Ok(Self { alg, bytes, kid })
    }

    /// The raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The signing algorithm of this key.
    #[must_use]
    pub fn algorithm(&self) -> SigningAlgorithm {
        self.alg
    }

    /// The key ID, when the configuration gives one.
    #[must_use]
    pub fn kid(&self) -> Option<&str> {
        self.kid.as_deref()
    }

    /// Lowercase hexadecimal SHA-256 of the key bytes. Safe to log.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        crate::fingerprint::public_fingerprint_hex(&self.bytes)
    }

    /// Check `signature` over `message`.
    pub(crate) fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, SecretError> {
        match self.alg {
            SigningAlgorithm::Ed25519 => {
                let key = PublicKey::from_slice(&self.bytes)
                    .map_err(|_| SecretError::BadKey("Ed25519 public key".to_string()))?;
                let sig =
                    Signature::from_slice(signature).map_err(|_| SecretError::BadSignature)?;
                Ok(key.verify(message, &sig).is_ok())
            }
            SigningAlgorithm::RsaPssSha256 => verify_rsa_pss(&self.bytes, message, signature),
        }
    }
}

/// Verify an RSA-PSS with SHA-256 signature. The public key is DER
/// `RSAPublicKey`.
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
fn verify_rsa_pss(
    public_key_der: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<bool, SecretError> {
    use rsa::pkcs1::DecodeRsaPublicKey as _;

    let public = rsa::RsaPublicKey::from_pkcs1_der(public_key_der)
        .map_err(|e| SecretError::BadKey(format!("RSA public key: {e}")))?;
    let verifying = rsa::pss::VerifyingKey::<sha2::Sha256>::new(public);
    let signature =
        rsa::pss::Signature::try_from(signature).map_err(|_| SecretError::BadSignature)?;
    Ok(verifying.verify(message, &signature).is_ok())
}

/// A build without the RSA code cannot verify an RSA secret. The answer is a
/// typed error, never a silent pass or fail.
#[cfg(not(all(feature = "secret-rsa", not(target_family = "wasm"))))]
fn verify_rsa_pss(_: &[u8], _: &[u8], _: &[u8]) -> Result<bool, SecretError> {
    Err(SecretError::UnsupportedAlgorithm(
        "rsa-pss-sha256 needs native + feature secret-rsa".to_string(),
    ))
}

/// A `VerifyingKey` that a configuration names with a JSON object.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyEntry {
    kid: Option<String>,
    alg: String,
    public_key: String,
}

/// The verifying keys from the JSON list of `AGENT_RPC_SECRET_PUBLIC_KEYS`.
///
/// The text is a JSON array of `{"kid": "...", "alg": "ed25519",
/// "public_key": "<base64url>"}`. `kid` is optional.
///
/// # Errors
/// [`super::keys::KeyLoadError`] for bad JSON, an unknown `alg`, or a bad
/// base64url key.
pub(crate) fn parse_verifying_keys(text: &str) -> Result<Vec<VerifyingKey>, String> {
    let entries: Vec<KeyEntry> =
        serde_json::from_str(text).map_err(|e| format!("not a JSON key list: {e}"))?;
    let mut keys = Vec::with_capacity(entries.len());
    for entry in entries {
        let alg = SigningAlgorithm::parse(&entry.alg)
            .ok_or_else(|| format!("unknown algorithm {:?}", entry.alg))?;
        let key = VerifyingKey::from_base64url(alg, &entry.public_key, entry.kid)
            .map_err(|e| e.to_string())?;
        keys.push(key);
    }
    Ok(keys)
}

/// The number of characters of hexadecimal text that form a key ID.
pub(crate) const KID_HEX_CHARS: usize = 16;

/// The derived key ID: the first 16 hexadecimal characters of the SHA-256 of
/// the public key.
#[must_use]
pub(crate) fn derived_kid(public_key: &[u8]) -> String {
    let digest = crate::fingerprint::public_fingerprint_hex(public_key);
    digest.chars().take(KID_HEX_CHARS).collect()
}
