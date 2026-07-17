---
feature: "Selective codegen (generate only the API surface we use)"
description: "Teach genapi to emit only an allowlisted slice of an OpenAPI spec — the endpoints we name plus their transitive schema closure — and give provider crates a build.rs-callable form to declare that slice"
status: "not-started"
priority: "high"
phase: 0
depends_on: ["01-provider-clients"]
estimated_effort: "medium"
created: 2026-07-17
blocks: ["01-hetzner", "02-digitalocean", "03-linode"]
---
# Feature 00: Selective Codegen

## Why

[Decision 01](../../decisions/01-provider-clients.md) resolved: **codegen for all
three providers**, on one condition — the generator must emit only what we ask
for. The numbers make the case:

| Provider spec | Size | Paths | Endpoints we need |
|---|---|---|---|
| Linode (`linode-api-openapi`, v4.229.1) | **9.3 MB** | **334** | 6 |
| DigitalOcean (v2 public) | ~3 MB | whole platform | 6 |
| Hetzner Cloud | small | servers-centric | 6 |

Generating those whole to create and delete a server would bury each crate in
dead types. The Cloudflare crate already shows the end state: thousands of
generated types, and a sweep of the tree found **not one consumer** for the vast
majority.

The owner's direction (2026-07-17): *"its ok for us to only pull out the parts we
care for, we should make it possible to tell generator we only care for specific
parts of a given spec, maybe we also provide a build.rs format that these crates
can have that calls into our codegen and indicate what part of the API specs we
want generated, ignoring the rest. This helps us zero down to just what we need to
support."*

## What exists today

`foundation_codegentools` (`genapi` bin + `cli`/`schema_gen` lib modules):

- Groups a spec's endpoints into modules **by tag** (`cloudflare_access`,
  `cloudflare_workers`, …), gating each behind a cargo feature
  (`<provider>_<group>`, plus `<provider>_all`).
- `--spec <name>` filters *which sub-spec* to use for multi-spec providers (gcp).
- Registers split-out crates in `SPLIT_OUT_PROVIDERS`.

**But it generates every group regardless.** Cargo features gate *compilation*,
not *generation* — the dead code is still emitted, still committed, still
reviewed. Closing that gap is this feature.

## Scope

### 0. Canonicalise the spec first (the transform stage) — **built 2026-07-17**

> **Status:** `foundation_openapi::transform` now does this.
> `canonicalize_operations` hoists every inline operation schema (request body and
> each response) into `components/schemas`, leaving a `$ref`; names come from the
> `operationId` (`post-linode-instance` → `PostLinodeInstanceRequest` /
> `…Response`), falling back to path+method — `path_to_type_name` alone collides,
> since it strips parameters and gives `/instances` and `/instances/{id}` the same
> name. Identical shapes share a name rather than duplicating; genuine clashes get
> suffixed. Measured on the real specs: **Linode 1042 inline → 0, Hetzner 633 → 0,
> Cloudflare 3090 → 0 — all three canonical, zero renames.**
>
> Still to build: the `genapi normalize <provider>` command and the
> provider→normalizer→raw registry (decision 05).

Owner (2026-07-17): *"Always good if needed to add spec transformation like we did
for cloudflare or others to make their spec standard OpenAPI spec where needed."*

This is not optional for two of our three, and the machinery already exists —
unwired.

`foundation_deployment::providers::standard::normalize` states the contract:

> *"The type and client generators expect a canonical OpenAPI 3.x structure:
> schemas in `components/schemas`, a `servers` array with at least one entry, and
> **`$ref` pointers instead of inline schemas**. Not all provider specs arrive in
> this form."*

It ships `ensure_servers`, `normalize_nullable_types` (3.1 nullable arrays),
`extract_inline_schemas` (hoists inline objects into `components/schemas` and
replaces them with `$ref`, recursively, arrays included) and `path_to_type_name`.
All are tested — and **nothing calls them**: `fetch_standard_spec` does not, and
the doc's "providers compose them in their `fetch.rs`" never happened (the GCP
fetch path that would have was since deleted).

**Why this is load-bearing here:** Linode and Hetzner are *fully bundled* — zero
`$ref`s, every schema inlined. Our extractor derives a response's type name **from
its `ref_path`** (`ResponseType::Generated`), so a bundled spec yields no named
types at all. Without hoisting, Linode's six endpoints generate nothing usable.
That is why the pipeline canonicalises before it analyses.

Per-provider quirks the transform stage should absorb:

| Provider | Quirk | Transform |
|---|---|---|
| Linode | fully bundled; `/{apiVersion}/…` paths | hoist inline → `$ref`; pin `apiVersion` (decision 01) so it is not a per-call argument |
| Hetzner | fully bundled; OpenAPI **3.1.2** (nullable arrays) | hoist inline → `$ref`; `normalize_nullable_types` |
| DigitalOcean | ref-heavy, incl. `components/{responses,parameters,headers}` | none for refs; needs `Components` extended (§2) |

### Pipeline order

```
fetch/vendor  ->  canonicalise (§0)  ->  select (§1)  ->  closure (§2)  ->  generate
```

Select **before** hoisting, so we only canonicalise the six paths we keep rather
than all 334 — and the closure then has little left to prune. But the closure is
still required even for Linode: its 77 pre-existing `components/schemas` are
referenced by nothing (0 `$ref`s in the whole document), so without a reachability
prune they would all be emitted for six endpoints.

### 1. An allowlist honoured at generation time

Select by **path** and by **tag**:

- `include_paths` — exact or glob (`/servers`, `/servers/*`). Precise; the primary
  key.
- `include_tags` — the vendor's grouping. Convenient, but coarse: DO's "Droplets"
  tag is far more than the four droplet calls we want.
- Probably also `include_operations` (by `operationId`) since that is what the
  vendor's docs name.

Everything not selected is not emitted.

### 2. The transitive schema closure

Including `POST /servers` pulls its request body and responses, which pull their
`$ref`s, which pull theirs. Emit **exactly the reachable set**: too little and it
does not compile; too much and we are back where we started.

#### What the three specs actually look like (measured 2026-07-17)

This was worth checking before building, because it redirects the work:

| Spec | Size | `$ref` count | Style |
|---|---|---|---|
| **Linode** | 9.3 MB | **0** | **fully bundled** — every schema inlined into its operation |
| **Hetzner** | 3.4 MB | **0** | **bundled**; its only `#/components/schemas/` strings (35) are inside **discriminator `mapping`** blocks |
| **DigitalOcean** | 3.1 MB | **7814** | **ref-heavy**: `responses` 3758, `schemas` 1796, `headers` 1372, `parameters` 833, `examples` 105, `links` 8 |

So:

- **Selection by path is the primary win for all three** — and for Linode and
  Hetzner it is the *entire* win. Their schemas travel inline with the operation,
  so dropping an unselected path drops its schemas with it. 334 paths → 6.
- **The closure walk is load-bearing for DigitalOcean only.** That is also the one
  that needs the most from it (see below).
- **Discriminator `mapping` is a ref site.** Hetzner's only refs live there. A
  closure that walks `$ref` but not `mapping` would emit a schema whose
  discriminator points at a type we dropped — broken output from a spec that looks
  ref-free. Easy to miss; the fixture must cover it.

#### The gap DigitalOcean exposes

`foundation_openapi::spec::Components` **only parses `schemas`**:

```rust
pub struct Components { pub schemas: BTreeMap<String, Schema> }
```

DO refs into `components/responses` (3758), `components/headers` (1372) and
`components/parameters` (833) — none of which the model can represent, so those
refs cannot resolve today. Feature 00 has to extend `Components` before DO's
closure can be correct. (Nothing has noticed because nothing has generated DO.)

#### Still to handle

`allOf`/`oneOf`/`anyOf` composition, recursive schemas (a type that refs itself),
shared error envelopes, and — per above — `discriminator.mapping` and refs into
`components/{responses,parameters,headers}`.

### 3. Pin the API version, validate the spec revision — **resolved** (owner, 2026-07-17)

*"We should always pin the API version and validate the OpenAPI spec version
matches, then its left to users to decide to upgrade versions and regenerate."*

Two different things, both pinned in the declaration:

| | What | Example | Enforced |
|---|---|---|---|
| **API version** | the vendor's major API line — part of the URL | Linode `v4`, DO `v2`, Hetzner `v1` | the client fixes it; callers never pass it |
| **Spec revision** | the exact document we generated from (`info.version`) | Linode `4.229.1` | **generation fails** if the spec on disk says otherwise |

```rust
foundation_codegentools::generate()
    .provider("linode")
    .spec("artefacts/cloud_providers/linode/openapi.json")
    .api_version("v4")            // pinned into the client
    .spec_version("4.229.1")      // must match the spec's info.version, or fail
    .include_paths([...])
    .run()?;
```

**Why fail rather than adapt:** a spec that has moved under us is a decision, not
an event. Silently regenerating against a new revision would change the client's
types without anyone choosing to — the same class of surprise as the silently
dropped parameters spec-53's audit kept finding (`subnet`, `network_alias`,
`memory = "256m"`). Upgrading is deliberate: bump `spec_version`, regenerate,
read the diff (which is exactly why the output is checked in, §4).

The failure must name both versions — "spec says 4.230.0, manifest pins 4.229.1"
— so the fix is obvious.

This also answers **drift** below: we cannot detect a vendor changing a field
inside a type we already generate, but we *can* detect that the document is not
the one we pinned, which is the case that matters.

### 4. A build.rs-callable form

`foundation_codegentools` is already a lib (`pub mod cli; pub mod schema_gen;`)
with `genapi` as a bin, so a crate can call it from `build.rs` — subject to the
publishing constraint in §5:

```rust
// backends/foundation_deployment_hetzner/build.rs
foundation_codegentools::generate()
    .provider("hetzner")
    .spec("artefacts/cloud_providers/hetzner/openapi.json")
    .include_paths(["/servers", "/servers/{id}", "/ssh_keys"])
    .run()?;
```

The declaration lives **with the crate that needs it**, which is the point: the
slice is visible next to the code that uses it, not buried in the generator.

### 5. Where the output goes — **resolved** (owner, 2026-07-17)

**Every crate keeps a checked-in `src/generated/` module**, exactly as
`foundation_deployment_cloudflare` does today (45 committed files, no build.rs —
regenerated with the `genapi` CLI). Hand-written domain logic and wrappers live
**outside** it: `client.rs`, `types.rs`, `*_ops.rs`, `deployable.rs`.

So the output location is not `OUT_DIR`. Diffs stay reviewable, `git grep` finds
the code, and a build needs no spec artefact.

#### Two verbs, and the caller picks — **resolved** (owner, 2026-07-17)

*"Just add the ability to call check as well as write, then users can do checks
and fail aloud, and if it matches then they can call write() to write it to the
generated directory."*

The builder exposes **both** terminal operations over one declaration; the crate
decides its policy rather than the generator imposing one:

| Verb | Does | Fails when |
|---|---|---|
| `check()` | compares what *would* be generated against the committed `src/generated/`, and validates the pinned `spec_version` against the spec's `info.version` | the output is stale, or the spec has moved under the pin — **loudly, naming both versions** |
| `write()` | generates into `src/generated/` | the spec is missing or the pin does not match |

```rust
let codegen = foundation_codegentools::generate()
    .provider("hetzner")
    .spec("artefacts/cloud_providers/hetzner/openapi.json")
    .api_version("v1")
    .spec_version("1.0.0")
    .include_paths(["/servers", "/servers/{id}", "/ssh_keys"]);

codegen.check()?;   // fail aloud if the committed output is stale
codegen.write()?;   // or regenerate it
```

So a build.rs can `check()` (never touching `src/`), the `genapi` CLI can
`write()`, and a developer can do either — one declaration, no duplication.

**The caveat that constrains where `write()` is called from:** these crates are
publishable (none set `publish = false`). A build script that calls `write()`
would have a crates.io consumer needing the spec artefact — which is not
published — and writing into the registry's read-only source directory; cargo's
rule is that build scripts write only to `OUT_DIR`, and `cargo package --verify`
may see a dirty tree. So `write()` belongs in the CLI or an explicit developer
step; `check()` is what a build.rs (or CI) should call.

### 6. Leave `foundation_openapi` green — **done 2026-07-17**

This feature works inside `foundation_openapi`, and its suite was **red on arrival**:
`integration_spec_processing` had 2 failures (`processes_gcp_abusiveexperiencereport_spec`,
`gcp_endpoint_extracted_with_full_structure`), verified pre-existing by stashing this
work. Building selective codegen on top of a red suite means never knowing which
failures are yours.

**Diagnosis — the tests were stale, not the code.** They looked the endpoint up by
GCP Discovery's `flatPath` (`v1/sites/{sitesId}`) while the extractor keys by
`path` (`v1/{+name}`). The fixture settles which is right:

| | value | in `parameters`? |
|---|---|---|
| `path` | `v1/{+name}` | **yes** — `parameterOrder: ["name"]`, `parameters: ["name"]` |
| `flatPath` | `v1/sites/{sitesId}` | **no** — `sitesId` is declared nowhere |

GCP v2 APIs use resource-name expansion: one `{+name}` holds the whole resource
path. Generating from `flatPath` would emit a client with an **unbound `sitesId`
path param**. So the extractor is correct — and deliberately so
(commit `4b1f62fc5`, 2026-04-21, "fix GCP generation"), with the reasoning in a
comment. The tests were only half-updated then: they already asserted
`path_params == ["name"]` (the `path` semantics) while still looking up the
flatPath key, and had been red for ~3 months.

**Fixed** by keying on `v1/{+name}` and recording *why* in the test, so the next
person does not "fix" it back. `foundation_openapi` is now 10/10 green.

### 7. Wiring the selection into `generate` — **done 2026-07-17**

`Pipeline` could cut Linode's 334 paths to 3 from the day it was written, and
**nothing called it**. `genapi generate` still read the whole spec and handed it
to `UnifiedGenerator`, so every path came out the other side. Code present with no
consumer does not work — the same signal that condemned F14/F15/F16/F19 in
spec-53.

`foundation_codegentools::generate()` is now the one declaration (§4's shape), and
the CLI routes through it, so a selection means the same thing from a build script
and from the command line. The generator is never *shown* the paths we did not
ask for, which is the only place the selection cannot be forgotten.

```
genapi generate <provider> --include-path /servers --include-tag Servers \
                           --spec-version 1.0.0 --api-version v1 [--check]
```

Measured: Linode's real spec, 334 paths → 3, 50 schemas, 4 files.

#### What running it against a real committed tree found

`--check` against Cloudflare's 45 committed files reported all 44 differing, which
exposed **four real bugs** — none of which any fixture had caught, because the
fixtures were written by the same person as the code:

| Bug | Symptom | Cause |
|---|---|---|
| Slash in a hoisted name | 61 unresolvable `$ref`s | a property keyed `application/json` became `…ContentApplication/json`; a component name is a **JSON-pointer segment**, so the `/` splits it |
| `sanitize_identifier` was a blocklist | `pub struct Rules*ContentSignal` | fourteen punctuation marks were listed; vendors use `*`, `$metadata`, `+gt`, `1.1.1.1`, `pg_partman_bgw.interval` |
| Field names never sanitised | `pub 13335: …`, `pub : …` | the generator used `escape_field_keyword(to_snake_case(k))` — keywords and casing only. `sanitize_field_name`, which does the right thing, existed with **zero callers** |
| Hoisting could take a name the vendor owns | `recursive type has infinite size` | Cloudflare bundles `vectorize_index_info_response` *and* has operation `vectorize-index-info`; both pascal-case to one Rust type, so a field rendered as its own parent. `unique_name` only checked other **hoisted** names |
| responses behind a `$ref` resolved to nothing | **every one of DigitalOcean's 447 paths generated as `ApiResponse<()>`**, body discarded | our `Response` model has `description` + `content` and **no `$ref` field**, so DO's `"202": {"$ref": "#/components/responses/droplet_create"}` deserialised to an empty `Response`. Fixed in canonicalisation (`resolve_response_refs`), not the model — that stage exists so the generator does not grow a branch per vendor. **3758** operation responses rewired on DO's real spec |
| `resolve_schema_key` guessed at the inverse of pascal-casing | 52 of Hetzner's 100 types silently became `HashMap<String, Value>` blobs | it tried three specific spellings (direct, snake_case, lower-first). Hetzner keys a schema `CreateServerResponseServerPublic_net` (its property is `public_net`); the reference site calls it `…PublicNet`, and **no snake-casing of that name returns the original**. Pascal-casing is not invertible — the lookup now matches in the pascal-cased space, which is the direction that is well defined |

The last one is the one to remember: it presents as a compiler error about
recursion and is really **two different schemas fighting over one name**. Silently
picking a winner would have been worse than the error.

All four are fixed and pinned by tests that use the real vendor keys. Canonicalising
Cloudflare's 19 MB spec now hoists 3090 schemas and introduces **zero** unresolvable
refs (it ships 57 of its own — see below).

#### Whose broken `$ref` is it — **resolved**

`resolve()` used to reject **any** dangling ref. Cloudflare's spec references 57
schemas it never bundles (`abuse-reports_CSAMReport`, …), so that rule meant we
simply could not generate a provider we already ship. The invariant is now the
narrower one that is actually ours to hold: **a ref that resolved in the vendor's
document must still resolve in ours**. Theirs are reported via `tracing::warn` —
never silent, since those fields generate untyped — but not fatal.

### 8. Regenerating the existing providers — **OPEN, needs the owner**

Routing the CLI through `Pipeline` means every provider now gets canonicalised,
which the old path never did. For the VPS crates (01–03) that is the whole point.
For **Cloudflare, which is already committed**, it is a decision:

- **The generated code is better and it compiles.** 6214 → 9945 structs, and
  **1349 → 2241 typed responses** — the 3090 inline schemas that made 1224 of its
  2442 fns return `serde_json::Value` become named types. The `resolve_schema_key`
  fix (found via Hetzner) very likely helps here too: it turned **52 of Hetzner's
  100 types from opaque blobs into real ones**, and Cloudflare's spec has the same
  underscore-bearing keys.
- **But it breaks Cloudflare's hand-written wrappers.** 9 call sites in
  `dns_ops.rs` and `provider_client.rs` fail to compile: request fns gained a
  typed body parameter they did not have. The generated tree is clean; the
  hand-written code was built against the untyped API.

So regenerating Cloudflare (and auditing docker/stripe/supabase/neon/planetscale/
fly_io/prisma_postgres the same way) is **its own deliberate task**, not a side
effect of wiring the selection. Until it is done, `genapi generate cloudflare`
produces something different from what is committed — `genapi generate cloudflare
--check` reports exactly that, which is how anyone will find out.

**Recommendation:** take it. The typed-response gain is the payoff feature 00 was
written for, and the 9 call sites are a morning's work. It just should not ride
along inside a commit about selection.

## Verification

All local — no network, no accounts:

- **Closure correctness** — a fixture spec with a deliberately gnarly ref graph
  (nested `$ref`, `allOf`, a recursive type, a shared error envelope, refs into
  `components/parameters`): assert the emitted set is exactly the reachable
  closure of the selected paths. Both directions matter: nothing missing (it must
  compile) and nothing extra (assert an unselected type is **absent**).
- **The real payoff, measured** — generate the six Linode endpoints from the real
  9.3 MB spec and assert the output is a small, bounded set of types. This is the
  test that would have caught "we generated all 334 paths anyway".
- **A discriminator's `mapping` targets survive** — the Hetzner case: a bundled
  spec whose only refs are in `mapping`, where dropping a mapped schema yields
  output that references a type we never emitted.
- **DO's `components/{responses,parameters,headers}` refs resolve** — currently
  unrepresentable in `Components`.
- **Glob/tag selection** — selecting by tag pulls the tag's operations; selecting
  by path pulls just that path.
- **Version pinning** — a spec whose `info.version` differs from the pinned
  `spec_version` fails generation with both versions named; a matching one
  proceeds.
- **A generated crate compiles and its calls work against a mock server** — which
  the provider features (01–03) then build on.

## Acceptance criteria

- [x] Output is a checked-in `src/generated/` per crate; hand-written code and wrappers live outside it — `crate_dir()` writes to `<crate>/src/generated/`, and `--output-dir` now actually works (it was accepted, shadowed, and ignored)
- [ ] **Existing providers regenerated under canonicalisation** (§8) — needs the owner's call; Cloudflare's generated code improves and compiles, its 9 hand-written call sites do not
- [x] One declaration, two verbs: `check()` (fails aloud on stale output or a moved spec) and `write()`
- [x] `write()` is not called from a build script of a publishable crate — `check()` is the build.rs/CI verb, and never writes
- [x] `api_version` pinned into the client; callers never pass it (`Pipeline::api_version`)
- [x] `spec_version` validated against the spec's `info.version` — **fails on a mismatch**, naming both versions, in `resolve()` **and** `check()`
- [x] Upgrading is a deliberate act: bump the pin, regenerate, review the diff
- [x] `foundation_openapi`'s suite is green before building on it (§6) — 2 stale GCP tests fixed 2026-07-17
- [x] Selection (§1) — `Selection` with path globs/tags/operationIds; 11 tests incl. Linode's real 334→3
- [x] Canonicalisation stage (§0) — transforms moved to `foundation_openapi::transform`; `canonicalize_operations` hoists every inline operation schema to `components/schemas` + `$ref`; `validate_canonical` proves the result
- [x] A bundled spec (Linode/Hetzner) yields **named** request/response types — measured: Linode 1042 inline → 0, Hetzner 633 → 0, both canonical
- [x] `genapi normalize <provider>` CLI + the provider→normalizer→raw registry (decision 05) — all three vendored under `artefacts/cloud_providers/raw/` and normalizing clean
- [x] `include_paths` (with globs) and `include_tags` honoured **at generation time** — the CLI and build.rs both route through `generate()`, which hands the generator the *resolved* spec (§7); Linode 334 → 3 measured end-to-end
- [x] Transitive schema closure emitted — nothing missing (`dangling_refs` empty), nothing extra (`unreachable_components` empty)
- [x] `allOf`/`oneOf`/`anyOf`, recursive schemas, `discriminator.mapping`, and `components/{parameters,responses,headers}` refs handled — walking raw JSON follows them all, so **no model extension was needed**
- [x] build.rs-callable API on `foundation_codegentools` — `generate().provider(…).spec(…).include_paths(…).crate_dir(…)` with `check()`/`write()`; `check()` proven to touch neither `src/` nor `Cargo.toml`
- [x] An unselected operation's types are **provably absent** from the output
- [x] Linode's endpoints resolve from the 9.3 MB spec into a bounded surface — **334 paths → 3, 50 schemas**
- [ ] Existing providers (cloudflare, docker, stripe, …) regenerate unchanged, or their diffs are reviewed deliberately — **not yet done**: `normalize` currently registers only the three new providers
- [ ] Wire `Pipeline` into `genapi generate` so the selection actually drives code generation — **the remaining gap**: the stages exist and are proven, but `generate` still consumes a whole spec
