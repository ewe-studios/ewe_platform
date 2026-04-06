#[test]
fn json_hash_compile_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/pass/json_hash_basic.rs");
    t.compile_fail("tests/trybuild/fail/json_hash_missing_serialize.rs");
}
