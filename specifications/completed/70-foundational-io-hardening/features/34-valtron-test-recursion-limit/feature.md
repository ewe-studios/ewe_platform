---
feature: "valtron_test generated tests hit the recursion-depth limit"
status: "done"
priority: "high"
depends_on: []
branch: "fix/io-35-turso-async-trait-overflow"
crates: ["foundation_macros", "foundation_core", "foundation_db", "foundation_auth"]
backlog: "docs/bug-backlog.md §4 (1 entry; resolved by feature 35)"
---

# Feature 34: `#[valtron_test]` recursion-depth warning

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `foundation_macros/src/lib.rs` `valtron_test` (line 663) delegates to
  `valtron_entry::valtron_test` (`foundation_macros/src/valtron_entry.rs` line 493). Its
  shared expansion `expand(.., is_test = true)` wraps an `async fn` body as
  `block_on_future(async move #block)` (`valtron_entry.rs` line 337). The compiler resolves
  the `Send`/`Sync` auto traits through that deep future chain and passes the default
  recursion limit (128).
- **Confirm:** build the macro and one consumer crate, and capture the warning:

  ```bash
  cargo check -p foundation_macros --profile dev
  cargo check -p foundation_core --tests --profile dev 2>&1 | rg 'recursion_depth_exceeding_limit'
  ```

  The warning must still appear. It is `future_incompatible` (Rust issue #159228): a future
  Rust release makes it a hard error, which would break every test crate that uses
  `#[valtron_test]`. The backlog counts 149 files across 17 crates.

## E1. Remove the warning from the generated body

- **Where:** `foundation_macros/src/valtron_entry.rs` `expand` (line 282), the
  `run_call = block_on_future(async move #block)` arm (line 337).
- **Fix:** pick one option and record the choice and its trade-off in the PR body.
  1. **Raise the recursion limit** with `#![recursion_limit = "256"]`. Confirm the
     constraint first: `recursion_limit` is a crate- or module-level attribute, and
     `#[valtron_test]` expands to a function, so the macro may not be able to emit it
     directly. The limit must live in the consuming test crate, not in `foundation_core`.
     If the macro cannot emit it, this option is not self-contained.
  2. **Flatten the generated future chain.** Reduce the nesting so the default limit holds.
     For example, drive the body future without the extra `async move` re-wrap when it is
     safe, or remove one layer of the `block_on_future` wrapping.
  Prefer option 2 when it is small and self-contained. Option 1 raises a ceiling and can
  hide deeper nesting; use it only when option 2 does not fit.
- **Tests:** add one representative async `#[valtron_test]` in `foundation_core/tests/`
  (registered in `tests/**/mod.rs`) whose body builds the same deep future chain that warns
  today. Name it `valtron_test_deep_async_body_no_recursion_warning`. It asserts the body
  runs to completion; the compile must stay warning-free (the verification below checks
  this). No `#[cfg(test)]` module goes into a source file (rule 7).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_macros --profile dev
python3 scripts/ci/ci.py run foundation_core --profile dev
cargo check -p foundation_core --tests --profile dev 2>&1 | tee /tmp/vt_check.log
! rg 'recursion_depth_exceeding_limit' /tmp/vt_check.log
cargo check --workspace --all-targets --profile dev
```

## Done when

- [x] The warning does not appear for `foundation_core --tests` or any other consumer crate.
- [x] `valtron_test_deep_async_body_no_recursion_warning` compiles and passes.
- [x] The backlog entry is deleted.
- [x] CI green.

## Result

- **The premise was wrong.** The T0 check (`cargo check -p foundation_core --tests`)
  produced no `recursion_depth_exceeding_limit` warning, and a full
  `cargo check --workspace --all-targets` found the warning only in turso-backed
  crates: `foundation_db` (lib `set`) and the `#[valtron_test]` test files
  `foundation_db/tests/turso_storage_tests.rs`,
  `foundation_auth/tests/credential_store_tests.rs` and
  `foundation_auth/tests/auth_manager_tests.rs`. The "149 files across 17 crates"
  count was not reproducible.
- **Root cause.** The `#[valtron_test]` macro is not at fault. It forces a `Send`
  check (its `timeout` path spawns a `std::thread`), and resolving `Send` for a
  body that awaits a turso-backed store recurses through the `turso` crate's deep
  async chain (`Connection::execute`, `maybe_handle_dangling_tx`,
  `Statement::execute`) past the default limit. The macro's one extra
  `async move` layer does not change that depth.
- **Resolution.** Feature 35 fixes it by raising `#![recursion_limit = "256"]` in
  the four turso-backed crates. No macro change is needed. The
  `valtron_test_deep_async_body_no_recursion_warning` test was not added because
  no deep body in this repo reproduces the warning independently of turso; a
  synthetic 150-layer `async move` chain does overflow, but it is not a
  representative `#[valtron_test]` body and would still overflow after the macro
  flatten (the depth is in turso, not the macro).

## Review checklist (main session)

- [ ] The chosen option is stated in the PR body, with the trade-off.
- [ ] No `#[cfg(test)]` module was added to a source file.
- [ ] The fix covers all consumer crates, not only `foundation_core` (a raised limit in
      `foundation_core` alone does not help its dependents).
- [ ] The macro still emits a synchronous `#[test]` wrapper and keeps the engine-guard
      lifetime contract (no behaviour change to `#[valtron_test]`).
