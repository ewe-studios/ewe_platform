//! HTTP/2 stream-level types — shared between server and client (F47).
//!
//! WHY: HTTP/2 splits a request into HEADERS + body frames. [`SimpleIncomingRequestHeader`]
//! holds the decoded HEADERS; [`H2IncomingFrame`] streams the body lazily.
//! The response direction uses [`H2Frame`] — handlers push output into a pipe;
//! the connection handler serializes to the shared write buffer.
//!
//! WHAT: Three public types plus [`header_from_hpack`] for building
//! a header from decoded HPACK (name, value) pairs.
//!
//! HOW: Pure data types — no I/O, no state.

use std::sync::Arc;

use bytes::Bytes;

use foundation_core::url::{InvalidUri, Uri};

use crate::http2::frame::ErrorCode;
use crate::netcap::context::ConnectionContext;
use crate::simple_http::shared::{SimpleHeaders, SimpleMethod, SimpleUrl};

/// Decoded HTTP/2 HEADERS frame — everything except the body.
///
/// `uri` and `url` are computed once from `:scheme`, `:authority`, `:path`
/// so callers never need to reconstruct them.
#[derive(Debug, Clone)]
pub struct SimpleIncomingRequestHeader {
    pub method: SimpleMethod,
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub headers: SimpleHeaders,
    pub connection: Arc<ConnectionContext>,
    /// The fully-qualified request URI (absolute when `:authority` is present).
    pub uri: Uri,
    /// The origin-form request URL (path + query) — the routing key.
    pub url: SimpleUrl,
}

/// Build a [`SimpleIncomingRequestHeader`] from decoded HPACK (name, value) pairs.
///
/// The `:scheme`, `:authority` and `:path` pseudo-headers are folded into a
/// single [`Uri`], which in turn backs `url`. A request that omits `:authority`
/// (legal for h2c origin-form) yields a path-only `Uri`.
///
/// # Errors
///
/// Returns the offending URI string if `:scheme`/`:authority`/`:path` do not
/// compose into a parsable URI. Callers should answer with a stream
/// `PROTOCOL_ERROR` — there is no meaningful fallback request to serve.
pub fn header_from_hpack(
    pairs: &[(Bytes, Bytes)],
    connection: Arc<ConnectionContext>,
) -> Result<SimpleIncomingRequestHeader, InvalidUri> {
    let mut method = SimpleMethod::POST;
    let mut scheme = String::from("http");
    let mut authority = String::new();
    let mut path = String::from("/");
    let mut headers = SimpleHeaders::new();

    for (name, value) in pairs {
        // Match on an explicit byte slice: `as_ref()` can resolve ambiguously and
        // unify the scrutinee with the first arm's array type (`&[u8; 7]`), which
        // then rejects the shorter arms such as `b":path"`.
        match &name[..] {
            b":method" => {
                let s = String::from_utf8_lossy(value).to_uppercase();
                method = if s == "GET" {
                    SimpleMethod::GET
                } else {
                    SimpleMethod::POST
                };
            }
            b":scheme" => scheme = String::from_utf8_lossy(value).into_owned(),
            b":authority" => authority = String::from_utf8_lossy(value).into_owned(),
            b":path" => path = String::from_utf8_lossy(value).into_owned(),
            b":status" => {} // response pseudo-header — ignore
            _ => {
                let k = crate::simple_http::shared::SimpleHeader::from(
                    String::from_utf8_lossy(name).to_string(),
                );
                headers
                    .entry(k)
                    .or_default()
                    .push(String::from_utf8_lossy(value).to_string());
            }
        }
    }

    // `uri` is absolute when `:authority` is present; origin-form requests stay
    // path-only, which `Uri::parse` accepts for a leading '/'.
    let uri = if authority.is_empty() {
        Uri::parse(&path)?
    } else {
        Uri::parse(&format!("{scheme}://{authority}{path}"))?
    };
    // `url` stays origin-form (path + query): it is the routing key, and routers
    // match on paths, not absolute URLs. `:path` already carries the query.
    let url = SimpleUrl::url_with_query(path.clone());

    Ok(SimpleIncomingRequestHeader {
        method,
        scheme,
        authority,
        path,
        headers,
        connection,
        uri,
        url,
    })
}

/// One frame after HEADERS on an incoming h2 stream.
#[derive(Debug, Clone)]
pub enum H2IncomingFrame {
    Data(Bytes),
    Reset(ErrorCode),
}

/// One frame the handler pushes into the per-stream response pipe.
#[derive(Debug, Clone)]
pub enum H2Frame {
    Headers {
        status: u16,
        headers: Vec<(Bytes, Bytes)>,
        end_stream: bool,
    },
    Data {
        payload: Bytes,
        end_stream: bool,
    },
    Reset {
        error_code: ErrorCode,
    },
}
