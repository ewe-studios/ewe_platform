//! A source scan of the client: invariant I1 (no untyped JSON and no
//! hand-written request body).

#[test]
fn no_untyped_json() {
    let source = include_str!("../../src/native/firecracker/client.rs");
    assert!(
        !source.contains("serde_json::Value"),
        "client.rs uses serde_json::Value; every body must use a generated type"
    );
    for line in source.lines() {
        let trimmed = line.trim_start();
        let declared = trimmed
            .strip_prefix("pub struct ")
            .or_else(|| trimmed.strip_prefix("struct "));
        if let Some(name) = declared {
            assert!(
                !(name.starts_with("Request") || name.starts_with("Body")),
                "client.rs declares a hand-written request body type: {line}"
            );
        }
    }
}
