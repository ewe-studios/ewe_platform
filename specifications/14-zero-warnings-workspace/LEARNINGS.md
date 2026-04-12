# Learnings - Zero Warnings Workspace

_Created: 2026-04-12_

---

## Key Decisions

### 1. No Suppression (With One Exception)

**Decision:** Never use `#[allow(...)]` or `#![allow(...)]` to silence warnings,
except for llama.cpp FFI bindings.

**Why:** Suppressions hide technical debt. A warning is a signal that the code
can be improved. Silencing it without fixing the underlying issue means:
- The same mistake will be repeated elsewhere
- New contributors learn the wrong patterns
- The codebase drifts back to warning-heavy over time

**Exception:** `infrastructure/llama-bindings/` and `infrastructure/llama-cpp/`
wrap C FFI where the API dictates patterns clippy dislikes (raw pointer derefs,
missing safety docs on FFI functions). These are documented in `requirements.md`.

---

### 2. Generated Code: Fix the Generator, Not the Output

**Decision:** When warnings appear in generated code, fix the **generator** and
regenerate. Suppression at the top of generated files is a **last resort** only
if the generator cannot be fixed.

**Why:** Generated code is a multiplier — fixing the generator fixes all
future output. Suppressing warnings in generated files means:
- Every regeneration re-introduces the warnings (or loses your suppressions)
- The generator continues producing problematic code elsewhere
- You're treating symptoms, not the disease

**Process:**
1. Identify the generator (macro, build script, codegen tool)
2. Find the generator source
3. Fix the generator to produce clean code
4. Regenerate (usually `cargo test` or a specific script)
5. Verify the output is clean

**Authorized suppression locations (last resort only):**
- `bin/platform/src/gen_resources/*.rs` — if generator fix is impossible
- `crates/html-macro/test/**/*.rs` — if macro cannot be fixed
- `crates/template-macro/test/**/*.rs` — if macro cannot be fixed

---

### 3. Dead Code Must Be Removed (Hand-Written Only)

**Decision:** `#[allow(dead_code)]` is never the answer for hand-written code.
Remove unused code. For generated code, fix the **generator** — never edit
generated output directly.

**Why:** Dead code is a liability:
- It confuses readers ("why is this here?")
- It can be accidentally resurrected
- It increases cognitive load and IDE noise

**Generated code exception:** If dead code appears in generated files, the
generator template is producing it. Fix the generator, regenerate.

**Exception:** If the code is part of a public API that consumers might use,
consider deprecation first (`#[deprecated]`), then remove in a follow-up.

---

### 4. `todo!()` and `unreachable!()` Are Stubs, Not Code

**Decision:** Replace `todo!()` with real implementation or remove the code
entirely. `unreachable!()` must be proven unreachable (e.g., via match exhaustiveness).

**Why:** Stubs in production code:
- Panic at runtime, causing unexpected failures
- Hide incomplete implementation
- Make the codebase unreliable

**Process:**
1. Find the `todo!()`
2. Understand what it was meant to implement
3. Implement it properly, or remove the code path
4. If the code path is truly unreachable, use `unreachable!()` with a comment
   explaining **why** it's unreachable (e.g., "enum is exhaustive, this arm
   cannot be reached")

---

### 5. Documentation Warnings Are Real Warnings

**Decision:** `clippy::doc_markdown`, `missing_errors_doc`, `missing_panics_doc`
are treated the same as code warnings — fix them, don't suppress.

**Why:** Documentation is part of the code. Poor docs:
- Mislead users of the API
- Cause incorrect usage patterns
- Increase support burden ("how do I use X?")

**Fix patterns:**
- `doc_markdown`: Add backticks to code items (`` `SQLite` ``, [`Type`]`)
- `missing_errors_doc`: Add `# Errors` section describing error conditions
- `missing_panics_doc`: Add `# Panics` section describing panic conditions

---

## Anti-Patterns to Avoid

### ❌ Editing Generated Code Directly

```rust
// WRONG: Manually editing generated output
// File: bin/platform/src/gen_resources/types.rs (generated)
#[allow(dead_code)]  // Added by hand - will be lost on regen!
struct Foo { ... }

// WRONG: Removing dead code from generated file
// (The generator will just recreate it next time)

// RIGHT: Fix the generator
// File: bin/platform/src/gen_resources/generator.rs
fn generate_struct(...) {
    // Only emit fields that are actually used
    // or add #[allow] at module level in the template
}
```

---

### ❌ Adding `#[allow]` Without Thinking

```rust
// WRONG: Suppressing without understanding
#[allow(clippy::cast_possible_truncation)]
fn foo(x: u64) -> u32 { x as u32 }

// RIGHT: Fix the underlying issue
fn foo(x: u64) -> Result<u32, CastError> {
    x.try_into().map_err(|_| CastError::OutOfRange)
}
```

---

### ❌ Fixing Generated Output

```rust
// WRONG: Editing generated code
// File: bin/platform/src/gen_resources/types.rs (generated)
#[allow(dead_code)]  // Added by hand, will be lost on regen
struct Foo { ... }

// RIGHT: Fix the generator
// File: bin/platform/src/gen_resources/generator.rs
fn generate_struct(...) {
    // ... produce code without dead_code issues
}
```

---

### ❌ Removing Dead Code Without Checking

```rust
// WRONG: Blindly removing "dead" code
// (What if it's used via reflection, macros, or external crates?)

// RIGHT: Verify it's truly unused
// 1. grep for usages
// 2. Check public API surface
// 3. Check for macro/reflection usage
// 4. Then remove
```

---

## Patterns That Work

### ✅ Mechanical Fixes First

```bash
# Let clippy fix what it can automatically
cargo clippy --fix --package <crate> --all-targets --allow-dirty

# Review the diff, stage what's safe
git diff
git add -p

# Then fix the remaining warnings by hand
```

---

### ✅ Doc Comment Backticks (Quick Win)

```rust
/// Before
/// Use the SQLite backend for persistence.
/// See OAuthConfig for configuration.

/// After
/// Use the [`SQLite`] backend for persistence.
/// See [`OAuthConfig`] for configuration.

/// Even better (for external items)
/// Use the `SQLite` backend for persistence.
/// See [`crate::oauth::OAuthConfig`] for configuration.
```

---

### ✅ Signature Improvements (Semantic Fix)

```rust
// Before: Takes ownership unnecessarily
pub fn process(&self, data: String) -> Result<()> {
    // ... only reads data
}

// After: Borrows instead
pub fn process(&self, data: &str) -> Result<()> {
    // ... reads data
}

// Or: Accept anything convertible
pub fn process(&self, data: impl AsRef<str>) -> Result<()> {
    // ... reads data
}
```

---

## Open Questions

### Q: What if a warning is a false positive?

**A:** First, verify it's truly a false positive. Clippy is usually right.
If it genuinely is:
1. Refactor the code to satisfy clippy anyway (if feasible)
2. If not feasible, add a **localized** `#[allow]` with a comment explaining why

```rust
// This cast is safe because we validated the range above
#[allow(clippy::cast_possible_truncation)]
let truncated = value as u32;
```

---

### Q: How do I know if code is generated?

**A:** Check for these signals:
- `// @generated` or `// Generated by` comments
- Files in `test/` directories that look templated
- Files matching a pattern (e.g., `*_generated.rs`)
- Build scripts (`build.rs`) that emit Rust code
- Procedural macros producing output

If generated, find the generator source and fix **that**.

---

### Q: What if fixing a warning breaks downstream crates?

**A:** This is expected and correct. Fix crates in dependency order:
1. Fix the upstream crate, commit
2. Fix the downstream crate that broke, commit
3. Continue down the dependency chain

This is why the spec recommends starting with small, leaf crates first.

---

## Tooling Notes

### Clippy Versions

Clippy lints change between Rust versions. This spec targets:
- **Rust:** 1.85+ (or whatever the workspace `rust-toolchain` specifies)
- **Clippy:** Bundled with Rust

If you see different warnings on your machine, check:
```bash
rustc --version
cargo clippy --version
```

---

### Auto-Fix Coverage

Based on the baseline snapshot (~1,500 warnings):
- **Auto-fixable:** ~60% (~900 warnings) via `cargo clippy --fix`
- **Manual fixes:** ~30% (~450 warnings) — signatures, docs, casts
- **Requires judgment:** ~10% (~150 warnings) — dead code, `todo!()`, generated

---

_Maintained by: Main Agent_
_Last updated: 2026-04-12_
