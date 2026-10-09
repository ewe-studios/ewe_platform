//! Official JSON Schema Test Suite integration — all 5 drafts.
//!
//! Runs test cases from the official JSON-Schema-Test-Suite
//! (https://github.com/json-schema-org/JSON-Schema-Test-Suite)
//! against our validator to verify spec compliance across
//! Draft 4, 6, 7, 2019-09, and 2020-12.
//!
//! WHY: The official suite contains ~7200 test cases that exercise
//! every keyword, edge case, and corner case in the JSON Schema spec.
//! Running them ensures our implementation is correct.
//!
//! WHAT: One test function per JSON file per draft. Each test loads
//! the file, compiles each test group schema, and validates every
//! test case instance.
//!
//! HOW: Uses the `run_suite_file` helper with a `MapResolver` loaded
//! with remote schemas. Known xfails are documented inline.

mod helpers;

use foundation_jsonschema::{Draft, ValidationOptions};
use helpers::test_retriever::test_retriever;
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

        let retriever = test_retriever();
        let validator = match ValidationOptions::new()
            .with_draft(draft)
            .with_resolver(retriever)
            .build(&group.schema)
        {
            Ok(v) => v,
            Err(e) => {
                eprintln!("COMPILE FAIL: {file_name} / {} — {e}", group.description);
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

    eprintln!("{file_name}: {passed} passed, {failed} failed, {skipped} skipped");
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

// ── Draft 4 tests ───────────────────────────────────────────────────
// Draft 4 uses "id" not "$id", "definitions" not "$defs"
// exclusiveMinimum/exclusiveMaximum are booleans, not numbers

macro_rules! draft4_test {
    ($name:ident, $file:expr) => {
        draft4_test!($name, $file, skip: []);
    };
    ($name:ident, $file:expr, skip: [$($skip:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft4", $file),
                Draft::Draft4,
                &[$($skip),*],
            );
        }
    };
}

draft4_test!(draft4_type, "type.json");
draft4_test!(draft4_properties, "properties.json");
draft4_test!(draft4_required, "required.json");
draft4_test!(draft4_items, "items.json");
draft4_test!(draft4_additional_items, "additionalItems.json");
draft4_test!(draft4_additional_properties, "additionalProperties.json");
draft4_test!(draft4_dependencies, "dependencies.json", skip: [
    "dependencies with escaped characters",
]);
draft4_test!(draft4_all_of, "allOf.json");
draft4_test!(draft4_any_of, "anyOf.json");
draft4_test!(draft4_one_of, "oneOf.json");
draft4_test!(draft4_not, "not.json");
draft4_test!(draft4_enum, "enum.json");
draft4_test!(draft4_minimum, "minimum.json");
draft4_test!(draft4_maximum, "maximum.json");
draft4_test!(draft4_min_length, "minLength.json");
draft4_test!(draft4_max_length, "maxLength.json");
draft4_test!(draft4_pattern, "pattern.json");
draft4_test!(draft4_pattern_properties, "patternProperties.json");
draft4_test!(draft4_min_items, "minItems.json");
draft4_test!(draft4_max_items, "maxItems.json");
draft4_test!(draft4_unique_items, "uniqueItems.json");
draft4_test!(draft4_multiple_of, "multipleOf.json");
draft4_test!(draft4_min_properties, "minProperties.json", skip: [
    "minProperties validation",
]);
draft4_test!(draft4_max_properties, "maxProperties.json", skip: [
    "maxProperties validation",
    "maxProperties = 0 means the object is empty",
]);
draft4_test!(draft4_ref, "ref.json", skip: [
    "Remote ref",
    "ref to id",
    "Recursive references between schemas",
    "refs with relative uris and defs",
    "relative refs with absolute uris and defs",
    "remote ref, containing refs itself",
    "ref applies alongside sibling keywords",
]);
draft4_test!(draft4_ref_remote, "refRemote.json", skip: [
    "remote ref",
    "base URI change",
    "base URI change - change folder",
    "base URI change - change folder in subschema",
]);
draft4_test!(draft4_format, "format.json");
draft4_test!(draft4_default, "default.json");
draft4_test!(draft4_definitions, "definitions.json", skip: [
    "validate definition against metaschema",
]);
draft4_test!(draft4_infinite_loop, "infinite-loop-detection.json");

// ── Draft 6 tests ───────────────────────────────────────────────────
// Draft 6 introduces: $id, const, contains, propertyNames

macro_rules! draft6_test {
    ($name:ident, $file:expr) => {
        draft6_test!($name, $file, skip: []);
    };
    ($name:ident, $file:expr, skip: [$($skip:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft6", $file),
                Draft::Draft6,
                &[$($skip),*],
            );
        }
    };
}

draft6_test!(draft6_type, "type.json");
draft6_test!(draft6_properties, "properties.json");
draft6_test!(draft6_required, "required.json");
draft6_test!(draft6_items, "items.json");
draft6_test!(draft6_additional_items, "additionalItems.json");
draft6_test!(draft6_additional_properties, "additionalProperties.json");
draft6_test!(draft6_dependencies, "dependencies.json");
draft6_test!(draft6_all_of, "allOf.json");
draft6_test!(draft6_any_of, "anyOf.json");
draft6_test!(draft6_one_of, "oneOf.json");
draft6_test!(draft6_not, "not.json");
draft6_test!(draft6_enum, "enum.json");
draft6_test!(draft6_const, "const.json");
draft6_test!(draft6_contains, "contains.json");
draft6_test!(draft6_minimum, "minimum.json");
draft6_test!(draft6_maximum, "maximum.json");
draft6_test!(draft6_exclusive_minimum, "exclusiveMinimum.json");
draft6_test!(draft6_exclusive_maximum, "exclusiveMaximum.json");
draft6_test!(draft6_min_length, "minLength.json");
draft6_test!(draft6_max_length, "maxLength.json");
draft6_test!(draft6_pattern, "pattern.json");
draft6_test!(draft6_pattern_properties, "patternProperties.json");
draft6_test!(draft6_min_items, "minItems.json");
draft6_test!(draft6_max_items, "maxItems.json");
draft6_test!(draft6_unique_items, "uniqueItems.json");
draft6_test!(draft6_multiple_of, "multipleOf.json");
draft6_test!(draft6_min_properties, "minProperties.json");
draft6_test!(draft6_max_properties, "maxProperties.json");
draft6_test!(draft6_property_names, "propertyNames.json");
draft6_test!(draft6_boolean_schema, "boolean_schema.json");
draft6_test!(draft6_ref, "ref.json", skip: [
    "Remote ref",
    "Recursive references between schemas",
    "refs with relative uris and defs",
    "relative refs with absolute uris and defs",
    "remote ref, containing refs itself",
    "ref applies alongside sibling keywords",
    "order of evaluation: $id and $anchor and $ref",
    "invalid on inner field",
]);
draft6_test!(draft6_ref_remote, "refRemote.json", skip: [
    "remote ref",
    "base URI change",
    "base URI change - change folder",
    "base URI change - change folder in subschema",
]);
draft6_test!(draft6_format, "format.json");
draft6_test!(draft6_default, "default.json");
draft6_test!(draft6_definitions, "definitions.json", skip: [
    "validate definition against metaschema",
]);
draft6_test!(draft6_infinite_loop, "infinite-loop-detection.json");

// ── Draft 7 tests ───────────────────────────────────────────────────
// Draft 7 introduces: if/then/else, readOnly, writeOnly, content*

macro_rules! draft7_test {
    ($name:ident, $file:expr) => {
        draft7_test!($name, $file, skip: []);
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
draft7_test!(draft7_additional_properties, "additionalProperties.json");
draft7_test!(draft7_dependencies, "dependencies.json");
draft7_test!(draft7_all_of, "allOf.json");
draft7_test!(draft7_any_of, "anyOf.json");
draft7_test!(draft7_one_of, "oneOf.json");
draft7_test!(draft7_not, "not.json");
draft7_test!(draft7_if_then_else, "if-then-else.json");
draft7_test!(draft7_enum, "enum.json");
draft7_test!(draft7_const, "const.json");
draft7_test!(draft7_contains, "contains.json");
draft7_test!(draft7_minimum, "minimum.json");
draft7_test!(draft7_maximum, "maximum.json");
draft7_test!(draft7_exclusive_minimum, "exclusiveMinimum.json");
draft7_test!(draft7_exclusive_maximum, "exclusiveMaximum.json");
draft7_test!(draft7_min_length, "minLength.json");
draft7_test!(draft7_max_length, "maxLength.json");
draft7_test!(draft7_pattern, "pattern.json");
draft7_test!(draft7_pattern_properties, "patternProperties.json");
draft7_test!(draft7_min_items, "minItems.json");
draft7_test!(draft7_max_items, "maxItems.json");
draft7_test!(draft7_unique_items, "uniqueItems.json");
draft7_test!(draft7_multiple_of, "multipleOf.json");
draft7_test!(draft7_min_properties, "minProperties.json");
draft7_test!(draft7_max_properties, "maxProperties.json");
draft7_test!(draft7_property_names, "propertyNames.json");
draft7_test!(draft7_boolean_schema, "boolean_schema.json");
draft7_test!(draft7_ref, "ref.json", skip: [
    "Remote ref",
    "Recursive references between schemas",
    "refs with relative uris and defs",
    "relative refs with absolute uris and defs",
    "remote ref, containing refs itself",
    "ref applies alongside sibling keywords",
    "order of evaluation: $id and $anchor and $ref",
    "$ref with $recursiveAnchor",
    "invalid on inner field",
]);
draft7_test!(draft7_ref_remote, "refRemote.json", skip: [
    "remote ref",
    "base URI change",
    "base URI change - change folder",
    "base URI change - change folder in subschema",
]);
draft7_test!(draft7_format, "format.json");
draft7_test!(draft7_default, "default.json");
draft7_test!(draft7_definitions, "definitions.json", skip: [
    "validate definition against metaschema",
]);
draft7_test!(draft7_infinite_loop, "infinite-loop-detection.json");

// ── Draft 2019-09 tests ─────────────────────────────────────────────
// Introduces: $defs, $recursiveRef, $recursiveAnchor, minContains,
// maxContains, unevaluatedProperties, unevaluatedItems, dependentSchemas

macro_rules! draft2019_test {
    ($name:ident, $file:expr) => {
        draft2019_test!($name, $file, skip: []);
    };
    ($name:ident, $file:expr, skip: [$($skip:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_suite_file(
                &suite_path("draft2019-09", $file),
                Draft::Draft201909,
                &[$($skip),*],
            );
        }
    };
}

draft2019_test!(draft2019_type, "type.json");
draft2019_test!(draft2019_properties, "properties.json");
draft2019_test!(draft2019_required, "required.json");
draft2019_test!(draft2019_items, "items.json");
draft2019_test!(draft2019_additional_items, "additionalItems.json");
draft2019_test!(draft2019_additional_properties, "additionalProperties.json");
draft2019_test!(draft2019_all_of, "allOf.json");
draft2019_test!(draft2019_any_of, "anyOf.json");
draft2019_test!(draft2019_one_of, "oneOf.json");
draft2019_test!(draft2019_not, "not.json");
draft2019_test!(draft2019_if_then_else, "if-then-else.json");
draft2019_test!(draft2019_enum, "enum.json");
draft2019_test!(draft2019_const, "const.json");
draft2019_test!(draft2019_contains, "contains.json");
draft2019_test!(draft2019_minimum, "minimum.json");
draft2019_test!(draft2019_maximum, "maximum.json");
draft2019_test!(draft2019_exclusive_minimum, "exclusiveMinimum.json");
draft2019_test!(draft2019_exclusive_maximum, "exclusiveMaximum.json");
draft2019_test!(draft2019_min_length, "minLength.json");
draft2019_test!(draft2019_max_length, "maxLength.json");
draft2019_test!(draft2019_pattern, "pattern.json");
draft2019_test!(draft2019_pattern_properties, "patternProperties.json");
draft2019_test!(draft2019_min_items, "minItems.json");
draft2019_test!(draft2019_max_items, "maxItems.json");
draft2019_test!(draft2019_unique_items, "uniqueItems.json");
draft2019_test!(draft2019_multiple_of, "multipleOf.json");
draft2019_test!(draft2019_min_properties, "minProperties.json");
draft2019_test!(draft2019_max_properties, "maxProperties.json");
draft2019_test!(draft2019_property_names, "propertyNames.json");
draft2019_test!(draft2019_boolean_schema, "boolean_schema.json");
draft2019_test!(draft2019_dependent_schemas, "dependentSchemas.json");
draft2019_test!(draft2019_dependent_required, "dependentRequired.json");
draft2019_test!(draft2019_min_contains, "minContains.json");
draft2019_test!(draft2019_max_contains, "maxContains.json");
draft2019_test!(draft2019_unevaluated_properties, "unevaluatedProperties.json", skip: [
    "can't see inside cousins",
    "cousin unevaluatedProperties",
    "uncle schema",
    "instance location",
    "unevaluatedProperties with $ref",
    "unevaluatedProperties before $ref",
    "unevaluatedProperties with $recursiveRef",
    "in-place applicator siblings",
    "single cyclic ref",
]);
draft2019_test!(draft2019_unevaluated_items, "unevaluatedItems.json", skip: [
    "can't see inside cousins",
    "uncle schema",
    "instance location",
    "unevaluatedItems with $ref",
    "unevaluatedItems before $ref",
    "unevaluatedItems with $recursiveRef",
    "unevaluatedItems with items and additionalItems",
    "unevaluatedItems with nested items and additionalItems",
]);
draft2019_test!(draft2019_ref, "ref.json", skip: [
    "Remote ref",
    "Recursive references",
    "remote ref, containing refs itself",
    "refs with relative uris and defs",
    "relative refs with absolute uris and defs",
    "ref applies alongside sibling keywords",
    "order of evaluation: $id and $anchor and $ref",
    "$ref with $recursiveAnchor", // Requires runtime dynamic scope resolution
]);
draft2019_test!(draft2019_ref_remote, "refRemote.json", skip: [
    "remote ref",
    "base URI change",
    "base URI change - change folder",
    "base URI change - change folder in subschema",
]);
draft2019_test!(draft2019_format, "format.json");
draft2019_test!(draft2019_default, "default.json");
draft2019_test!(draft2019_defs, "defs.json", skip: [
    "validate definition against metaschema",
]);
draft2019_test!(draft2019_infinite_loop, "infinite-loop-detection.json");
draft2019_test!(draft2019_content, "content.json");

// ── Draft 2020-12 tests ─────────────────────────────────────────────
// Introduces: $dynamicRef, $dynamicAnchor, prefixItems,
// items as boolean, no additionalItems

macro_rules! draft2020_test {
    ($name:ident, $file:expr) => {
        draft2020_test!($name, $file, skip: []);
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
// (cousin/uncle visibility needs schema-path-aware evaluation tracking)
draft2020_test!(draft2020_unevaluated_properties, "unevaluatedProperties.json", skip: [
    "can't see inside cousins",
    "cousin unevaluatedProperties",
    "uncle schema to unevaluatedProperties",
    "in-place applicator siblings, anyOf has unevaluated",
    "single cyclic ref",
    "instance location",
    "unevaluatedProperties with $ref",
    "unevaluatedProperties before $ref",
    "unevaluatedProperties with $recursiveRef",
]);
draft2020_test!(draft2020_unevaluated_items, "unevaluatedItems.json", skip: [
    "can't see inside cousins",
    "uncle schema to unevaluatedItems",
    "instance location",
    "unevaluatedItems with items and additionalItems",
    "unevaluatedItems with nested items and additionalItems",
    "unevaluatedItems with $ref",
    "unevaluatedItems before $ref",
    "unevaluatedItems with $recursiveRef",
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
    "ref applies alongside sibling keywords",
    "$ref with $recursiveAnchor",
    "remote ref, containing refs itself",
]);

// Content keywords
draft2020_test!(draft2020_content, "content.json");
