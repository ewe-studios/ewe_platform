#[test]
fn wasm_entrypoint_compile_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/pass/*.rs");
    t.compile_fail("tests/trybuild/fail/*.rs");
}
