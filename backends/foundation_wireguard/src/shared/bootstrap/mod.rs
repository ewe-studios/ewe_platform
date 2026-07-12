//! Bootstrap input parsing + token codec (spec-55, feature 02; decision 04).
//!
//! WHY: A node must get *into* the network from a single pasted value — either a
//! self-contained `wg1_` token (network id + seed + endpoints, zero other config) or a
//! bare seed whose endpoint/network arrive out-of-band (the Docker env path).
//!
//! WHAT: [`WgBootstrap::parse`] (one parser, two forms) and [`BootstrapToken`] with a
//! versioned, CRC-guarded `wg1_` wire format.
//!
//! HOW: The token body is a sequence of `tag / varint-length / value` fields (TLV),
//! base64url-encoded behind a `wg1_` prefix, with a trailing CRC32C (integrity/typo guard
//! only — confidentiality comes from TLS-PSK, decision 06).

pub mod rpc;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

pub use rpc::{
    decode_request, decode_response, encode_request, encode_response, Admission, BootstrapRequest,
    BootstrapResponse,
};

use crate::shared::error::{WgError, WgResult};
use crate::shared::keys::{NetworkId, WgSeed};

/// Version-tagged, human-greppable token prefix. A future `wg2_` may change layout/KDF.
pub const TOKEN_PREFIX: &str = "wg1_";

// TLV tags (decision 04, normative).
const TAG_NETWORK_ID: u8 = 0x01;
const TAG_SEED: u8 = 0x02;
const TAG_BOOTSTRAP_PUBKEY: u8 = 0x03;
const TAG_ENDPOINT: u8 = 0x04;
const TAG_FLAGS: u8 = 0x05;
const TAG_EXPIRES_AT: u8 = 0x06;

/// Token flag bits.
pub mod flag {
    /// The minting node hints it is relay-capable (decision 07).
    pub const RELAY_CAPABLE_HINT: u16 = 0x0001;
    /// The seed is ephemeral and expected to be rotated (decision 11).
    pub const EPHEMERAL: u16 = 0x0002;
    /// The network is IPv6-only.
    pub const IPV6_ONLY: u16 = 0x0004;
}

/// A u16 bitfield of bootstrap [`flag`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BootstrapFlags(pub u16);

impl BootstrapFlags {
    /// Whether a given flag bit is set.
    #[must_use]
    pub fn has(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    /// This token hints the minter is relay-capable.
    #[must_use]
    pub fn relay_capable_hint(self) -> bool {
        self.has(flag::RELAY_CAPABLE_HINT)
    }

    /// This token's seed is ephemeral.
    #[must_use]
    pub fn ephemeral(self) -> bool {
        self.has(flag::EPHEMERAL)
    }
}

/// The secret half carried by a token: the seed itself, or (seedless) only the bootstrap
/// public key (the PSK must then be supplied out-of-band).
#[derive(Clone)]
pub enum TokenSecret {
    /// The derivation seed — the common case.
    Seed(WgSeed),
    /// Only the bootstrap public key (seedless / advanced path).
    BootstrapPubkey([u8; 32]),
}

impl std::fmt::Debug for TokenSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenSecret::Seed(_) => write!(f, "Seed(redacted)"),
            TokenSecret::BootstrapPubkey(_) => write!(f, "BootstrapPubkey(..)"),
        }
    }
}

/// A fully self-describing bootstrap token.
#[derive(Clone, Debug)]
pub struct BootstrapToken {
    /// The network this token joins.
    pub network_id: NetworkId,
    /// The secret material (seed or bootstrap pubkey).
    pub secret: TokenSecret,
    /// One or more seed endpoints to dial (>= 1; multiple = redundancy).
    pub endpoints: Vec<SocketAddr>,
    /// Flag bitfield.
    pub flags: BootstrapFlags,
    /// Optional token TTL as a unix timestamp (seconds).
    pub expires_at: Option<u64>,
}

impl BootstrapToken {
    /// WHY: Minters build a token from a network, secret, and endpoint(s).
    ///
    /// WHAT: Construct a token with no flags and no expiry.
    ///
    /// HOW: Stores the fields; use the `with_*` setters to add flags/expiry.
    #[must_use]
    pub fn new(network_id: NetworkId, secret: TokenSecret, endpoints: Vec<SocketAddr>) -> Self {
        Self {
            network_id,
            secret,
            endpoints,
            flags: BootstrapFlags::default(),
            expires_at: None,
        }
    }

    /// Set the flag bitfield.
    #[must_use]
    pub fn with_flags(mut self, flags: BootstrapFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the token expiry (unix seconds).
    #[must_use]
    pub fn with_expiry(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Whether this token is expired relative to `now_unix_secs`.
    #[must_use]
    pub fn is_expired(&self, now_unix_secs: u64) -> bool {
        self.expires_at.is_some_and(|exp| now_unix_secs > exp)
    }

    /// WHY: Distribute the network as one paste-once string.
    ///
    /// WHAT: Encode this token as a `wg1_` string.
    ///
    /// HOW: TLV-encodes the fields, appends a CRC32C, and base64url-encodes the blob.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn to_token(&self) -> String {
        let mut body = Vec::with_capacity(96);
        put_tlv(&mut body, TAG_NETWORK_ID, self.network_id.as_bytes());
        match &self.secret {
            TokenSecret::Seed(seed) => put_tlv(&mut body, TAG_SEED, seed.as_bytes()),
            TokenSecret::BootstrapPubkey(pk) => put_tlv(&mut body, TAG_BOOTSTRAP_PUBKEY, pk),
        }
        for endpoint in &self.endpoints {
            put_tlv(&mut body, TAG_ENDPOINT, &encode_endpoint(*endpoint));
        }
        put_tlv(&mut body, TAG_FLAGS, &self.flags.0.to_be_bytes());
        if let Some(expires) = self.expires_at {
            put_tlv(&mut body, TAG_EXPIRES_AT, &expires.to_be_bytes());
        }
        let crc = crc32c(&body);
        body.extend_from_slice(&crc.to_be_bytes());

        format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(&body))
    }

    /// WHY: A pasted `wg1_` string must be decoded and integrity-checked.
    ///
    /// WHAT: Parse a `wg1_` token, verifying the CRC and required fields.
    ///
    /// HOW: Base64url-decodes, checks the trailing CRC32C, then walks the TLV fields.
    ///
    /// # Errors
    /// [`WgError::InvalidToken`] on a bad prefix, bad base64, CRC mismatch, truncation,
    /// or missing required fields (network id, secret, at least one endpoint).
    pub fn parse(token: &str) -> WgResult<Self> {
        let body_b64 = token
            .strip_prefix(TOKEN_PREFIX)
            .ok_or_else(|| WgError::InvalidToken(format!("missing `{TOKEN_PREFIX}` prefix")))?;
        let blob = URL_SAFE_NO_PAD
            .decode(body_b64.trim())
            .map_err(|e| WgError::InvalidToken(format!("base64url: {e}")))?;
        if blob.len() < 4 {
            return Err(WgError::InvalidToken("token too short".into()));
        }
        let (body, crc_bytes) = blob.split_at(blob.len() - 4);
        let expected = u32::from_be_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        if crc32c(body) != expected {
            return Err(WgError::InvalidToken("CRC mismatch (corrupt/truncated)".into()));
        }

        let mut network_id = None;
        let mut secret = None;
        let mut endpoints = Vec::new();
        let mut flags = BootstrapFlags::default();
        let mut expires_at = None;

        let mut pos = 0usize;
        while pos < body.len() {
            let tag = body[pos];
            pos += 1;
            let len = read_varint(body, &mut pos)? as usize;
            let value = body
                .get(pos..pos + len)
                .ok_or_else(|| WgError::InvalidToken("truncated field value".into()))?;
            pos += len;
            match tag {
                TAG_NETWORK_ID => {
                    let arr: [u8; 16] = value
                        .try_into()
                        .map_err(|_| WgError::InvalidToken("network_id must be 16 bytes".into()))?;
                    network_id = Some(NetworkId::from_bytes(arr));
                }
                TAG_SEED => {
                    let seed = WgSeed::from_bytes(value)
                        .map_err(|e| WgError::InvalidToken(format!("seed: {e}")))?;
                    secret = Some(TokenSecret::Seed(seed));
                }
                TAG_BOOTSTRAP_PUBKEY => {
                    let arr: [u8; 32] = value
                        .try_into()
                        .map_err(|_| WgError::InvalidToken("pubkey must be 32 bytes".into()))?;
                    secret = Some(TokenSecret::BootstrapPubkey(arr));
                }
                TAG_ENDPOINT => endpoints.push(decode_endpoint(value)?),
                TAG_FLAGS => {
                    let arr: [u8; 2] = value
                        .try_into()
                        .map_err(|_| WgError::InvalidToken("flags must be 2 bytes".into()))?;
                    flags = BootstrapFlags(u16::from_be_bytes(arr));
                }
                TAG_EXPIRES_AT => {
                    let arr: [u8; 8] = value
                        .try_into()
                        .map_err(|_| WgError::InvalidToken("expires_at must be 8 bytes".into()))?;
                    expires_at = Some(u64::from_be_bytes(arr));
                }
                // Unknown tags are skipped for forward compatibility.
                _ => {}
            }
        }

        let network_id = network_id
            .ok_or_else(|| WgError::InvalidToken("missing network_id field".into()))?;
        let secret = secret.ok_or_else(|| WgError::InvalidToken("missing secret field".into()))?;
        if endpoints.is_empty() {
            return Err(WgError::InvalidToken("token carries no endpoints".into()));
        }
        Ok(Self {
            network_id,
            secret,
            endpoints,
            flags,
            expires_at,
        })
    }
}

/// The two accepted forms of bootstrap input (decision 04).
#[derive(Debug)]
pub enum WgBootstrap {
    /// A fully self-describing `wg1_` token.
    Token(BootstrapToken),
    /// A bare seed; `network_id` + `endpoint(s)` must be supplied via builder/config/env.
    Seed(WgSeed),
}

impl WgBootstrap {
    /// WHY: One front door for both the copy-paste token and the env-injected bare seed.
    ///
    /// WHAT: Parse either a `wg1_` token or a bare 16/32-byte seed (hex or base64url).
    ///
    /// HOW: Dispatches on the `wg1_` prefix; otherwise decodes a bare seed. A bare seed
    /// with no endpoint is a *hard error at connect time*, never a silent default
    /// (enforced by the mesh layer, not here).
    ///
    /// # Errors
    /// [`WgError::InvalidToken`] for a malformed token; [`WgError::InvalidSeedLength`] or
    /// a decode error for a malformed bare seed.
    pub fn parse(input: &str) -> WgResult<Self> {
        let trimmed = input.trim();
        if trimmed.starts_with(TOKEN_PREFIX) {
            return Ok(WgBootstrap::Token(BootstrapToken::parse(trimmed)?));
        }
        Ok(WgBootstrap::Seed(parse_bare_seed(trimmed)?))
    }

    /// The seed carried by this bootstrap input, if any (a seedless token has none).
    #[must_use]
    pub fn seed(&self) -> Option<&WgSeed> {
        match self {
            WgBootstrap::Seed(seed) => Some(seed),
            WgBootstrap::Token(token) => match &token.secret {
                TokenSecret::Seed(seed) => Some(seed),
                TokenSecret::BootstrapPubkey(_) => None,
            },
        }
    }
}

/// Decode a bare seed from hex-32/64 or base64url.
fn parse_bare_seed(s: &str) -> WgResult<WgSeed> {
    let looks_hex = (s.len() == 32 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
    if looks_hex {
        let bytes = hex::decode(s)?;
        return WgSeed::from_bytes(&bytes);
    }
    WgSeed::from_base64url(s)
}

// ---------------------------------------------------------------------------
// TLV + endpoint + CRC + varint helpers
// ---------------------------------------------------------------------------

fn put_tlv(out: &mut Vec<u8>, tag: u8, value: &[u8]) {
    out.push(tag);
    write_varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn encode_endpoint(endpoint: SocketAddr) -> Vec<u8> {
    let mut out = Vec::with_capacity(19);
    match endpoint {
        SocketAddr::V4(v4) => {
            out.push(4);
            out.extend_from_slice(&v4.ip().octets());
            out.extend_from_slice(&v4.port().to_be_bytes());
        }
        SocketAddr::V6(v6) => {
            out.push(6);
            out.extend_from_slice(&v6.ip().octets());
            out.extend_from_slice(&v6.port().to_be_bytes());
        }
    }
    out
}

fn decode_endpoint(value: &[u8]) -> WgResult<SocketAddr> {
    let family = *value
        .first()
        .ok_or_else(|| WgError::InvalidToken("empty endpoint".into()))?;
    match family {
        4 if value.len() == 7 => {
            let ip = Ipv4Addr::new(value[1], value[2], value[3], value[4]);
            let port = u16::from_be_bytes([value[5], value[6]]);
            Ok(SocketAddr::new(IpAddr::V4(ip), port))
        }
        6 if value.len() == 19 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&value[1..17]);
            let port = u16::from_be_bytes([value[17], value[18]]);
            Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port))
        }
        _ => Err(WgError::InvalidToken("malformed endpoint field".into())),
    }
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn read_varint(data: &[u8], pos: &mut usize) -> WgResult<u64> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *data
            .get(*pos)
            .ok_or_else(|| WgError::InvalidToken("truncated varint".into()))?;
        *pos += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err(WgError::InvalidToken("varint overflow".into()));
        }
    }
}

/// CRC32C (Castagnoli) — integrity/typo guard for the token body (not security).
fn crc32c(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0x82F6_3B78
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
