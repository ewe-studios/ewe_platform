//! Feature 67: the public fallible API returns an errstacks trace.

use ewe_watch_utils::{watch_path, WatchUtilsError};

fn missing_path_error() -> foundation_errstacks::ErrorTrace<WatchUtilsError> {
    match watch_path(
        10,
        vec!["/nonexistent/ewe-67-watch".to_string()],
        false,
        |_, _, _| Ok(()),
    ) {
        Ok(_) => panic!("a missing path cannot be watched"),
        Err(trace) => trace,
    }
}

#[test]
fn api_error_is_an_errstacks_trace_with_watch_context() {
    let trace = missing_path_error();

    assert!(
        matches!(trace.current_context(), WatchUtilsError::Notify(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = missing_path_error()
        .attach("watch=src")
        .attach("recursive=false");

    let rendered = format!("{trace:?}");
    assert!(rendered.contains("watch=src"), "watch context lost: {rendered}");
    assert!(
        rendered.contains("recursive=false"),
        "recursive context lost: {rendered}"
    );
}
