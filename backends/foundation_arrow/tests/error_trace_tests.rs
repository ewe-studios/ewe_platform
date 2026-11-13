//! Feature 67: the public fallible API returns an errstacks trace.

use foundation_arrow::{decode_ipc, ArrowIpcError};

#[test]
fn decode_error_is_an_errstacks_trace_with_arrow_context() {
    let trace = decode_ipc(b"this is not an arrow IPC file").expect_err("garbage is rejected");

    assert!(
        matches!(trace.current_context(), ArrowIpcError::Arrow(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = decode_ipc(b"this is not an arrow IPC file")
        .expect_err("garbage is rejected")
        .attach("topic=dom_ops")
        .attach("codec=arrow");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("topic=dom_ops"),
        "topic context lost: {rendered}"
    );
    assert!(
        rendered.contains("codec=arrow"),
        "codec context lost: {rendered}"
    );
}
