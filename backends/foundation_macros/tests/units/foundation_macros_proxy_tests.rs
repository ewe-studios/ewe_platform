//! Compile-fail tests for the `proxy!` macro (Decision 19).
//!
//! These test macro-expansion-time errors: missing required fields, unknown keys,
//! wrong SSL provider names. Expansion fails before the generated code is
//! type-checked, so no `foundation_proxy` dependency is needed here.

#[test]
fn proxy_compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/proxy_*.rs");
}
