use foundation_core::extensions::serde_ext::{DynamicValueExt, JsonValueExt};
use serde_json::json;

#[test]
fn test_value_can_walk() {
    // -- Setup & Fixtures
    let mut value = json!({"tokens": 3, "hello": {"word": "hello"}});

    // -- Exec
    assert!(value.d_walk(|tree, key| {
        assert!(tree.contains_key(key));
        true
    }));
}

#[test]
fn test_value_insert_ok() {
    // -- Setup & Fixtures
    let mut value = json!({"tokens": 3});
    let fx_node_value = "hello";

    // -- Exec
    value.d_insert("/happy/word", fx_node_value).unwrap();

    // -- Check
    let actual_value: String = value.d_get("/happy/word").unwrap();
    dbg!(&actual_value);

    assert_eq!(actual_value.as_str(), fx_node_value);
}

#[test]
fn test_value_can_take() {
    // -- Setup & Fixtures
    let mut value = json!({"tokens": 3, "hello": {"word": "hello"}});

    // -- Exec
    let content: String = value.d_take("/hello/word").unwrap();
    assert_eq!(&content, "hello");

    // Should
    assert!(value.d_get::<String>("hello").is_err());
    assert!(value.d_get::<String>("hello/word").is_err());
}
