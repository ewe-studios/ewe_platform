//! Integration tests — full build → compile → validate pipeline with scheme builders.

use foundation_jsonschema::scheme;
use serde_json::json;

// ── User profile schema ────────────────────────────────────────

#[test]
fn user_profile_valid() {
    let validator = scheme::object()
        .required("username", scheme::string().min_len(3).max_len(32))
        .required("email", scheme::string().email())
        .required("age", scheme::integer().min(0).max(150))
        .optional("bio", scheme::string().max_len(500))
        .strict()
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({
        "username": "darkvo1d",
        "email": "darkvoid@example.com",
        "age": 28
    })));

    assert!(validator.is_valid(&json!({
        "username": "darkvo1d",
        "email": "darkvoid@example.com",
        "age": 28,
        "bio": "A short bio"
    })));
}

#[test]
fn user_profile_invalid() {
    let validator = scheme::object()
        .required("username", scheme::string().min_len(3).max_len(32))
        .required("email", scheme::string().email())
        .required("age", scheme::integer().min(0).max(150))
        .strict()
        .build()
        .compile()
        .unwrap();

    // username too short
    assert!(!validator.is_valid(&json!({
        "username": "ab",
        "email": "test@example.com",
        "age": 25
    })));

    // age out of range
    assert!(!validator.is_valid(&json!({
        "username": "darkvo1d",
        "email": "test@example.com",
        "age": 200
    })));

    // extra field (strict)
    assert!(!validator.is_valid(&json!({
        "username": "darkvo1d",
        "email": "test@example.com",
        "age": 25,
        "extra": true
    })));

    // missing required
    assert!(!validator.is_valid(&json!({
        "username": "darkvo1d",
        "email": "test@example.com"
    })));
}

// ── Nested object ──────────────────────────────────────────────

#[test]
fn nested_address() {
    let validator = scheme::object()
        .required("name", scheme::string().min_len(1))
        .required("address", scheme::object()
            .required("street", scheme::string())
            .required("city", scheme::string())
            .required("zip", scheme::string().len(5).pattern(r"^\d{5}$"))
            .strict()
        )
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({
        "name": "Alice",
        "address": {
            "street": "123 Main St",
            "city": "Springfield",
            "zip": "62701"
        }
    })));

    // invalid zip length
    assert!(!validator.is_valid(&json!({
        "name": "Alice",
        "address": {
            "street": "123 Main St",
            "city": "Springfield",
            "zip": "1234"
        }
    })));
}

// ── Array of items ─────────────────────────────────────────────

#[test]
fn tag_list_validation() {
    let validator = scheme::object()
        .required("name", scheme::string())
        .required("tags", scheme::array()
            .items(scheme::string().min_len(1).max_len(20))
            .min_items(1)
            .max_items(5)
            .unique()
        )
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({
        "name": "post",
        "tags": ["rust", "json", "schema"]
    })));

    // empty tags (min_items: 1)
    assert!(!validator.is_valid(&json!({
        "name": "post",
        "tags": []
    })));

    // duplicate tags
    assert!(!validator.is_valid(&json!({
        "name": "post",
        "tags": ["rust", "rust"]
    })));

    // too many tags
    assert!(!validator.is_valid(&json!({
        "name": "post",
        "tags": ["a", "b", "c", "d", "e", "f"]
    })));
}

// ── Nullable field ─────────────────────────────────────────────

#[test]
fn nullable_field() {
    let validator = scheme::object()
        .required("name", scheme::string().nullable())
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice"})));
    assert!(validator.is_valid(&json!({"name": null})));
    assert!(!validator.is_valid(&json!({"name": 42})));
}

// ── Optional field via anyOf wrapper ───────────────────────────

#[test]
fn optional_field_via_anyof() {
    let validator = scheme::object()
        .required("name", scheme::string())
        .required("middle_name", scheme::string().optional())
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice", "middle_name": "Marie"})));
    assert!(validator.is_valid(&json!({"name": "Alice", "middle_name": null})));
}

// ── Discriminated union ────────────────────────────────────────

#[test]
fn circle_rectangle_discriminated() {
    let schema = scheme::if_then_else(
        scheme::object()
            .required("kind", scheme::literal(json!("circle")))
            .build_schema(),
        scheme::object()
            .required("kind", scheme::literal(json!("circle")))
            .required("radius", scheme::number().positive())
            .build_schema(),
        scheme::object()
            .required("kind", scheme::literal(json!("rect")))
            .required("width", scheme::number().positive())
            .required("height", scheme::number().positive())
            .build_schema(),
    );

    let validator = scheme::ValidationOptions::new().build(&schema).unwrap();

    // circle with radius
    assert!(validator.is_valid(&json!({"kind": "circle", "radius": 5.0})));
    // rect with width/height
    assert!(validator.is_valid(&json!({"kind": "rect", "width": 10.0, "height": 20.0})));
}

// ── Tuple validation ───────────────────────────────────────────

#[test]
fn coordinate_tuple() {
    let validator = scheme::array()
        .prefix_items(vec![
            scheme::number(),
            scheme::number(),
            scheme::number(),
        ])
        .len(3)
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!([1.0, 2.0, 3.0])));
    assert!(!validator.is_valid(&json!([1.0, 2.0])));
    assert!(!validator.is_valid(&json!([1.0, 2.0, 3.0, 4.0])));
}

// ── Enum values on string ──────────────────────────────────────

#[test]
fn priority_enum() {
    let validator = scheme::object()
        .required("task", scheme::string())
        .required("priority", scheme::string()
            .enum_values(vec!["low".into(), "medium".into(), "high".into()])
        )
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"task": "deploy", "priority": "high"})));
    assert!(!validator.is_valid(&json!({"task": "deploy", "priority": "critical"})));
}

// ── Port number with constraints ───────────────────────────────

#[test]
fn port_number_validation() {
    let validator = scheme::object()
        .required("host", scheme::string())
        .required("port", scheme::integer().min(1).max(65535))
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"host": "localhost", "port": 8080})));
    assert!(!validator.is_valid(&json!({"host": "localhost", "port": 0})));
    assert!(!validator.is_valid(&json!({"host": "localhost", "port": 70000})));
}

// ── build().compile() with custom format ───────────────────────

#[test]
fn build_with_assert_format() {
    let validator = scheme::object()
        .required("uri", scheme::string().uri())
        .build()
        .assert_format(true)
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"uri": "https://example.com"})));
    assert!(!validator.is_valid(&json!({"uri": "not-a-uri"})));
}

// ── Complex nested structure ───────────────────────────────────

#[test]
fn complex_nested() {
    let validator = scheme::object()
        .required("users", scheme::array()
            .items(scheme::object()
                .required("id", scheme::integer().min(1))
                .required("name", scheme::string().min_len(1))
                .optional("email", scheme::string().email())
                .strict()
            )
            .min_items(1)
        )
        .required("metadata", scheme::object()
            .required("total", scheme::integer().min(1))
            .optional("page", scheme::integer().min(1))
        )
        .strict()
        .build()
        .assert_format(true)
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({
        "users": [
            {"id": 1, "name": "Alice", "email": "alice@example.com"},
            {"id": 2, "name": "Bob"}
        ],
        "metadata": {
            "total": 2,
            "page": 1
        }
    })));

    // empty users array (min_items: 1)
    assert!(!validator.is_valid(&json!({
        "users": [],
        "metadata": {"total": 0}
    })));

    // invalid email
    assert!(!validator.is_valid(&json!({
        "users": [{"id": 1, "name": "Alice", "email": "not-email"}],
        "metadata": {"total": 1}
    })));
}

// ── Schema inspector can be reused for multiple validators ─────

#[test]
fn build_schema_for_inspection_and_validation() {
    let schema = scheme::object()
        .required("name", scheme::string())
        .build_schema();

    // Inspect
    assert_eq!(schema["type"], "object");
    assert!(schema["required"].as_array().is_some());

    // Validate using the same schema value
    let v1 = foundation_jsonschema::validator_for(&schema).unwrap();
    let v2 = foundation_jsonschema::validator_for(&schema).unwrap();

    assert!(v1.is_valid(&json!({"name": "test"})));
    assert!(v2.is_valid(&json!({"name": "test"})));
}

// ── Percentage validation (number with range) ──────────────────

#[test]
fn percentage_validation() {
    let validator = scheme::object()
        .required("progress", scheme::number().min(0.0).max(100.0))
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"progress": 0.0})));
    assert!(validator.is_valid(&json!({"progress": 50.5})));
    assert!(validator.is_valid(&json!({"progress": 100.0})));
    assert!(!validator.is_valid(&json!({"progress": -1.0})));
    assert!(!validator.is_valid(&json!({"progress": 101.0})));
}
