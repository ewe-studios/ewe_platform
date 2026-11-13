//! Feature 67: the public fallible API returns an errstacks trace.
//!
//! The trace keeps the [`WgError`] as its current context and keeps every
//! attached context when it crosses a layer.

use foundation_wireguard::{WgError, WgSeed};

#[test]
fn api_error_is_an_errstacks_trace_with_wg_context() {
    let trace = WgSeed::from_bytes(&[0u8; 7]).expect_err("7-byte seed is rejected");

    assert!(
        matches!(trace.current_context(), WgError::InvalidSeedLength(7)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = WgSeed::from_base64url("not base64 !")
        .expect_err("malformed base64 is rejected")
        .attach("peer=alpha")
        .attach("bootstrap=seed-1");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("peer=alpha"),
        "peer context lost: {rendered}"
    );
    assert!(
        rendered.contains("bootstrap=seed-1"),
        "bootstrap context lost: {rendered}"
    );
    assert!(matches!(
        trace.current_context(),
        WgError::Base64(_) | WgError::InvalidSeedLength(_)
    ));
}
