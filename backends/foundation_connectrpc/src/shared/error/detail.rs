//! `ErrorDetail` — a strongly-typed error detail with `google.protobuf.Any`
//! semantics (Decision 03).
//!
//! WHY: Connect errors may carry structured details for cross-language interop
//! (connect-go / connect-es / grpc decode them by `type_url`). Details are
//! codec-independent — even a JSON/Arrow-only service can emit protobuf-`Any`
//! details — so the detail is just `{type_url, value}` plus an optional original
//! JSON for round-trip fidelity.
//!
//! WHAT: [`ErrorDetail`] with codec-agnostic constructors ([`ErrorDetail::from_raw`],
//! [`ErrorDetail::from_errstacks`]) and accessors. The protobuf-typed
//! constructors (`from_message` / `decode`) require the codec system and land
//! with the `proto` feature (spec-41 F13/F26); they are intentionally not part
//! of this error-model feature.
//!
//! HOW: `value` holds the serialized bytes (protobuf for interop details, JSON
//! for the errstacks well-known detail). `type_name` strips the URL prefix.
//!
//! > Note: this is the *protobuf* `google.protobuf.Any` (`{type_url, value}`),
//! > NOT Rust's `std::any::Any` — see Decision 03 ("Two different Any").

use foundation_errstacks::StructuredErrorTrace;

/// The assigned well-known `type_url` for our errstacks rich-diagnostics detail.
/// Standard clients ignore this unknown type; our clients decode the full trace.
pub const ERRSTACKS_TYPE_URL: &str =
    "type.googleapis.com/foundation.errstacks.StructuredErrorTrace";

/// A single structured error detail (`google.protobuf.Any` semantics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDetail {
    /// Fully-qualified type URL, e.g. `"type.googleapis.com/google.rpc.RetryInfo"`.
    type_url: String,
    /// Serialized message bytes (protobuf for interop details; JSON for the
    /// errstacks well-known detail).
    value: Vec<u8>,
    /// Original JSON representation, kept for round-trip fidelity when a detail
    /// arrived from the wire with a `debug` object.
    wire_json: Option<String>,
}

impl ErrorDetail {
    /// Create a detail from a raw type URL and serialized bytes (codec-agnostic).
    #[must_use]
    pub fn from_raw(type_url: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            type_url: type_url.into(),
            value,
            wire_json: None,
        }
    }

    /// Ride an errstacks [`StructuredErrorTrace`] as one well-known detail entry
    /// (Decision 03 §3): our clients decode the full trace; standard clients see
    /// only code + message and ignore this unknown `type_url`. No proto needed —
    /// the value is the trace's JSON.
    ///
    /// # Panics
    /// Never in practice: `StructuredErrorTrace` is plain data and always
    /// serializes; a serialization failure falls back to an empty JSON object.
    #[must_use]
    pub fn from_errstacks(trace: &StructuredErrorTrace) -> Self {
        let json = serde_json::to_string(trace).unwrap_or_else(|_| "{}".to_string());
        Self {
            type_url: ERRSTACKS_TYPE_URL.to_string(),
            value: json.clone().into_bytes(),
            wire_json: Some(json),
        }
    }

    /// The full type URL (with the `type.googleapis.com/` prefix).
    #[must_use]
    pub fn type_url(&self) -> &str {
        &self.type_url
    }

    /// The fully-qualified protobuf message type name (URL prefix stripped),
    /// e.g. `"google.rpc.RetryInfo"`.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self.type_url.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.type_url,
        }
    }

    /// The raw serialized bytes.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// The original wire JSON (`debug` representation), if this detail carried one.
    #[must_use]
    pub fn wire_json(&self) -> Option<&str> {
        self.wire_json.as_deref()
    }

    /// Attach/replace the original wire JSON (round-trip fidelity).
    #[must_use]
    pub fn with_wire_json(mut self, json: impl Into<String>) -> Self {
        self.wire_json = Some(json.into());
        self
    }
}
