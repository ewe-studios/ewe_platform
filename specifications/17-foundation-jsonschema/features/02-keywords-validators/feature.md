---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/02-keywords-validators"
this_file: "specifications/17-foundation-jsonschema/features/02-keywords-validators/feature.md"

status: pending
priority: high
created: 2026-04-28

depends_on: ["00-core-types", "01-referencing"]

tasks:
  completed: 0
  uncompleted: 52
  total: 52
  completion_percentage: 0
---

# Feature 2: Keywords & Validators

## Overview

Implement all 35+ JSON Schema keyword validators. Each keyword in JSON Schema (e.g., `type`, `properties`, `minimum`, `$ref`) has a corresponding validator struct that checks whether a JSON instance satisfies the constraint. These validators implement a common `Validate` trait and are created by the compiler (Feature 3) during schema compilation.

This is derived from the `keywords/` module in the reference project (`crates/jsonschema/src/keywords/`).

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Feature 0 (core-types) for JsonType, JsonTypeSet, Location, Draft
- [ ] Read Feature 1 (referencing) for Resolver, Resolved, Registry
- [ ] Read Feature 5 (error-reporting) for ValidationError, ErrorKind, ErrorIterator

---

## Requirements

1. **`Validate` trait** — The core trait every keyword validator implements:
   - `is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool` — fast boolean check
   - `validate(&self, instance: &Value, instance_path: &LazyLocation, ctx: &mut ValidationContext) -> Result<(), ValidationError>` — first-error semantics
   - `iter_errors(&self, instance: &Value, instance_path: &LazyLocation, ctx: &mut ValidationContext) -> ErrorIterator` — collect all errors

2. **Keyword categories** — Implement validators grouped by category:
   - **Type**: `type`, `const`, `enum`
   - **Logical composition**: `allOf`, `anyOf`, `oneOf`, `not`, `if`/`then`/`else`
   - **Object**: `properties`, `additionalProperties`, `patternProperties`, `required`, `propertyNames`, `minProperties`, `maxProperties`, `dependentRequired`, `dependentSchemas`, `unevaluatedProperties`, `dependencies` (legacy)
   - **Array**: `items`, `additionalItems`, `prefixItems`, `contains`, `minItems`, `maxItems`, `uniqueItems`, `unevaluatedItems`
   - **String**: `minLength`, `maxLength`, `pattern`
   - **Number**: `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`
   - **Reference**: `$ref`, `$dynamicRef`, `$recursiveRef`
   - **Content**: `contentEncoding`, `contentMediaType`, `contentSchema`
   - **Custom**: User-defined keyword support via `KeywordFactory` trait

3. **BoxedValidator** — Type alias `Box<dyn Validate>` for runtime dispatch. Each `SchemaNode` holds a `Vec<BoxedValidator>` representing the compiled keywords for that schema.

4. **BuiltinKeyword enum** — An enum of all recognized keywords for lookup during compilation.

## Architecture (COMPREHENSIVE)

### Technical Approach

- **One struct per keyword**: Each JSON Schema keyword gets its own struct implementing `Validate`. This keeps validation logic isolated and testable.
- **Compiled state**: Validators store pre-computed state. For example, `PatternValidator` stores the compiled `Regex`, `TypeValidator` stores the `JsonTypeSet`, `PropertiesValidator` stores the compiled sub-validators for each property schema.
- **Recursive compilation**: Object/array/composition validators contain nested `SchemaNode` instances for sub-schemas. These are compiled by the compiler (Feature 3) and stored as fields.
- **format keyword**: Handled separately in Feature 7 — the `FormatValidator` dispatches to a format-specific checker.

### Component Structure

```
backends/foundation_jsonschema/src/keywords/
├���─ mod.rs                      # Validate trait, BuiltinKeyword enum, BoxedValidator alias
├── custom.rs                   # KeywordFactory trait, CustomKeyword wrapper
├── type_.rs                    # TypeValidator
├── const_.rs                   # ConstValidator
├── enum_.rs                    # EnumValidator
├── all_of.rs                   # AllOfValidator
├── any_of.rs                   # AnyOfValidator
├── one_of.rs                   # OneOfValidator
├── not.rs                      # NotValidator
├── if_.rs                      # IfThenElseValidator
├── ref_.rs                     # RefValidator, DynamicRefValidator, RecursiveRefValidator
├── properties.rs               # PropertiesValidator
├── additional_properties.rs    # AdditionalPropertiesValidator (multiple variants)
├── pattern_properties.rs       # PatternPropertiesValidator
├── required.rs                 # RequiredValidator
├── property_names.rs           # PropertyNamesValidator
├── min_properties.rs           # MinPropertiesValidator
├── max_properties.rs           # MaxPropertiesValidator
���── dependent_required.rs       # DependentRequiredValidator
├── dependent_schemas.rs        # DependentSchemasValidator
├── unevaluated_properties.rs   # UnevaluatedPropertiesValidator
├── items.rs                    # ItemsValidator (covers items + additionalItems)
├── prefix_items.rs             # PrefixItemsValidator (2020-12)
├── contains.rs                 # ContainsValidator (with minContains/maxContains)
├─�� min_items.rs                # MinItemsValidator
├── max_items.rs                # MaxItemsValidator
├── unique_items.rs             # UniqueItemsValidator
├── unevaluated_items.rs        # UnevaluatedItemsValidator
├── min_length.rs               # MinLengthValidator
├── max_length.rs               # MaxLengthValidator
├── pattern.rs                  # PatternValidator
├── format.rs                   # FormatValidator (dispatcher — impl in Feature 7)
├── minimum.rs                  # MinimumValidator
├── maximum.rs                  # MaximumValidator
├─��� exclusive_minimum.rs        # ExclusiveMinimumValidator
├── exclusive_maximum.rs        # ExclusiveMaximumValidator
├── multiple_of.rs              # MultipleOfValidator
├── content.rs                  # ContentEncodingValidator, ContentMediaTypeValidator
└── legacy.rs                   # DependenciesValidator (Draft 4/6/7 legacy)
```

### Core Trait Definition

```rust
/// The core validation trait implemented by all keyword validators.
///
/// WHY: Each JSON Schema keyword defines a constraint on the instance.
/// The Validate trait provides three levels of validation output:
/// - is_valid: fast boolean (no error details, no allocation)
/// - validate: first error only (early exit on first failure)
/// - iter_errors: all errors (for comprehensive reporting)
///
/// HOW: The compiler creates a Vec<BoxedValidator> for each schema node.
/// At validation time, each validator is called in sequence.
pub trait Validate: Send + Sync {
    /// Fast boolean validation. No error details, no path tracking.
    /// Implementations should early-exit on first failure.
    fn is_valid(&self, instance: &serde_json::Value, ctx: &mut ValidationContext) -> bool;
    
    /// Validate and return the first error encountered.
    fn validate(
        &self,
        instance: &serde_json::Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError>;
    
    /// Return an iterator over all validation errors.
    fn iter_errors(
        &self,
        instance: &serde_json::Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator;
}

/// Type alias for a heap-allocated validator.
pub type BoxedValidator = Box<dyn Validate>;
```

### Keyword Implementation Pattern

Each keyword validator follows the same pattern. Example for `MinLengthValidator`:

```rust
/// Validates that string instances have at least `min` characters.
///
/// JSON Schema keyword: "minLength"
/// Applies to: string values only (non-strings are always valid).
/// Draft support: 4, 6, 7, 2019-09, 2020-12
pub struct MinLengthValidator {
    min: u64,
    schema_path: Location,
}

impl MinLengthValidator {
    pub fn new(min: u64, schema_path: Location) -> Self {
        Self { min, schema_path }
    }
}

impl Validate for MinLengthValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::String(s) = instance {
            s.chars().count() as u64 >= self.min
        } else {
            true // non-strings are valid for minLength
        }
    }
    
    fn validate(&self, instance: &Value, instance_path: &LazyLocation<'_>, _ctx: &mut ValidationContext) -> Result<(), ValidationError> {
        if let Value::String(s) = instance {
            let len = s.chars().count() as u64;
            if len < self.min {
                return Err(ValidationError::new(
                    ErrorKind::MinLength { min: self.min, actual: len },
                    instance_path.materialize(),
                    self.schema_path.clone(),
                ));
            }
        }
        Ok(())
    }
    
    fn iter_errors(&self, instance: &Value, instance_path: &LazyLocation<'_>, ctx: &mut ValidationContext) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
    }
}
```

### Complex Validator: additionalProperties

`additionalProperties` is one of the most complex validators because it must work with `properties` and `patternProperties` to determine which properties are "additional". It has multiple internal variants:

```rust
pub enum AdditionalPropertiesValidator {
    /// additionalProperties: true (no-op)
    Allow,
    /// additionalProperties: false — reject any property not in properties/patternProperties
    Deny {
        known_properties: BTreeSet<String>,
        pattern_properties: Vec<Regex>,
        schema_path: Location,
    },
    /// additionalProperties: {schema} — validate additional properties against a sub-schema
    Schema {
        known_properties: BTreeSet<String>,
        pattern_properties: Vec<Regex>,
        validator: SchemaNode,
        schema_path: Location,
    },
}
```

### Reference Validators ($ref, $dynamicRef, $recursiveRef)

These validators delegate to the referencing engine:

```rust
/// Validator for $ref — resolved at compile time, delegates to the referenced schema.
pub struct RefValidator {
    /// The compiled validator tree for the referenced schema.
    inner: SchemaNode,
    /// The reference string for error reporting.
    reference: String,
    schema_path: Location,
}

/// Validator for $dynamicRef (Draft 2020-12) — resolution depends on runtime scope.
pub struct DynamicRefValidator {
    /// Fallback compiled validator (from compile-time resolution).
    fallback: SchemaNode,
    /// The anchor name to search in the dynamic scope.
    anchor_name: String,
    reference: String,
    schema_path: Location,
}

/// Validator for $recursiveRef (Draft 2019-09).
pub struct RecursiveRefValidator {
    fallback: SchemaNode,
    reference: String,
    schema_path: Location,
}
```

### Unevaluated Properties/Items

These are the most complex validators in the spec. They require knowledge of which properties/items were evaluated by sibling keywords (`properties`, `patternProperties`, `additionalProperties`, `items`, `prefixItems`, etc.). This requires the evaluation context to track "evaluated" state:

```rust
/// Validator for unevaluatedProperties (Draft 2019-09, 2020-12).
///
/// WHY: unevaluatedProperties applies to properties that were not evaluated by
/// any other keyword in the same schema object or its allOf/if-then-else subschemas.
/// This requires tracking which properties were "evaluated" during validation.
pub struct UnevaluatedPropertiesValidator {
    schema: Option<SchemaNode>,  // None means unevaluatedProperties: false
    schema_path: Location,
}
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| One file per keyword | Keeps each validator small and focused; easy to find | Single large file (harder to navigate); grouped by category (still large files) |
| Box<dyn Validate> | Runtime dispatch needed because schema structure isn't known at compile time | Enum of all validators (37+ variants — unwieldy, loses extensibility) |
| BTreeSet for known_properties | no_std compatible, deterministic | HashSet (needs std or ahash); Vec with binary search (more code) |
| chars().count() for string length | JSON Schema counts Unicode characters, not bytes | len() for bytes (wrong per spec); grapheme clusters (too complex, not what spec says) |
| f64 for numeric comparisons | Matches serde_json Number representation | i64/u64 separate paths (more code but more precise for integers) — implement both where needed |

## Tasks

### Core Infrastructure
- [ ] Task 1: Define `Validate` trait in `keywords/mod.rs`
- [ ] Task 2: Define `BoxedValidator` type alias
- [ ] Task 3: Define `BuiltinKeyword` enum with all 35+ keyword variants
- [ ] Task 4: Implement `BuiltinKeyword::from_str()` for keyword name → enum mapping
- [ ] Task 5: Define `KeywordFactory` trait in `keywords/custom.rs` for custom keywords

### Type Validators
- [ ] Task 6: Implement `TypeValidator` — validates `type` keyword (single type or array of types)
- [ ] Task 7: Implement `ConstValidator` — validates `const` keyword (exact equality)
- [ ] Task 8: Implement `EnumValidator` — validates `enum` keyword (value must be in list)

### Logical Composition
- [ ] Task 9: Implement `AllOfValidator` — all sub-schemas must validate
- [ ] Task 10: Implement `AnyOfValidator` — at least one sub-schema must validate
- [ ] Task 11: Implement `OneOfValidator` — exactly one sub-schema must validate
- [ ] Task 12: Implement `NotValidator` — sub-schema must NOT validate
- [ ] Task 13: Implement `IfThenElseValidator` — conditional validation

### Object Validators
- [ ] Task 14: Implement `PropertiesValidator` — validate named properties against their schemas
- [ ] Task 15: Implement `AdditionalPropertiesValidator` — Allow, Deny, and Schema variants
- [ ] Task 16: Implement `PatternPropertiesValidator` — regex-matched property validation
- [ ] Task 17: Implement `RequiredValidator` — check property presence
- [ ] Task 18: Implement `PropertyNamesValidator` — validate property names against a schema
- [ ] Task 19: Implement `MinPropertiesValidator`
- [ ] Task 20: Implement `MaxPropertiesValidator`
- [ ] Task 21: Implement `DependentRequiredValidator` — property presence triggers required others
- [ ] Task 22: Implement `DependentSchemasValidator` — property presence triggers schema validation
- [ ] Task 23: Implement `UnevaluatedPropertiesValidator`
- [ ] Task 24: Implement `DependenciesValidator` (legacy Draft 4/6/7)

### Array Validators
- [ ] Task 25: Implement `ItemsValidator` — items as schema (all items) or array of schemas
- [ ] Task 26: Implement `PrefixItemsValidator` — Draft 2020-12 positional item schemas
- [ ] Task 27: Implement `AdditionalItemsValidator` — items beyond prefixItems/items array
- [ ] Task 28: Implement `ContainsValidator` — at least one item matches (with min/maxContains)
- [ ] Task 29: Implement `MinItemsValidator`
- [ ] Task 30: Implement `MaxItemsValidator`
- [ ] Task 31: Implement `UniqueItemsValidator` — all items must be distinct
- [ ] Task 32: Implement `UnevaluatedItemsValidator`

### String Validators
- [ ] Task 33: Implement `MinLengthValidator` — character count (not byte count)
- [ ] Task 34: Implement `MaxLengthValidator`
- [ ] Task 35: Implement `PatternValidator` — regex matching with compiled regex

### Number Validators
- [ ] Task 36: Implement `MinimumValidator` — handles both Draft 4 (with exclusiveMinimum bool) and Draft 6+ (separate keyword)
- [ ] Task 37: Implement `MaximumValidator`
- [ ] Task 38: Implement `ExclusiveMinimumValidator` (Draft 6+: numeric value)
- [ ] Task 39: Implement `ExclusiveMaximumValidator`
- [ ] Task 40: Implement `MultipleOfValidator` — floating-point remainder check

### Reference Validators
- [ ] Task 41: Implement `RefValidator` — compile-time resolved $ref
- [ ] Task 42: Implement `DynamicRefValidator` — Draft 2020-12 $dynamicRef
- [ ] Task 43: Implement `RecursiveRefValidator` — Draft 2019-09 $recursiveRef

### Content Validators
- [ ] Task 44: Implement `ContentEncodingValidator` (base64 check)
- [ ] Task 45: Implement `ContentMediaTypeValidator` (basic media type check)

### Unit Tests
- [ ] Task 46: Write unit tests for type validators (type, const, enum)
- [ ] Task 47: Write unit tests for composition validators (allOf, anyOf, oneOf, not, if/then/else)
- [ ] Task 48: Write unit tests for object validators (properties, required, additionalProperties, etc.)
- [ ] Task 49: Write unit tests for array validators (items, contains, uniqueItems, etc.)
- [ ] Task 50: Write unit tests for string validators (minLength, maxLength, pattern)
- [ ] Task 51: Write unit tests for number validators (minimum, maximum, multipleOf, etc.)
- [ ] Task 52: Write unit tests for reference validators (ref, dynamicRef, recursiveRef)

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Every keyword validator implements all 3 Validate methods
- [ ] is_valid() never allocates on valid input
- [ ] Each validator has WHY/WHAT/HOW doc comments explaining the keyword's semantics
- [ ] Zero clippy warnings

---

_Created: 2026-04-28_
