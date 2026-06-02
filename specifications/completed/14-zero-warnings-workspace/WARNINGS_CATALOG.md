# Warnings Catalog - Zero Warnings Workspace

_Captured: 2026-04-12 | Baseline snapshot before cleanup_

---

## How to Use This Catalog

1. Find your crate in the **By Crate** section
2. See the per-file breakdown with line numbers
3. Fix mechanically fixable warnings first (`cargo clippy --fix`)
4. Fix remaining warnings by hand following the guidance
5. Check off items as you go

---

## By Crate

### foundation_auth (1 warning)

**Status:** ✅ Complete — Fixed `clippy::doc_markdown` on line 377 (`SQLite` → `` `SQLite` ``)

| File | Line | Lint | Fix Guidance |
|------|------|------|--------------|
| `src/credential_store.rs` | 377 | `clippy::doc_markdown` | ~~Change `SQLite` → `` `SQLite` ``~~ Done |

**Command:** `cargo clippy --package foundation_auth --all-targets -- -D warnings`

---

### foundation_testing (1 warning)

**Status:** ⬜ Pending

| File | Line | Lint | Fix Guidance |
|------|------|------|--------------|
| `src/lib.rs` (test) | TBD | `clippy::unused_unit` | Remove `-> ()` return type |

**Command:** `cargo clippy --package foundation_testing --all-targets -- -D warnings`

---

### foundation_macros (1 warning)

**Status:** ⬜ Pending

| File | Line | Lint | Fix Guidance |
|------|------|------|--------------|
| `tests/json_hash_tests.rs` | TBD | Mixed | Run `--fix` first, review remainder |

**Command:** `cargo clippy --package foundation_macros --all-targets -- -D warnings`

---

### foundation_db (26 warnings)

**Status:** ⬜ Pending

All warnings are in test files. Library code is clean.

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `tests/d1_kvstore_tests.rs` | 8 | `redundant_closure`, `doc_markdown` | `--fix` handles most |
| `tests/store_state_task.rs` | 11 | `needless_pass_by_value`, `doc_markdown` | Signature changes + backticks |
| `tests/r2_blobstore_tests.rs` | 1 | `doc_markdown` | Add backticks |
| `tests/blobstore_tests.rs` | 1 | `redundant_closure` | `--fix` |
| `tests/json_blobstore_tests.rs` | 1 | `redundant_closure` | `--fix` |
| `tests/json_file_storage_tests.rs` | 1 | (none shown) | Run `--fix` |
| `tests/memory_storage_tests.rs` | 1 | (none shown) | Run `--fix` |
| `tests/turso_storage_tests.rs` | 1 | (none shown) | Run `--fix` |
| `tests/common/mod.rs` | ~2 | Various | Check shared test utilities |

**Command:** `cargo clippy --package foundation_db --all-targets -- -D warnings`

---

### foundation_nostd (23 warnings)

**Status:** ⬜ Pending

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `src/**/*.rs` (lib test) | 21 | `explicit_iter_loop`, `cast_possible_truncation` | `.iter()` removal, `try_into` |
| `examples/comp_usage.rs` | 1 | TBD | Run `--fix` |
| `examples/cross_platform.rs` | 1 | TBD | Run `--fix` |

**Command:** `cargo clippy --package foundation_nostd --all-targets -- -D warnings`

---

### foundation_codegen (21 warnings)

**Status:** ⬜ Pending

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `tests/foundation_codegen_tests.rs` | 21 | `doc_markdown`, `needless_pass_by_value` | Backticks, `&T` signatures |

**Command:** `cargo clippy --package foundation_codegen --all-targets -- -D warnings`

---

### foundation_deployment (42 warnings)

**Status:** ⬜ Pending

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `src/**/*.rs` (lib) | 37 | `doc_markdown` (~25), `match_same_arms` | Backticks, merge arms |
| `src/**/*.rs` (lib test) | 38 (dup) | Same as lib | Same fixes |
| `tests/huggingface_integration.rs` | 4 | `doc_markdown`, `needless_pass_by_value` | Backticks, signatures |

**Command:** `cargo clippy --package foundation_deployment --all-targets -- -D warnings`

---

### foundation_openapi (173 warnings)

**Status:** ⬜ Pending

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `src/**/*.rs` (lib) | 163 | `doc_markdown` (~100), `needless_pass_by_value` (~40) | Backticks, `&T` signatures |
| `src/**/*.rs` (lib test) | 163 (dup) | Same as lib | Same fixes |
| `tests/integration_spec_processing.rs` | 10 | `doc_markdown`, `redundant_closure` | Backticks, `--fix` |

**Command:** `cargo clippy --package foundation_openapi --all-targets -- -D warnings`

---

### ewe_platform (bin) (63 warnings)

**Status:** ⬜ Pending

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `bin/platform/src/**/*.rs` | 63 | Dead code (~12), `doc_markdown` (~25), `needless_pass_by_value` (~15) | Remove unused, backticks, signatures |

**Special notes:**
- 12 warnings are dead code in `gen_resources/types.rs` — may be generated
- Check if `gen_resources/` files are generated; if so, fix the generator

**Command:** `cargo clippy --package ewe_platform --all-targets -- -D warnings`

---

### foundation_ai (224 warnings)

**Status:** ⬜ Pending

**llama.cpp FFI exception applies** — some suppressions may remain in
`infrastructure/llama-cpp/` and `infrastructure/llama-bindings/`.

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `src/**/*.rs` (lib) | 215 | `doc_markdown` (~150), `needless_pass_by_value` (~40) | Backticks, `&T` signatures |
| `src/**/*.rs` (lib test) | 215 (dup) | Same as lib | Same fixes |
| `tests/llamacpp_integration.rs` | 9 | `doc_markdown`, `needless_pass_by_value` | Backticks, signatures |

**Command:** `cargo clippy --package foundation_ai --all-targets -- -D warnings`

---

### ewe_platform_tests (320 warnings)

**Status:** ⬜ Pending

**⚠️ Check if generated first** — This is a test harness crate. If the code
is generated, fix the generator, regenerate, then verify clean.

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `tests/**/*.rs` (lib test) | 320 | TBD — needs capture | Run `--fix` first, categorize remainder |

**Command:** `cargo clippy --package ewe_platform_tests --all-targets -- -D warnings`

---

### foundation_core (601 warnings)

**Status:** ⬜ Pending

Largest crate. Split into sub-phases by module.

| File | Warnings | Top Lints | Fix Guidance |
|------|----------|-----------|--------------|
| `tests/mod.rs` | 465 | `doc_markdown` (~200), `needless_pass_by_value` (~80), `match_same_arms` (~50) | Backticks, signatures, merge arms |
| `tests/units.rs` | 41 | `needless_pass_by_value`, `doc_markdown` | Signatures, backticks |
| `tests/chunked_tests.rs` | 26 | `needless_pass_by_value`, `doc_markdown` | Signatures, backticks |
| `tests/sync_boundary_helpers.rs` | 24 | `needless_pass_by_value`, `cast_possible_truncation` | Signatures, `try_into` |
| `tests/flatten_combinators.rs` | 24 | Various | Run `--fix` |
| `tests/stream_iterators.rs` | 7 | Various | Run `--fix` |
| `tests/task_iterators.rs` | 4 | Various | Run `--fix` |
| `tests/threaded_future.rs` | 6 | Various | Run `--fix` |
| `tests/threaded_future_std.rs` | 1 | Various | Run `--fix` |
| `tests/chunked_encoding.rs` | 1 | Various | Run `--fix` |
| `tests/map_circuit.rs` | 1 | Various | Run `--fix` |
| `src/**/*.rs` (lib test) | 11 | Various | Run `--fix` |

**Sub-phase recommendation:**
1. `src/` lib (11 warnings) — quick win
2. Small test files (`chunked_encoding`, `map_circuit`, etc.) — ~15 warnings
3. Medium test files (`stream_iterators`, `task_iterators`, `threaded_*`) — ~20 warnings
4. Large test files (`sync_boundary_helpers`, `flatten_combinators`, `chunked_tests`, `units`) — ~115 warnings
5. `tests/mod.rs` (465 warnings) — largest, do last

**Command:** `cargo clippy --package foundation_core --all-targets -- -D warnings`

---

## By Lint Type (Cross-Crate)

### doc_markdown (~600 warnings)

**Fix:** Add backticks to code items in doc comments.

```rust
/// Before: Use the SQLite backend.
/// After:  Use the [`SQLite`] backend.
/// After:  Use the `SQLite` backend.
```

**Files affected:** Nearly all crates.

---

### needless_pass_by_value (~200 warnings)

**Fix:** Change `fn foo(self)` to `fn foo(&self)` or use `impl Into<T>`.

```rust
// Before
fn process(self, data: String) -> Result<()> { ... }

// After
fn process(&self, data: &str) -> Result<()> { ... }
// or
fn process(&self, data: impl Into<String>) -> Result<()> { ... }
```

**Files affected:** `foundation_core`, `foundation_ai`, `foundation_openapi`, `foundation_deployment`.

---

### unused_unit (~150 warnings)

**Fix:** Remove `-> ()` return type annotations.

```rust
// Before
fn foo() -> () { ... }

// After
fn foo() { ... }
```

---

### explicit_iter_loop (~120 warnings)

**Fix:** Remove `.iter()` when already borrowed.

```rust
// Before
for x in vec.iter() { ... }

// After
for x in &vec { ... }
```

---

### redundant_closure (~100 warnings)

**Fix:** Simplify closures.

```rust
// Before
.map(|x| foo(x))

// After
.map(foo)
```

---

### match_same_arms (~80 warnings)

**Fix:** Merge identical arms.

```rust
// Before
match x {
    A | B => println!("same"),
    C => println!("same"),
    D => println!("different"),
}

// After
match x {
    A | B | C => println!("same"),
    D => println!("different"),
}
```

---

### missing_errors_doc (~70 warnings)

**Fix:** Add `# Errors` section.

```rust
/// # Errors
///
/// Returns a [`FooError`] if the operation fails.
pub fn foo() -> Result<(), FooError> { ... }
```

---

### missing_panics_doc (~60 warnings)

**Fix:** Add `# Panics` section.

```rust
/// # Panics
///
/// Panics if the input is empty.
pub fn foo(s: &str) -> usize { ... }
```

---

### cast_possible_truncation (~50 warnings)

**Fix:** Use `try_into()` or prove bounds.

```rust
// Before
let x: u32 = value as u32;

// After (safe)
let x: u32 = value.try_into().expect("value out of range");
// or prove bounds statically
```

---

## Generated Code Audit

| Location | Generator | Warnings | Action |
|----------|-----------|----------|--------|
| `bin/platform/src/gen_resources/*.rs` | `bin/platform/src/gen_resources/types.rs` | ~12 dead code | Fix generator, regenerate |
| `crates/html-macro/test/**/*.rs` | `crates/html-macro/src/lib.rs` | TBD | Fix macro, re-run tests |
| `crates/template-macro/test/**/*.rs` | `crates/template-macro/src/lib.rs` | TBD | Fix macro, re-run tests |

---

## Progress Checklist

- [x] `foundation_auth` (1) — clean
- [ ] `foundation_testing` (1) — clean
- [ ] `foundation_macros` (1) — clean
- [ ] `foundation_db` (26) — clean
- [ ] `foundation_nostd` (23) — clean
- [ ] `foundation_codegen` (21) — clean
- [ ] `foundation_deployment` (42) — clean
- [ ] `foundation_openapi` (173) — clean
- [ ] `ewe_platform` bin (63) — clean
- [ ] `foundation_ai` (224) — clean (document FFI exceptions)
- [ ] `ewe_platform_tests` (320) — clean (generated code policy)
- [ ] `foundation_core` (601) — clean

**Total:** 1 / 1,496 warnings eliminated.
