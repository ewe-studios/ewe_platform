//! Feature 67: the public fallible API returns an errstacks trace.

use ewe_domain::domains::{self, DomainErrors, DomainResult};
use ewe_domain::pending_chan::{PendingChannelError, PendingChannelsRegistry};

fn failing_operation() -> DomainResult<()> {
    Err(DomainErrors::ProblematicState.into())
}

#[test]
fn domain_error_is_an_errstacks_trace_with_domain_context() {
    let trace = failing_operation().expect_err("the domain is in a bad state");

    assert!(
        matches!(trace.current_context(), DomainErrors::ProblematicState),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn pending_channel_error_carries_attached_context() {
    let mut registry = PendingChannelsRegistry::<u8>::new();
    let trace = match registry.resolve(domains::Id("missing".into())) {
        Err(trace) => trace,
        Ok(_) => panic!("no pending channel exists"),
    }
    .attach("request=42")
    .attach("registry=responses");

    assert!(
        matches!(trace.current_context(), PendingChannelError::NotFound(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("request=42"),
        "request context lost: {rendered}"
    );
    assert!(
        rendered.contains("registry=responses"),
        "registry context lost: {rendered}"
    );
}
