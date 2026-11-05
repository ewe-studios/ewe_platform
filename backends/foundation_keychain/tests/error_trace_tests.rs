//! Feature 67: the public fallible API returns an errstacks trace.

use foundation_keychain::{AppError, AppResult};

fn forbidden() -> AppResult<()> {
    Err(AppError::Forbidden.into())
}

fn storage_failure() -> AppResult<()> {
    Err(AppError::Internal("storage failed".into()).into())
}

#[test]
fn api_error_is_an_errstacks_trace_with_keychain_context() {
    let trace = forbidden().expect_err("the handler rejects the request");

    assert!(
        matches!(trace.current_context(), AppError::Forbidden),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = storage_failure()
        .expect_err("the handler fails")
        .attach("route=/api/sync")
        .attach("user=42");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("route=/api/sync"),
        "route context lost: {rendered}"
    );
    assert!(
        rendered.contains("user=42"),
        "user context lost: {rendered}"
    );
}
