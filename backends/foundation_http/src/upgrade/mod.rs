//! Protocol upgrade helpers — WebSocket accept.
//!
//! SSE is provided by `foundation_core::wire::event_source::EventWriter`.

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleHeader, SimpleHeaders, SimpleIncomingRequest,
    SimpleOutgoingResponse, SendSafeBody, Status,
};

/// Accept a WebSocket upgrade and return the raw connection stream.
///
/// Writes the 101 Switching Protocols response to the connection.
/// Returns `true` if the upgrade was accepted, `false` if the request
/// was not a valid WebSocket upgrade.
///
/// # Panics
///
/// Panics if the 101 response cannot be built (this should never happen
/// with valid input).
pub fn accept_websocket(
    conn: &mut SharedByteBufferStream<RawStream>,
    req: &SimpleIncomingRequest,
) -> bool {
    let upgrade = get_header_value(&req.headers, "upgrade");
    let connection = get_header_value(&req.headers, "connection");
    let ws_key = get_header_value(&req.headers, "sec-websocket-key");

    if !upgrade.eq_ignore_ascii_case("websocket")
        || !connection.to_lowercase().contains("upgrade")
        || ws_key.is_empty()
    {
        return false;
    }

    let accept_key = {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(ws_key.as_bytes());
        h.update(b"258EAFA5-E914-47DA-95CA-5AB3977F1E39");
        let result = h.finalize();
        STANDARD.encode(result)
    };

    let response = SimpleOutgoingResponse::builder()
        .with_status(Status::SwitchingProtocols)
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .add_header(SimpleHeader::CONNECTION, "Upgrade")
        .add_header(SimpleHeader::custom("Sec-WebSocket-Accept"), accept_key)
        .with_body(SendSafeBody::None)
        .build()
        .expect("valid 101 response");

    match Http11::response(response).http_render_to_writer(conn) {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Failed to write WebSocket upgrade response: {e}");
            false
        }
    }
}

fn get_header_value(headers: &SimpleHeaders, name: &str) -> String {
    for (key, values) in headers {
        let key_name = format!("{key}");
        if key_name.eq_ignore_ascii_case(name) {
            return values.first().cloned().unwrap_or_default();
        }
    }
    String::new()
}
