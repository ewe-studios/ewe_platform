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

use crate::http2::frame::ErrorCode;
use crate::netcap::context::ConnectionContext;
use crate::simple_http::shared::{SimpleHeaders, SimpleMethod};

/// Decoded HTTP/2 HEADERS frame — everything except the body.
#[derive(Debug, Clone)]
pub struct SimpleIncomingRequestHeader {
    pub method: SimpleMethod,
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub headers: SimpleHeaders,
    pub connection: Arc<ConnectionContext>,
}

/// Build a [`SimpleIncomingRequestHeader`] from decoded HPACK (name, value) pairs.
#[must_use]
pub fn header_from_hpack(
    pairs: &[(Bytes, Bytes)],
    connection: Arc<ConnectionContext>,
) -> SimpleIncomingRequestHeader {
    let mut method = SimpleMethod::POST;
    let mut scheme = String::from("http");
    let mut authority = String::new();
    let mut path = String::from("/");
    let mut headers = SimpleHeaders::new();

    for (name, value) in pairs {
        match name.as_ref() {
            b":method" => {
                let s = String::from_utf8_lossy(value).to_uppercase();
                method = if s == "GET" { SimpleMethod::GET } else { SimpleMethod::POST };
            }
            b":scheme" => scheme = String::from_utf8_lossy(value).into_owned(),
            b":authority" => authority = String::from_utf8_lossy(value).into_owned(),
            b":path" => path = String::from_utf8_lossy(value).into_owned(),
            b":status" => {} // response pseudo-header — ignore
            _ => {
                let k = crate::simple_http::shared::SimpleHeader::from(
                    String::from_utf8_lossy(name).to_string(),
                );
                headers.entry(k).or_default().push(String::from_utf8_lossy(value).to_string());
            }
        }
    }

    SimpleIncomingRequestHeader { method, scheme, authority, path, headers, connection }
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
