//! Check the diagnostics users see when a template cannot compile.

#[test]
fn invalid_templates_report_the_offending_input() {
    trybuild::TestCases::new().compile_fail("tests/trybuild/view/fail_*.rs");
}
