//! Compile-fail tests for `#[valtron::task]` signature validation.
//!
//! The macro rejects malformed transitions at compile time with a descriptive
//! `syn::Error`. Because validation fails before any code is generated, these
//! fixtures depend only on `foundation_macros` (not `foundation_core`): the
//! attributed `fn` is replaced by the error, so the `Ctx` / `TaskStatus` names in
//! its signature never need to resolve.

#[test]
fn state_task_compile_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/task_*.rs");
}
