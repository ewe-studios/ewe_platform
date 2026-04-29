---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/10-fuzz-targets"
this_file: "specifications/17-foundation-jsonschema/features/10-fuzz-targets/feature.md"

status: complete
priority: medium
created: 2026-04-28

depends_on: ["00-core-types", "01-referencing", "02-keywords-validators", "03-compiler", "04-validation-engine", "05-error-reporting", "06-draft-support", "07-format-validation", "08-custom-extensions", "09-test-suite"]

tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100
---

# Feature 10: Fuzz Targets

## Overview

Create fuzz testing targets using `cargo-fuzz` (libfuzzer) that exercise the three critical code paths: validator construction (compilation), instance validation, and reference resolution. Fuzz testing finds panics, infinite loops, and memory safety issues that unit tests miss.

Derived from the `fuzz/` directory in the reference project.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Study reference project fuzz targets at `/home/darkvoid/Boxxed/@formulas/src.rust/src.jsonschema/jsonschema/fuzz/`

---

## Requirements

1. **Builder fuzz target** — Feed arbitrary bytes as a JSON schema to `validator_for()`. Goal: find panics or infinite loops during schema compilation. Must not panic on any input — malformed JSON should return an error, not crash.

2. **Validation fuzz target** — Feed two arbitrary byte slices: one as schema, one as instance. Compile the schema, then validate the instance using all four methods (`is_valid`, `validate`, `iter_errors`, `evaluate`). Goal: find panics during validation.

3. **Referencing fuzz target** — Feed arbitrary bytes as schema, plus arbitrary strings for base URI and reference. Build a registry and attempt resolution. Goal: find panics in URI parsing, JSON Pointer traversal, and registry building.

4. **Seed corpus** — Provide initial seed inputs that exercise different code paths (valid schemas, invalid schemas, schemas with `$ref`, nested schemas).

5. **Fuzzing dictionary** — Provide a dictionary of JSON Schema keywords and structural tokens to guide the fuzzer toward meaningful inputs.

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/fuzz/
├── Cargo.toml
├── fuzz_targets/
│   ├── builder.rs          # Fuzz schema compilation
│   ├── validation.rs       # Fuzz all validation paths
│   └── referencing.rs      # Fuzz reference resolution
├── seeds/
│   ├── builder/            # Seed schemas for builder fuzzing
│   ├── validation/         # Seed (schema, instance) pairs
│   └── referencing/        # Seed (schema, uri, ref) triples
└── dict                    # Fuzzing dictionary
```

### Fuzz Target Implementations

```rust
// fuzz_targets/builder.rs
//
// WHY: Schema compilation processes arbitrary user-provided JSON. It must
// never panic, regardless of input — malformed JSON, unexpected types,
// circular references, or absurdly deep nesting must all be handled
// gracefully by returning an error.

#![no_main]
use libfuzzer_sys::fuzz_target;
use foundation_jsonschema::validator_for;

fuzz_target!(|data: &[u8]| {
    if let Ok(schema) = serde_json::from_slice::<serde_json::Value>(data) {
        // We don't care if compilation succeeds or fails — we only care
        // that it doesn't panic.
        let _ = validator_for(&schema);
    }
});
```

```rust
// fuzz_targets/validation.rs
//
// WHY: The validation engine traverses both the compiled schema tree and
// the instance data. Edge cases in recursion, type coercion, and error
// collection could cause panics. We exercise all four validation methods
// to catch issues in any code path.

#![no_main]
use libfuzzer_sys::fuzz_target;
use foundation_jsonschema::validator_for;

fuzz_target!(|data: &[u8]| {
    // Split input: first half is schema, second half is instance
    let mid = data.len() / 2;
    let (schema_bytes, instance_bytes) = data.split_at(mid);
    
    let Ok(schema) = serde_json::from_slice::<serde_json::Value>(schema_bytes) else { return };
    let Ok(instance) = serde_json::from_slice::<serde_json::Value>(instance_bytes) else { return };
    
    let Ok(validator) = validator_for(&schema) else { return };
    
    // Exercise all validation code paths
    let _ = validator.is_valid(&instance);
    let _ = validator.validate(&instance);
    let _ = validator.iter_errors(&instance).count();
    let evaluation = validator.evaluate(&instance);
    let _ = evaluation.flag();
    let _ = evaluation.list();
    let _ = evaluation.hierarchical();
});
```

```rust
// fuzz_targets/referencing.rs
//
// WHY: URI parsing and JSON Pointer traversal process arbitrary strings.
// Edge cases in percent-encoding, fragment resolution, and anchor lookup
// could cause panics. The registry builder also crawls arbitrary JSON
// looking for $id, $ref, and anchors — it must handle malformed input.

#![no_main]
use libfuzzer_sys::fuzz_target;
use foundation_jsonschema::{Draft, referencing::Registry};

fuzz_target!(|data: &[u8]| {
    // Split into three parts: schema, base_uri, reference
    if data.len() < 6 { return; }
    let schema_end = data.len() / 3;
    let uri_end = 2 * data.len() / 3;
    
    let Ok(schema) = serde_json::from_slice::<serde_json::Value>(&data[..schema_end]) else { return };
    let Ok(base_uri) = core::str::from_utf8(&data[schema_end..uri_end]) else { return };
    let Ok(reference) = core::str::from_utf8(&data[uri_end..]) else { return };
    
    // Try building a registry and resolving
    for draft in [Draft::Draft4, Draft::Draft6, Draft::Draft7, Draft::Draft201909, Draft::Draft202012] {
        let registry = Registry::builder()
            .with_draft(draft)
            .add_resource(base_uri, schema.clone())
            .build();
        
        if let Ok(registry) = registry {
            let resolver = registry.resolver(base_uri);
            let _ = resolver.lookup(reference);
        }
    }
});
```

### Seed Corpus

Provide meaningful seeds to help the fuzzer explore quickly:

```json
// seeds/builder/simple_type.json
{"type": "string"}

// seeds/builder/nested_object.json
{"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "integer"}}, "required": ["name"]}

// seeds/builder/with_ref.json
{"$ref": "#/$defs/item", "$defs": {"item": {"type": "string"}}}

// seeds/builder/all_keywords.json
{"allOf": [{"type": "object"}, {"required": ["id"]}], "properties": {"id": {"type": "integer", "minimum": 1}}}
```

### Fuzzing Dictionary

```
# dict — JSON Schema keywords for guided fuzzing
"type"
"properties"
"required"
"$ref"
"$id"
"$schema"
"$defs"
"allOf"
"anyOf"
"oneOf"
"not"
"if"
"then"
"else"
"items"
"prefixItems"
"additionalProperties"
"patternProperties"
"minimum"
"maximum"
"minLength"
"maxLength"
"pattern"
"format"
"enum"
"const"
"contains"
"uniqueItems"
"$anchor"
"$dynamicRef"
"$dynamicAnchor"
"$recursiveRef"
"$recursiveAnchor"
"dependentRequired"
"dependentSchemas"
"unevaluatedProperties"
"unevaluatedItems"
"string"
"number"
"integer"
"boolean"
"null"
"array"
"object"
```

## Tasks

- [ ] Task 1: Create `fuzz/Cargo.toml` with libfuzzer-sys dependency and target configuration
- [ ] Task 2: Implement `fuzz/fuzz_targets/builder.rs` — schema compilation fuzzing
- [ ] Task 3: Implement `fuzz/fuzz_targets/validation.rs` — all validation path fuzzing
- [ ] Task 4: Implement `fuzz/fuzz_targets/referencing.rs` — URI/pointer/registry fuzzing
- [ ] Task 5: Create seed corpus for builder target (5-10 diverse schemas)
- [ ] Task 6: Create seed corpus for validation target (5-10 schema+instance pairs)
- [ ] Task 7: Create seed corpus for referencing target (5-10 schema+uri+ref triples)
- [ ] Task 8: Create fuzzing dictionary with all JSON Schema keywords
- [ ] Task 9: Verify all fuzz targets build (`cargo fuzz build`)
- [ ] Task 10: Run each fuzz target for a short duration (60s) to verify no immediate panics on seed corpus

## Success Criteria

- [ ] All tasks completed
- [ ] All three fuzz targets compile and run
- [ ] Seed corpus provides meaningful starting points
- [ ] Dictionary covers all JSON Schema keywords
- [ ] No panics on seed corpus after 60s run
- [ ] Fuzz target code has clear WHY comments

---

_Created: 2026-04-28_
