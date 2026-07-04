//! Connect-protocol JSON wire shapes for errors (Decision 03).
//!
//! WHY: Connect protocol errors are always JSON on the wire, regardless of the
//! RPC codec, and the streaming terminator (`EndStreamResponse`) embeds the same
//! error shape. These `serde` structs match the Connect spec byte-for-byte.
//!
//! WHAT: [`WireError`], [`WireErrorDetail`], [`EndStreamResponse`], plus
//! conversions between [`WireErrorDetail`] and the internal [`ErrorDetail`].
//!
//! HOW: Detail `value` is base64 with the standard alphabet and **no padding**
//! (Connect's `RawStdEncoding`); the JSON `type` is the message type name (URL
//! prefix stripped).

use std::collections::HashMap;

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::detail::ErrorDetail;

/// The Connect JSON error body: `{"code","message"?,"details"?}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireError {
    /// Lowercase code name (e.g. `"unavailable"`).
    pub code: String,
    /// Human-readable message, omitted when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Structured details, omitted when there are none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<WireErrorDetail>>,
}

/// One error detail on the wire: `{"type","value","debug"?}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireErrorDetail {
    /// Message type name, e.g. `"google.rpc.RetryInfo"` (no URL prefix).
    #[serde(rename = "type")]
    pub type_url: String,
    /// Base64-encoded serialized bytes (standard alphabet, unpadded).
    pub value: String,
    /// Optional human-readable debug object, preserved for round-trips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<serde_json::Value>,
}

impl WireErrorDetail {
    /// Render an internal [`ErrorDetail`] into its wire form: the JSON `type` is
    /// the type name (URL prefix stripped), `value` is unpadded base64, and any
    /// stored wire JSON becomes the `debug` object.
    #[must_use]
    pub fn from_detail(detail: &ErrorDetail) -> Self {
        let debug = detail
            .wire_json()
            .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok());
        Self {
            type_url: detail.type_name().to_string(),
            value: STANDARD_NO_PAD.encode(detail.value()),
            debug,
        }
    }

    /// Decode this wire detail into an internal [`ErrorDetail`]. The type name is
    /// re-expanded to a full `type.googleapis.com/…` URL; invalid base64 yields
    /// an empty value rather than failing (details are best-effort diagnostics).
    #[must_use]
    pub fn to_detail(&self) -> ErrorDetail {
        let value = STANDARD_NO_PAD.decode(&self.value).unwrap_or_default();
        let type_url = if self.type_url.contains('/') {
            self.type_url.clone()
        } else {
            format!("type.googleapis.com/{}", self.type_url)
        };
        let detail = ErrorDetail::from_raw(type_url, value);
        match &self.debug {
            Some(dbg) => detail.with_wire_json(dbg.to_string()),
            None => detail,
        }
    }
}

/// The final message of a Connect streaming response: an optional error plus
/// trailing metadata (`{"error"?,"metadata"?}`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EndStreamResponse {
    /// The terminal error, if the stream ended in failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
    /// Trailing metadata (response trailers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Vec<String>>>,
}
