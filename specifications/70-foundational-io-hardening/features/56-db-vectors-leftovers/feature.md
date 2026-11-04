---
feature: "foundation_db and foundation_vectors leftovers: timestamp units, rate-limit dead code, recursion lint, code-graph clippy"
status: "done"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db", "foundation_vectors"]
catalogue_rows: [25, 26, 39, 40, 50]
---

# Feature 56: db and vectors leftovers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## E1. Timestamp units (backlog §11)

Migrations still default `created_at` to milliseconds (`strftime('%s','now') * 1000` in
`004_create_accounts.sql`, `019_create_device_codes.sql`, …). Spec 70 feature 17 E7 chose a
unit; T0: read what it chose (`specifications/70-foundational-io-hardening/features/17-db/`)
and check every table: defaults, `AsyncStorageCleanup`, and the IdP writes. If all agree,
delete the entry. If not, finish it with a migration and the test
`cleanup_deletes_expired_rows_by_documented_unit`.

## E2. Rate-limit dead code in some feature sets (backlog §16)

Gate `CREATE_TABLE_SQL`, `RECORD_SQL`, `SELECT_SQL`, `sql_check`, `sql_record`,
`sql_reset` (`src/core/rate_limit.rs`) on the features that use them. Verify:
`cargo check -p foundation_connectrpc --tests 2>&1 | rg 'foundation_db.*never used'` is empty.

## E3. vectors SQL tests hit the recursion lint (backlog §17)

Add `#![recursion_limit = "256"]` to `sql_vector_store_tests`, `libsql_vector_store_tests`,
`sqlite_vec_store_tests` with the same comment as `foundation_db/tests/turso_storage_tests.rs`.

## E4. `code-graph` clippy deny (backlog §17)

`tests/lang_config_tests.rs:123` `assert!(ext.raw_calls.len() >= 0)`: replace with a real
assertion (what the test means to check — read it), then make
`cargo clippy -p foundation_vectors --all-targets --all-features` clean and add a
`code-graph` run to CI.

## E5. D1 `Real` NaN or infinity silently becomes NULL (found in feature 52)

`data_value_to_json` (`src/native/d1_kvstore.rs`) maps a non-finite `f64` to JSON `null`, so a
write stores NULL with no error. Return a typed `StorageError` (for example
`InvalidValue("non-finite REAL")`) instead. Test: `d1_rejects_non_finite_real`.

## E6. D1 `/query` responses are read whole with no cap (found in feature 52, rules 4 and 8)

Check the status first; read a success body up to `max_response_body` on the D1 store config
(builder method; pick the default from D1's documented maximum result size and cite it); over
the cap → typed error, not a partial parse. Error bodies: up to the existing error-body cap.
Test: `d1_response_over_cap_is_an_error` (mock).

## E7. `FileStateStore::set` writes in place; the trait has no conditional write (found in feature 45)

`std::fs::write` on the target path lets a reader see a torn file. Write to a temp file in the
same directory, `fsync`, then `rename` over the target (atomic on POSIX). The missing
conditional write (version/etag) in `StateStore`: record the design need in the entry; do not
add it in this feature. Test: `file_state_store_write_is_atomic` (a reader loading in a loop
never sees a partial file).

## E8. Prove D1 number and null parameters with local wrangler (owner decision 2026-11-02)

The owner accepts local wrangler (`wrangler dev --local`, the D1 engine in workerd) as the
check for D1 parameter types; no hosted-account test. Local wrangler does not run Cloudflare's
hosted REST front end, so write that limit once in the test's doc comment.
- Fix the worker stub (`worker-stub.js`, feature 52) so it binds values the way the D1 binding
  does: check D1's documented JS-to-SQLite type mapping and bind an integer-valued number so
  that it is stored as INTEGER (not REAL), a fractional number as REAL, `null` as NULL, a
  string as TEXT. Cite the D1 doc section in a comment.
- Strengthen the wrangler test (`tests/d1_batch_wrangler_tests.rs` or a new
  `d1_param_types_wrangler`): insert `[7, 1.5, null, "x"]` through the store, then read back
  `typeof(col)` for each and assert `integer`, `real`, `null`, `text`; also a batch with mixed
  types, and a round trip of `i64::MAX`.
- Run it with `CF_INTEGRATION_TEST=1` against `wrangler dev --local` (wrangler 4; install it
  in a scratch directory outside the repo with `npm install wrangler@4`). 20 runs.
- Delete the §11 entry "hosted D1 REST acceptance of number/null params is unproven" and the
  `d1-query.json` overlay's note that says it is unproven (say "verified with local wrangler,
  <date>").

## E9. `foundation_db` clippy debt, then deny warnings in CI (found in feature 71)

12 clippy errors with default features and 25 with `libsql` (cast truncation and sign loss,
missing `# Errors` sections). Fix them (casts: `try_from` with the error handled, or an
`#[allow]` with a one-line reason where the value is proven in range). Then set
`clippy-deny-warnings = true` for `foundation_db` in `ci/packages.toml` (feature 71 E3 added the
key) and show `ci.py run foundation_db --step clippy` passes.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
python3 scripts/ci/ci.py run foundation_vectors --profile dev
cargo clippy -p foundation_vectors --profile dev --all-targets --all-features 2>&1 | rg -c '^(warning|error)'   # 0
```

## Done when

- [x] E1–E9 fixed and their entries deleted; the wrangler type test passes 20/20. CI green.
