---
feature: "D1 Workers binding keeps integer types and i64 precision; d1r2 document store passes on local wrangler; code-graph method ids; StateStore conditional write"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db", "foundation_vectors", "foundation_proxy (consumer only)"]
catalogue_rows: [91, 92, 93, 94]
---

# Feature 76: D1 binding types and db follow-ups

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 56 (2026-11-03).

## E1. `D1WasmStorage` may store integers as REAL and lose i64 precision

Feature 56 measured the D1 binding in workerd (local wrangler): `bind(7)` stores `real`; a JS
Number is always REAL and becomes INTEGER only through column affinity; `bind(7n)` throws
`D1_TYPE_ERROR: Type 'bigint' not supported`. `D1WasmStorage` (`src/wasm/wasm_storage/d1_wasm.rs`)
binds `DataValue::Integer(i64)` through the binding. Check how it converts i64 (to f64? to a
JS Number?). Risks: (a) columns without INTEGER affinity store REAL; (b) any i64 above 2^53 is
rounded.
Fix: bind integers so the stored type is INTEGER and the value is exact (for example bind the
decimal text and use `CAST(?N AS INTEGER)` in the SQL the store generates, as the fixed stub
does, or another method you prove). Read integers back exactly (a column read as a JS Number
loses precision above 2^53: read such columns as text with `CAST(col AS TEXT)` where the store
needs exact i64, or document the limit per column). Tests against local wrangler
(`CF_INTEGRATION_TEST=1`, a Workers test harness for `D1WasmStorage` if one exists; if the
wasm store cannot run under the stub, say how you tested it): `i64::MAX` round-trips;
`typeof` is `integer` in an untyped column.

## E2. `d1r2_document_store_tests` fail 2 of 4 against local wrangler

With the old and the new stub. Likely cause (not confirmed): column order lost in the JSON row
objects. Find the cause and fix it in the store (not in the test). 20/20 against wrangler.

## E3. code-graph method ids ignore the class (§17)

`foundation_vectors` code-graph: two classes with a method of the same name produce one id, so
the second is dropped. Include the class (and namespace/module) in the id. Test with the
original two-class TypeScript source from `ts_raw_calls_extraction` (it failed with `got []`).

## E4. `StateStore` has no conditional write (§11)

The proxy (feature 45) had to add a file lock because the trait has no compare-and-set. Add
`set_if_version(resource_id, state, expected_version) -> Result<bool>` (or an etag) to
`StateStore`, implemented atomically per backend (SQL: `UPDATE … WHERE version = ?`; file: under
the store's lock with rename; KV/R2: conditional put where the API has one, else document the
non-atomic default). Do not change the proxy (lane A); record in the backlog that it can drop its
flock once this lands.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
python3 scripts/ci/ci.py run foundation_vectors --profile dev
CF_INTEGRATION_TEST=1 cargo test -p foundation_db --profile dev --test d1r2_document_store_tests
```

## Done when

- [ ] E1–E4 fixed with tests; backlog entries deleted. CI green.
