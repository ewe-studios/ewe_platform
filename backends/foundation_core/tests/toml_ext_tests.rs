use foundation_core::extensions::serde_ext::{DynamicValueExt, TomlValueExt};
use toml::toml;

#[test]
fn test_value_can_walk() {
    // -- Setup & Fixtures
    let mut value = toml::Value::Table(toml! {
        token=3

        [hello]
        word = "hello"

        [hello.wreckage]
        where = "londo"
    });

    // -- Exec
    assert!(value.d_walk(|tree, key| {
        assert!(tree.contains_key(key));
        true
    }));
}

#[test]
fn test_value_can_take() {
    // -- Setup & Fixtures
    let mut value = toml::Value::Table(toml! {
        token=3

        [hello]
        word = "hello"
    });

    // -- Exec
    let content: String = value.d_take("/hello/word").unwrap();
    assert_eq!(&content, "hello");

    // Should
    assert!(value.d_get::<String>("hello").is_err());
    assert!(value.d_get::<String>("hello/word").is_err());
}

#[test]
fn test_value_insert_ok() {
    // -- Setup & Fixtures
    let mut value = toml::Value::Table(toml! {
        token=3

        [hello]
        word = "hello"
    });

    let fx_node_value = "hello";

    // -- Exec
    let result = value.d_insert("/happy/word", fx_node_value);
    assert!(matches!(result, Ok(())));

    // -- Check
    let actual_value: String = value.d_get("/happy/word").unwrap();
    assert_eq!(actual_value.as_str(), fx_node_value);
}
