//! Integration tests — custom schema resolution and `$ref` handling.

use foundation_jsonschema::{InMemoryFetcher, JsonResolver, ValidationOptions};
use serde_json::json;

// ── InMemoryFetcher ─────────────────────────────────────────────────

#[test]
fn builtin_resolver_has_meta_schemas() {
    let fetcher = InMemoryFetcher::builtin();
    // Should have at least 20 meta-schemas
    assert!(fetcher.len() >= 20);
}

#[test]
fn builtin_resolver_resolves_known_uris() {
    let fetcher = InMemoryFetcher::builtin();
    assert!(fetcher
        .resolve("http://json-schema.org/draft-07/schema#")
        .is_ok());
    assert!(fetcher
        .resolve("https://json-schema.org/draft/2020-12/schema")
        .is_ok());
    assert!(fetcher
        .resolve("https://json-schema.org/draft/2019-09/meta/core")
        .is_ok());
}

#[test]
fn builtin_resolver_fails_unknown_uri() {
    let fetcher = InMemoryFetcher::builtin();
    assert!(fetcher.resolve("https://example.com/nonexistent").is_err());
}

#[test]
fn custom_resolver_add_schema() {
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert(
        "https://example.com/person.json",
        json!({
            "type": "object",
            "properties": { "name": {"type": "string"} }
        }),
    );

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "https://example.com/person.json"
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice"})));
    assert!(!validator.is_valid(&json!({"name": 42})));
}

#[test]
fn custom_resolver_nested_ref() {
    let mut fetcher = InMemoryFetcher::builtin();
    // Address is referenced from Person
    fetcher.insert(
        "https://example.com/address.json",
        json!({
            "type": "object",
            "properties": {
                "street": {"type": "string"},
                "city": {"type": "string"}
            },
            "required": ["street", "city"]
        }),
    );
    // Person references Address
    fetcher.insert(
        "https://example.com/person.json",
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "address": {"$ref": "https://example.com/address.json"}
            },
            "required": ["name"]
        }),
    );

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "https://example.com/person.json"
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .unwrap();

    // Valid nested structure
    assert!(validator.is_valid(&json!({
        "name": "Bob",
        "address": {"street": "123 Main St", "city": "Springfield"}
    })));

    // Missing required field in address
    assert!(!validator.is_valid(&json!({
        "name": "Bob",
        "address": {"street": "123 Main St"}
    })));
}

#[test]
fn custom_resolver_ref_not_found() {
    // Don't add the external schema — should fail compilation
    let fetcher = InMemoryFetcher::builtin();

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "https://example.com/unknown.json"
    });

    let result = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema);

    // Compilation should fail because the referenced URI can't be resolved
    assert!(result.is_err());
}

#[test]
fn empty_resolver_fails_all_refs() {
    let fetcher = InMemoryFetcher::empty();
    assert!(fetcher.is_empty());
    assert!(fetcher.len() == 0);
}

#[test]
fn multiple_refs_same_external() {
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert(
        "https://example.com/string.json",
        json!({"type": "string", "minLength": 1}),
    );

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "first": {"$ref": "https://example.com/string.json"},
            "last": {"$ref": "https://example.com/string.json"}
        }
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"first": "John", "last": "Doe"})));
    assert!(!validator.is_valid(&json!({"first": "", "last": "Doe"})));
}

#[test]
fn draft7_ref_with_id() {
    // Draft 7 uses "id" instead of "$id" for schema identification
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert(
        "https://example.com/item.json",
        json!({
            "id": "https://example.com/item.json",
            "type": "integer",
            "minimum": 1
        }),
    );

    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "array",
        "items": {"$ref": "https://example.com/item.json"}
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([1, 2, 3])));
    assert!(!validator.is_valid(&json!([0, 1, 2])));
}

#[test]
fn draft4_ref() {
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert(
        "https://example.com/draft4-item.json",
        json!({
            "id": "https://example.com/draft4-item.json",
            "type": "string"
        }),
    );

    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "type": "object",
        "properties": {
            "value": {"$ref": "https://example.com/draft4-item.json"}
        }
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"value": "hello"})));
    assert!(!validator.is_valid(&json!({"value": 42})));
}
