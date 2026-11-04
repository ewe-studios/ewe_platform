---
feature: "Wasm stores tested on the real wrangler bindings (no stub); D1 binding keeps integer types and i64 precision; native REST clients tested on wrangler's local REST API (stub deleted); d1r2 document store passes on local wrangler; code-graph method ids; StateStore conditional write"
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

## E1b. Test the native REST clients against wrangler's own local REST API — delete the stub (owner decision 2026-11-03)

Owner rule again: no stub where wrangler serves the real thing. It does. `wrangler dev --local`
(miniflare) serves a Cloudflare-shaped REST API at `http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`,
backed by the same local D1, R2 and KV storage as the bindings. Measured with wrangler 4.156.2
(miniflare 5.20261102.0-beta), 2026-11-03:
- Routes (no `/accounts/:account_id` prefix): `GET /d1/database`, `POST /d1/database/:id/raw`;
  `GET|PUT /r2/buckets/:b/objects/:key`, `DELETE /r2/buckets/:b/objects/:key{.+}`,
  `GET|DELETE /r2/buckets/:b/objects`; `/storage/kv/namespaces/:id/{keys,values/:key,bulk,bulk/get,bulk/delete}`.
  The D1 database id is the binding's `preview_database_id` (for example `test-db-preview`).
- D1: only `/raw` exists (`/query` gives 404). `/raw` returns `{columns, rows}` with rows as
  ordered arrays, so column order is kept.
- D1 params are strings only (`zD1Params = z.array(z.string())`, the same as the published
  Cloudflare spec). `[7, 1.5, null, "x"]` gives `{"code":10001,"message":"Invalid input"}`;
  `["7","1.5","x"]` works and every value is stored as `text`.
- R2 has no range read and no conditional put; object metadata comes from
  `GET` with header `cf-metadata-only: true` (not `HEAD`). GET and PUT match `:key` without
  `{.+}`, so test whether keys with `/` work, and encode them if needed.

So feature 52's finding was wrong: it was made against `worker-stub.js`, which calls the binding,
not against REST. The REST API accepts string params only. Work:
1. `D1Store` (native REST) sends every param as a JSON string and gives it a type in SQL:
   `CAST(?N AS INTEGER)` / `CAST(?N AS REAL)` for numbers, the `NULL` literal (not a bound
   param) for null, blobs as hex with `unhex(?N)` or another method you prove. Read rows through
   `/raw` (ordered arrays, column names once) on both hosted and local. Exact i64 on read:
   `CAST(col AS TEXT)` where the store reads an integer column, as in E1.
2. Remove the `d1_params` part of `artefacts/cloud_providers/cloudflare/overlays/d1-query.json`
   (keep the `d1_query_meta` integer fix), regenerate with the generator, and use the generated
   `/raw` request and response types (CLAUDE.md rule 5).
3. Make the REST path layout configurable on the client config: hosted
   (`https://api.cloudflare.com/client/v4/accounts/{account}`) or local explorer
   (`http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`, no account segment). The existing
   `*_with_base_url` constructors take the full prefix; no `const` in the code path.
4. Point every `CF_INTEGRATION_TEST` REST test (foundation_db `common/mod.rs`, `d1_*`, `r2_*`,
   `d1r2_document_store_tests`; foundation_nativeapis `d1_delta_vfs`, `r2_delta_vfs`,
   `vfs_shim_vfs`) at the explorer API. R2 operations the explorer does not serve (range read,
   conditional put) are tested on the real binding in the E0 harness, and the doc comment says so.
5. Delete `worker-stub.js` and its `mise` wiring. `d1_param_types_wrangler` moves: the binding's
   type semantics go to the E0 harness; the REST side becomes
   `d1_rest_params_are_strings_and_typed_by_cast` (integers incl. `i64::MAX`/`i64::MIN`, reals,
   null, text, blob round-trip with the right `typeof`). Add `d1_rest_batch_rolls_back` (a
   failing statement in a batch leaves no rows from the batch).
6. Pin the wrangler version in the test tooling (the explorer API is beta and may change), and
   say in `common/mod.rs` that it is the local explorer, not the hosted front end; the hosted
   API follows the same published spec (string params).

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

- [ ] E0 harness runs the wasm stores on real bindings in CI; the REST tests run on the local explorer API and `worker-stub.js` is gone; E1–E4 fixed with tests; backlog entries deleted. CI green.
