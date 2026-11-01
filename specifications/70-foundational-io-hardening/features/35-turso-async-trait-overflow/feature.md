---
feature: "TursoStorage::set async-trait impl overflows the trait resolver"
status: "pending"
priority: "medium"
depends_on: []
branch: "fix/io-35-turso-async-trait-overflow"
crates: ["foundation_db"]
backlog: "docs/bug-backlog.md §11 (1 entry)"
---

# Feature 35: `TursoStorage::set` async-trait resolver overflow

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `foundation_db/src/native/turso_backend.rs` line 461, `set` in the
  `#[async_trait::async_trait] impl KeyValueStore for TursoStorage` (line 455). The generic
  `set<V: Serialize + Send + 'static>` desugars to a boxed future; the resolver overflows on
  the `Pin<Box<…>: CoerceUnsized<…>>` check.
- **Confirm:**

  ```bash
  cargo check -p foundation_db --tests --profile dev 2>&1 | rg 'overflow evaluating the requirement'
  ```

  The warning must still appear. It is `future_incompatible`: a future Rust release makes it
  a hard error. Also check whether `get` (line 457, also generic) warns the same way; fix it
  in the same change if it does.

## E1. Remove the overflow

- **Where:** `foundation_db/src/native/turso_backend.rs` `set` (line 461) and, if it warns,
  `get` (line 457).
- **Fix:** pick one option and record the choice and its trade-off in the PR body.
  1. **Raise the recursion limit.** Add `#![recursion_limit = "256"]` (or higher, as needed)
     to the `foundation_db` crate root. One line; it raises a ceiling for the whole crate and
     can hide deeper nesting.
  2. **Split the generic async method.** Remove the type parameter from the part of the
     method that `async-trait` boxes into the future — for example, serialize the value in a
     non-generic helper and have the trait method take the serialized bytes, or return a
     hand-written `Pin<Box<dyn Future + Send>>` so `async-trait` does not build the deep
     generic future. More code, but surgical and it keeps the default limit.
  Record which option and why. Confirm no other `#[async_trait]` impl in the crate warns
  after the fix.
- **Tests:** `foundation_db/tests/turso_storage_tests.rs` `test_turso_storage_basic` already
  exercises the generic `set`/`get` (it stores `"test_value"` and reads it back). Keep it;
  it is the regression test. If option 2 changes the `set`/`get` signature, update its call
  sites to the new signature. The compile must stay warning-free (the verification below
  checks this). No `#[cfg(test)]` module goes into a source file (rule 7).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
cargo check -p foundation_db --tests --profile dev 2>&1 | tee /tmp/turso_check.log
! rg 'overflow evaluating the requirement' /tmp/turso_check.log
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] The overflow warning is gone for `foundation_db --tests`.
- [ ] `test_turso_storage_basic` (and the rest of the turso suite) passes.
- [ ] The backlog entry is deleted.
- [ ] CI green.

## Review checklist (main session)

- [ ] The chosen option is stated in the PR body, with the trade-off.
- [ ] If `recursion_limit` was raised, the value is justified and not a hidden ceiling.
- [ ] `get` was checked too; if it warned, it is fixed in the same change.
- [ ] The `KeyValueStore` trait surface was not changed unless the split required it, and any
      change is noted for the callers.
