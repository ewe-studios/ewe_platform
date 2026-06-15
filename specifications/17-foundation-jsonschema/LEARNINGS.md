---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
this_file: "specifications/17-foundation-jsonschema/LEARNINGS.md"

created: 2026-04-28
last_updated: 2026-04-29
---

# Learnings: Foundation JSON Schema

## Read By

1. **Implementation Agent** reads this for context on previous discoveries
2. **Main Agent** reads this when resuming work on specification
3. **Future agents** read this to avoid repeating mistakes

## Purpose

Document non-obvious discoveries, critical decisions, and gotchas specific to THIS specification.

**Note**: Generic language patterns go in `.agents/skills/rust-clean-code/` instead.

---

## Critical Implementation Details

- The reference project (Stranger6667/jsonschema at `/home/darkvoid/Boxxed/@formulas/src.rust/src.jsonschema/jsonschema/`) has 14 crates. We consolidate into a single crate `foundation_jsonschema` with internal modules instead.
- The reference project uses `fluent-uri` for RFC 3986 URI resolution. We copy the URI module from `foundation_core` (`wire/simple_http/url/`) which provides `Uri`, `Scheme`, `Authority`, `PathAndQuery`, `Query`, and percent-decoding — then adapt to no_std and add RFC 3986 §5 reference resolution and JSON Pointer percent-decoding on top. This avoids depending on foundation_core (which is std-only and pulls in heavy deps like `url`, `regex`, TLS, compression).
- Draft 4 uses `id` (not `$id`) for schema identification — this is a common source of bugs.
- `$recursiveRef` (Draft 2019-09) and `$dynamicRef` (Draft 2020-12) have subtly different semantics — both walk the dynamic scope but `$dynamicRef` uses named anchors while `$recursiveRef` checks `$recursiveAnchor: true`.
- JSON Pointer escaping: `~0` → `~`, `~1` → `/` — order matters (unescape `~1` first, then `~0`).

## Common Failures and Fixes

- Keyword compilation ordering: `additionalProperties` runs before `properties` if iterated by BTreeMap (alphabetical). Fixed by sorting keywords with priority — late-phase validators (`additionalProperties`, `unevaluatedProperties`, `unevaluatedItems`) get priority 2, others priority 1.
- if/then/else must be compiled as a group from the parent schema object, not individually per keyword value. Detection of any of the three triggers reading all three.
- FormatChecker trait requires `clone_box()` for trait object cloning — `Box<dyn FormatChecker>` does not implement `Clone` natively. Added `clone_box(&self) -> Box<dyn FormatChecker>` to trait and `impl Clone for Box<dyn FormatChecker>` delegation.
- Custom format lookup in `compile_format` must clone from `ctx.custom_formats`, not from builtins. Using `builtin_format` for custom names returns `None`, making all validation pass.
- Test helpers for `CompilerContext::new()` need 7 arguments: `resolver`, `schema_path`, `vocabulary`, `draft`, `assert_format`, `custom_keywords`, `custom_formats`. All 3 test helper functions across 9 keyword test modules need updating together.

## Testing Insights

- The official JSON Schema Test Suite has ~7200 tests across 5 drafts
- The referencing test suite has ~20 known xfails related to RFC 3986 port normalization
- Test suite integration in the reference project uses proc macros for code generation — we should use a simpler approach (build.rs or manual test modules with clear commentary)

## Dependencies and Interactions

- `serde` + `serde_json` are workspace dependencies — use workspace versions
- `regex` is a workspace dependency — use workspace version
- `foundation_errstacks` is the workspace error handling crate — ALL error types use `ErrorTrace<ContextType>`, with `#[derive(Display, Error)]` context types from `derive_more`. Context is attached via `.attach()` as errors bubble up, not stored in the context type directly.
- `derive_more` is a workspace dependency — used for `Display` and `Error` derives on context types
- `foundation_nostd` provides spin-based sync primitives if needed for no_std memoization caches

## Future Considerations

- Schema bundling (combining external refs into single document) could be added as a later feature
- Schema dereferencing (inlining $ref targets) could be added as a later feature
- Python/Ruby bindings are out of scope — this is a pure Rust library

## Schema Builder (Feature 12)

- Consuming builder pattern (`self` not `&mut self`) is the Rust-idiomatic approach — each method call moves the builder, enabling fluent chains without `Arc`.
- `From<XxxSchema> for Value` impls are essential for ergonomics — without them, every `required("field", scheme::string())` must be `required("field", scheme::string().build_schema())`.
- The `schema` field on `ValidationOptions` must be `pub` (not `pub(crate)`) because the scheme module is a sibling public module, not an internal module. The field is visible outside the crate but not re-exported in `lib.rs`, so users won't find it accidentally.
- `pub mod compound;` already makes the module public — `pub use compound;` causes a duplicate-definition error. Remove redundant re-exports for modules that are already `pub mod`.
- **`optional()` on builders**: calling `self.build_schema()` inside `optional()` consumes `self` (since `build_schema` takes `self` by value), then trying to use `self` again causes a borrow-after-move error. Fix: use `Value::Object(self.schema.clone().into_iter().collect())` instead.
- **Naming conflict on ObjectSchema**: `ObjectSchema` already has `optional(name, schema)` for adding optional properties. The universal `optional()` (wrapping in anyOf with null) conflicts. Renamed to `optional_value()` on ObjectSchema only.
- **Always check spec for universal modifiers**: `optional()`, `nullable()`, `description()`, `default()` are listed on "every builder" in the spec. Easy to miss during initial implementation — do a pass specifically for these after building the core methods.

## URI Normalization (RFC 3986 §6.2.2.1)

- Schemes and hostnames are case-insensitive. The registry normalizes URIs on both insertion and lookup via `uri::normalize()`. The `InMemoryFetcher` also normalizes on insert/resolve.
- This was the root cause of the `uri_normalization_case_insensitive_host` test failure — `http://Example.COM` and `http://example.com` were treated as different URIs before normalization.
- Builtin meta-schema URIs (with trailing `#`) are also normalized, which strips the trailing fragment marker.

## `$ref` Sibling Keyword Suppression

- In Draft 4/6/7/2019-09, `$ref` suppresses all sibling keywords EXCEPT `$id`, `$anchor`, `$dynamicAnchor`, and `$recursiveAnchor` which must be processed for URI resolution and dynamic ref tracking.
- The original suppression (`key != "$ref"`) was too aggressive — it skipped `$recursiveAnchor` which broke the `$recursiveRef` test group.

## Test Suite Data

- The official JSON Schema Test Suite is vendored from the reference project's git submodule at `/home/darkvoid/Boxxed/@formulas/src.rust/src.jsonschema/jsonschema/crates/jsonschema/tests/suite/`.
- Test data lives in `tests/suite/draft{4,6,7,2019-09,2020-12}/` with `remotes/` for `$ref` test schemas.
- The referencing suite data is in `tests/suite/referencing-suite/`.

---

_Last Updated: 2026-06-15_
