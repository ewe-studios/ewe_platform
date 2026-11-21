//! Build-time checks of `embed_migrations!`.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

#[test]
fn embed_migrations_rejects_a_gap() {
    // The path in the test file is relative to the workspace root. The macro
    // walks up from the scratch crate that trybuild makes to find it.
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/embed_migrations_gap.rs");
}
