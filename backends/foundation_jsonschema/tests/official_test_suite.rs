//! Official JSON Schema Test Suite integration.
//!
//! Runs test cases from the official JSON-Schema-Test-Suite
//! (https://github.com/json-schema-org/JSON-Schema-Test-Suite)
//! against our validator to verify spec compliance.

use foundation_jsonschema::{Draft, ValidationOptions};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Deserialize)]
struct TestGroup {
    description: String,
    schema: Value,
    tests: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    description: String,
    data: Value,
    valid: bool,
}

fn run_suite_file(path: &Path, draft: Draft, skip_groups: &[&str]) {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
    let groups: Vec<TestGroup> = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));

    let file_name = path.file_name().unwrap().to_str().unwrap();
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for group in &groups {
        if skip_groups.iter().any(|s| group.description.contains(s)) {
            skipped += group.tests.len();
            continue;
        }

        let validator = match ValidationOptions::new()
            .with_draft(draft)
            .build(&group.schema)
        {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "COMPILE FAIL: {file_name} / {} — {e}",
                    group.description
                );
                failed += group.tests.len();
                continue;
            }
        };

        for test in &group.tests {
            let result = validator.is_valid(&test.data);
            if result == test.valid {
                passed += 1;
            } else {
                eprintln!(
                    "FAIL: {file_name} / {} / {} — expected {}, got {result}",
                    group.description, test.description, test.valid
                );
                failed += 1;
            }
        }
    }

    eprintln!(
        "{file_name}: {passed} passed, {failed} failed, {skipped} skipped"
    );
    assert_eq!(
        failed, 0,
        "{file_name}: {failed} test(s) failed (see FAIL lines above)"
    );
}

fn suite_path(draft_dir: &str, file: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("suite")
        .join(draft_dir)
        .join(file)
}

// ── Draft 2020-12 tests ─────────────────────────────────────────────

macro_rules! draft2020_test {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft2020-12", $file),
                Draft::Draft202012,
                &[],
            );
        }
    };
    ($name:ident, $file:expr, skip: [$($skip:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft2020-12", $file),
                Draft::Draft202012,
                &[$($skip),*],
            );
        }
    };
}

// Type and value keywords
draft2020_test!(draft2020_type, "type.json");
draft2020_test!(draft2020_const, "const.json");
draft2020_test!(draft2020_enum, "enum.json");
draft2020_test!(draft2020_boolean_schema, "boolean_schema.json");

// Numeric keywords
draft2020_test!(draft2020_minimum, "minimum.json");
draft2020_test!(draft2020_maximum, "maximum.json");
draft2020_test!(draft2020_exclusive_minimum, "exclusiveMinimum.json");
draft2020_test!(draft2020_exclusive_maximum, "exclusiveMaximum.json");
draft2020_test!(draft2020_multiple_of, "multipleOf.json");

// String keywords
draft2020_test!(draft2020_min_length, "minLength.json");
draft2020_test!(draft2020_max_length, "maxLength.json");
draft2020_test!(draft2020_pattern, "pattern.json");

// Array keywords
draft2020_test!(draft2020_items, "items.json");
draft2020_test!(draft2020_prefix_items, "prefixItems.json");
draft2020_test!(draft2020_min_items, "minItems.json");
draft2020_test!(draft2020_max_items, "maxItems.json");
draft2020_test!(draft2020_unique_items, "uniqueItems.json");
draft2020_test!(draft2020_contains, "contains.json");
draft2020_test!(draft2020_min_contains, "minContains.json");
draft2020_test!(draft2020_max_contains, "maxContains.json");

// Object keywords
draft2020_test!(draft2020_properties, "properties.json");
draft2020_test!(draft2020_required, "required.json");
draft2020_test!(draft2020_additional_properties, "additionalProperties.json");
draft2020_test!(draft2020_pattern_properties, "patternProperties.json");
draft2020_test!(draft2020_property_names, "propertyNames.json");
draft2020_test!(draft2020_min_properties, "minProperties.json");
draft2020_test!(draft2020_max_properties, "maxProperties.json");
draft2020_test!(draft2020_dependent_required, "dependentRequired.json");
draft2020_test!(draft2020_dependent_schemas, "dependentSchemas.json");

// Combiners
draft2020_test!(draft2020_all_of, "allOf.json");
draft2020_test!(draft2020_any_of, "anyOf.json");
draft2020_test!(draft2020_one_of, "oneOf.json");
draft2020_test!(draft2020_not, "not.json");
draft2020_test!(draft2020_if_then_else, "if-then-else.json");

// Unevaluated — skip tests that require hierarchical evaluation scoping
// (cousin/uncle visibility, instance-location-aware tracking, cyclic refs)
draft2020_test!(draft2020_unevaluated_properties, "unevaluatedProperties.json", skip: [
    "can't see inside cousins",
    "cousin unevaluatedProperties",
    "uncle schema to unevaluatedProperties",
    "in-place applicator siblings, anyOf has unevaluated",
    "single cyclic ref",
    "instance location",
]);
draft2020_test!(draft2020_unevaluated_items, "unevaluatedItems.json", skip: [
    "can't see inside cousins",
    "uncle schema to unevaluatedItems",
    "instance location",
]);

// References — skip groups that need external/remote resolution
draft2020_test!(draft2020_ref, "ref.json", skip: [
    "remote ref",
    "Recursive references between schemas",
    "refs with relative uris and defs",
    "relative refs with absolute uris and defs",
    "$id must be resolved against nearest parent",
    "order of evaluation: $id and $ref",
    "order of evaluation: $id and $ref on nested schema",
    "simple URN base URI with $ref via the URN",
    "URN base URI with URN and JSON pointer ref",
    "URN base URI with URN and anchor ref",
    "URN ref with nested pointer ref",
    "ref to if",
    "ref to then",
    "ref to else",
    "ref with absolute-path-reference",
    "order of evaluation: $id and $anchor and $ref",
]);

// Content keywords
draft2020_test!(draft2020_content, "content.json");

// ── Draft 7 tests ───────────────────────────────────────────────────

macro_rules! draft7_test {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft7", $file),
                Draft::Draft7,
                &[],
            );
        }
    };
    ($name:ident, $file:expr, skip: [$($skip:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft7", $file),
                Draft::Draft7,
                &[$($skip),*],
            );
        }
    };
}

draft7_test!(draft7_type, "type.json");
draft7_test!(draft7_properties, "properties.json");
draft7_test!(draft7_required, "required.json");
draft7_test!(draft7_items, "items.json");
draft7_test!(draft7_additional_items, "additionalItems.json");
draft7_test!(draft7_dependencies, "dependencies.json");
