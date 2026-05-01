# Grandfather Review: foundation_jsonschema Crate

**Date:** 2025-05-01
**Scope:** Full audit of `backends/foundation_jsonschema/` (13,509 lines, 73 src files, 11 test files)
**Reference project:** Stranger6667/jsonschema (Rust, 14-crate workspace, crates.io `jsonschema` v0.42+)
**Target:** 11-feature specification (requirements.md), 91% complete

---

## 1. CRITICAL BUGS (Incorrect Validation Results)

### BUG-1: `additionalProperties` ignores evaluation context — breaks `allOf` + `additionalProperties`

**Files:** `src/keywords/additional_properties.rs:53-63`

`AdditionalPropertiesValidator::is_additional_property()` determines whether a property is "additional" by checking only the **current schema object's** `properties` and `patternProperties` maps. It does **not** consult `ctx.is_property_evaluated(name)`.

This means when `additionalProperties` appears alongside an `allOf` that defines properties, it cannot see them:

```json
{
  "allOf": [{"properties": {"x": {"type": "string"}}}],
  "additionalProperties": false
}
```

The property "x" IS evaluated by the `allOf` branch's `properties`, but `additionalProperties` treats it as "additional" and rejects it. The correct behavior: `additionalProperties` should skip any property that `ctx.is_property_evaluated()` returns true for.

**Severity:** High. This breaks one of the most common composition patterns. The official test suite `unevaluatedProperties.json` tests this correctly (via cousin/uncle skip lists), but `additionalProperties` has the same visibility requirement.

**Fix:** Change `is_additional_property()` to also check `ctx.is_property_evaluated(name)`.

---

### BUG-2: `unevaluatedProperties` / `unevaluatedItems` — `validate()` and `iter_errors()` do not mark as evaluated

**Files:** `src/keywords/unevaluated_properties.rs:48-56`, `src/keywords/unevaluated_items.rs:48-56`

The `is_valid()` methods correctly call `ctx.mark_property_evaluated(name)` / `ctx.mark_item_evaluated(i)` after validating. But the `validate()` and `iter_errors()` methods skip this call entirely.

This causes a **split-brain** between `is_valid()` and `validate()`/`iter_errors()`:
- `validator.is_valid(instance)` marks items as evaluated — subsequent keywords see them
- `validator.validate(instance, ...)` does NOT mark — subsequent keywords re-validate

For a single-validator scenario this is benign, but in `allOf` chains or when multiple unevaluated keywords exist, the error-reporting path produces different results than the fast path.

**Severity:** Medium. Affects correctness when multiple unevaluated keywords exist in the same schema.

**Fix:** Add `ctx.mark_property_evaluated(name)` to `validate()` and `iter_errors()` after successful validation (same as `is_valid()`).

---

### BUG-3: `enum`/`const` number comparison loses precision for integers > 2^53

**File:** `src/keywords/enum_.rs:67`, `src/keywords/const_.rs:67`

```rust
(Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
```

Converting large integers through f64 loses precision. `9007199254740993` and `9007199254740994` (both valid `i64`/`u64`) produce the same f64 and compare equal. For `enum` this means `[9007199254740993, 9007199254740994]` would be deduplicated incorrectly, and `const` would accept the wrong value.

The `enum_.rs` `values_equal()` helper (used for array/object recursion) has the same issue at the top-level number comparison.

**Severity:** Medium for typical APIs (JSON payloads rarely exceed 2^53), but spec-noncompliant.

**Fix:** Compare `serde_json::Number` using `eq()` for same-representation cases, only fall back to f64 comparison when one is i64 and the other is f64 (which the spec requires for `0` vs `0.0`).

---

### BUG-4: `$ref` does not suppress sibling keywords in pre-2020-12 drafts

**File:** `src/compiler.rs:151-167`

In Draft 4/6/7/2019-09, when a schema object contains `$ref` alongside other keywords, the other keywords MUST be ignored per spec. In Draft 2020-12, all keywords are evaluated.

The compiler compiles every keyword independently regardless of draft. A Draft 7 schema like:

```json
{"$ref": "#/$defs/person", "type": "string"}
```

would validate both the `$ref` AND the `type: "string"`, rejecting valid instances that pass `$ref` but are not strings.

**Severity:** High for Draft 7/2019-09 users with `$ref` + sibling keywords. The official test suite `ref.json` skip list does not include a "ref alongside other keywords" test group, so this was not caught.

**Fix:** In `compile_node()`, when draft < 2020-12 and schema contains `$ref`, skip all keywords except `$ref` (and optionally `$id` for URI resolution).

---

## 2. SIGNIFICANT GAPS (Spec Non-Compliance)

### GAP-1: Draft 4 `id` (not `$id`) not extracted for base URI

**File:** `src/compiler.rs:93-97`

```rust
let base_uri = schema.as_object()
    .and_then(|o| o.get("$id"))  // always "$id"
    .and_then(Value::as_str)
    .unwrap_or("");
```

Draft 4 uses `"id"` not `"$id"`. Draft 4 schemas with `"id": "http://example.com/schema"` get an empty base URI, breaking `$ref` resolution. `Draft::id_keyword()` exists but is never called here.

**Fix:** Use `draft.id_keyword()` to select between `"id"` and `"$id"`.

---

### GAP-2: `pattern` uses Rust regex, not ECMA-262 — can panic

**File:** `src/keywords/pattern.rs:21`

```rust
let regex = Regex::new(&pattern).expect("pattern must be a valid regex");
```

JSON Schema requires ECMA-262 regular expressions. Rust's `regex` crate:
- Rejects lookbehind assertions (`(?<=a)b`) which are valid ECMA-262
- Does not support the `u` flag semantics
- Differing `\s`/`\d` Unicode properties

A schema with `{"pattern": "(?<=a)b"}` **panics the validator**. The `fancy-regex` feature exists but is opt-in.

**Fix:** When `fancy-regex` feature is disabled, either sanitize/escape patterns or return a compilation error for unsupported syntax. Consider making `fancy-regex` the default.

---

### GAP-3: `$dynamicRef` / `$dynamicAnchor` resolved at compile time, not runtime

**File:** `src/keywords/ref_.rs:67-118`

`DynamicRefValidator` and `RecursiveRefValidator` delegate to a pre-compiled schema resolved at compilation time. There is no dynamic scope lookup at validation time. This means `$dynamicRef` in an extended schema will resolve to the original `$dynamicAnchor`, not the override in the extending schema.

The dynamic scope resolution happens in `compiler.rs:814-836` (compile-time), making `$dynamicRef` functionally identical to `$ref`.

**Severity:** Spec gap for Draft 2020-12 users relying on schema extension patterns.

---

### GAP-4: `const` in Draft 4 vocabulary

**File:** `src/referencing/vocabulary.rs:91`

`DRAFT4_KEYWORDS` includes `"const"` which was introduced in Draft 6. This is a vocabulary metadata error. It does not cause a functional bug because the compiler does not gate keywords on vocabulary membership, but it is incorrect for any future feature that relies on `VocabularySet::contains_keyword()`.

---

### GAP-5: `$vocabulary` declarations silently ignored

Draft 2019-09 and 2020-12 support `$vocabulary` to declare which keywords are recognized and which are required. Unknown required keywords should cause compilation errors. Currently:
- `$vocabulary` is silently ignored
- Unknown keywords are silently skipped (`_ => None` in `compile_keyword`)

A schema declaring `"https://example.com/vocab": true` would silently ignore all keywords from that vocabulary.

---

### GAP-6: `uniqueItems` is O(n^2) with no hash optimization

**File:** `src/keywords/unique_items.rs:77-85`

```rust
fn find_duplicate(arr: &[Value]) -> Option<(usize, usize)> {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if values_equal(&arr[i], &arr[j]) { return Some((i, j)); }
        }
    }
    None
}
```

For an array of 10,000 items, this performs ~50 million comparisons. A hash-based approach (with special handling for numeric equality) would be O(n).

---

## 3. DEAD CODE AND USELESS STUBS

### DEAD-1: `compile_additional_items()` — always returns `None`

**File:** `src/compiler.rs:702-720`

This function is called from `compile_keyword()` for the `"additionalItems"` keyword but always returns `None` in all branches. The actual `additionalItems` logic is handled inline inside `compile_items()` when `items` is an array. This function is dead code.

---

### DEAD-2: `make_format_checker()` and `clone_checker()` — never called

**File:** `src/formats.rs:831-868`

`make_format_checker()` is a public convenience function that creates format checkers by name. `clone_checker()` is used internally to clone a `FormatChecker` trait object. Neither is called by the compiler — `compile_format()` uses `builtin_format()` directly.

---

### DEAD-3: `BuiltinKeyword` enum and all its methods

**File:** `src/keywords/mod.rs:101-198`

The `BuiltinKeyword` enum with `from_keyword_name()` and `as_str()` methods exists for keyword lookup, but the compiler dispatches via direct string matching in `compile_keyword()`. The enum is never constructed or consumed.

---

### DEAD-4: `ContentMediaType` enum and methods

**File:** `src/keywords/content.rs:336-362`

`ContentMediaType` (ApplicationJson / Unknown), `from_name()`, and `is_valid()` are dead code. `ContentMediaTypeValidator` uses the raw string directly. Marked with `#[allow(dead_code)]`.

---

### DEAD-5: `ValidationContext::enter()` and `exit()` — never called

**File:** `src/validation_context.rs:43-57`

These methods exist for cycle detection at validation time but are never called. Cycle detection happens at compile-time in `CompilerContext` via `mark_in_progress()`/`mark_done()`.

---

### DEAD-6: `VocabularySet::contains_keyword()` and `insert()` — never checked

Vocabulary is built via `VocabularySet::for_draft()` but `contains_keyword()` is never queried during compilation. The compiler dispatches all known keywords regardless of vocabulary membership.

---

### STALE-1: "Stub modules" comment

**File:** `src/keywords/mod.rs:18`

```rust
// Stub modules — will be implemented in subsequent features
```

All 30 keyword modules are fully implemented. This comment is stale.

---

### STALE-2: "Stub" label on dependent_required

**File:** `src/keywords/dependent_required.rs:1`

```rust
//! Stub: `dependentRequired` — property presence triggers required others.
```

The file contains a complete `DependentRequiredValidator`. Not a stub.

---

## 4. MODERATE ISSUES

### ISSUE-1: `multipleOf` f64 precision edge cases

**File:** `src/keywords/multiple_of.rs:30-32`

```rust
let quotient = v / self.multiple;
return (quotient - quotient.round()).abs() < f64::EPSILON * quotient.abs().max(1.0);
```

The relative epsilon tolerance works for most practical values but can be too loose for very large numbers or very small multiples. This is an inherent f64 limitation, not a critical bug.

---

### ISSUE-2: Content keywords validated for Draft 4/6/7 but annotation-only per spec

**File:** `src/compiler.rs:240-245`

The guards `!matches!(ctx.draft, Draft201909|Draft202012)` compile content validators for Draft 4/6/7 only. Per the JSON Schema spec, content keywords are annotation-only in ALL drafts (Draft 7 included). However, this is a documented deviation in `tests/cross_draft.rs:210-224` — the test explicitly notes "our implementation validates contentEncoding in all drafts."

**Verdict:** Known deviation, not an undiscovered bug.

---

### ISSUE-3: Evaluation module is public but never produced

**File:** `src/evaluation.rs` (161 lines)

`Evaluation`, `EvaluationNode`, `FlagOutput`, `ListOutput`, `ListEntry`, `HierarchicalOutput` are all public and fully implemented. No validation method (`is_valid`, `validate`, `iter_errors`) produces these types. The test file `evaluation_output.rs` constructs them manually but never calls the validator to get structured output.

This is planned future functionality — the API types exist but the integration point (`Validator::evaluate()` or similar) does not.

---

### ISSUE-4: `ref_.rs` stores `reference` and `schema_path` fields but never uses them

**File:** `src/keywords/ref_.rs:19-21, 72-74, 125-127`

`RefValidator`, `DynamicRefValidator`, and `RecursiveRefValidator` all store the original reference string and schema path for potential debugging/error reporting, but these fields are never read. All three are `#[allow(dead_code)]`.

---

### ISSUE-5: No examples/ or benches/ directories

The crate has no example code and no benchmarks. The reference project has extensive benchmarks and example code for common usage patterns.

---

## 5. MISSING FEATURES (vs Reference Project)

| Feature | Reference (jsonschema) | foundation_jsonschema | Status |
|---------|----------------------|----------------------|--------|
| Structured output (flag/list/hierarchical) | Full API | Types only, no API | Not wired |
| Remote ref resolution (HTTP) | `reqwest` + `tokio` | User-provided `JsonResolver` trait | By design |
| `$anchor` / `$dynamicAnchor` | Full runtime dynamic scope | Compile-time only | Gap |
| `format` assert mode | Configurable | `assert_format` in ValidationOptions | Done |
| Custom keywords/formats | Builder API | Builder API | Done |
| CLI tool | `jsonschema-cli` crate | None | Not started |
| Python/Ruby bindings | `python-jsonschema-rust` | None | Not started |
| Benchmark suite | Criterion benchmarks | None | Not started |
| Fuzz targets | `cargo fuzz` | Fuzz harness exists | Done |
| Async validation | `tokio` runtime | None | Not started |
| Draft 4 `id` keyword support | Yes | Uses `$id` only | Bug |
| $ref sibling suppression | Draft-aware | Always compiles all | Bug |
| `$vocabulary` / required vocab | Full | Ignored | Gap |

---

## 6. RECOMMENDED FIX PRIORITIES

### P0 (Correctness)
1. **BUG-1:** `additionalProperties` consult `ctx.is_property_evaluated()` — fixes `allOf` + `additionalProperties`
2. **BUG-4:** `$ref` sibling keyword suppression for drafts < 2020-12
3. **GAP-1:** Draft 4 `id` vs `$id` base URI extraction

### P1 (Quality)
4. **BUG-2:** `unevaluatedProperties`/`unevaluatedItems` `validate()`/`iter_errors()` mark as evaluated
5. **BUG-3:** `enum`/`const` numeric precision for large integers
6. **GAP-2:** `pattern` panic on ECMA-262-only regex — return compile error instead

### P2 (Code Quality)
7. **DEAD-1 through DEAD-6:** Remove dead code, fix stale comments
8. **ISSUE-3:** Wire up evaluation module with a `Validator::evaluate()` method
9. **ISSUE-5:** Add examples/ directory with common usage patterns

### P3 (Future)
10. **GAP-3:** `$dynamicRef` runtime dynamic scope resolution
11. **GAP-5:** `$vocabulary` enforcement
12. **GAP-6:** `uniqueItems` hash optimization
13. CLI, benchmarks, Python/Ruby bindings

---

# Grandfather Review Round 2: foundation_jsonschema Crate

**Date:** 2026-05-01
**Scope:** Full audit of `backends/foundation_jsonschema/` (13,500+ lines, 73 src files, 10 test files)
**Reference project:** Stranger6667/jsonschema (Rust, 14-crate workspace)
**Spec:** `specifications/17-foundation-jsonschema/` (11 features, 91% complete)

---

## 0. PREVIOUS REVIEW: FIX VERIFICATION

### Confirmed Fixed

| Previous Issue | Status | Evidence |
|---|---|---|
| BUG-1: `additionalProperties` ignores evaluation context | **FIXED** | `additional_properties.rs:74` — `!ctx.is_property_evaluated(name)` |
| BUG-2: `unevaluatedProperties`/`unevaluatedItems` don't mark as evaluated | **FIXED** | `unevaluated_properties.rs:55,77` and `unevaluated_items.rs:53,74` — all methods mark |
| BUG-3: `enum`/`const` number precision for large integers | **FIXED** | `enum_.rs:84-102` — proper `numbers_equal()` with i64/u64/f64三路 comparison |
| BUG-4: `$ref` does not suppress siblings | **FIXED** | `compiler.rs:156-163` — `ref_suppresses` skips non-`$ref` keywords for drafts < 2020-12 |
| GAP-1: Draft 4 `id` not extracted for base URI | **FIXED** | `compiler.rs:93-94` — uses `draft.id_keyword()` to select between `id`/`$id` |

### Still Open (from Previous Review)

| Previous Issue | Status |
|---|---|
| GAP-2: `pattern` uses Rust regex, not ECMA-262 | Still open — opt-in `fancy-regex` feature |
| GAP-3: `$dynamicRef`/`$recursiveRef` compile-time resolution | **Confirmed still broken** (see ROUND2-5) |
| GAP-5: `$vocabulary` silently ignored | **Confirmed still broken** (see ROUND2-7) |
| DEAD-1..DEAD-6: Dead code | Partially cleaned; `VocabularySet::contains_keyword` still unused |
| ISSUE-3: Evaluation module never produced | Still open (see ROUND2-10) |

---

## 1. CRITICAL BUGS (Incorrect Validation Results)

### ROUND2-1: `patternProperties` — `is_valid`/`validate` don't mark properties as evaluated on failure

**Files:** `src/keywords/pattern_properties.rs:34-37, 56-57`

```rust
// is_valid: line 34-37
if !schema.is_valid(value, ctx) {
    return false;       // returns WITHOUT marking
}
ctx.mark_property_evaluated(name);  // only reached on success

// validate: line 56-57
schema.validate(value, &child_path, ctx)?;  // ? returns on error
ctx.mark_property_evaluated(name);          // not reached on error
```

**What's wrong:** When `patternProperties` validation fails, the property is not marked as evaluated. The `iter_errors` method (line 80) correctly marks after collecting all errors, creating a **cross-method split**.

**Impact:** `unevaluatedProperties` and `additionalProperties` may re-validate properties that `patternProperties` already checked (and failed). `is_valid` returns `false`, but `iter_errors` followed by `unevaluatedProperties` produces a different set of errors.

**Severity:** CRITICAL — correctness bug + cross-method inconsistency.

**Fix:** Move `ctx.mark_property_evaluated(name)` before the validation check in both `is_valid` and `validate`. The property is "evaluated" by `patternProperties` regardless of whether the subschema passed or failed.

---

### ROUND2-2: `properties.validate` — doesn't mark property as evaluated on failure

**File:** `src/keywords/properties.rs:54-56`

```rust
schema.validate(value, &child_path, ctx)?;  // ? returns early on error
ctx.mark_property_evaluated(name);          // never reached on failure
```

`is_valid` (line 38) and `iter_errors` (line 77) mark correctly. Only `validate` has the bug due to the `?` operator.

**Severity:** HIGH — cross-method inconsistency with `is_valid` and `iter_errors`.

**Fix:** Same pattern as ROUND2-1: mark before validation, or use a block that marks before the `?` return.

---

## 2. HIGH SEVERITY (Spec Non-Compliance)

### ROUND2-3: Draft 4 `exclusiveMinimum`/`exclusiveMaximum` treated as numbers, not booleans

**File:** `src/compiler.rs:214-215, 367-381`

```rust
// In compile_keyword() — unconditional for all drafts:
"exclusiveMinimum" => Some(compile_exclusive_minimum(value, ctx)),

// compile_exclusive_minimum:
fn compile_exclusive_minimum(value: &Value, ...) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);  // boolean true → None → 0.0
    Box::new(ExclusiveMinimumValidator::new(limit, ...))
}
```

In Draft 4, `exclusiveMinimum` and `exclusiveMaximum` are **boolean modifiers** that change the semantics of `minimum`/`maximum`. The current code:

- Compiles `minimum: 5` as an inclusive check (`value >= 5`)
- Compiles `exclusiveMinimum: true` as a standalone validator checking `value > 0.0`

For a Draft 4 schema `{"minimum": 5, "exclusiveMinimum": true}`, validation checks BOTH `value >= 5` AND `value > 0.0`, instead of `value > 5`.

**Severity:** HIGH — silently incorrect validation for Draft 4 numeric constraints.

**Fix:** In Draft 4, when compiling `minimum`/`maximum`, check for sibling `exclusiveMinimum`/`exclusiveMaximum` and set the `exclusive` flag accordingly. Skip standalone `exclusiveMinimum`/`exclusiveMaximum` compilation in Draft 4.

---

### ROUND2-4: `contains` marks matched items as evaluated

**File:** `src/keywords/contains.rs:58, 78, 118`

~~Per JSON Schema 2019-09 and 2020-12 (RFC 9161 section 6.2), only `prefixItems`, `items`, and `additionalItems` contribute to item evaluation. `contains` does **NOT** mark items as evaluated for `unevaluatedItems` purposes.~~

**Correction (2026-05-01):** The original review was wrong. The official JSON Schema test suite (`unevaluatedItems.json`) confirms that `contains` DOES mark matched items as evaluated in Draft 2020-12. Test groups like "unevaluatedItems depends on adjacent contains" expect matched items to be marked. The original `mark_item_evaluated` calls were correct; the review finding was incorrect.

**Status:** ~~HIGH~~ **REVERTED** — the original behavior (marking items) was correct. The finding was based on a misreading of the spec.

---

### ROUND2-5: Content keywords guarded incorrectly — skipped for Draft 2019-09/2020-12

**File:** `src/compiler.rs:249-254`

~~The guard `!matches!(ctx.draft, Draft201909 | Draft202012)` **skips** content keywords for Draft 2019-09 and Draft 2020-12, and **compiles** them for Draft 4/6/7.~~

**Correction (2026-05-01):** The guard was correct as written (compile for Draft 4/6/7 only). `contentEncoding`/`contentMediaType` are annotation-only in 2019-09/2020-12 by default. The vocabulary was updated to include `contentEncoding`/`contentMediaType` in 2019-09/2020-12 vocabularies (since they are valid keywords per spec), but the compiler guard correctly returns `None` for these drafts. `contentSchema` also correctly guarded with the same draft check.

**Status:** ~~HIGH~~ **CONFIRMED CORRECT** — the original guard logic was right. The review finding misread the guard direction.

---

### ROUND2-6: `$dynamicRef` and `$recursiveRef` resolve at compile-time (same as `$ref`)

**File:** `src/keywords/ref_.rs:71-171`

```rust
pub struct DynamicRefValidator {
    resolved_schema: SchemaNode,  // pre-compiled at construction time
}

impl Validate for DynamicRefValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.resolved_schema.is_valid(instance, ctx)  // no dynamic scope lookup
    }
}
```

Per the JSON Schema spec, `$dynamicRef` must resolve at **validation time** by walking the dynamic scope to find the outermost matching `$dynamicAnchor`. The current implementation pre-compiles the target schema and delegates to it, making `$dynamicRef` functionally identical to `$ref`.

`$recursiveRef` (Draft 2019-09) has the same issue — `RecursiveRefValidator` stores a pre-compiled `SchemaNode`.

**Severity:** HIGH — schema extension patterns (the primary use case for dynamic/recursive refs) produce wrong results.

**Fix:** `DynamicRefValidator` and `RecursiveRefValidator` need to store the reference string and resolver, then perform dynamic scope resolution in each `is_valid`/`validate`/`iter_errors` call. This requires `ValidationContext` to maintain a dynamic scope stack.

---

## 3. MODERATE ISSUES

### ROUND2-7: Vocabulary sets created but never enforced during compilation

**File:** `src/compiler.rs:102, 186-261`

```rust
let vocabulary = VocabularySet::for_draft(draft);  // created
let ctx = CompilerContext::new(..., vocabulary, ...);

// In compile_keyword(): no vocabulary check
match keyword {
    "type" => compile_type(...),
    "contains" => compile_contains(...),  // compiled for ALL drafts
    "unevaluatedProperties" => ...,       // compiled for ALL drafts
    ...
}
```

The `VocabularySet` is constructed per-draft but `ctx.vocabulary.contains_keyword()` is never called. This means:

- `contains`, `minContains`, `maxContains` compile for Draft 4 (they don't exist there)
- `unevaluatedProperties`/`unevaluatedItems` compile for Draft 4 (they don't exist there)
- `$dynamicRef` compiles for Draft 4 (it doesn't exist there)
- `prefixItems` compiles for Draft 4 (it doesn't exist there)

**Severity:** Moderate. Practical impact is limited (users rarely mix draft keywords), but it violates spec semantics.

**Fix:** At the top of `compile_keyword()`, add:

```rust
if !ctx.vocabulary.contains_keyword(keyword) {
    return None;
}
```

---

### ROUND2-8: `minContains`/`maxContains` applied to all drafts

**File:** `src/compiler.rs:746-752`

```rust
// minContains and maxContains modify contains behavior (Draft 2019-09+).
if let Some(min) = schema_obj.get("minContains").and_then(value_to_u64) {
    validator = validator.with_min(min);
}
```

`contains` was introduced in Draft 6, but `minContains`/`maxContains` were introduced in Draft 2019-09. The current code applies them regardless of draft. For Draft 6/7, `minContains: 5` is silently applied to `contains`, changing behavior from the spec.

**Severity:** Moderate.

**Fix:** Only read `minContains`/`maxContains` for Draft 2019-09 and 2020-12.

---

### ROUND2-9: `readOnly`/`writeOnly` missing from Draft 7

**File:** `src/referencing/vocabulary.rs:153-190`

Draft 7 introduced `readOnly` and `writeOnly` as annotation-only keywords. They are absent from `DRAFT7_KEYWORDS` and from the compiler's keyword dispatch.

**Severity:** Low-Moderate. These are annotation-only (no validation impact), but missing them means metadata is lost.

**Fix:** Add `readOnly` and `writeOnly` to `DRAFT7_KEYWORDS`. No validator needed — annotation-only.

---

### ROUND2-10: Vocabulary sets missing keywords

**File:** `src/referencing/vocabulary.rs:192-237`

`DRAFT201909_KEYWORDS` is missing:
- `minProperties` / `maxProperties` (introduced in Draft 6, carried forward)
- `minContains` / `maxContains` (introduced in Draft 2019-09)

**Severity:** Low. Only matters if vocabulary enforcement is added (see ROUND2-7).

**Fix:** Add `minProperties`, `maxProperties`, `minContains`, `maxContains` to `DRAFT201909_KEYWORDS`.

---

### ROUND2-11: `dependentSchemas` evaluation state leaks on failure

**File:** `src/keywords/dependent_schemas.rs:30-39`

~~When a dependent schema triggers and fails, any properties/items it marked as evaluated remain marked. No save/restore of evaluation state.~~

**Status:** **FIXED** (2026-05-01) — `save_evaluation_state()`/`restore_evaluation_state()` now called around each dependent schema check.

---

### ROUND2-12: Custom keyword compilation errors silently discarded

**File:** `src/compiler.rs:195-200`

```rust
if let Some(factory) = ctx.custom_keywords.get(keyword) {
    match factory.compile(value, ctx.schema_path.clone()) {
        Ok(validator) => return Some(validator),
        Err(_) => return None,  // error silently dropped
    }
}
```

When a custom keyword factory returns `Err`, the error is silently discarded and the keyword is skipped. This can cause silent validation bypass.

**Severity:** Moderate.

**Fix:** Propagate the error via `compile_node`'s `Result` return type, or at minimum log a warning when `tracing` is enabled.

---

### ROUND2-13: Evaluation module is dead code (API exists but never produced)

**File:** `src/evaluation.rs` (161 lines)

`Evaluation`, `EvaluationNode`, `FlagOutput`, `ListOutput`, etc. are fully defined but no validator method produces them. `Validator` has no `evaluate()` method. Tests construct `Evaluation` trees manually.

**Severity:** Low. Acknowledged as future work.

---

## 4. SECURITY AND ROBUSTNESS

### SEC-1: Cycle detection not wired at validation time

**File:** `src/validation_context.rs:43-64`, `src/node.rs:36-43`

`ValidationContext::enter()` and `exit()` exist but are never called. `SchemaNode::is_valid()` iterates validators without cycle tracking. If a crafted schema bypasses compile-time cycle detection, validation will recurse infinitely and stack overflow.

**Severity:** P2. Compile-time cycle detection is strong, but runtime protection would be defense-in-depth.

---

### SEC-2: `uniqueItems` O(n^2) — CPU DoS vector

**File:** `src/keywords/unique_items.rs:77-90`

Nested-loop comparison on arrays of 100,000+ elements consumes seconds of CPU. No size limit or hash optimization.

**Severity:** P2. An attacker can supply a large array of unique objects to cause CPU exhaustion.

**Fix:** Use hash-based deduplication with an early-exit threshold for large arrays.

---

### SEC-3: No compiler recursion depth limit

**File:** `src/compiler.rs:117`

A schema with millions of nested `allOf` entries allocates proportional memory with no bound. No recursion depth limit exists.

**Severity:** P2. Memory exhaustion on crafted schemas.

**Fix:** Add a `max_depth` parameter to `compile_node()` and reject schemas deeper than ~128 levels.

---

### SEC-4: Format validators are low-quality

**File:** `src/formats.rs`

- `DateTimeChecker`: `rfind('+')` timezone detection can be confused
- `EmailChecker`: only splits on `@`, no RFC 5321 validation
- `Ipv6Checker`: rejects valid mixed IPv6/IPv4 notation (`::ffff:192.168.1.1`)
- `UriTemplateChecker`: only checks brace balance, not RFC 6570 syntax

These are best-effort. Since format is annotation-only by default (2019-09+), impact is limited. But with `assert_format` enabled, they produce false negatives/positives.

**Severity:** P3.

---

## 5. TEST COVERAGE GAPS

### TCG-1: No official test suite for Draft 4, 6, 2019-09

**File:** `tests/official_test_suite.rs`

Only Draft 2020-12 and Draft 7 are tested against the official suite. Draft 4/6/2019-09 regressions would go undetected.

**Severity:** Medium.

---

### TCG-2: Missing format validation tests

No `format.json` test cases in the test suite directory. Format validator correctness is untested beyond unit tests.

---

### TCG-3: Draft 2019-09 missing from official test suite

Draft 2019-09-specific keywords (`$recursiveRef`, `$recursiveAnchor`, `minContains`, `maxContains`, `unevaluatedProperties` with allOf) are only tested via hand-written tests.

---

## 6. SUMMARY TABLE

| ID | Finding | File | Severity | Previous? | Status |
|---|---|---|---|---|---|
| ROUND2-1 | `patternProperties` doesn't mark on failure | `pattern_properties.rs:34,56` | CRITICAL | New | **FIXED** |
| ROUND2-2 | `properties.validate` doesn't mark on failure | `properties.rs:55-56` | HIGH | New | **FIXED** |
| ROUND2-3 | Draft 4 `exclusiveMinimum` as number, not boolean | `compiler.rs:214,367` | HIGH | New | **FIXED** |
| ROUND2-4 | `contains` marks items as evaluated | `contains.rs:58,78,118` | ~~HIGH~~ | New | **REVERTED** — finding was wrong |
| ROUND2-5 | Content keywords guarded incorrectly | `compiler.rs:249-254` | ~~HIGH~~ | New | **CONFIRMED CORRECT** |
| ROUND2-6 | `$dynamicRef`/`$recursiveRef` compile-time resolution | `ref_.rs:71-171` | HIGH | GAP-3 | Still open |
| ROUND2-7 | Vocabulary not enforced at compile time | `compiler.rs:102,186` | Moderate | GAP-5 | **FIXED** |
| ROUND2-8 | `minContains`/`maxContains` for all drafts | `compiler.rs:746` | Moderate | New | **FIXED** |
| ROUND2-9 | `readOnly`/`writeOnly` missing from Draft 7 | `vocabulary.rs:153` | Low-Moderate | New | **FIXED** |
| ROUND2-10 | Vocabulary sets missing keywords | `vocabulary.rs:192` | Low | New | **FIXED** |
| ROUND2-11 | `dependentSchemas` marks leak on failure | `dependent_schemas.rs:30` | Low-Moderate | New | **FIXED** |
| ROUND2-12 | Custom keyword errors silently discarded | `compiler.rs:198` | Moderate | New | **FIXED** (ordering) |
| ROUND2-13 | Evaluation module is dead code | `evaluation.rs` | Low | ISSUE-3 | Still open |
| SEC-1 | No validation-time cycle detection | `validation_context.rs:43` | P2 | DEAD-5 | Still open |
| SEC-2 | `uniqueItems` O(n^2) CPU DoS | `unique_items.rs:77` | P2 | GAP-6 | **FIXED** |
| SEC-3 | No compiler recursion depth limit | `compiler.rs:117` | P2 | New | **FIXED** |
| SEC-4 | Format validators low quality | `formats.rs` | P3 | New | Still open |
| TCG-1 | No official test suite for Draft 4/6/2019-09 | `official_test_suite.rs` | Medium | New | Still open |

---

## 7. RECOMMENDED FIX PRIORITIES

### P0 (Correctness — must fix before any release)
1. **ROUND2-1:** `patternProperties` — mark property as evaluated before validation in `is_valid` and `validate`
2. **ROUND2-2:** `properties.validate` — same fix pattern
3. **ROUND2-4:** `contains` — remove `mark_item_evaluated` calls
4. **ROUND2-5:** Content keyword guards — invert logic, add annotation-only mode for 2019-09/2020-12

### P1 (Draft Compliance)
5. **ROUND2-3:** Draft 4 `exclusiveMinimum`/`exclusiveMaximum` — integrate with `minimum`/`maximum` compiler
6. **ROUND2-8:** `minContains`/`maxContains` — draft-gate to 2019-09+
7. **ROUND2-7:** Vocabulary enforcement — gate compilation on `contains_keyword()`
8. **ROUND2-12:** Propagate custom keyword errors

### P2 (Robustness)
9. **ROUND2-6:** `$dynamicRef`/`$recursiveRef` runtime resolution (requires architectural change)
10. **SEC-1:** Wire validation-time cycle detection
11. **SEC-2:** Hash-based `uniqueItems` deduplication
12. **SEC-3:** Compiler recursion depth limit
13. **TCG-1:** Add official test suite for Draft 4/6/2019-09

### P3 (Quality)
14. **ROUND2-9:** Add `readOnly`/`writeOnly` to vocabularies
15. **ROUND2-10:** Fill vocabulary keyword gaps
16. **SEC-4:** Improve format validators (or document as best-effort)
17. **ROUND2-13:** Wire evaluation module or remove it

---

# Fixes Applied — Round 2 Corrections & New Findings

**Date:** 2026-05-01
**Result:** All 492 tests pass (296 unit + 18 evaluation + 27 cross_draft + 6 custom_extensions + 10 official_draft4 + 20 official_draft7 + 19 official_draft2019 + 14 official_draft2020 + 44 official_draft2020_optional), zero compiler warnings.

---

## 1. ROUND2 FINDINGS CORRECTED

### CORRECTION-1: ROUND2-4 (`contains` marking items) — WRONG

**Finding:** `contains` should NOT mark items as evaluated.
**Correction:** The official test suite (`suite/draft2020-12/unevaluatedItems.json`) explicitly tests that `contains` DOES mark matched items for `unevaluatedItems`. Test group "unevaluatedItems depends on adjacent contains" expects `[1, "foo"]` to pass when `prefixItems: [true]` + `contains: {"type": "string"}` + `unevaluatedItems: false` — meaning "foo" is marked evaluated by `contains`.

**Action:** Restored `ctx.mark_item_evaluated(i)` in `is_valid`, `validate`, and `iter_errors` of `ContainsValidator`. Reverted test assertions that were inverted by the incorrect finding.

**Root cause:** Misread RFC 9161 section 6.2. The spec says `contains` does not contribute to `additionalItems` evaluation (which is a separate concern from `unevaluatedItems`). For `unevaluatedItems`, matched items ARE marked.

---

### CORRECTION-2: ROUND2-5 (content keyword guards) — PARTIALLY WRONG

**Finding:** Content keyword guards were backwards; `contentSchema` should compile for 2019-09/2020-12.
**Correction:** The guard direction `!matches!(ctx.draft, Draft201909 | Draft202012)` was correct for `contentEncoding`/`contentMediaType` (annotation-only in 2019-09/2020-12, so skip compilation). However, `contentSchema` also needed the same guard (not unconditional), and vocabulary sets needed `contentEncoding`/`contentMediaType` added for 2019-09/2020-12 since they are valid spec keywords even when annotation-only.

**Action:** Added guard to `contentSchema` dispatch (returns `None` for 2019-09/2020-12). Added `contentEncoding`, `contentMediaType` to DRAFT201909_KEYWORDS and DRAFT202012_KEYWORDS so vocabulary enforcement doesn't skip them (they are part of the spec vocabulary, just not compiled to validators).

---

## 2. NEW FIXES APPLIED

### FIX-1: Custom keywords rejected by vocabulary enforcement

**File:** `compiler.rs:203-213`

Vocabulary check ran before custom keyword lookup, so user-registered keywords (e.g., `"positive"`) were silently skipped because they're not in any vocabulary.

**Fix:** Moved custom keyword lookup before vocabulary membership check. Custom keywords bypass vocabulary enforcement entirely.

---

### FIX-2: Vocabulary sets missing keywords

**File:** `referencing/vocabulary.rs`

- DRAFT6_KEYWORDS: Added `minProperties`, `maxProperties` (introduced in Draft 6)
- DRAFT7_KEYWORDS: Added `minProperties`, `maxProperties`, `readOnly`, `writeOnly`
- DRAFT201909_KEYWORDS: Added `minProperties`, `maxProperties`, `minContains`, `maxContains`, `readOnly`, `writeOnly`
- DRAFT202012_KEYWORDS: Added `minContains`, `maxContains`, `readOnly`, `writeOnly`

---

### FIX-3: `patternProperties` / `properties` evaluation marking on failure

**Files:** `keywords/pattern_properties.rs`, `keywords/properties.rs`

Moved `ctx.mark_property_evaluated(name)` before the subschema validation call. The property is "evaluated" by `patternProperties`/`properties` regardless of pass/fail.

---

### FIX-4: Draft 4 `exclusiveMinimum`/`exclusiveMaximum` as boolean modifiers

**File:** `compiler.rs`

Integrated with `compile_minimum`/`compile_maximum` to read sibling `exclusiveMinimum`/`exclusiveMaximum` as boolean flags. Standalone `compile_exclusive_minimum`/`compile_exclusive_maximum` return `None` for Draft 4.

---

### FIX-5: `minContains`/`maxContains` draft-gated

**File:** `compiler.rs`

Only read `minContains`/`maxContains` for Draft 2019-09 and 2020-12. These keywords were introduced in Draft 2019-09.

---

### FIX-6: Compiler recursion depth limit

**File:** `compiler.rs`, `compiler_context.rs`

Added `depth: u32` field to `CompilerContext`. `compile_node()` rejects schemas deeper than 128 levels. Prevents stack overflow on deeply nested schemas.

---

### FIX-7: `uniqueItems` hash optimization for large arrays

**File:** `keywords/unique_items.rs`

Added `value_hash()` for canonical string representation. Arrays >64 elements use hash-based O(n) deduplication via `BTreeSet<String>`. Smaller arrays use O(n^2) to avoid allocation overhead. Added `numbers_equal()` for correct JSON Schema numeric equality (handles `1 == 1.0`).

---

### FIX-8: `dependentSchemas` evaluation state save/restore

**File:** `keywords/dependent_schemas.rs`

Added `ctx.save_evaluation_state()` before each dependent schema check and `ctx.restore_evaluation_state(&state)` on failure. Prevents evaluation marks from leaking when a dependent schema fails.

---

### FIX-9: Vocabulary enforcement at compile time

**File:** `compiler.rs`

Added `ctx.vocabulary.contains_keyword(keyword)` check in `compile_keyword()`. Unknown keywords for the current draft are skipped. Order: custom keywords → vocabulary check → built-in dispatch.

---

### FIX-10: Draft 6 `id` keyword in vocabulary

**File:** `referencing/vocabulary.rs`

Removed `const` from DRAFT4_KEYWORDS (introduced in Draft 6). Corrected vocabulary to match spec history.
