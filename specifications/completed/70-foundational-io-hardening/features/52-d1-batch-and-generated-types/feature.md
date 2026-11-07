---
feature: "D1: real batch request with per-statement params, proven atomic; D1 and R2 wire types generated, not hand-written"
status: "done"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db", "foundation_codegentools", "foundation_cloudflare_types", "foundation_deployment_cloudflare"]
catalogue_rows: [18]
---

# Feature 52: D1 batch and generated Cloudflare types

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (review findings)

1. `D1Store::execute_batch_statements` (`src/native/d1_kvstore.rs` ~755) joins all
   statements into one `sql` string with `"; "` and appends every parameter into one list.
   Parameters with numbers (`?1`) restart per statement, so they bind wrongly; and
   atomicity is proven only against a mock.
2. `foundation_cloudflare_types/src/lib.rs` holds **hand-written** D1 `/query` response
   types ("match the generated output exactly") and hand-written R2 object-listing types (the
   endpoint is not in the vendored OpenAPI spec). Rule 5: use generated types; if the generator
   lacks them, fix the generator.
3. `execute_sql_async` builds the request body with `serde_json::json!({ "sql", "params" })`
   (untyped).

## Facts (checked in the vendored spec `artefacts/cloud_providers/cloudflare/openapi.json`)

- `POST /accounts/{account_id}/d1/database/{database_id}/query` takes `d1_batch_query`:
  `oneOf [ d1_single_query {sql, params?}, { batch: [d1_single_query] } ]`.
- `d1_params` is `array of string` in the spec. Our code sends JSON numbers and nulls too.

## Design

1. **Generator** (`foundation_codegentools`): make the Cloudflare generation emit the D1
   query request (`D1BatchQuery` as an untagged enum for the `oneOf`, `D1SingleQuery`) and
   the D1 response types into `foundation_cloudflare_types` (the crate `foundation_db` can
   depend on). If the generator cannot express "emit these schemas into the types crate", add
   that (a list of schema names hoisted to the shared types file, in the generator config).
   For R2 ListObjects, add a small vendored OpenAPI overlay file (for example
   `artefacts/cloud_providers/cloudflare/overlays/r2-list-objects.json`) that the generator
   merges before generating, with a comment naming the Cloudflare doc page it copies. Then
   delete every hand-written type in `foundation_cloudflare_types/src/lib.rs`.
   `d1_params` as `string`: keep the spec type in the generated output, and add an overlay
   that widens `d1_params.items` to any JSON value **only if** you prove D1 accepts numbers
   and null (test against wrangler, below); otherwise convert values to strings in
   `foundation_db` and say so.
2. **D1 batch**: send `D1BatchQuery::Batch { batch: [D1SingleQuery { sql, params }, …] }`.
   Each statement keeps its own params. Parse one result per statement.
3. **Atomicity**: Cloudflare documents batched statements as one transaction (a failure rolls
   back the batch). Prove it with the wrangler-backed tests that already exist for D1
   (`CF_INTEGRATION_TEST=1`, the worker stub): `d1_batch_rolls_back_on_failure_wrangler`. If
   the local D1 does not support the REST batch shape, report it and keep the mock test plus a
   backlog entry.
4. Use the generated request type in `execute_sql_async` too (no `json!`).

## Tests

- `d1_batch_sends_one_entry_per_statement` (mock; fails on master): two statements with
  `?1` params each; assert the request body has `batch` with two entries and the right params.
- `d1_batch_rolls_back_on_failure_wrangler` (opt-in, `CF_INTEGRATION_TEST=1`).
- Generator: `cloudflare_types_are_generated` — regenerate and byte-compare (the build-script
  check already does this; make sure it covers `foundation_cloudflare_types`).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_codegentools --profile dev
python3 scripts/ci/ci.py run foundation_db --profile dev
cargo check -p foundation_deployment_cloudflare --profile dev
rg -n 'json!\(' backends/foundation_db/src/native/d1_kvstore.rs     # no request-body json!
```

## Done when

- [x] No hand-written Cloudflare wire type left; generator and overlays committed.
- [x] Batch test fails on master, passes now; wrangler result (or its backlog entry) reported.
