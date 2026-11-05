//! Feature 67: the public fallible API returns an errstacks trace.

use std::path::Path;

use foundation_codegen::parser::parse_rust_file;
use foundation_codegen::CodegenError;

#[test]
fn api_error_is_an_errstacks_trace_with_codegen_context() {
    let trace =
        parse_rust_file(Path::new("/nonexistent/codegen_source.rs")).expect_err("missing file");

    assert!(
        matches!(trace.current_context(), CodegenError::Io { .. }),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = parse_rust_file(Path::new("/nonexistent/codegen_source.rs"))
        .expect_err("missing file")
        .attach("crate=demo")
        .attach("phase=scan");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("crate=demo"),
        "crate context lost: {rendered}"
    );
    assert!(
        rendered.contains("phase=scan"),
        "phase context lost: {rendered}"
    );
}
