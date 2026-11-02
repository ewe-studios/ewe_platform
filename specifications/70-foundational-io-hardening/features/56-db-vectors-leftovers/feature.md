---
feature: "foundation_db and foundation_vectors leftovers: timestamp units, rate-limit dead code, recursion lint, code-graph clippy"
status: "pending"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db", "foundation_vectors"]
catalogue_rows: [25, 26, 39, 40]
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

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
python3 scripts/ci/ci.py run foundation_vectors --profile dev
cargo clippy -p foundation_vectors --profile dev --all-targets --all-features 2>&1 | rg -c '^(warning|error)'   # 0
```

## Done when

- [ ] E1–E6 fixed and their entries deleted. CI green.
