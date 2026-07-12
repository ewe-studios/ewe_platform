//! Seed-derived and random-identity key handling (spec-55, feature 01; decision 03).
//!
//! WHY: The bootstrap "128/256 key" is a **derivation seed**, not a raw WireGuard key —
//! sharing the small secret shares the identical bootstrap keypair. Steady-state
//! per-peer identity keys, by contrast, are random and per-node.
//!
//! WHAT: [`WgSeed`] (16/32-byte seed with base64url encoding), [`WgSeed::derive_bootstrap`]
//! (HKDF-BLAKE2s → deterministic x25519 [`BootstrapKeys`] + PSKs), [`IdentityKeypair`]
//! (random), and [`PeerPublicKey`] parsing (hex-64 / base64).
//!
//! HOW: `HKDF-Extract` with the version-bound salt `foundation_wireguard/v1`, then
//! `HKDF-Expand` with per-purpose, network-bound `info` strings (decision 03, normative).

use base64::engine::general_purpose::{STANDARD as B64_STD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use blake2::Blake2s256;
use boringtun::x25519::{PublicKey, StaticSecret};
use hkdf::SimpleHkdf;
use zeroize::Zeroizing;

use crate::shared::error::{WgError, WgResult};

/// Version-bound HKDF salt. **Normative** — changing it is a wire-breaking change and
/// must bump the bootstrap token version prefix (decision 03/04).
const HKDF_SALT: &[u8] = b"foundation_wireguard/v1";

/// HKDF-Expand `info` prefixes for domain separation (decision 03, normative).
const INFO_BOOTSTRAP_X25519: &[u8] = b"wg-bootstrap-x25519|";
const INFO_BOOTSTRAP_PSK: &[u8] = b"wg-bootstrap-psk|";
const INFO_TLS_PSK: &[u8] = b"tls-psk|";
const INFO_NETWORK_ID: &[u8] = b"network-id";

/// Seed strength: 128-bit (short, human-copyable) or 256-bit (long-lived).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedBits {
    /// 16-byte seed.
    Bits128,
    /// 32-byte seed (recommended for long-lived networks).
    Bits256,
}

impl SeedBits {
    #[must_use]
    fn bytes(self) -> usize {
        match self {
            SeedBits::Bits128 => 16,
            SeedBits::Bits256 => 32,
        }
    }
}

/// A network identifier bound into every derived key so one seed used for two networks
/// (or two protocol versions) never produces colliding keys.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetworkId([u8; 16]);

impl NetworkId {
    /// WHY: Operators may name a network explicitly rather than deriving it.
    ///
    /// WHAT: Wrap 16 raw bytes as a network id.
    ///
    /// HOW: Stores the bytes verbatim.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw 16-byte identifier.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Lowercase-hex rendering of the id.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Debug for NetworkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetworkId({})", self.to_hex())
    }
}

impl std::fmt::Display for NetworkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// The deterministic bootstrap material derived from a seed for one network.
pub struct BootstrapKeys {
    /// Deterministic bootstrap x25519 static secret (all seed holders derive the same).
    pub static_secret: StaticSecret,
    /// The matching bootstrap public key.
    pub public: PublicKey,
    /// WireGuard preshared key for bootstrap-phase tunnels (Noise defense-in-depth).
    pub psk: [u8; 32],
    /// External PSK for the TLS bootstrap channel (decision 06).
    pub tls_psk: [u8; 32],
}

impl std::fmt::Debug for BootstrapKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print secret material.
        f.debug_struct("BootstrapKeys")
            .field("public", &encode_public(&self.public))
            .finish_non_exhaustive()
    }
}

/// A derivation seed (16 or 32 bytes). Zeroized on drop.
#[derive(Clone)]
pub struct WgSeed {
    bytes: Zeroizing<Vec<u8>>,
}

impl WgSeed {
    /// WHY: Networks are bootstrapped from a freshly generated high-entropy seed.
    ///
    /// WHAT: Generate a random seed of the requested strength.
    ///
    /// HOW: Fills 16 or 32 bytes from the OS CSPRNG via `getrandom`.
    ///
    /// # Errors
    /// Returns [`WgError::Random`] if the OS random source is unavailable.
    pub fn generate(bits: SeedBits) -> WgResult<Self> {
        let mut bytes = vec![0u8; bits.bytes()];
        getrandom::getrandom(&mut bytes)?;
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    /// WHY: Seeds arrive as raw bytes from tokens or env.
    ///
    /// WHAT: Wrap raw bytes as a seed, rejecting anything but 16 or 32 bytes.
    ///
    /// HOW: Validates the length (no silent default — decision 03).
    ///
    /// # Errors
    /// Returns [`WgError::InvalidSeedLength`] if `bytes` is not 16 or 32 bytes long.
    pub fn from_bytes(bytes: &[u8]) -> WgResult<Self> {
        if bytes.len() != 16 && bytes.len() != 32 {
            return Err(WgError::InvalidSeedLength(bytes.len()));
        }
        Ok(Self {
            bytes: Zeroizing::new(bytes.to_vec()),
        })
    }

    /// The raw seed bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The seed strength.
    #[must_use]
    pub fn bits(&self) -> SeedBits {
        if self.bytes.len() == 16 {
            SeedBits::Bits128
        } else {
            SeedBits::Bits256
        }
    }

    /// Encode the seed as unpadded base64url (the bootstrap-token representation).
    #[must_use]
    pub fn to_base64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.bytes.as_slice())
    }

    /// WHY: Seeds travel through tokens/env as base64url text.
    ///
    /// WHAT: Decode an unpadded base64url seed string.
    ///
    /// HOW: Base64url-decodes then validates the length via [`Self::from_bytes`].
    ///
    /// # Errors
    /// [`WgError::Base64`] on malformed base64; [`WgError::InvalidSeedLength`] on a bad
    /// decoded length.
    pub fn from_base64url(s: &str) -> WgResult<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s.trim())?;
        Self::from_bytes(&bytes)
    }

    /// Extract an HKDF instance keyed by this seed.
    fn hkdf(&self) -> SimpleHkdf<Blake2s256> {
        SimpleHkdf::<Blake2s256>::new(Some(HKDF_SALT), &self.bytes)
    }

    /// WHY: When no network id is supplied, one is derived from the seed so keys are
    /// still network-bound and stable.
    ///
    /// WHAT: Deterministically derive this seed's default [`NetworkId`].
    ///
    /// HOW: `HKDF-Expand(prk, "network-id", 16)`.
    ///
    /// # Panics
    /// Never panics (16-byte output is always within HKDF limits).
    #[must_use]
    pub fn derive_network_id(&self) -> NetworkId {
        let mut out = [0u8; 16];
        self.hkdf()
            .expand(INFO_NETWORK_ID, &mut out)
            .expect("16 bytes is within HKDF-BLAKE2s output limits");
        NetworkId(out)
    }

    /// WHY: Every seed holder must derive the *same* bootstrap keypair + PSKs for a given
    /// network — that is what makes "share the secret, share the network" work.
    ///
    /// WHAT: Deterministically derive [`BootstrapKeys`] for `network_id`.
    ///
    /// HOW: Per-purpose `HKDF-Expand` with network-bound `info` strings (decision 03);
    /// the x25519 secret bytes are curve25519-clamped before use.
    ///
    /// # Panics
    /// Never panics (all outputs are within HKDF limits).
    #[must_use]
    pub fn derive_bootstrap(&self, network_id: &NetworkId) -> BootstrapKeys {
        let hk = self.hkdf();

        let mut sk_bytes = [0u8; 32];
        hk.expand_multi_info(&[INFO_BOOTSTRAP_X25519, network_id.as_bytes()], &mut sk_bytes)
            .expect("32 bytes within HKDF limits");
        clamp_x25519(&mut sk_bytes);
        let static_secret = StaticSecret::from(sk_bytes);
        let public = PublicKey::from(&static_secret);

        let mut psk = [0u8; 32];
        hk.expand_multi_info(&[INFO_BOOTSTRAP_PSK, network_id.as_bytes()], &mut psk)
            .expect("32 bytes within HKDF limits");

        let mut tls_psk = [0u8; 32];
        hk.expand_multi_info(&[INFO_TLS_PSK, network_id.as_bytes()], &mut tls_psk)
            .expect("32 bytes within HKDF limits");

        // The intermediate secret bytes are copied into StaticSecret; wipe our copy.
        sk_bytes.iter_mut().for_each(|b| *b = 0);

        BootstrapKeys {
            static_secret,
            public,
            psk,
            tls_psk,
        }
    }
}

impl std::fmt::Debug for WgSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print seed bytes.
        write!(f, "WgSeed({} bytes, redacted)", self.bytes.len())
    }
}

/// A random per-peer identity keypair (post-join; decision 10). **Not** seed-derived.
pub struct IdentityKeypair {
    secret: StaticSecret,
    public: PublicKey,
}

impl IdentityKeypair {
    /// WHY: After bootstrap, each node re-pairs on its own individually-authenticated
    /// keypair rather than the shared bootstrap key.
    ///
    /// WHAT: Generate a fresh random x25519 keypair.
    ///
    /// HOW: 32 OS-random bytes → clamped x25519 static secret → public key.
    ///
    /// # Errors
    /// Returns [`WgError::Random`] if the OS random source is unavailable.
    pub fn generate() -> WgResult<Self> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)?;
        clamp_x25519(&mut bytes);
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret);
        bytes.iter_mut().for_each(|b| *b = 0);
        Ok(Self { secret, public })
    }

    /// This identity's static secret (for constructing a `Tunn`).
    #[must_use]
    pub fn secret(&self) -> &StaticSecret {
        &self.secret
    }

    /// This identity's public key.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        self.public
    }

    /// Public key as WireGuard-style base64 (44 chars).
    #[must_use]
    pub fn public_base64(&self) -> String {
        encode_public(&self.public)
    }
}

impl std::fmt::Debug for IdentityKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityKeypair")
            .field("public", &self.public_base64())
            .finish_non_exhaustive()
    }
}

/// A peer's x25519 public key, parseable from hex-64 or base64 (mirrors WireGuard's
/// `KeyBytes`, which is `pub(crate)` in boringtun so we own this parser).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PeerPublicKey(PublicKey);

impl PeerPublicKey {
    /// Wrap a raw 32-byte public key.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(PublicKey::from(bytes))
    }

    /// WHY: Peer keys arrive as text in configs and gossip records.
    ///
    /// WHAT: Parse a key from either 64-char hex or 43/44-char base64.
    ///
    /// HOW: Detects hex (all hex digits, len 64) vs base64 and decodes to 32 bytes.
    ///
    /// # Errors
    /// [`WgError::InvalidKey`] if the string is neither a valid hex-64 nor base64 key.
    pub fn parse(s: &str) -> WgResult<Self> {
        let s = s.trim();
        if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
            let raw = hex::decode(s)?;
            let arr: [u8; 32] = raw
                .try_into()
                .map_err(|_| WgError::InvalidKey("hex key must be 32 bytes".into()))?;
            return Ok(Self::from_bytes(arr));
        }
        // base64 (standard alphabet, with or without padding).
        let raw = B64_STD
            .decode(s)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(s))?;
        let arr: [u8; 32] = raw
            .try_into()
            .map_err(|_| WgError::InvalidKey("base64 key must be 32 bytes".into()))?;
        Ok(Self::from_bytes(arr))
    }

    /// The inner x25519 public key.
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        self.0
    }

    /// The raw 32-byte key.
    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        *self.0.as_bytes()
    }

    /// Standard WireGuard base64 (44 chars).
    #[must_use]
    pub fn to_base64(&self) -> String {
        encode_public(&self.0)
    }

    /// Lowercase hex (64 chars).
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }
}

impl std::fmt::Debug for PeerPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PeerPublicKey({})", self.to_base64())
    }
}

impl std::fmt::Display for PeerPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_base64())
    }
}

/// Apply the standard curve25519 clamp to a 32-byte scalar.
fn clamp_x25519(bytes: &mut [u8; 32]) {
    bytes[0] &= 248;
    bytes[31] &= 127;
    bytes[31] |= 64;
}

/// Encode a public key as standard-alphabet base64 (WireGuard's canonical form).
fn encode_public(key: &PublicKey) -> String {
    B64_STD.encode(key.as_bytes())
}
