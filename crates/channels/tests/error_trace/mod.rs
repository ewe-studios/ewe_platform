//! Feature 67: the public fallible API returns an errstacks trace.

use ewe_channels::mspc::{create, ChannelError};

#[test]
fn api_error_is_an_errstacks_trace_with_channel_context() {
    let (mut sender, mut receiver) = create::<String>();
    sender.close().expect("close the sender");
    let trace = receiver.try_receive().expect_err("the channel is closed");

    assert!(
        matches!(trace.current_context(), ChannelError::Closed),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let (mut sender, mut receiver) = create::<String>();
    sender.close().expect("close the sender");
    let trace = receiver
        .try_receive()
        .expect_err("the channel is closed")
        .attach("subscriber=12")
        .attach("topic=events");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("subscriber=12"),
        "subscriber context lost: {rendered}"
    );
    assert!(
        rendered.contains("topic=events"),
        "topic context lost: {rendered}"
    );
}
