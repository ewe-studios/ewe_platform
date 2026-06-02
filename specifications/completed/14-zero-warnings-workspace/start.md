---
workspace_name: "ewe_platform"
spec_directory: "specifications/14-zero-warnings-workspace"
this_file: "specifications/14-zero-warnings-workspace/start.md"
created: 2026-04-12
---

# Start: Zero Warnings Workspace

## Quick Start

1. Read `requirements.md` (full specification, warning inventory, success criteria)
2. Read `WARNINGS_CATALOG.md` (detailed per-file breakdown with fix guidance)
3. Pick a crate from the **Phase** queues below (start with Phase 1)
4. Follow the **Per-Crate Workflow** in `requirements.md`
5. Update `PROGRESS.md` when the crate is clean
6. Move to the next crate

---

## Implementation Queue

### Phase 1: Small Crates (<25 warnings each)

Start here to establish the pattern and verify no cross-crate breakage.

| Crate | Warnings | Files | Priority Lints |
|-------|----------|-------|----------------|
| `foundation_auth` | 1 | `src/lib.rs` (test) | `needless_pass_by_value` |
| `foundation_testing` | 1 | `src/lib.rs` (test) | `unused_unit` |
| `foundation_macros` | 1 | `tests/json_hash_tests.rs` | Varies |
| `foundation_db` | 26 | `tests/*.rs` | `redundant_closure`, `doc_markdown` |
| `foundation_nostd` | 23 | `src/*.rs`, `examples/`, tests | `explicit_iter_loop`, `cast_possible_truncation` |

**Entry point:** `cargo clippy --package foundation_auth --all-targets`

---

### Phase 2: Medium Crates (25-200 warnings)

| Crate | Warnings | Files | Priority Lints |
|-------|----------|-------|----------------|
| `foundation_codegen` | 21 | `tests/foundation_codegen_tests.rs` | `doc_markdown`, `needless_pass_by_value` |
| `foundation_deployment` | 42 | `src/providers/**/*.rs`, `tests/` | `doc_markdown`, `match_same_arms` |
| `foundation_openapi` | 173 | `src/**/*.rs`, `tests/` | `doc_markdown` (~100), `needless_pass_by_value` |
| `ewe_platform` (bin) | 63 | `bin/platform/src/**/*.rs` | Dead code (~12), `doc_markdown`, `needless_pass_by_value` |

**Entry point:** `cargo clippy --package foundation_codegen --all-targets`

---

### Phase 3: Large Crates (200+ warnings)

| Crate | Warnings | Files | Priority Lints |
|-------|----------|-------|----------------|
| `foundation_ai` | 224 | `src/**/*.rs`, `tests/` | `doc_markdown` (~150), `needless_pass_by_value` |
| `ewe_platform_tests` | 320 | `tests/**/*.rs` | **May be generated** — check first |
| `foundation_core` | 601 | `src/**/*.rs`, `tests/**/*.rs` | `doc_markdown` (~200), `needless_pass_by_value` (~80) |

**Entry point:** `cargo clippy --package foundation_ai --all-targets`

**Special handling:**
- `foundation_ai`: llama.cpp FFI exception applies — document allowed suppressions
- `ewe_platform_tests`: Verify if generated; if so, fix generator first
- `foundation_core`: Largest crate — split into sub-phases by module

---

## Per-Crate Workflow

```
1. CAPTURE
   cargo clippy --package <crate> --all-targets 2>&1 | tee /tmp/<crate>.clippy

2. CATEGORIZE
   grep -oE "warning\[([a-z_]+)" /tmp/<crate>.clippy | sort | uniq -c | sort -rn

3. MECHANICAL FIX
   cargo clippy --fix --package <crate> --all-targets --allow-dirty
   # Review changes, stage what's safe

4. MANUAL FIX
   # Read remaining warnings, fix by hand:
   # - Add doc comment backticks
   # - Change fn signatures (&self vs self)
   # - Add # Errors / # Panics sections
   # - Remove dead code
   # - Replace todo!() with real code

5. GENERATED CODE CHECK
   # If warnings are in generated files:
   # - Find the generator source
   # - Fix the generator
   # - Regenerate (cargo test or custom script)
   # - Verify clean

6. VERIFY
   cargo check --package <crate>
   cargo clippy --package <crate> --all-targets -- -D warnings
   cargo test --package <crate>

7. COMMIT
   git add <files>
   git commit -m "[cleanup] <crate>: zero warnings

   - Fixed: <lint1>, <lint2>, ...
   - Removed: <dead code items>
   - Generator fix: <if applicable>

   Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Generated Code Locations

| Generator | Output | Fix Command |
|-----------|--------|-------------|
| `bin/platform/src/gen_resources/types.rs` | `bin/platform/src/gen_resources/*.rs` | Fix generator, re-run generation script |
| `crates/html-macro/` | `crates/html-macro/test/**/*.rs` | Fix macro, `cargo test -p html-macro` |
| `crates/template-macro/` | `crates/template-macro/test/**/*.rs` | Fix macro, `cargo test -p template-macro` |

---

## llama.cpp FFI Exception

The following locations may retain `#[allow(...)]` attributes because they
wrap C FFI where the API dictates patterns clippy dislikes:

- `infrastructure/llama-bindings/src/lib.rs` — bindgen output + FFI shims
- `infrastructure/llama-cpp/src/*.rs` — safe wrapper, but some FFI leakage OK

**Rule:** Only suppress at the **minimum scope** (per-function or per-module),
and document **why** in a comment:

```rust
// FFI boundary: C API requires raw pointer deref
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub unsafe fn llama_decode(...) { ... }
```

---

## When You Get Stuck

1. **Warning seems wrong** — Check if it's a false positive; if so, is there
   a way to refactor to satisfy clippy without suppression?
2. **Generated code** — Don't fix the output; fix the **generator**.
3. **`todo!()` with no clear implementation** — Flag in `PROGRESS.md` for
   user decision.
4. **Signature change breaks downstream** — Fix the downstream crate next;
   this is why we go in dependency order.

---

_This spec is feature-less — it's a cleanup effort, not a feature addition._
