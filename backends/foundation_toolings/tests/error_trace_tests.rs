//! Feature 67: the public fallible API returns an errstacks trace.

use std::path::PathBuf;

use foundation_toolings::watcher::FileChange;
use foundation_toolings::{CargoBuilder, ProjectBuilder, ToolingError};

fn builder() -> CargoBuilder {
    CargoBuilder {
        // A missing directory makes the `cargo` spawn fail at once.
        workspace_root: "/nonexistent/toolings-workspace".to_string(),
        crate_name: "demo".to_string(),
        build_args: vec![],
        skip_check: true,
    }
}

#[test]
fn api_error_is_an_errstacks_trace_with_tooling_context() {
    let change = FileChange::Rust(PathBuf::from("src/lib.rs"));
    let trace = builder().build(&change).expect_err("the spawn fails");

    assert!(
        matches!(trace.current_context(), ToolingError::Io(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let change = FileChange::Rust(PathBuf::from("src/lib.rs"));
    let trace = builder()
        .build(&change)
        .expect_err("the spawn fails")
        .attach("project=demo")
        .attach("builder=cargo");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("project=demo"),
        "project context lost: {rendered}"
    );
    assert!(
        rendered.contains("builder=cargo"),
        "builder context lost: {rendered}"
    );
}
