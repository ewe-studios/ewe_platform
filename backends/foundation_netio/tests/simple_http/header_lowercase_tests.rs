//! Feature 06 (Decision 12 §4): lowercase wire rendering of known `SimpleHeader`
//! variants + first-class gRPC/Connect protocol header variants, with
//! case-insensitive inbound parsing preserved (RFC 9110).

use foundation_netio::simple_http::shared::*;

/// Known variants render canonical lowercase on the wire (HTTP/2 mandates it).
#[test]
fn known_variants_render_lowercase() {
    assert_eq!(SimpleHeader::CONTENT_TYPE.to_string(), "content-type");
    assert_eq!(SimpleHeader::CONTENT_LENGTH.to_string(), "content-length");
    assert_eq!(
        SimpleHeader::CONTENT_ENCODING.to_string(),
        "content-encoding"
    );
    assert_eq!(
        SimpleHeader::TRANSFER_ENCODING.to_string(),
        "transfer-encoding"
    );
    assert_eq!(SimpleHeader::SET_COOKIE.to_string(), "set-cookie");
    assert_eq!(
        SimpleHeader::WWW_AUTHENTICATE.to_string(),
        "www-authenticate"
    );
    assert_eq!(
        SimpleHeader::X_CONTENT_TYPE_OPTIONS.to_string(),
        "x-content-type-options"
    );
    assert_eq!(SimpleHeader::TE.to_string(), "te");
}

/// The gRPC / Connect protocol headers are first-class lowercase variants.
#[test]
fn protocol_variants_render_expected_tokens() {
    assert_eq!(SimpleHeader::GRPC_STATUS.to_string(), "grpc-status");
    assert_eq!(SimpleHeader::GRPC_MESSAGE.to_string(), "grpc-message");
    assert_eq!(SimpleHeader::GRPC_ENCODING.to_string(), "grpc-encoding");
    assert_eq!(
        SimpleHeader::GRPC_ACCEPT_ENCODING.to_string(),
        "grpc-accept-encoding"
    );
    assert_eq!(SimpleHeader::GRPC_TIMEOUT.to_string(), "grpc-timeout");
    assert_eq!(
        SimpleHeader::GRPC_STATUS_DETAILS_BIN.to_string(),
        "grpc-status-details-bin"
    );
    assert_eq!(
        SimpleHeader::CONNECT_PROTOCOL_VERSION.to_string(),
        "connect-protocol-version"
    );
    assert_eq!(
        SimpleHeader::CONNECT_TIMEOUT_MS.to_string(),
        "connect-timeout-ms"
    );
    assert_eq!(
        SimpleHeader::CONNECT_CONTENT_ENCODING.to_string(),
        "connect-content-encoding"
    );
    assert_eq!(
        SimpleHeader::CONNECT_ACCEPT_ENCODING.to_string(),
        "connect-accept-encoding"
    );
}

/// `Custom` headers keep the caller's exact casing.
#[test]
fn custom_headers_preserve_case() {
    assert_eq!(
        SimpleHeader::Custom("X-Villa".into()).to_string(),
        "X-Villa"
    );
    assert_eq!(SimpleHeader::custom("MixedCase").to_string(), "MixedCase");
}

/// Inbound parsing is case-insensitive and maps to the known variants (so a
/// round-trip from any casing renders canonical lowercase).
#[test]
fn inbound_parsing_is_case_insensitive() {
    for spelling in [
        "content-type",
        "Content-Type",
        "CONTENT-TYPE",
        "cOnTeNt-TyPe",
    ] {
        assert_eq!(
            SimpleHeader::from(spelling.to_string()),
            SimpleHeader::CONTENT_TYPE
        );
    }
    // Protocol headers parse case-insensitively too.
    for spelling in ["grpc-status", "GRPC-STATUS", "Grpc-Status"] {
        assert_eq!(
            SimpleHeader::from(spelling.to_string()),
            SimpleHeader::GRPC_STATUS
        );
    }
    assert_eq!(
        SimpleHeader::from("Connect-Protocol-Version".to_string()),
        SimpleHeader::CONNECT_PROTOCOL_VERSION
    );
    // Any casing round-trips to canonical lowercase on the wire.
    assert_eq!(
        SimpleHeader::from("Content-Encoding".to_string()).to_string(),
        "content-encoding"
    );
}

/// A rendered response head uses lowercase known-header names.
#[test]
fn rendered_response_uses_lowercase_headers() {
    let response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .add_header(SimpleHeader::CONTENT_LENGTH, "0")
        .build()
        .expect("build");

    let wire = Http11::response(response)
        .http_render_string()
        .expect("render");

    assert!(
        wire.contains("content-type: application/json\r\n"),
        "{wire:?}"
    );
    assert!(wire.contains("content-length: 0\r\n"), "{wire:?}");
    assert!(
        !wire.contains("CONTENT-TYPE"),
        "no uppercase leak: {wire:?}"
    );
}
