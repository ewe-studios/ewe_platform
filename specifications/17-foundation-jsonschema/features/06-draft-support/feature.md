---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/06-draft-support"
this_file: "specifications/17-foundation-jsonschema/features/06-draft-support/feature.md"

status: complete
priority: medium
created: 2026-04-28

depends_on: ["00-core-types", "01-referencing", "02-keywords-validators"]

tasks:
  completed: 18
  uncompleted: 0
  total: 18
  completion_percentage: 100
---

# Feature 6: Draft Support

## Overview

Create per-draft public API modules (`draft4`, `draft6`, `draft7`, `draft201909`, `draft202012`) that provide draft-locked validation entry points, and implement meta-schema validation (validating schemas themselves). Each draft module exposes `is_valid()`, `validate()`, and `options()` functions pre-configured for that specific draft.

Derived from the `pub mod draft4/draft6/draft7/draft201909/draft202012` modules in the reference project's `lib.rs`.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Features 0, 1, 2, 3

---

## Requirements

1. **Per-draft modules** — Each draft gets a public module with:
   - `is_valid(schema, instance) -> bool` — validate with this specific draft
   - `validate(schema, instance) -> Result<(), ValidationError>` — first error
   - `validator_for(schema) -> Result<Validator, ValidationError>` — compile with this draft
   - `options() -> ValidationOptions` — pre-configured for this draft

2. **Draft differences** — Each module must enforce the correct keyword set and behavior:
   - **Draft 4**: `id` (not `$id`), `dependencies` (not dependentSchemas), no `const`/`contains`/`if`/`then`/`else`, `exclusiveMinimum`/`exclusiveMaximum` are booleans
   - **Draft 6**: `$id`, adds `const`, `contains`, `propertyNames`, `exclusiveMinimum`/`exclusiveMaximum` are numbers
   - **Draft 7**: Adds `if`/`then`/`else`, `readOnly`, `writeOnly`, `contentMediaType`, `contentEncoding`
   - **Draft 2019-09**: Adds vocabularies, `$recursiveRef`/`$recursiveAnchor`, `unevaluatedProperties`/`unevaluatedItems`, `dependentSchemas`/`dependentRequired`, `$anchor`
   - **Draft 2020-12**: Replaces `$recursiveRef` with `$dynamicRef`/`$dynamicAnchor`, `prefixItems` replaces tuple-form `items`, `items` replaces `additionalItems`

3. **Meta-schema validation** — Validate that a schema document is itself a valid JSON Schema. Each draft has its own meta-schema. The `meta` module provides:
   - `meta::is_valid(schema) -> bool` — auto-detect draft and validate
   - `meta::validate(schema) -> Result<(), ValidationError>` — auto-detect draft, first error
   - `draft4::meta::is_valid(schema) -> bool` — validate against draft 4 meta-schema
   - etc. for each draft

4. **Meta-schema embedding** — Meta-schemas for all 5 drafts must be embedded as constants (not fetched from the network).

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/src/
├── drafts/
│   ├── mod.rs              # Re-exports all draft modules
│   ├── draft4.rs           # Draft 4 API + keyword set
│   ├── draft6.rs           # Draft 6 API + keyword set
│   ├── draft7.rs           # Draft 7 API + keyword set
│   ├── draft201909.rs      # Draft 2019-09 API + keyword set
│   └── draft202012.rs      # Draft 2020-12 API + keyword set
├── meta.rs                 # Meta-schema validation
└── meta_schemas/
    ├── mod.rs              # Lazy-loaded meta-schema values
    ├── draft4.json         # Embedded meta-schema (included via include_str!)
    ├── draft6.json
    ├── draft7.json
    ├── draft201909.json    # Core + applicator + validation vocabularies
    └── draft202012.json
```

### Draft Module Pattern

Each draft module follows the same API pattern:

```rust
/// JSON Schema Draft 7 validation.
///
/// WHY: Sometimes you know which draft a schema targets and want to
/// enforce that specific draft's rules, rather than auto-detecting.
pub mod draft7 {
    use crate::{Draft, ValidationOptions, Validator, ValidationError};
    
    /// Validate an instance against a schema using Draft 7 rules.
    pub fn is_valid(schema: &serde_json::Value, instance: &serde_json::Value) -> bool {
        validator_for(schema)
            .map(|v| v.is_valid(instance))
            .unwrap_or(false)
    }
    
    /// Validate and return the first error using Draft 7 rules.
    pub fn validate(
        schema: &serde_json::Value,
        instance: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        validator_for(schema)?.validate(instance)
    }
    
    /// Compile a schema under Draft 7 rules.
    pub fn validator_for(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        options().build(schema)
    }
    
    /// Create ValidationOptions pre-configured for Draft 7.
    pub fn options() -> ValidationOptions {
        ValidationOptions::new().with_draft(Draft::Draft7)
    }
    
    /// Meta-schema validation for Draft 7.
    pub mod meta {
        pub fn is_valid(schema: &serde_json::Value) -> bool { ... }
        pub fn validate(schema: &serde_json::Value) -> Result<(), ValidationError> { ... }
    }
}
```

### Draft Keyword Matrix

| Keyword | Draft 4 | Draft 6 | Draft 7 | 2019-09 | 2020-12 |
|---------|---------|---------|---------|---------|---------|
| id / $id | `id` | `$id` | `$id` | `$id` | `$id` |
| type | ✅ | ✅ | ✅ | ✅ | ✅ |
| enum | ✅ | ✅ | ✅ | ✅ | ✅ |
| const | ❌ | ✅ | ✅ | ✅ | ✅ |
| properties | ✅ | ✅ | ✅ | ✅ | ✅ |
| additionalProperties | ✅ | ✅ | ✅ | ✅ | ✅ |
| patternProperties | ✅ | ✅ | ✅ | ✅ | ✅ |
| required | ✅ | ✅ | ✅ | ✅ | ✅ |
| propertyNames | ❌ | ✅ | ✅ | ✅ | ✅ |
| dependencies | ✅ | ✅ | ✅ | ❌¹ | ❌¹ |
| dependentRequired | ❌ | ❌ | ❌ | ✅ | ✅ |
| dependentSchemas | ❌ | ❌ | ❌ | ✅ | ✅ |
| unevaluatedProperties | ❌ | ❌ | ❌ | ✅ | ✅ |
| items (single schema) | ✅ | ✅ | ✅ | ✅ | ✅² |
| items (tuple/array) | ✅ | ✅ | ✅ | ✅ | ❌³ |
| prefixItems | ❌ | ❌ | ❌ | ❌ | ✅ |
| additionalItems | ✅ | ✅ | ✅ | ✅ | ❌ |
| contains | ❌ | ✅ | ✅ | ✅ | ✅ |
| unevaluatedItems | ❌ | ❌ | ❌ | ✅ | ✅ |
| if/then/else | ❌ | ❌ | ✅ | ✅ | ✅ |
| $ref | ✅⁴ | ✅ | ✅ | ✅ | ✅ |
| $recursiveRef | ❌ | ❌ | ❌ | ✅ | ❌ |
| $dynamicRef | ❌ | ❌ | ❌ | ❌ | ✅ |
| exclusiveMinimum (bool) | ✅ | ❌ | ❌ | ❌ | ❌ |
| exclusiveMinimum (number) | ❌ | ✅ | ✅ | ✅ | ✅ |

Notes:
1. `dependencies` renamed to `dependentRequired`/`dependentSchemas` in 2019-09
2. In 2020-12, `items` only takes a schema (not array) — array form replaced by `prefixItems`
3. Tuple-form `items` replaced by `prefixItems` in 2020-12
4. In Draft 4, `$ref` overrides all sibling keywords

## Tasks

- [ ] Task 1: Create `src/drafts/mod.rs` re-exporting all draft modules
- [ ] Task 2: Implement `draft4` module with `is_valid()`, `validate()`, `validator_for()`, `options()`
- [ ] Task 3: Implement `draft6` module
- [ ] Task 4: Implement `draft7` module
- [ ] Task 5: Implement `draft201909` module
- [ ] Task 6: Implement `draft202012` module
- [ ] Task 7: Embed Draft 4 meta-schema via `include_str!`
- [ ] Task 8: Embed Draft 6 meta-schema
- [ ] Task 9: Embed Draft 7 meta-schema
- [ ] Task 10: Embed Draft 2019-09 meta-schema (core + vocabularies)
- [ ] Task 11: Embed Draft 2020-12 meta-schema (core + vocabularies)
- [ ] Task 12: Implement `meta.rs` with auto-detect `is_valid()` and `validate()`
- [ ] Task 13: Implement per-draft `meta` sub-modules
- [ ] Task 14: Write tests for each draft module — compile and validate a simple schema
- [ ] Task 15: Write tests for draft-specific keyword behavior differences
- [ ] Task 16: Write tests for meta-schema validation — valid schema, invalid schema
- [ ] Task 17: Write tests for Draft 4 `id` vs `$id` handling
- [ ] Task 18: Write tests for Draft 4 boolean `exclusiveMinimum` vs Draft 6+ numeric `exclusiveMinimum`

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Each draft module enforces the correct keyword set
- [ ] Meta-schemas are embedded, not fetched
- [ ] Draft-specific behaviors (id vs $id, boolean vs numeric exclusive, etc.) work correctly
- [ ] Zero clippy warnings

---

_Created: 2026-04-28_
