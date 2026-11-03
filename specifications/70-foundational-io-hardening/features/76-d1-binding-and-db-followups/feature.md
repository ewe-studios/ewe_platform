---
feature: "Wasm stores tested on the real wrangler bindings (no stub); D1 binding keeps integer types and i64 precision; REST stub a plain pass-through; d1r2 document store passes on local wrangler; code-graph method ids; StateStore conditional write"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db", "foundation_vectors", "foundation_testbed (template and harness only)"]
catalogue_rows: [91, 92, 93, 94]
---

# Feature 76: D1 binding types and db follow-ups

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 56 (2026-11-03).

## E0. Test the wasm stores against the REAL bindings under wrangler — no stub (owner decision 2026-11-03)

Owner rule: when wrangler can run the real thing, tests use it, not a stub. `wrangler dev
--local` runs workerd with real local D1, R2 and KV bindings.
- Use `foundation_testbed`'s `wasm-testbed` wrangler mode (it already runs wasm test code inside
  workerd; template in `backends/foundation_testbed/src/wasm/templates/wrangler/`). Add D1, R2
  and KV bindings to the template's `wrangler.toml` (local only), and a way for a test to reach
  them (the `worker::Env` the testbed passes in).
- Write the wasm-store tests as wasm tests that run inside that worker: `D1WasmStorage`,
  `R2WasmStorage`, `KVWasmStorage`, `CfD1R2DocumentStore` — the contract tests that the native
  stores already have (get/set/list/batch/take/blob range/conditional put …), run against
  `env.DB` / `env.BUCKET` / `env.KV`.
- CI: add them to the wasm runtime job that feature 22 E4 created (`mise run
  test:wasm-testbed:...`), so they run on every PR.

## E1. `D1WasmStorage` integer types and i64 precision (on the real binding)

Feature 56 measured the binding: a JS Number is always stored as REAL (it becomes INTEGER only
through column affinity), and `BigInt` is rejected (`D1_TYPE_ERROR`). Check how `D1WasmStorage`
binds `DataValue::Integer(i64)` and reads integers back. Fix it so integers are stored as
INTEGER and are exact (for example bind the decimal text with `CAST(?N AS INTEGER)` in the SQL
the store builds, and read exact i64 columns with `CAST(col AS TEXT)`; or another method you
prove). Tests in the E0 harness, on the real binding: `typeof` is `integer` in an untyped
column; `i64::MAX`, `i64::MIN` and 2^53+1 round-trip exactly.

## E1b. The REST stub becomes a plain pass-through again

`worker-stub.js` serves only the native REST clients (`D1Store`, `R2Store`): wrangler does not run
Cloudflare's hosted REST front end, so locally the stub is the only way to exercise those clients.
It must not add behaviour. Revert feature 56's type rewriting (the source-text reviver and the
`CAST(?N AS INTEGER)` placeholder rewrite) so the stub passes `sql` and `params` to
`env.DB.prepare(...).bind(...)` unchanged. Move the type-semantics assertions
(`d1_param_types_wrangler`) to the E0 harness, where they test the real binding; keep in the REST
suite only tests of our client's request and response handling, and say in the stub's header and
in those tests' doc comments that they do not prove hosted REST behaviour.

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

- [ ] E0 harness runs the wasm stores on real bindings in CI; E1–E4 fixed with tests; the stub adds no behaviour; backlog entries deleted. CI green.
