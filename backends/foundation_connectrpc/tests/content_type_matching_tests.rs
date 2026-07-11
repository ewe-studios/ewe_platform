//! Content-type → protocol classification.
//!
//! WHY: `application/grpc` is a *string prefix* of `application/grpc-web+proto`.
//! While each protocol matched its content-type with a bare `strip_prefix`, the
//! gRPC handler claimed every gRPC-Web request. Handlers are consulted in
//! registration order (`[Connect, Grpc, GrpcWeb]`) and the first `can_handle`
//! wins, so the gRPC-Web handler was never offered them. The request was then
//! judged against gRPC's capabilities — which require HTTP/2 — and every
//! HTTP/1.1 gRPC-Web call was refused with `505 HTTP Version Not Supported`.
//!
//! WHAT: These lock the full matrix. A match must end at the subtype boundary,
//! so no family can swallow a longer one, and the classification cannot depend
//! on the order handlers happen to be registered in.
//!
//! HOW: Assert on the parsers directly rather than through a live server: this
//! is a claim about the matching rule, and a server test would only reach it
//! after transport, routing and capability checks had all agreed.

use foundation_connectrpc::shared::protocol::{grpc, grpc_web};

// ── gRPC ─────────────────────────────────────────────────────────────────────

/// Valid gRPC content-types resolve to their codec, defaulting to `proto`.
#[test]
fn grpc_accepts_its_own_content_types() {
    assert_eq!(grpc::parse_content_type("application/grpc").as_deref(), Some("proto"));
    assert_eq!(
        grpc::parse_content_type("application/grpc+proto").as_deref(),
        Some("proto")
    );
    assert_eq!(
        grpc::parse_content_type("application/grpc+json").as_deref(),
        Some("json")
    );
}

/// The regression this file exists for: gRPC must not claim gRPC-Web.
#[test]
fn grpc_rejects_grpc_web_content_types() {
    for content_type in [
        "application/grpc-web",
        "application/grpc-web+proto",
        "application/grpc-web+json",
        "application/grpc-web-text",
        "application/grpc-web-text+proto",
    ] {
        assert_eq!(
            grpc::parse_content_type(content_type),
            None,
            "{content_type} is gRPC-Web, not gRPC"
        );
    }
}

/// Any other subtype that merely starts with `application/grpc` is refused.
#[test]
fn grpc_rejects_unrelated_and_malformed_subtypes() {
    for content_type in [
        "application/grpcfoo",
        "application/grpc-",
        "application/grpc+", // names no codec
        "application/json",
        "application/proto",
        "application/connect+json",
        "text/plain",
        "",
    ] {
        assert_eq!(
            grpc::parse_content_type(content_type),
            None,
            "{content_type} must not parse as gRPC"
        );
    }
}

// ── gRPC-Web ─────────────────────────────────────────────────────────────────

/// gRPC-Web resolves both its binary and its text framing, with the codec.
#[test]
fn grpc_web_accepts_its_own_content_types() {
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web"),
        Some(("proto".to_string(), false))
    );
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web+proto"),
        Some(("proto".to_string(), false))
    );
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web+json"),
        Some(("json".to_string(), false))
    );
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web-text"),
        Some(("proto".to_string(), true))
    );
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web-text+proto"),
        Some(("proto".to_string(), true))
    );
}

/// `application/grpc-web` is itself a prefix of `application/grpc-web-text`.
/// The binary variant must not swallow the text one.
#[test]
fn grpc_web_binary_does_not_swallow_text() {
    let (codec, is_text) =
        grpc_web::parse_content_type("application/grpc-web-text+json").expect("is gRPC-Web text");
    assert_eq!(codec, "json");
    assert!(is_text, "the -text subtype must be recognised as text framing");
}

/// gRPC-Web must not claim plain gRPC either — the confusion runs both ways.
#[test]
fn grpc_web_rejects_grpc_and_unrelated_content_types() {
    for content_type in [
        "application/grpc",
        "application/grpc+proto",
        "application/grpc-webfoo",
        "application/grpc-web+", // names no codec
        "application/json",
        "",
    ] {
        assert_eq!(
            grpc_web::parse_content_type(content_type),
            None,
            "{content_type} must not parse as gRPC-Web"
        );
    }
}

// ── Shared canonicalization ──────────────────────────────────────────────────

/// Parameters and casing are canonicalized away before matching, for both
/// families. A browser sending `; charset=utf-8` still routes correctly.
#[test]
fn matching_is_case_insensitive_and_ignores_parameters() {
    assert_eq!(
        grpc::parse_content_type("APPLICATION/GRPC+PROTO").as_deref(),
        Some("proto")
    );
    assert_eq!(
        grpc::parse_content_type("application/grpc+json; charset=utf-8").as_deref(),
        Some("json")
    );
    assert_eq!(
        grpc::parse_content_type("  application/grpc  ").as_deref(),
        Some("proto")
    );

    assert_eq!(
        grpc_web::parse_content_type("Application/gRPC-Web-Text+Proto"),
        Some(("proto".to_string(), true))
    );
    assert_eq!(
        grpc_web::parse_content_type("application/grpc-web+proto; charset=utf-8"),
        Some(("proto".to_string(), false))
    );

    // Canonicalization must not resurrect the prefix bug.
    assert_eq!(
        grpc::parse_content_type("APPLICATION/GRPC-WEB+PROTO"),
        None,
        "case-folding must not make gRPC-Web look like gRPC"
    );
}

/// The two families partition the `application/grpc*` space: no content-type is
/// ever claimed by both, which is what makes handler order irrelevant.
#[test]
fn grpc_and_grpc_web_never_both_claim_a_content_type() {
    for content_type in [
        "application/grpc",
        "application/grpc+proto",
        "application/grpc+json",
        "application/grpc-web",
        "application/grpc-web+proto",
        "application/grpc-web-text",
        "application/grpc-web-text+json",
        "application/grpcfoo",
        "application/json",
    ] {
        let as_grpc = grpc::parse_content_type(content_type).is_some();
        let as_grpc_web = grpc_web::parse_content_type(content_type).is_some();
        assert!(
            !(as_grpc && as_grpc_web),
            "{content_type} was claimed by both gRPC and gRPC-Web"
        );
    }
}
