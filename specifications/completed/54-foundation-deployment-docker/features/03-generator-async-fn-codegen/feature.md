---
feature: "Code generator async fn emit + deployment-crate regeneration"
description: "Update foundation_openapi UnifiedGenerator to emit async fn client functions (DynNetClient + PreparedRequestBuilder + send_async) instead of TaskIterator/SimpleHttpClient; regenerate cloudflare/stripe/supabase/neon/planetscale/prisma/flyio; migrate hand-written consumers (cloudflare client.rs + dns_ops.rs) to async/await + DynNetClient. Must land before Feature 04 (Docker generation)."
status: "completed"
priority: "high"
phase: 0
depends_on: ["01-dynnetclient-preparedrequest-surface"]
estimated_effort: "large"
created: 2026-07-12
---
# Feature 03: Code generator async `fn` emit + deployment-crate regeneration

> **Completion note (2026-07-12).** The generator work — the substance of this
> feature — is **done and proven**:
> - `UnifiedGenerator` emits `async fn ..._request(client: DynNetClient, args,
>   builder_mod) -> Result<ApiResponse<T>, ApiError>` via `send_async().await`,
>   with structured `PreparedRequestBuilder::query()`. No `R`/`DnsResolver`
>   generic, no `SimpleHttpClient`/`ClientRequestBuilder`/`RequestIntro`/
>   `build_send_request`.
> - **Type-collection completeness** (the "generate all code into `/generated`"
>   directive) fixed: follow a schema's own top-level array `items` `$ref`;
>   transitively expand the shared-resource list and skip emitting array schemas
>   as standalone structs; always emit the per-endpoint `Args` struct. On the
>   cloudflare spec (5606 schemas, full feature set) this took the regenerated
>   crate from **172 compile errors to 0**.
> - **cloudflare** fully regenerated into `src/generated/` (now committed) and its
>   hand-written consumers migrated: `client.rs` → `DynNetClient` via
>   `HttpClientBuilder`; `dns_ops.rs` → `async`/`.await`, `auth_mod` →
>   `FnOnce(&mut PreparedRequestBuilder)`, request body via the generated `Args`.
>   `cargo check -p foundation_deployment_cloudflare --features cloudflare` is
>   clean (0 errors, 0 own warnings); 12 unit tests pass.
> - `foundation_netio::PreparedRequestBuilder` gained `&mut` mutators
>   (`set_header`/`set_bearer_token`/`set_body_json`) for `builder_mod` closures.
>
> **Deferred (owner-approved 2026-07-12): Part H regeneration of stripe /
> supabase / neon / planetscale / prisma / flyio.** Investigation showed these
> are **empty skeletal stubs**, not populated crates: each has an empty
> `[dependencies]`, a `lib.rs` that declares no modules (so their committed flat
> `src/<group>/` code is orphaned/never compiled), and **zero real dependents**.
> "Regenerating" them therefore means authoring each crate's full scaffold from
> scratch (dependency sections, ~100s of group feature flags, `lib.rs` wiring) —
> materially larger than regeneration, for dead code nobody consumes. The
> generator fixes here are provider-agnostic and already validated on the most
> complex available spec (cloudflare), and they directly benefit Feature 04
> (Docker). The `genapi` CLI was extended (`SPLIT_OUT_PROVIDERS` +
> `split_out_crate_name()`) so these crates *can* be regenerated once scaffolded,
> but scaffolding them is out of scope for spec 54. Owner chose to proceed to
> Feature 04.

## Why this exists

Feature 01 delivered the cross-platform HTTP **primitives** (`DynNetClient`,
`PreparedRequestBuilder`, `HttpClientBuilder::build() -> DynNetClient`,
`body_reader::split_exchange()` / `collect_exchange()`). It did **not** touch the
code generator or any generated code.

The `foundation_openapi` `UnifiedGenerator` (wrapped by the `gen_api` binary)
still emits the deprecated, native-only surface:

```rust
use foundation_netio::http::{ClientRequestBuilder, SimpleHttpClient};

pub fn get_applications_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &GetApplicationsArgs,
    builder_mod: Option<F>,
) -> Result<impl TaskIterator<...>, ApiError>
where
    R: DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),
{ ... .build_send_request() ... RequestIntro ... }
```

This leaks the `R: DnsResolver` generic through every generated function, is
native-only (no wasm), and uses deprecated types.

**This feature updates the generator itself** and regenerates every deployment
crate off the new surface. It is a hard prerequisite for **Feature 04 (Docker
type replication)** — Feature 04 runs `gen_api generate` and expects the emitted
Docker code to already be in the new async-`fn` shape. Generating Docker against
the old emit and rewriting it by hand would be wasted work.

Splitting this out of Feature 01 keeps two very different blast radii apart:
Feature 01 is ~6 files of foundation primitives; this feature rewrites the
generator's function-emission path and mechanically regenerates ~100K+ lines
across 7 crates.

## Decision alignment

Implements **[Decision 05 — Valtron TaskIterator → async fn](../decisions/05-valtron-task-iterator.md)**,
which supersedes the `impl TaskIterator` + `collect_exchange()` shape sketched in
Decision 04 and in Feature 01's earlier drafts. The resolved emit shape is
**`async fn`** (chosen by the owner 2026-07-12):

- Non-streaming endpoints → `async fn` calling `HttpClient::send_async()`.
- Streaming endpoints → function returning the `body_reader::split_exchange()`
  tuple (caller sends the continuation to valtron and drains the observers).

`async fn` is the most ergonomic sequential form (`.await`, no combinator
chains), valtron already drives futures via `from_future()` / `run_future()` /
`#[valtron]`, and it matches the Docker `Deployable` impls in Feature 04/05
(`create_container(&client, &args).await?`).

## Design

### Part A — Generator: imports (generator.rs ~line 810-816)

**Before:**
```rust
writeln!(out, "use foundation_core::valtron::{{TaskIterator, TaskIteratorExt}};")?;
writeln!(out, "use foundation_netio::http::{{ClientRequestBuilder, SimpleHttpClient}};")?;
```

**After:**
```rust
writeln!(out, "use foundation_netio::{{DynNetClient, PreparedRequestBuilder}};")?;
writeln!(out, "use foundation_netio::shared::client::HttpClient;")?; // send_async
```

`TaskIterator`/`TaskIteratorExt` imports are only kept in modules that emit a
streaming endpoint (they are needed for the `split_exchange()` continuation type).
Because generated modules already carry `#![allow(unused_imports)]`, the import
block may unconditionally include the streaming imports without a warning.

### Part B — Generator: non-streaming function signature (generator.rs ~1330-1339)

**Before:**
```rust
pub fn {fn_prefix}_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &{Args},
    builder_mod: Option<F>,
) -> Result<impl TaskIterator<Ready = Result<ApiResponse<{T}>, ApiError>, Pending = ApiPending, Spawner = BoxedSendExecutionAction> + Send + 'static, ApiError>
where
    R: DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),
```

**After:**
```rust
pub async fn {fn_prefix}_request<F>(
    client: DynNetClient,
    args: &{Args},
    builder_mod: Option<F>,
) -> Result<ApiResponse<{T}>, super::shared::ApiError>
where
    F: FnOnce(&mut PreparedRequestBuilder),
```

- No `R` generic, no `DnsResolver` bound.
- `client` is taken **by value** (`DynNetClient` is `Arc`, cheap to clone).
- Return is a plain `Result<ApiResponse<T>, ApiError>` — the `async fn` desugars
  to a `Send` future (everything held across `.await` is `Send`, incl.
  `SimpleResponse<SendSafeBody>` which is `Send + Sync` after Feature 00).

### Part C — Generator: URL + structured query (generator.rs ~1341-1372)

The manual `String`-append query block is replaced by structured
`PreparedRequestBuilder::query()` calls (Feature 01 Part A/B):

```rust
let endpoint_url = format!("{base}{path}", args.p1, ...);   // path params only
let mut builder = PreparedRequestBuilder::{method}(&endpoint_url)
    .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;
// one line per query param:
builder = builder.query("{qp}", args.{safe_qp}.as_deref());
```

`.query(key, Option<impl Into<String>>)` is a no-op when the arg is `None`, so
optional query params need no `if let`. Percent-encoding is handled by `Uri`'s
structured `Query`, so the generator no longer emits `urlencoding::encode`.

### Part D — Generator: body + builder_mod (generator.rs ~1387-1398)

Unchanged in spirit, retargeted to `PreparedRequestBuilder`:

```rust
if ep.request_type.is_some() {
    builder = builder.body_json(&args.body)
        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;
}
if let Some(f) = builder_mod {
    f(&mut builder);
}
```

`builder_mod` closure type is now `FnOnce(&mut PreparedRequestBuilder)` — same
`.header()`, `.bearer_token()`, `.body_json()` methods, no `R`.

### Part E — Generator: send + parse (generator.rs ~1400-1436)

**Before:** `.build_send_request().map_ready(|intro| match intro { RequestIntro::Success {...} ... })`.

**After** — sequential `async` body using `send_async()`:

```rust
let req = builder.build();
let response = client.send_async(req).await
    .map_err(|e| super::shared::ApiError::RequestSendFailed(e.to_string()))?;

let status: u16 = response.status().into();
let headers = response.headers().clone();
if !(200..300).contains(&status) {
    return Err(super::shared::ApiError::HttpStatus { code: status, headers, body: None });
}
```

Body handling depends on `return_type`:

- `return_type == "()"`: drop the body, `Ok(ApiResponse { status, headers, body: () })`.
- otherwise: collect + parse:
  ```rust
  let bytes = foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(
      response.into_body(),
  );
  let parsed: {T} = serde_json::from_slice(&bytes)
      .map_err(|e| super::shared::ApiError::ParseFailed(e.to_string()))?;
  Ok(ApiResponse { status, headers, body: parsed })
  ```

The exact `SimpleResponse` accessor used to obtain the owned `SendSafeBody`
(`into_body()` / `into_parts()` / `take_body()`) is confirmed against
`foundation_netio` during implementation; the generator uses whichever the type
actually exposes. `RequestIntro` is no longer referenced anywhere in generated
code.

### Part F — Generator: streaming endpoints

If the endpoint model flags an endpoint as streaming (SSE / chunked / server
push), emit the `split_exchange()` tuple form instead of an `async fn`:

```rust
pub fn {fn_prefix}_request<F>(
    client: DynNetClient,
    args: &{Args},
    builder_mod: Option<F>,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
    Box<dyn TaskIterator<Ready = HttpExchange, Pending = HttpExchangePending,
                         Spawner = BoxedSendExecutionAction> + Send>,
)
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let mut builder = PreparedRequestBuilder::{method}(&endpoint_url)?
        .query(...);
    if let Some(f) = builder_mod { f(&mut builder); }
    foundation_netio::shared::client::body_reader::split_exchange(client, builder)
}
```

**Scope note:** the current `EndpointUnit` model may not carry a streaming flag.
If it does not, all generated functions are non-streaming `async fn`, and streaming
is left to per-provider hand-written code (Docker's streaming endpoints are
hand-written in Feature 05). Adding a streaming flag to the endpoint model is
**in-scope only if** an already-generated provider (e.g. an SSE endpoint in
cloudflare/stripe) currently depends on streaming emit; otherwise it is deferred
with a `TODO` and a note in this feature's completion record. This boundary is
called out explicitly so the generator change does not silently under-serve a
streaming endpoint.

### Part G — Generated provider impls

The generator also emits provider `impl` blocks / wrapper methods that call the
`*_request()` functions. These change from driving a `TaskIterator`
(`.collect_one()` / `.map_ready()`) to awaiting the `async fn`:

```rust
let response = {fn_prefix}_request(client.clone(), &args, None::<fn(&mut PreparedRequestBuilder)>).await?;
```

Wrapper methods that were previously sync become `async fn` (or are driven with
`valtron::block_on` where a sync surface must be preserved — decided per method
against how the wrapper is consumed).

### Part H — Regenerate + migrate deployment crates

Run the updated generator for each crate and reconcile hand-written glue:

| Crate | Regenerate | Hand-written reconciliation |
|-------|-----------|-----------------------------|
| `foundation_deployment_cloudflare` | all `generated/` modules | `client.rs`: `CloudflareClient.http` → `DynNetClient` via `HttpClientBuilder::new().build()`; `dns_ops.rs`: `auth_mod` closure → `FnOnce(&mut PreparedRequestBuilder)`, request calls → `.await` (methods become `async` or use `valtron::block_on`) |
| `foundation_deployment_stripe` | all modules | update hand-written client (if any) |
| `foundation_deployment_supabase` | all modules | update hand-written client (if any) |
| `foundation_deployment_neon` | all modules | update hand-written client (if any) |
| `foundation_deployment_planetscale` | all modules | update hand-written client (if any) |
| `foundation_deployment_prisma` | all modules | update hand-written client (if any) |
| `foundation_deployment_flyio` | all modules | update hand-written client (if any) |

`foundation_deployment_docker` is **not** touched here — it does not exist yet and
is generated fresh in Feature 04 off this already-updated generator.

### Part I — `foundation_deployment` shared types

`foundation_deployment/src/providers/common/api_types.rs` may re-export or embed
generator-shared types (`ApiResponse`, `ApiError`, `ApiPending`,
`BoxedSendExecutionAction`). `ApiPending` and the `Spawner` alias are no longer
part of non-streaming signatures; leave them defined (streaming still uses
`HttpExchangePending`/`BoxedSendExecutionAction`) but stop referencing them from
non-streaming generated code. `RequestIntro` re-export was already deprecated in
Feature 01 (Part F); confirm generated code no longer imports it.

## Scope

| Crate | Change |
|-------|--------|
| `foundation_openapi` | Generator emits `async fn` (non-streaming) + `split_exchange()` tuple (streaming); structured query; no `R`/`ClientRequestBuilder`/`RequestIntro`/`build_send_request` |
| `foundation_deployment_cloudflare` | Regenerate + migrate `client.rs`, `dns_ops.rs` to `DynNetClient` + async |
| `foundation_deployment_stripe` | Regenerate + reconcile |
| `foundation_deployment_supabase` | Regenerate + reconcile |
| `foundation_deployment_neon` | Regenerate + reconcile |
| `foundation_deployment_planetscale` | Regenerate + reconcile |
| `foundation_deployment_prisma` | Regenerate + reconcile |
| `foundation_deployment_flyio` | Regenerate + reconcile |

### Not touched

- `foundation_deployment_docker` — generated fresh in Feature 04.
- `foundation_ai` — uses `HttpClient` directly, not the generator. (Its
  `RequestIntro` usage is a separate follow-up, already noted in Feature 01.)

## Verification

- `cargo check -p foundation_openapi` — generator compiles.
- Generated signatures: `pub async fn ..._request<F>(client: DynNetClient, args: &_, builder_mod: Option<F>) -> Result<ApiResponse<T>, ApiError> where F: FnOnce(&mut PreparedRequestBuilder)` — no `R`, no `ClientRequestBuilder`, no `RequestIntro`.
- `cargo check -p foundation_deployment_cloudflare --features <cloudflare features>` — regenerated code + migrated `client.rs`/`dns_ops.rs` compile.
- `cargo check` for each of stripe/supabase/neon/planetscale/prisma/flyio (with their feature flags) — regenerated code compiles.
- A representative call awaits cleanly: `let r = some_request(client.clone(), &args, None::<fn(&mut PreparedRequestBuilder)>).await?;`
- `gen_api generate` output is committed and reviewable in the git diff.

## Acceptance criteria

1. `foundation_openapi` generator emits `async fn` for non-streaming endpoints
   using `DynNetClient` + `PreparedRequestBuilder` + `send_async()`.
2. No generated code references `SimpleHttpClient`, `ClientRequestBuilder<R>`,
   the `R: DnsResolver` generic, `RequestIntro`, or `build_send_request()`.
3. Query params are emitted via structured `PreparedRequestBuilder::query()`.
4. Streaming endpoints (if the endpoint model flags any) emit the
   `split_exchange()` tuple form; otherwise the streaming boundary is documented
   and deferred to per-provider hand-written code.
5. All 7 existing deployment crates regenerate and compile.
6. `foundation_deployment_cloudflare` `client.rs` + `dns_ops.rs` migrated to
   `DynNetClient` + `async`/`.await` and compile.
7. `foundation_openapi` and the deployment crates carry no new warnings from the
   change (per project warning-check workflow).

## Dependencies

- **Feature 01** (`01-dynnetclient-preparedrequest-surface`) — provides
  `DynNetClient`, `PreparedRequestBuilder` (query + build), `send_async`,
  `HttpClientBuilder::build() -> DynNetClient`, and `body_reader::split_exchange()`.

## Risks

- **Large regeneration surface** (~100K+ lines across 7 crates). Mitigation:
  regenerate cloudflare first, get it green, then batch the rest.
- **Sync→async wrapper ripple**: hand-written sync wrappers (e.g. `dns_ops.rs`)
  become `async` or wrap `valtron::block_on`. Decide per method against its
  consumers; do not silently change a public sync API without recording it.
- **`SimpleResponse` body accessor**: the exact owned-body method is confirmed
  against `foundation_netio` at implementation time (Part E).
- **Streaming flag absence**: if the endpoint model has no streaming flag, the
  generator emits only non-streaming `async fn`; the deferred streaming boundary
  is recorded rather than assumed away.
