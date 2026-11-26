#[test]
fn retired_operation_type_is_unavailable() {
    trybuild::TestCases::new().compile_fail("tests/trybuild/fail_dom_op_removed.rs");
}
