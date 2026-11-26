//! The bootstrap payload: everything a client needs to connect, in one string.
//!
//! WHY: an operator should hand a CLI, SDK or phone a single base64 string (or
//! show it as a QR code) and the client connects with no manual configuration.
//!
//! WHAT: [`BootstrapPayload`] (the generated `ewe.agent_rpc.v1.BootstrapPayload`:
//! agent address, bootstrap address, server certificate fingerprint, signed
//! bootstrap secret, capabilities), [`encode`]/[`decode`] (protobuf → base64url, unpadded),
//! [`qr_text`]/[`qr_svg`] (the same string as a QR code), and
//! [`EncodedPayload`] (the encoded string with a redacting `Debug`).
//!
//! HOW: protobuf keeps the payload small (a QR code's capacity is the limit)
//! and versioned; base64url keeps it safe in URLs, shells and QR alphanumerics.
//! The `token` field holds the shared form of a
//! [`SignedSecret`](crate::secret::SignedSecret) (base64url JSON: public key,
//! signature, random secret, validity window). The payload is therefore a
//! secret until that window closes or the secret is used: emitted to the
//! operator, never logged, never written to disk by this crate.

use base64::Engine;
use buffa::Message;
use zeroize::Zeroizing;

use crate::error::{AgentRpcError, AgentRpcResult};

pub use crate::proto::v1::BootstrapPayload;

/// Encode `payload` as unpadded base64url protobuf.
#[must_use]
pub fn encode(payload: &BootstrapPayload) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.encode_to_vec())
}

/// Decode a payload produced by [`encode`].
///
/// This is the legacy decoder of feature 01. New code uses
/// [`crate::core::bootstrap::decode`], which also checks the token.
///
/// # Errors
/// [`AgentRpcError::Codec`] for invalid base64, invalid protobuf, or a payload
/// that predates the `url` field.
pub fn decode(encoded: &str) -> AgentRpcResult<BootstrapPayload> {
    let codec = |what: String| -> foundation_errstacks::ErrorTrace<AgentRpcError> {
        AgentRpcError::Codec(format!("bootstrap payload: {what}")).into()
    };
    let bytes = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .map_err(|e| codec(format!("not base64url: {e}")))?,
    );
    let payload = BootstrapPayload::decode_from_slice(&bytes)
        .map_err(|e| codec(format!("not a payload: {e}")))?;
    if payload.url.is_empty() {
        return Err(codec(
            "this payload has no url; payloads made before feature 07 are not accepted, issue a new one"
                .to_string(),
        ));
    }
    if payload.token.is_empty() {
        return Err(codec("missing token".to_string()));
    }
    Ok(payload)
}

/// The QR code of `encoded`, rendered for a terminal (Unicode half blocks,
/// two modules per character row).
///
/// # Errors
/// [`AgentRpcError::Codec`] when the string does not fit in a QR code.
pub fn qr_text(encoded: &str) -> AgentRpcResult<String> {
    use qrcode::render::unicode::Dense1x2;
    Ok(qr(encoded)?
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .quiet_zone(true)
        .build())
}

/// The QR code of `encoded` as an SVG document.
///
/// # Errors
/// [`AgentRpcError::Codec`] when the string does not fit in a QR code.
pub fn qr_svg(encoded: &str) -> AgentRpcResult<String> {
    use qrcode::render::svg;
    Ok(qr(encoded)?
        .render::<svg::Color<'_>>()
        .min_dimensions(256, 256)
        .build())
}

fn qr(encoded: &str) -> AgentRpcResult<qrcode::QrCode> {
    qrcode::QrCode::new(encoded.as_bytes())
        .map_err(|e| AgentRpcError::Codec(format!("bootstrap payload QR code: {e}")).into())
}

/// An encoded payload whose `Debug`/`Display` never show its contents.
#[derive(Clone)]
pub struct EncodedPayload(Zeroizing<String>);

impl EncodedPayload {
    /// Encode `payload`.
    #[must_use]
    pub fn new(payload: &BootstrapPayload) -> Self {
        Self(Zeroizing::new(encode(payload)))
    }

    /// The base64url string — show it to the operator, nowhere else.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for EncodedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EncodedPayload(<redacted>)")
    }
}

impl std::fmt::Display for EncodedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted bootstrap payload>")
    }
}
