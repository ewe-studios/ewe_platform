#[test]
fn compile_errors() {
    trybuild::TestCases::new().compile_fail("tests/trybuild/action/fail_*.rs");
}
