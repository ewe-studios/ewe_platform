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
