//! Compile-fail tests for `#[docker_container]` (spec-53 F02, Decision 08).
//!
//! These test macro-expansion-time errors: a body with no `ContainerGroup`
//! parameter (it could not reach its containers), and unknown attribute keys.
//! Expansion fails before the generated code is type-checked, so no
//! `foundation_deployment_platform` dependency is needed here.

#[test]
fn docker_container_compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/docker_container_*.rs");
}
