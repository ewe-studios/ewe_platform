//! Smoke test moved out of `src/lib.rs` (spec 70 feature 59, rule 7).

use foundation_shell::add;

#[test]
fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
}
