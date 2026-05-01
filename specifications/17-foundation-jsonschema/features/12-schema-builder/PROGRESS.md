# Progress: Feature 12 — Schema Builder

## Completed

- **Task 1**: Modified `options.rs` — added `schema: Option<Value>` field, `compile()`, `schema()`, `into_schema()`, `clone_schema()` methods. Refactored `build()` into `_do_compile()`.
- **Task 2**: Created `scheme/mod.rs` — module root, submodules, re-exports, `From` impls.
- **Task 3**: Created `scheme/primitives.rs` — StringSchema, IntegerSchema, NumberSchema, BooleanSchema, NullSchema with all constraint methods, constructor functions, and universal modifiers (optional, nullable, description, default).
- **Task 4**: Created `scheme/object.rs` — ObjectSchema with required(), optional(), properties(), strict(), loose(), catchall(), pattern_property(), min/max_properties(), universal modifiers (optional_value(), nullable(), description, default).
- **Task 5**: Created `scheme/array.rs` — ArraySchema with items(), prefix_items(), additional_items(), min/max_items(), len(), unique(), nonempty(), contains(), min/max_contains(), universal modifiers (optional, nullable, description, default).
- **Task 6**: Created `scheme/compound.rs` — any_of(), union(), one_of(), all_of(), intersection(), not(), if_then(), if_then_else().
- **Task 7**: Created `scheme/literal.rs` — literal(), r#enum().
- **Task 8**: Created `scheme/reference.rs` — ref_(), dynamic_ref().
- **Task 16**: Wired `pub mod scheme;` into `lib.rs`.

All 490 unit tests + 10 doctests pass.

## Pending

- **Task 9**: Unit tests for primitives
- **Task 10**: Unit tests for object builder
- **Task 11**: Unit tests for array builder
- **Task 12**: Unit tests for compound types
- **Task 13**: Integration tests (build complex schemas, compile, validate)
- **Task 14**: Tests for ValidationOptions schema accessors
- **Task 15**: Examples demonstrating common patterns
