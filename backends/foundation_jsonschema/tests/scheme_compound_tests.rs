//! Unit tests — compound schema builders (union, one_of, all_of, not, if_then).

use foundation_jsonschema::scheme;
use serde_json::json;

// ── any_of / union ─────────────────────────────────────────────

#[test]
fn any_of_produces_anyof_keyword() {
    let schema = scheme::any_of(vec![
        scheme::string().build_schema(),
        scheme::integer().build_schema(),
    ]);
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "anyOf");
    assert_eq!(schema["anyOf"].as_array().unwrap().len(), 2);
}

#[test]
fn any_of_validates() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::any_of(vec![
        scheme::string().build_schema(),
        scheme::integer().build_schema(),
    ]);
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!(true)));
}

#[test]
fn union_is_alias_for_any_of() {
    let union_schema = scheme::union(vec![
        scheme::string().build_schema(),
        scheme::null().build_schema(),
    ]);
    let any_of_schema = scheme::any_of(vec![
        scheme::string().build_schema(),
        scheme::null().build_schema(),
    ]);
    assert_eq!(union_schema, any_of_schema);
}

// ── one_of ─────────────────────────────────────────────────────

#[test]
fn one_of_produces_oneof_keyword() {
    let schema = scheme::one_of(vec![
        scheme::string().build_schema(),
        scheme::integer().build_schema(),
    ]);
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "oneOf");
}

#[test]
fn one_of_validates_exactly_one() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::one_of(vec![
        scheme::string().build_schema(),       // matches strings
        scheme::integer().build_schema(),      // matches integers
    ]);

    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello"))); // matches string only
    assert!(v.is_valid(&json!(42)));      // matches integer only
    assert!(!v.is_valid(&json!(true)));   // matches neither
}

// ── all_of / intersection ──────────────────────────────────────

#[test]
fn all_of_produces_allof_keyword() {
    let schema = scheme::all_of(vec![
        scheme::object().required("a", scheme::string()).build_schema(),
        scheme::object().required("b", scheme::integer()).build_schema(),
    ]);
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "allOf");
    assert_eq!(schema["allOf"].as_array().unwrap().len(), 2);
}

#[test]
fn all_of_validates_all() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::all_of(vec![
        scheme::object().required("a", scheme::string()).build_schema(),
        scheme::object().required("b", scheme::integer()).build_schema(),
    ]);
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"a": "hi", "b": 1})));
    assert!(!v.is_valid(&json!({"a": "hi"})));
    assert!(!v.is_valid(&json!({"b": 1})));
}

#[test]
fn intersection_is_shorthand_for_all_of_two() {
    let left = scheme::string().build_schema();
    let right = scheme::object().required("a", scheme::string()).build_schema();

    let intersection_schema = scheme::intersection(left.clone(), right.clone());
    let all_of_schema = scheme::all_of(vec![left, right]);

    assert_eq!(intersection_schema, all_of_schema);
}

// ── not ────────────────────────────────────────────────────────

#[test]
fn not_produces_not_keyword() {
    let schema = scheme::not(scheme::null());
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "not");
}

#[test]
fn not_validates_inverted() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::not(scheme::null());
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!(null)));
}

#[test]
fn not_with_builder() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::not(scheme::string());
    let v = validator_for(&schema).unwrap();
    assert!(!v.is_valid(&json!("hello")));
    assert!(v.is_valid(&json!(42)));
}

// ── if/then ────────────────────────────────────────────────────

#[test]
fn if_then_produces_correct_schema() {
    let schema = scheme::if_then(
        scheme::object().required("type", scheme::literal(json!("circle"))).build_schema(),
        scheme::object().required("radius", scheme::number()).build_schema(),
    );
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "if");
    assert!(schema.get("then").is_some());
    assert!(schema.get("else").is_none());
}

#[test]
fn if_then_validates_conditional() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::if_then(
        scheme::object().required("role", scheme::literal(json!("admin"))).build_schema(),
        scheme::object().required("permissions", scheme::array().items(scheme::string())).build_schema(),
    );
    let v = validator_for(&schema).unwrap();
    // non-admin: no permissions required
    assert!(v.is_valid(&json!({"role": "user"})));
    // admin with permissions: valid
    assert!(v.is_valid(&json!({"role": "admin", "permissions": ["read", "write"]})));
}

// ── if/then/else ──────────────────────────────────────────────

#[test]
fn if_then_else_produces_correct_schema() {
    let schema = scheme::if_then_else(
        scheme::object().required("kind", scheme::literal(json!("a"))).build_schema(),
        scheme::object().required("x", scheme::integer()).build_schema(),
        scheme::object().required("y", scheme::string()).build_schema(),
    );
    assert!(schema.get("if").is_some());
    assert!(schema.get("then").is_some());
    assert!(schema.get("else").is_some());
}

#[test]
fn if_then_else_validates() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::if_then_else(
        scheme::object().required("active", scheme::literal(json!(true))).build_schema(),
        scheme::object().required("active", scheme::literal(json!(true))).required("since", scheme::string()).build_schema(),
        scheme::object().required("active", scheme::literal(json!(false))).build_schema(),
    );
    let v = validator_for(&schema).unwrap();
    // active true with since: valid
    assert!(v.is_valid(&json!({"active": true, "since": "2024-01-01"})));
    // active false: valid (else branch)
    assert!(v.is_valid(&json!({"active": false})));
}

// ── literal / r#enum ───────────────────────────────────────────

#[test]
fn literal_produces_const() {
    let schema = scheme::literal(json!("hello"));
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "const");
    assert_eq!(schema["const"], "hello");
}

#[test]
fn literal_validates() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::literal(json!("exact"));
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("exact")));
    assert!(!v.is_valid(&json!("not exact")));
}

#[test]
fn r#enum_produces_enum_keyword() {
    let schema = scheme::r#enum(vec![json!("red"), json!("green"), json!("blue")]);
    assert_eq!(schema.as_object().unwrap().keys().next().unwrap(), "enum");
    assert_eq!(schema["enum"].as_array().unwrap().len(), 3);
}

#[test]
fn r#enum_validates() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::r#enum(vec![json!("low"), json!("medium"), json!("high")]);
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("low")));
    assert!(v.is_valid(&json!("high")));
    assert!(!v.is_valid(&json!("critical")));
}

// ── ref / dynamic_ref ─────────────────────────────────────────

#[test]
fn ref_produces_ref_keyword() {
    let schema = scheme::ref_("#/$defs/user");
    assert_eq!(schema["$ref"], "#/$defs/user");
}

#[test]
fn dynamic_ref_produces_dynamic_ref_keyword() {
    let schema = scheme::dynamic_ref("#node");
    assert_eq!(schema["$dynamicRef"], "#node");
}
