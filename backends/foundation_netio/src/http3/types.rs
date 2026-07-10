//! HTTP/3 request/response mapping onto the universal Simple types (F34).
//!
//! WHY: Decision 01 §"Type sufficiency for HTTP/2 & HTTP/3" is explicit — *no new
//! handler-facing types*. A handler cannot tell which transport carried its
//! request, and that is the entire point of the shared `Router`.
//!
//! WHAT: [`request_from_fields`] turns a decoded QPACK field section into the same
//! [`SimpleIncomingRequestHeader`] HTTP/2 produces; [`response_to_fields`] turns a
//! [`SimpleOutgoingResponse`] back into the field list a `HEADERS` frame carries.
//!
//! HOW: HTTP/3 and HTTP/2 use the *same* pseudo-headers (`:method`, `:scheme`,
//! `:authority`, `:path`, `:status`) — RFC 9114 §4.3 defers to RFC 9113 §8.3. Only
//! the compression differs (QPACK vs HPACK), and both decode to `(name, value)`
//! byte pairs. So this module reuses [`crate::http2::types::header_from_hpack`]
//! rather than duplicating the pseudo-header folding, URI composition and routing
//! key derivation.
//!
//! ## What HTTP/3 forbids that HTTP/1.1 allowed
//!
//! RFC 9114 §4.2 bans the connection-specific header fields outright:
//! `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`, `Upgrade`,
//! and `TE` with any value other than `trailers`. Receiving one is a malformed
//! request. Sending one is our bug — QUIC has no hop-by-hop framing for them to
//! describe, and a proxy that forwarded them would be smuggling.
//!
//! Header names must be lowercase (§4.1.1). An uppercase name is malformed; we
//! lowercase on the way out and reject on the way in.

use std::sync::Arc;

use bytes::Bytes;

use foundation_core::url::InvalidUri;

use crate::http2::types::{header_from_hpack, SimpleIncomingRequestHeader};
use crate::netcap::context::ConnectionContext;
use crate::simple_http::shared::SimpleOutgoingResponse;

/// Why a field section cannot become a request.
#[derive(Debug)]
pub enum MalformedRequest {
    /// A connection-specific field that RFC 9114 §4.2 forbids.
    ConnectionSpecificField(String),
    /// `TE` carried a value other than `trailers`.
    BadTe(String),
    /// A header name contained an uppercase character (§4.1.1).
    UppercaseFieldName(String),
    /// A mandatory pseudo-header was missing.
    MissingPseudoHeader(&'static str),
    /// A pseudo-header appeared after a regular field (§4.3).
    PseudoHeaderAfterField(String),
    /// `:scheme`/`:authority`/`:path` did not compose into a valid URI.
    BadUri(InvalidUri),
}

impl std::fmt::Display for MalformedRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionSpecificField(n) => write!(
                f,
                "HTTP/3 forbids the connection-specific field {n:?} (RFC 9114 §4.2)"
            ),
            Self::BadTe(v) => write!(f, "TE may only carry \"trailers\" in HTTP/3, got {v:?}"),
            Self::UppercaseFieldName(n) => {
                write!(f, "HTTP/3 field names must be lowercase, got {n:?}")
            }
            Self::MissingPseudoHeader(p) => write!(f, "request is missing the {p} pseudo-header"),
            Self::PseudoHeaderAfterField(p) => {
                write!(f, "pseudo-header {p:?} appeared after a regular field")
            }
            Self::BadUri(e) => write!(f, "request pseudo-headers do not compose into a URI: {e}"),
        }
    }
}

impl std::error::Error for MalformedRequest {}

impl From<InvalidUri> for MalformedRequest {
    fn from(e: InvalidUri) -> Self {
        Self::BadUri(e)
    }
}

/// Fields HTTP/3 forbids outright (RFC 9114 §4.2).
const FORBIDDEN: &[&[u8]] = &[
    b"connection",
    b"keep-alive",
    b"proxy-connection",
    b"transfer-encoding",
    b"upgrade",
];

/// WHY: a request that reaches a handler must already be well-formed, or every
/// handler has to re-validate. Enforcing §4.2/§4.1.1/§4.3 here means the router
/// never sees a malformed request.
///
/// WHAT: validate a decoded QPACK field section and turn it into the same
/// [`SimpleIncomingRequestHeader`] the HTTP/2 path produces.
///
/// HOW: reject the connection-specific fields, uppercase names, and pseudo-headers
/// that follow a regular field; then hand the pairs to the shared
/// [`header_from_hpack`], because HTTP/3 and HTTP/2 share their pseudo-headers.
///
/// # Errors
/// [`MalformedRequest`] for any §4.2/§4.1.1/§4.3 violation, or if the
/// pseudo-headers do not compose into a URI. The caller answers with
/// `H3_MESSAGE_ERROR` on the request stream.
///
/// # Panics
/// Never panics.
pub fn request_from_fields(
    fields: &[(Bytes, Bytes)],
    connection: Arc<ConnectionContext>,
) -> Result<SimpleIncomingRequestHeader, MalformedRequest> {
    let mut seen_regular_field = false;
    let mut has_method = false;
    let mut has_scheme = false;
    let mut has_path = false;

    for (name, value) in fields {
        let is_pseudo = name.first() == Some(&b':');

        if is_pseudo {
            // RFC 9114 §4.3: all pseudo-headers precede the regular fields.
            if seen_regular_field {
                return Err(MalformedRequest::PseudoHeaderAfterField(
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
            match &name[..] {
                b":method" => has_method = true,
                b":scheme" => has_scheme = true,
                b":path" => has_path = true,
                _ => {}
            }
            continue;
        }

        seen_regular_field = true;

        if name.iter().any(u8::is_ascii_uppercase) {
            return Err(MalformedRequest::UppercaseFieldName(
                String::from_utf8_lossy(name).into_owned(),
            ));
        }
        if FORBIDDEN.contains(&&name[..]) {
            return Err(MalformedRequest::ConnectionSpecificField(
                String::from_utf8_lossy(name).into_owned(),
            ));
        }
        // §4.2: TE is allowed, but only with the value "trailers".
        if &name[..] == b"te" && &value[..] != b"trailers" {
            return Err(MalformedRequest::BadTe(String::from_utf8_lossy(value).into_owned()));
        }
    }

    // §4.3.1: a request must carry :method, :scheme and :path. `:authority` is
    // optional (a `Host` field may substitute), so it is not required here.
    if !has_method {
        return Err(MalformedRequest::MissingPseudoHeader(":method"));
    }
    if !has_scheme {
        return Err(MalformedRequest::MissingPseudoHeader(":scheme"));
    }
    if !has_path {
        return Err(MalformedRequest::MissingPseudoHeader(":path"));
    }

    // HTTP/3 and HTTP/2 share their pseudo-headers verbatim (RFC 9114 §4.3 defers
    // to RFC 9113 §8.3), so the folding, URI composition and routing-key
    // derivation are the same code.
    Ok(header_from_hpack(fields, connection)?)
}

/// WHY: a `HEADERS` frame carries a field section, and a handler produced a
/// `SimpleOutgoingResponse`. This is the bridge.
///
/// WHAT: the `(name, value)` pairs to QPACK-encode, `:status` first.
///
/// HOW: RFC 9114 §4.1.1 requires lowercase field names, so names are lowercased
/// here rather than trusting the handler. The connection-specific fields §4.2
/// forbids are dropped rather than sent: emitting them would make our response
/// malformed, and a handler that sets `Connection: keep-alive` out of HTTP/1.1
/// habit should not take the connection down.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn response_to_fields(response: &SimpleOutgoingResponse) -> Vec<(Bytes, Bytes)> {
    let mut fields = Vec::with_capacity(response.headers.len() + 1);

    // §4.3.2: the response pseudo-header, and it comes first. `:status` is the
    // bare three-digit code — `Status`'s `Display` renders "200 OK", which would
    // be a malformed pseudo-header.
    fields.push((
        Bytes::from_static(b":status"),
        Bytes::from(response.status.clone().into_usize().to_string()),
    ));

    for (name, values) in &response.headers {
        let lowered = name.to_string().to_ascii_lowercase();

        if FORBIDDEN.contains(&lowered.as_bytes()) {
            tracing::debug!(
                field = %lowered,
                "dropping connection-specific header from an HTTP/3 response (RFC 9114 §4.2)"
            );
            continue;
        }

        for value in values {
            fields.push((
                Bytes::from(lowered.clone()),
                Bytes::from(value.clone()),
            ));
        }
    }

    fields
}
