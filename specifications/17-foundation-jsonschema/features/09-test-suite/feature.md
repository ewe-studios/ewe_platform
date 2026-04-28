---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/09-test-suite"
this_file: "specifications/17-foundation-jsonschema/features/09-test-suite/feature.md"

status: pending
priority: high
created: 2026-04-28

depends_on: ["00-core-types", "01-referencing", "02-keywords-validators", "03-compiler", "04-validation-engine", "05-error-reporting", "06-draft-support", "07-format-validation", "08-custom-extensions"]

tasks:
  completed: 0
  uncompleted: 30
  total: 30
  completion_percentage: 0
---

# Feature 9: Test Suite

## Overview

Replicate all tests from the reference project and integrate with the official JSON Schema Test Suite. Every test must include clear commentary explaining WHAT it tests and WHY that test case matters. The goal is comprehensive coverage that serves as living documentation of the validator's behavior.

This feature covers:
1. Official JSON Schema Test Suite (~7200 tests across 5 drafts)
2. Referencing test suite (reference resolution tests)
3. Unit tests for each module (replicated from reference project's inline tests)
4. Integration tests (compilation, validation, error reporting, evaluation output)

Test data source: The reference project at `/home/darkvoid/Boxxed/@formulas/src.rust/src.jsonschema/jsonschema/` uses git submodules for official test suites. We will vendor the test data or use a build script to load test cases from JSON files.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read all feature implementations (0–8)
- [ ] Study reference project test structure at `crates/jsonschema/tests/` and `crates/jsonschema-referencing/tests/`

---

## Requirements

1. **Official JSON Schema Test Suite** — Load and run the official test suite from [json-schema-org/JSON-Schema-Test-Suite](https://github.com/json-schema-org/JSON-Schema-Test-Suite). This suite contains ~7200 test cases organized by draft and keyword. Each test case is a JSON file with:
   ```json
   [
     {
       "description": "test group description",
       "schema": { ... },
       "tests": [
         {
           "description": "individual test description",
           "data": ...,
           "valid": true/false
         }
       ]
     }
   ]
   ```

2. **Test runner infrastructure** — Build a test runner that:
   - Loads test case JSON files from `tests/data/json-schema-test-suite/`
   - Generates individual `#[test]` functions per test case (via build script or test macro)
   - Reports which draft and keyword each test covers
   - Maintains an explicit xfail list for known failures with documented reasons

3. **Referencing test suite** — Load and run tests from [python-jsonschema/referencing-suite](https://github.com/python-jsonschema/referencing-suite) covering reference resolution behavior.

4. **Unit tests with commentary** — Each module gets unit tests that explain the edge case or behavior being verified. Comments should answer: "Why does this test exist? What would break if this behavior changed?"

5. **Integration tests** — End-to-end tests that:
   - Compile a schema and validate multiple instances
   - Test error reporting (correct paths, correct error kinds)
   - Test evaluation output (flag, list, hierarchical formats)
   - Test custom keywords and formats
   - Test the `JsonResolver` trait with `MapResolver`

6. **Cross-draft tests** — Tests that verify the same schema behaves differently under different drafts (e.g., `exclusiveMinimum` as boolean in Draft 4 vs number in Draft 6+).

## Architecture (COMPREHENSIVE)

### Test Directory Structure

```
backends/foundation_jsonschema/tests/
├── suite.rs                    # Official JSON Schema Test Suite runner
├── referencing_suite.rs        # Referencing test suite runner
├── integration/
│   ├── mod.rs
│   ├── basic_validation.rs     # Simple schema compilation + validation
│   ├── error_reporting.rs      # Error path tracking, error kind accuracy
│   ├── evaluation_output.rs    # Flag, list, hierarchical output tests
│   ├── custom_extensions.rs    # Custom keywords and formats
│   ├── resolver.rs             # JsonResolver + MapResolver integration
│   ├── cross_draft.rs          # Same schema, different draft behavior
│   └── meta_schema.rs          # Meta-schema validation tests
├── data/
│   ├── json-schema-test-suite/ # Vendored official test suite
│   │   ├── draft4/
│   │   ├── draft6/
│   │   ├── draft7/
│   │   ├── draft2019-09/
│   │   ├── draft2020-12/
│   │   ├── remotes/            # Remote schemas used by $ref tests
│   │   └── optional/           # Optional format tests
│   └── referencing-suite/      # Vendored referencing test suite
└── helpers/
    ├── mod.rs                  # Shared test utilities
    └── test_retriever.rs       # MapResolver pre-loaded with remote schemas
```

### Test Suite Runner Pattern

Rather than proc macros (which add complexity), use a data-driven approach:

```rust
/// Load test cases from a JSON file and run each one.
///
/// WHY: The official JSON Schema Test Suite provides ~7200 tests as JSON files.
/// This runner loads them at test time and generates assertions. Each test is
/// a separate assertion so failures are clearly identified.
fn run_test_file(path: &str, draft: Draft) {
    let contents = std::fs::read_to_string(path).unwrap();
    let groups: Vec<TestGroup> = serde_json::from_str(&contents).unwrap();
    
    for group in &groups {
        let retriever = test_retriever(); // MapResolver with remote schemas
        let validator = match draft_options(draft)
            .with_resolver(retriever)
            .build(&group.schema) {
            Ok(v) => v,
            Err(e) => {
                // Some test schemas are intentionally invalid
                for test in &group.tests {
                    if test.valid {
                        panic!("Schema compilation failed for valid test: {}: {e}", test.description);
                    }
                }
                continue;
            }
        };
        
        for test in &group.tests {
            let result = validator.is_valid(&test.data);
            assert_eq!(
                result, test.valid,
                "Draft {:?} | {} | {}: expected valid={}, got valid={}",
                draft, group.description, test.description, test.valid, result
            );
        }
    }
}

#[derive(Deserialize)]
struct TestGroup {
    description: String,
    schema: serde_json::Value,
    tests: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    description: String,
    data: serde_json::Value,
    valid: bool,
}
```

### Test Coverage Matrix

| Category | Test Source | Estimated Count |
|----------|-----------|-----------------|
| Draft 4 validation | Official suite `draft4/` | ~800 |
| Draft 6 validation | Official suite `draft6/` | ~1000 |
| Draft 7 validation | Official suite `draft7/` | ~1200 |
| Draft 2019-09 validation | Official suite `draft2019-09/` | ~1600 |
| Draft 2020-12 validation | Official suite `draft2020-12/` | ~1600 |
| Optional format tests | Official suite `optional/` | ~500 |
| Referencing | Referencing suite | ~100 |
| Unit tests (per-module) | Custom | ~200 |
| Integration tests | Custom | ~50 |
| **Total** | | **~7050** |

### XFail List

An explicit list of known failures with documented reasons:

```rust
/// Tests that are expected to fail, with documented reasons.
///
/// WHY: Some test cases test behaviors we intentionally don't support,
/// or edge cases in URI normalization that our minimal parser doesn't handle.
const XFAILS: &[(&str, &str)] = &[
    // URI normalization: port normalization for default schemes
    // Our minimal URI parser doesn't remove default ports (80 for http, 443 for https)
    ("draft2020-12/refRemote.json/base URI change - change folder in subschema", "port normalization not implemented"),
    // ... other documented xfails
];
```

### Remote Schema Handling

The official test suite has `remotes/` containing schemas that `$ref` tests reference. Instead of HTTP, we load them into a `MapResolver`:

```rust
/// Create a MapResolver pre-loaded with the official test suite's remote schemas.
///
/// WHY: The test suite's $ref tests reference schemas like
/// "http://localhost:1234/integer.json". Rather than running an HTTP server,
/// we load these from the vendored remotes/ directory into a MapResolver.
fn test_retriever() -> MapResolver {
    let mut resolver = MapResolver::new();
    
    // Load all files from tests/data/json-schema-test-suite/remotes/
    for entry in walkdir("tests/data/json-schema-test-suite/remotes/") {
        let uri = format!("http://localhost:1234/{}", entry.relative_path);
        let schema: Value = serde_json::from_str(&entry.contents).unwrap();
        resolver.insert(uri, schema);
    }
    
    resolver
}
```

### Commentary Requirements

Every test must have a comment or doc-string explaining:
1. **What** it tests (which keyword, which edge case)
2. **Why** it matters (what would break if this behavior changed)

Example:
```rust
#[test]
fn type_integer_rejects_float() {
    // WHAT: The "type": "integer" constraint must reject numbers with
    // fractional parts, even if the fractional part is zero (e.g., 1.0).
    //
    // WHY: JSON has no integer type — all numbers are IEEE 754 doubles.
    // JSON Schema defines "integer" as "a number with no fractional part".
    // Some implementations incorrectly accept 1.0 as an integer because
    // the underlying representation can represent it exactly. The spec
    // says 1.0 IS a valid integer (it has zero fractional part).
    let schema = json!({"type": "integer"});
    let validator = validator_for(&schema).unwrap();
    
    assert!(validator.is_valid(&json!(1)));      // integer literal
    assert!(validator.is_valid(&json!(1.0)));    // 1.0 has zero fractional part → valid
    assert!(!validator.is_valid(&json!(1.5)));   // has fractional part → invalid
}
```

## Tasks

### Test Infrastructure
- [ ] Task 1: Create `tests/helpers/mod.rs` with shared test utilities (schema compilation helpers, assertion helpers)
- [ ] Task 2: Create `tests/helpers/test_retriever.rs` with `test_retriever()` function loading remote schemas into MapResolver
- [ ] Task 3: Vendor the official JSON Schema Test Suite into `tests/data/json-schema-test-suite/`
- [ ] Task 4: Vendor the referencing test suite into `tests/data/referencing-suite/`
- [ ] Task 5: Define `TestGroup` and `TestCase` deserializable structs
- [ ] Task 6: Implement `run_test_file()` test runner function
- [ ] Task 7: Define and document the xfail list

### Official Test Suite
- [ ] Task 8: Create `tests/suite.rs` with test functions for each Draft 4 test file
- [ ] Task 9: Add test functions for each Draft 6 test file
- [ ] Task 10: Add test functions for each Draft 7 test file
- [ ] Task 11: Add test functions for each Draft 2019-09 test file
- [ ] Task 12: Add test functions for each Draft 2020-12 test file
- [ ] Task 13: Add test functions for optional format tests
- [ ] Task 14: Create `tests/referencing_suite.rs` with referencing test runner

### Unit Tests (per-module)
- [ ] Task 15: Write unit tests for `types.rs` — JsonType::of() edge cases, JsonTypeSet operations
- [ ] Task 16: Write unit tests for `paths.rs` — Location formatting, LazyLocation materialization, JSON Pointer escaping
- [ ] Task 17: Write unit tests for `draft.rs` — detection from $schema, URI mapping
- [ ] Task 18: Write unit tests for `referencing/uri.rs` — parsing, resolution, normalization
- [ ] Task 19: Write unit tests for `referencing/pointer.rs` — traversal, escaping
- [ ] Task 20: Write unit tests for `referencing/registry.rs` — build, index, resolve
- [ ] Task 21: Write unit tests for `error.rs` — Display formatting, serialization
- [ ] Task 22: Write unit tests for `evaluation.rs` — flag/list/hierarchical output

### Integration Tests
- [ ] Task 23: Write `tests/integration/basic_validation.rs` — compile + validate simple schemas
- [ ] Task 24: Write `tests/integration/error_reporting.rs` — verify error paths, kinds, messages
- [ ] Task 25: Write `tests/integration/evaluation_output.rs` — verify structured output JSON
- [ ] Task 26: Write `tests/integration/custom_extensions.rs` — custom keyword + format tests
- [ ] Task 27: Write `tests/integration/resolver.rs` — MapResolver with external refs
- [ ] Task 28: Write `tests/integration/cross_draft.rs` — same schema, different draft behavior
- [ ] Task 29: Write `tests/integration/meta_schema.rs` — validate valid/invalid schemas
- [ ] Task 30: Verify total test count and document coverage summary

## Success Criteria

- [ ] All tasks completed
- [ ] Official JSON Schema Test Suite passes (minus documented xfails)
- [ ] Referencing test suite passes (minus documented xfails)
- [ ] Every test has commentary explaining what and why
- [ ] Integration tests cover error reporting, evaluation output, and custom extensions
- [ ] Test count documented with coverage summary
- [ ] Zero clippy warnings in test code

---

_Created: 2026-04-28_
