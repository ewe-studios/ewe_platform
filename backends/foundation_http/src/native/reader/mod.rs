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

use foundation_netio::simple_http::shared::{
    HTTPStreams, HttpReaderError, IncomingRequestParts, SimpleIncomingRequest,
};

use crate::shared::client_ip::ClientIp;

/// Read the next complete request from the stream.
///
/// Returns `None` when the connection is closed (no more data).
/// Returns `Some(Err(_))` on parse errors.
#[must_use]
#[tracing::instrument(skip(streams))]
pub fn read_next_request<T: std::io::Read + Send + 'static>(
    streams: &HTTPStreams<T>,
    client_ip: &str,
) -> Option<Result<SimpleIncomingRequest, HttpReaderError>> {
    let mut request_stream = streams.next_request();

    let mut builder = SimpleIncomingRequest::builder();

    let mut total_skips = 0;
    let mut seen_starter: bool = false;

    tracing::trace!("Getting next request stream");
    for part_result in &mut request_stream {
        let part = match part_result {
            Ok(p) => {
                tracing::trace!("Received OK(IncomingRequestParts): {:?}", &p);
                p
            }
            Err(e) => {
                tracing::trace!("Received Err(Error): {:?}", &e);
                return Some(Err(e));
            }
        };

        tracing::trace!(
            "Received new request part: total_skips={}, seen_starter={}",
            &total_skips,
            &seen_starter
        );
        match part {
            IncomingRequestParts::NoBody => {
                // Keep-alive artifact or no body — defaults are fine
                tracing::trace!("Seen IncomingRequestParts::NoBody");
            }
            IncomingRequestParts::Intro(method, url, proto) => {
                seen_starter = true;
                tracing::trace!(
                    "Seen IncomingRequestParts::Intro({:?}, {:?}, {:?})",
                    &method,
                    &url,
                    &proto
                );
                builder = builder.with_method(method).with_url(url).with_proto(proto);
            }
            IncomingRequestParts::Headers(headers) => {
                tracing::trace!("Seen IncomingRequestParts::Headers({:?})", &headers);
                builder = builder.with_headers(headers);
            }
            IncomingRequestParts::SizedBody(body) => {
                tracing::trace!("Seen IncomingRequestParts::SizedBody(_)");
                builder = builder.with_body(body);
            }
            IncomingRequestParts::StreamedBody(body) => {
                tracing::trace!("Seen IncomingRequestParts::StreamedBody(_)");
                builder = builder.with_some_body(Some(body));
            }
            IncomingRequestParts::SKIP => {
                tracing::trace!("Seen IncomingRequestParts::SKIP(_)");
                // always break on skip because it means we found an extra
                // newline after a request was finished.
                if seen_starter {
                    break;
                }
                // if we see more than 1 skip without seeing a starter then stop
                total_skips += 1;
                if !seen_starter && total_skips >= 2 {
                    break;
                }
                break;
            }
        }
    }

    // if starter is not seen and we stopped then we cant really build a request.
    tracing::trace!(
        "Finished request accumulation loop: total_skips={}, seen_starter={}",
        &total_skips,
        &seen_starter
    );
    if !seen_starter {
        tracing::trace!("No request identified returning None");
        return None;
    }

    // Iterator exhausted — if we got any parts, return the built request.
    // If we got nothing at all, the connection is closed.
    let mut req = match builder.build() {
        Ok(req) => req,
        Err(err) => {
            tracing::error!("Failed to correctly build request due to: {:?}", &err);
            return Some(Err(err.into()));
        }
    };

    // Attach the client IP to request extensions.
    if let Some(extensions) = &mut req.extensions {
        extensions.insert(ClientIp(client_ip.to_string()));
    }

    Some(Ok(req))
}
