//! Request reader — consumes `HttpRequestReader` iterator to build
//! `SimpleIncomingRequest` from per-request parts.
//!
//! WHY: The `HTTPStreams` wrapper is created once per connection in the
//! server's accept loop. Each call to `.next_request()` returns a fresh
//! `HttpRequestReader` iterator that reads the next complete HTTP request
//! from the shared byte buffer stream.
//!
//! WHAT: `read_next_request` takes the streams handle and a mutable reference
//! to the request reader iterator. It consumes parts until the iterator is
//! exhausted, building the request along the way.

use foundation_core::wire::simple_http::{
    IncomingRequestParts, SimpleIncomingRequest, HTTPStreams, HttpReaderError,
};

use crate::client_ip::ClientIp;

/// Read the next complete request from the stream.
///
/// Returns `None` when the connection is closed (no more data).
/// Returns `Some(Err(_))` on parse errors.
#[must_use]
pub fn read_next_request<T: std::io::Read + Send + 'static>(
    streams: &HTTPStreams<T>,
    client_ip: &str,
) -> Option<Result<SimpleIncomingRequest, HttpReaderError>> {
    let mut request_stream = streams.next_request();

    let mut builder = SimpleIncomingRequest::builder();

    for part_result in &mut request_stream {
        let part = match part_result {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };

        match part {
            IncomingRequestParts::SKIP
            | IncomingRequestParts::NoBody => {
                // Keep-alive artifact or no body — defaults are fine
            }
            IncomingRequestParts::Intro(method, url, proto) => {
                builder = builder.with_method(method).with_url(url).with_proto(proto);
            }
            IncomingRequestParts::Headers(headers) => {
                builder = builder.with_headers(headers);
            }
            IncomingRequestParts::SizedBody(body) => {
                builder = builder.with_body(body);
            }
            IncomingRequestParts::StreamedBody(body) => {
                builder = builder.with_some_body(Some(body));
            }
        }
    }

    // Iterator exhausted — if we got any parts, return the built request.
    // If we got nothing at all, the connection is closed.
    let mut req = builder.build().ok()?;

    // Attach the client IP to request extensions.
    if let Some(extensions) = &mut req.extensions {
        extensions.insert(ClientIp(client_ip.to_string()));
    }

    Some(Ok(req))
}
