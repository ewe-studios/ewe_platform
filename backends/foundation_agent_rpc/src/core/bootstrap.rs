//! The checked bootstrap payload.
//!
//! WHY: an operator gives a client one base64 string. The client must prove
//! that the string is well formed, that its token is genuine, and which
//! address and access mode it names, before it sends anything.
//!
//! WHAT: [`Bootstrap`] (a checked payload), [`TlsTrust`] (how to check the
//! server certificate) and [`BootstrapAccess`] (how the client gets access).
//!
//! HOW: [`decode`] checks the wire message and returns a Rust value in which
//! each case is a type. It reads no clock and no network: the token's own
//! window and signature are checked, and the caller decides when to use it.

use base64::Engine;
use buffa::Message;
use foundation_auth::secret::{DecodeLimits, SecretPurpose, SignedSecret};
use url::Url;
use zeroize::Zeroizing;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;

/// A checked bootstrap payload. Made by [`decode`]; read by the native client
/// and the web client.
#[derive(Debug, Clone)]
pub struct Bootstrap {
    /// The `AgentService` base URL.
    pub url: Url,
    /// How the client checks the server certificate.
    pub trust: TlsTrust,
    /// How the client gets access.
    pub access: BootstrapAccess,
    /// What the host says it supports.
    pub capabilities: pb::Capabilities,
}

/// How the client checks the server certificate. Derived, not on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsTrust {
    /// `tls_fingerprint_sha256` was set: pin this SHA-256.
    PinnedSha256([u8; 32]),
    /// The fingerprint was empty and the URL is `https`: system roots.
    SystemRoots,
    /// The URL is `http` with a loopback host. Development only.
    Plaintext,
}

/// How the client gets access. Derived from the token purpose.
#[derive(Debug, Clone)]
pub enum BootstrapAccess {
    /// Purpose `Enroll`: call `Enroll` at `enroll_url` with a client key (any
    /// client) or a certificate signing request (CSR) (native, then mutual
    /// Transport Layer Security (mTLS)).
    Enroll {
        /// The signed enrollment secret.
        token: SignedSecret,
        /// The `BootstrapService` base URL.
        enroll_url: Url,
    },
    /// Purpose `Access`: send the token as a bearer credential on each call.
    Bearer {
        /// The signed access secret.
        token: SignedSecret,
    },
}

/// Encode `payload` as unpadded base64url protobuf.
#[must_use]
pub fn encode(payload: &pb::BootstrapPayload) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.encode_to_vec())
}

/// Decode and check a payload produced by [`encode`].
///
/// # Errors
/// [`AgentRpcError::Codec`] for invalid text, invalid protobuf, a missing or
/// bad URL, a bad fingerprint, or a missing, invalid, foreign or expired
/// token.
pub fn decode(encoded: &str) -> AgentRpcResult<Bootstrap> {
    let codec = |what: String| -> foundation_errstacks::ErrorTrace<AgentRpcError> {
        AgentRpcError::Codec(format!("bootstrap payload: {what}")).into()
    };
    let bytes = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .map_err(|error| codec(format!("not base64url: {error}")))?,
    );
    let payload = pb::BootstrapPayload::decode_from_slice(&bytes)
        .map_err(|error| codec(format!("not a payload: {error}")))?;

    if payload.url.is_empty() {
        return Err(codec(
            "this payload has no url; payloads made before feature 07 are not accepted, issue a new one"
                .to_string(),
        ));
    }
    let url = parse_url(&payload.url, "url")?;

    let fingerprint = parse_fingerprint(&payload.tls_fingerprint_sha256)?;
    let trust = match (fingerprint, url.scheme()) {
        (Some(fingerprint), "https") => TlsTrust::PinnedSha256(fingerprint),
        (Some(_), _) => {
            return Err(codec("a pinned fingerprint needs an https url".to_string()));
        }
        (None, "https") => TlsTrust::SystemRoots,
        (None, "http") if is_loopback(&url) => TlsTrust::Plaintext,
        (None, _) => {
            return Err(codec("an http url needs a loopback host".to_string()));
        }
    };

    if payload.token.is_empty() {
        return Err(codec("missing token".to_string()));
    }
    let token = SignedSecret::decode(&payload.token, &DecodeLimits::default())
        .map_err(|error| codec(format!("token: {error}")))?;
    if !token.verify_self_signed() {
        return Err(codec("the token signature is not valid".to_string()));
    }

    let access = match token.purpose() {
        SecretPurpose::Enroll => {
            if !token.audience().is_some_and(|aud| same_url(aud, &url)) {
                return Err(codec(
                    "the token audience is not this payload's url".to_string(),
                ));
            }
            if payload.enroll_url.is_empty() {
                return Err(codec("an enroll token needs an enroll_url".to_string()));
            }
            let enroll_url = parse_url(&payload.enroll_url, "enroll_url")?;
            BootstrapAccess::Enroll { token, enroll_url }
        }
        SecretPurpose::Access => {
            if !payload.enroll_url.is_empty() {
                return Err(codec(
                    "an access token needs an empty enroll_url".to_string(),
                ));
            }
            BootstrapAccess::Bearer { token }
        }
    };

    Ok(Bootstrap {
        url,
        trust,
        access,
        capabilities: payload.capabilities.into_option().unwrap_or_default(),
    })
}

fn parse_url(text: &str, field: &str) -> AgentRpcResult<Url> {
    let url = Url::parse(text)
        .map_err(|error| AgentRpcError::Codec(format!("bootstrap payload: {field}: {error}")))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback(&url) => Ok(url),
        other => Err(AgentRpcError::Codec(format!(
            "bootstrap payload: {field}: scheme {other:?} needs a loopback http host or https"
        ))
        .into()),
    }
}

/// True for an address on the loopback interface. A domain is loopback only
/// when it is exactly `localhost`.
fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// True when the token audience names `url`.
///
/// A URL parser may add a trailing slash to an empty path. The comparison
/// accepts the exact text and the parsed form.
fn same_url(audience: &str, url: &Url) -> bool {
    audience == url.as_str() || Url::parse(audience).is_ok_and(|parsed| parsed == *url)
}

/// Read a lowercase hexadecimal SHA-256 fingerprint.
///
/// An empty field gives `None`. A field of the wrong length or with a byte
/// outside `0-9a-f` is an error.
fn parse_fingerprint(text: &str) -> AgentRpcResult<Option<[u8; 32]>> {
    if text.is_empty() {
        return Ok(None);
    }
    if text.len() != 64 {
        return Err(AgentRpcError::Codec(format!(
            "bootstrap payload: tls_fingerprint_sha256 has {} characters; it needs 64",
            text.len()
        ))
        .into());
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = &text[index * 2..index * 2 + 2];
        if !pair
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(AgentRpcError::Codec(
                "bootstrap payload: tls_fingerprint_sha256 is not lowercase hexadecimal"
                    .to_string(),
            )
            .into());
        }
        *byte = u8::from_str_radix(pair, 16).map_err(|error| {
            AgentRpcError::Codec(format!(
                "bootstrap payload: tls_fingerprint_sha256: {error}"
            ))
        })?;
    }
    Ok(Some(bytes))
}
