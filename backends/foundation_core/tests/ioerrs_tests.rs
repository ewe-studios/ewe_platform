use foundation_core::err;

#[test]
fn test() {
    let _ = err!(NotFound, "test");
    let _ = err!(NotFound, "test", "x {} y", 2 + 2);
}
