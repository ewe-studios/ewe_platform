//! Feature 67: the public fallible API returns an errstacks trace.

use foundation_proxy::config::ProxyConfig;
use foundation_proxy::ProxyError;

#[test]
fn api_error_is_an_errstacks_trace_with_proxy_context() {
    let trace = ProxyConfig::load_file("/nonexistent/proxy.toml").expect_err("missing file fails");

    assert!(
        matches!(trace.current_context(), ProxyError::Config(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = ProxyConfig::load_file("/nonexistent/proxy.toml")
        .expect_err("missing file fails")
        .attach("route=/api")
        .attach("config=proxy.toml");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("route=/api"),
        "route context lost: {rendered}"
    );
    assert!(
        rendered.contains("config=proxy.toml"),
        "config context lost: {rendered}"
    );
}
