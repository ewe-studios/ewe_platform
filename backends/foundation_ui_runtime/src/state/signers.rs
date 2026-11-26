use foundation_compact::crypto::chacha::{decrypt, encrypt_with_nonce};
use foundation_compact::rng::Rng;
use foundation_compact::SystemTime;
use foundation_context::Ctx;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use super::{StateConfig, StateError, StateKey, StateKeyRing, StateSigner};

const READABLE: [u8; 4] = *b"EWH1";
const PRIVATE: [u8; 4] = *b"EWC1";
const HEADER: usize = 8; // Format identifier and little-endian key identifier.
const WINDOW: usize = 32; // Two 128-bit Unix timestamps in nanoseconds.
const DIGEST: usize = 32;
const AEAD_OVERHEAD: usize = 12 + 16; // Nonce and tag for ChaCha20-Poly1305.

/// Authenticates readable state with a keyed hash (HMAC-SHA256).
pub struct HmacStateSigner {
    keys: StateKeyRing,
    config: StateConfig,
}

/// Seals private state with ChaCha20-Poly1305.
pub struct ChaChaStateSigner {
    keys: StateKeyRing,
    config: StateConfig,
}

macro_rules! signer_constructors {
    () => {
        /// Creates a signer for a single host key with identifier one.
        ///
        /// # Errors
        ///
        /// Returns an error if the lifetime is zero.
        pub fn new(mut key: [u8; 32], config: StateConfig) -> Result<Self, StateError> {
            let current = StateKey::new(1, key);
            key.zeroize();
            Self::from_key_ring(StateKeyRing::new(current?), config)
        }

        /// Uses the current key for signing and accepts previous keys for verification.
        ///
        /// # Errors
        ///
        /// Returns an error if the lifetime is zero.
        pub fn from_key_ring(keys: StateKeyRing, config: StateConfig) -> Result<Self, StateError> {
            if config.ttl_value().is_zero() {
                return Err(StateError::InvalidLifetime);
            }
            Ok(Self { keys, config })
        }
    };
}

impl HmacStateSigner {
    signer_constructors!();
}
impl ChaChaStateSigner {
    signer_constructors!();
}

impl StateSigner for HmacStateSigner {
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError> {
        check_body(self.config, body.len())?;
        let header = header(READABLE, self.keys.current_id());
        let mut signed = Vec::from(header);
        signed.extend(window(ctx, self.config)?);
        signed.extend_from_slice(body);
        let mut mac = state_mac(&self.keys.current)?;
        mac.update(&binding_digest(&header, binding));
        mac.update(&signed[HEADER..]);
        signed.extend_from_slice(&mac.finalize().into_bytes());
        Ok(signed)
    }

    fn verify(
        &self,
        ctx: &Ctx,
        binding: Option<&[u8]>,
        signed: &[u8],
    ) -> Result<Vec<u8>, StateError> {
        let key = parse_header(signed, READABLE, WINDOW + DIGEST, self.config, &self.keys)?;
        let (message, tag) = signed.split_at(signed.len() - DIGEST);
        let mut mac = state_mac(key)?;
        mac.update(&binding_digest(&message[..HEADER], binding));
        mac.update(&message[HEADER..]);
        mac.verify_slice(tag).map_err(|_| StateError::BadTag)?;
        verified_body(ctx, &message[HEADER..])
    }
}

impl StateSigner for ChaChaStateSigner {
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError> {
        check_body(self.config, body.len())?;
        let header = header(PRIVATE, self.keys.current_id());
        let mut clear = Zeroizing::new(Vec::from(binding_digest(&header, binding)));
        clear.extend(window(ctx, self.config)?);
        clear.extend_from_slice(body);
        let mut signed = Vec::from(header);
        let cipher = encrypt_with_nonce(&self.keys.current.secret, nonce(ctx)?, &clear)
            .map_err(|_| StateError::Encryption)?;
        signed.extend(cipher);
        Ok(signed)
    }

    fn verify(
        &self,
        ctx: &Ctx,
        binding: Option<&[u8]>,
        signed: &[u8],
    ) -> Result<Vec<u8>, StateError> {
        let key = parse_header(
            signed,
            PRIVATE,
            DIGEST + WINDOW + AEAD_OVERHEAD,
            self.config,
            &self.keys,
        )?;
        let clear = decrypt(&key.secret, &signed[HEADER..]).map_err(|_| StateError::BadTag)?;
        let (digest, payload) = clear.split_at(DIGEST);
        if !bool::from(digest.ct_eq(&binding_digest(&signed[..HEADER], binding))) {
            return Err(StateError::BadTag);
        }
        verified_body(ctx, payload)
    }
}

fn state_mac(key: &StateKey) -> Result<Hmac<Sha256>, StateError> {
    Hmac::<Sha256>::new_from_slice(key.secret.as_bytes()).map_err(|_| StateError::BadKey)
}

fn header(format: [u8; 4], key_id: u32) -> [u8; HEADER] {
    let mut result = [0; HEADER];
    result[..4].copy_from_slice(&format);
    result[4..].copy_from_slice(&key_id.to_le_bytes());
    result
}

fn parse_header<'a>(
    signed: &[u8],
    format: [u8; 4],
    overhead: usize,
    config: StateConfig,
    keys: &'a StateKeyRing,
) -> Result<&'a StateKey, StateError> {
    let invalid = || StateError::Malformed {
        detail: "Invalid signer format or length.".into(),
    };
    let body_len = signed
        .len()
        .checked_sub(HEADER + overhead)
        .ok_or_else(invalid)?;
    check_body(config, body_len)?;
    if signed[..4] != format {
        return Err(invalid());
    }
    let id = u32::from_le_bytes(signed[4..HEADER].try_into().map_err(|_| invalid())?);
    keys.find(id).ok_or(StateError::UnknownKey)
}

fn check_body(config: StateConfig, actual: usize) -> Result<(), StateError> {
    let limit = config.body_bytes();
    if actual > limit {
        return Err(StateError::BodyTooLarge { limit, actual });
    }
    Ok(())
}

fn binding_digest(header: &[u8], binding: Option<&[u8]>) -> [u8; DIGEST] {
    let mut digest = Sha256::new();
    digest.update(b"ewe.ui.signer.v1");
    digest.update(header);
    digest.update([u8::from(binding.is_some())]);
    let bytes = binding.unwrap_or_default();
    digest.update((bytes.len() as u128).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn wall_nanos(ctx: &Ctx) -> Result<u128, StateError> {
    let missing_clock = StateError::MissingEnv { what: "clock" };
    ctx.try_clock()
        .ok_or_else(|| missing_clock.clone())?
        .wall_now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .map_err(|_| missing_clock)
}

fn window(ctx: &Ctx, config: StateConfig) -> Result<[u8; WINDOW], StateError> {
    let issued = wall_nanos(ctx)?;
    let expires = issued + config.ttl_value().as_nanos();
    let mut window = [0; WINDOW];
    window[..16].copy_from_slice(&issued.to_le_bytes());
    window[16..].copy_from_slice(&expires.to_le_bytes());
    Ok(window)
}

fn verified_body(ctx: &Ctx, payload: &[u8]) -> Result<Vec<u8>, StateError> {
    let (window, body) = payload.split_at(WINDOW);
    let now = wall_nanos(ctx)?;
    let issued = u128::from_le_bytes(window[..16].try_into().expect("exact timestamp width"));
    let expires = u128::from_le_bytes(window[16..].try_into().expect("exact timestamp width"));
    if now < issued || now >= expires {
        return Err(StateError::Expired);
    }
    Ok(body.to_vec())
}

fn nonce(ctx: &Ctx) -> Result<[u8; 12], StateError> {
    let rng = ctx
        .try_rng()
        .ok_or(StateError::MissingEnv { what: "rng" })?;
    let mut nonce = [0; 12];
    rng.lock()
        .map_err(|_| StateError::MissingEnv { what: "rng" })?
        .fill_bytes(&mut nonce);
    Ok(nonce)
}
