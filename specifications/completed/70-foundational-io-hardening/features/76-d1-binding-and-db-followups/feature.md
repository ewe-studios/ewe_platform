---
feature: "Wasm stores tested on the real wrangler bindings (no stub); D1 binding keeps integer types and i64 precision; native REST clients tested on wrangler's local REST API (stub deleted); d1r2 document store passes on local wrangler; code-graph method ids; StateStore conditional write"
status: "done"
lane: "B"
model: "sonnet"
branch: "spec70/f76-f55-db-generators"
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

## E4b. `R2Store::set_if_version` uses R2's conditional put (owner 2026-11-04)

E4 left `R2Store` on the non-atomic read-compare-write default, with the reason "the state API
does not carry an ETag". That reason is wrong: the ETag is internal to the store, and
`R2Store` already has the conditional PUT (`put_object_conditional_async`, `If-Match` /
`If-None-Match`, `412` → `StorageError::PreconditionFailed`) for `BlobStore::put_blob_conditionally`.
Override `set_if_version` on `R2Store`:
- `expected = None`: PUT the state with `If-None-Match: *`; `412` → `Ok(false)`.
- `expected = Some(t)`: one GET that returns the body **and** its `ETag` (add a variant of
  `get_object_async` that returns the response `etag` header; the body is a small state JSON,
  so reading it whole is allowed). Missing → `Ok(false)`. If the stored `updated_at` is not `t`
  → `Ok(false)`. Else PUT with `If-Match: <that etag>`; `412` → `Ok(false)`.
  The ETag pins the exact object that was compared, so two writers cannot both win.
- The same on the wasm side if `R2WasmStorage` implements `StateStore`: use the binding's
  `put(key, value, { onlyIf: { etagMatches } })` (and `etagDoesNotMatch: "*"` for absent).
- Update the `StateStore::set_if_version` doc: `R2Store` is now in the atomic list.
- Tests: the contract test and the 20-racing-writers test on R2. Wrangler's local explorer API
  ignores conditional headers on PUT, so run them on the real binding in the E0 harness (the
  binding honours `onlyIf`). For the native REST client, confirm from Cloudflare's R2 API
  docs that the object PUT honours `If-Match` / `If-None-Match`, and say which doc in the
  test's comment; if it does not, say so in the report instead of claiming atomicity.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
python3 scripts/ci/ci.py run foundation_vectors --profile dev
CF_INTEGRATION_TEST=1 cargo test -p foundation_db --profile dev --test d1r2_document_store_tests
```

## Done when

- [x] E0 harness runs the wasm stores on real bindings in CI; the REST tests run on the local
      explorer API and `worker-stub.js` is gone; E1–E4 fixed with tests; backlog entries
      deleted.
- [x] CI green: run 42040473182 on `e01e83d80` (PR #179); the D1 and R2 jobs pass.

## Result (2026-11-05, branch `spec70/f76-f55-db-generators`)

- **E3 and E4 were already on master** (commits `e0eb231cf` and `5221cdade`, from the
  branch `fix/io-51-60-lane-b`). Re-verified by the `foundation_vectors` code-graph
  tests and the `StateStore::set_if_version` tests.
- **E1b and E2 were already on master** (`bf72c750d`): the REST clients use the
  explorer API and `worker-stub.js` is deleted. Re-verified against `wrangler dev
  --local` on 8789: `d1r2_document_store_tests` 4/4, then 20 of 20 runs green (each
  ~0.5 s); `d1_batch`, `d1_kvstore`, `d1_response`, `d1_rest_batch`,
  `d1_rest_params`, `d1_state_init`, `d1_state_store` and `r2_blobstore` all green
  (26 tests).
- **E4b done.** `R2Store::set_if_version` overrides the trait default:
  `expected = None` sends `If-None-Match: *`; `expected = Some(t)` reads the state and
  its `ETag` in one GET, compares `updated_at`, then sends `If-Match: <etag>`. A
  `412` becomes `Ok(false)`. The trait doc now lists `R2Store` as atomic and names the
  Cloudflare R2 object PUT reference
  (`https://developers.cloudflare.com/api/resources/r2/subresources/buckets/subresources/objects/methods/put/`).
  `r2_state_store_tests` (mock that honours both headers) passes 10 of 10, including
  the 20-writer race with exactly one winner. The wasm `R2WasmStorage` does not
  implement `StateStore`, so E4b's wasm part does not apply.
- **E1 done.** `D1WasmStorage` binds an integer as decimal text and rewrites its
  placeholder to `CAST(? AS INTEGER)`, so the value is an exact INTEGER in an untyped
  column. A `NULL` parameter is written as the `NULL` literal and dropped (workerd's
  D1 binding rejects a JS `null` parameter and names the global scope in the error);
  later `?N` placeholders are renumbered. `QueryStore::query` documents that a D1
  wasm INTEGER column reads back as a JS number, so a value past 2^53 needs
  `CAST(col AS TEXT)`.
- **E0 done.** New standalone wasm test crate
  `backends/foundation_db/integration/wasm_cf` with 7 `#[wasm_bindgen_test]` cases:
  D1 key-value, D1 blob, D1 integer types and `i64` precision, R2 blob (ETag, list,
  range), R2 conditional put, KV, and the `CfD1R2DocumentStore` round trip (including
  the >4 KB R2 offload). It runs inside workerd with `wasm-testbed test
  bindgen-wrangler`, on the real local D1/R2/KV bindings declared in the testbed's
  `wrangler.toml`; the worker passes `env` as `globalThis.__CF_TEST_ENV__`. The
  runner now probes a `/health` route (a TCP connect is too early), reads the body by
  `Content-Length`, answers 200 with `{"passed":true|false}` (a 500 goes through
  wrangler's error middleware), and stops the whole wrangler process group. Command
  output: `Discovered 7 tests`, `wrangler ready on port 17645`, `{"passed":true}`;
  the worker's console log shows `test result: ok. 7 passed; 0 failed`. The harness
  is `mise run test:wasm-testbed:wrangler`, and `wasm_tests_commands()` in
  `scripts/ci/ci.py` adds it to the wasm runtime CI job.
- **Blocked by the local tool:** workerd in wrangler 4.156.2 enforces `onlyIf` on an
  R2 PUT but resolves the promise instead of rejecting when the condition fails
  (and `etagDoesNotMatch: "*"` is not honoured). `R2WasmStorage` therefore keeps the
  trait's documented non-atomic check-then-write, which reports
  `PreconditionFailed`; the harness tests that. An atomic wasm conditional put needs
  a workerd that rejects. Recorded in `docs/bug-backlog.md`.
- **Backlog:** the §11 `set_if_version` R2 entry is replaced by the proxy-flock
  remainder and the workerd R2 limitation; the "R2 range read and conditional put
  have no test" entry is deleted. §17 was already empty.
- Verify commands run: the harness command above;
  `cargo test -p foundation_db --profile uat --test r2_state_store_tests` (10 passed);
  `cargo check -p foundation_db --target wasm32-unknown-unknown --no-default-features
  --features wasm-bindgen-storage` (clean); `cargo fmt --all --check` (clean);
  `python3 scripts/ci/ci.py run foundation_db --profile dev` and
  `foundation_vectors --profile dev`.

