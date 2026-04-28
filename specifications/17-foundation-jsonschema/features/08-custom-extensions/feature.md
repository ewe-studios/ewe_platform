---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/08-custom-extensions"
this_file: "specifications/17-foundation-jsonschema/features/08-custom-extensions/feature.md"

status: pending
priority: medium
created: 2026-04-28

depends_on: ["00-core-types", "02-keywords-validators", "03-compiler", "04-validation-engine"]

tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0
---

# Feature 8: Custom Extensions

## Overview

Implement the extensibility system that lets users define custom JSON Schema keywords and custom format validators. Users register custom keyword factories and format checkers via `ValidationOptions`, and the compiler integrates them alongside built-in keywords.

Derived from `crates/jsonschema/src/keywords/custom.rs` in the reference project.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Features 0, 2, 3, 4

---

## Requirements

1. **`KeywordFactory` trait** — Users implement this to create custom keyword validators:
   ```rust
   pub trait KeywordFactory: Send + Sync {
       fn compile(
           &self,
           value: &serde_json::Value,
           schema_path: Location,
       ) -> Result<BoxedValidator, ValidationError>;
   }
   ```

2. **`Keyword` trait** — Simpler alternative where the user implements the validation directly:
   ```rust
   pub trait Keyword: Send + Sync {
       fn is_valid(&self, instance: &serde_json::Value) -> bool;
       fn validate(&self, instance: &serde_json::Value) -> Result<(), String>;
   }
   ```
   A `KeywordAdapter` wraps a `Keyword` into a `BoxedValidator`.

3. **Custom format registration** — `ValidationOptions::with_format(name, checker)` registers custom `FormatChecker` implementations.

4. **Priority** — Custom keywords and formats take precedence over built-in ones with the same name.

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/src/keywords/
├── custom.rs    # KeywordFactory trait, Keyword trait, KeywordAdapter, CustomFormatChecker
```

### Component Details

```rust
/// Factory for creating custom keyword validators.
///
/// WHY: JSON Schema is extensible — application-specific keywords can
/// enforce constraints beyond the standard set. By registering a factory,
/// the compiler calls it when encountering the custom keyword name.
///
/// HOW: Register via ValidationOptions::with_keyword("x-my-keyword", factory).
/// The compiler calls factory.compile(value, schema_path) during compilation.
pub trait KeywordFactory: Send + Sync {
    fn compile(
        &self,
        value: &serde_json::Value,
        schema_path: Location,
    ) -> Result<BoxedValidator, ValidationError>;
}

/// Simplified custom keyword — just validation logic, no compilation.
pub trait Keyword: Send + Sync {
    fn is_valid(&self, instance: &serde_json::Value) -> bool;
    fn validate(&self, instance: &serde_json::Value) -> Result<(), String>;
}

/// Adapts a Keyword into a BoxedValidator.
struct KeywordAdapter {
    keyword: Box<dyn Keyword>,
    keyword_name: String,
    schema_path: Location,
}

impl Validate for KeywordAdapter {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        self.keyword.is_valid(instance)
    }
    
    fn validate(&self, instance: &Value, instance_path: &LazyLocation<'_>, _ctx: &mut ValidationContext) -> Result<(), ValidationError> {
        self.keyword.validate(instance).map_err(|msg| {
            ValidationError::new(
                ErrorKind::Custom { message: msg },
                instance_path.materialize(),
                self.schema_path.clone(),
            )
        })
    }
    
    fn iter_errors(&self, instance: &Value, instance_path: &LazyLocation<'_>, ctx: &mut ValidationContext) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
    }
}

/// Simple factory that wraps a Keyword clone for each schema occurrence.
pub struct SimpleKeywordFactory<K: Keyword + Clone + 'static> {
    keyword: K,
}

impl<K: Keyword + Clone + 'static> KeywordFactory for SimpleKeywordFactory<K> {
    fn compile(&self, _value: &Value, schema_path: Location) -> Result<BoxedValidator, ValidationError> {
        Ok(Box::new(KeywordAdapter {
            keyword: Box::new(self.keyword.clone()),
            keyword_name: /* from registration */,
            schema_path,
        }))
    }
}
```

### Integration with ValidationOptions

```rust
impl ValidationOptions {
    /// Register a custom keyword factory.
    pub fn with_keyword(
        mut self,
        name: impl Into<String>,
        factory: impl KeywordFactory + 'static,
    ) -> Self {
        self.custom_keywords.insert(name.into(), Box::new(factory));
        self
    }
    
    /// Register a simple custom keyword (cloned per schema occurrence).
    pub fn with_simple_keyword<K: Keyword + Clone + 'static>(
        mut self,
        name: impl Into<String>,
        keyword: K,
    ) -> Self {
        let name = name.into();
        self.custom_keywords.insert(
            name.clone(),
            Box::new(SimpleKeywordFactory { keyword }),
        );
        self
    }
    
    /// Register a custom format checker.
    pub fn with_format(
        mut self,
        name: impl Into<String>,
        checker: impl FormatChecker + 'static,
    ) -> Self {
        self.custom_formats.insert(name.into(), Box::new(checker));
        self
    }
}
```

## Tasks

- [ ] Task 1: Define `KeywordFactory` trait in `keywords/custom.rs`
- [ ] Task 2: Define `Keyword` trait (simplified interface)
- [ ] Task 3: Implement `KeywordAdapter` wrapping Keyword into Validate
- [ ] Task 4: Implement `SimpleKeywordFactory` for Clone-able Keywords
- [ ] Task 5: Add `with_keyword()` to `ValidationOptions`
- [ ] Task 6: Add `with_simple_keyword()` to `ValidationOptions`
- [ ] Task 7: Add `with_format()` to `ValidationOptions`
- [ ] Task 8: Wire custom keyword lookup into compiler keyword dispatch
- [ ] Task 9: Wire custom format lookup into FormatValidator (custom takes precedence over built-in)
- [ ] Task 10: Write tests for custom keyword — define, register, compile, validate
- [ ] Task 11: Write tests for custom format — define, register, validate with assert_format
- [ ] Task 12: Write tests for custom keyword overriding a built-in keyword name

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Custom keywords integrate seamlessly with the compiler
- [ ] Custom formats integrate with FormatValidator
- [ ] Custom overrides built-in when names collide
- [ ] Zero clippy warnings

---

_Created: 2026-04-28_
