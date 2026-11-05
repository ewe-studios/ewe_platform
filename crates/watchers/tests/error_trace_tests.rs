//! Feature 67: the public fallible API returns an errstacks trace.

use ewe_watchers::config::{Config, ConfigError};

#[test]
fn api_error_is_an_errstacks_trace_with_config_context() {
    let trace = Config::json("this is not json").expect_err("invalid JSON is rejected");

    assert!(
        matches!(trace.current_context(), ConfigError::BadConfiguration(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = Config::json("this is not json")
        .expect_err("invalid JSON is rejected")
        .attach("file=watchers.json")
        .attach("source=cli");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("file=watchers.json"),
        "file context lost: {rendered}"
    );
    assert!(
        rendered.contains("source=cli"),
        "source context lost: {rendered}"
    );
}
