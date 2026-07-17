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

### 0. Canonicalise the spec first (the transform stage)

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

- [ ] Output is a checked-in `src/generated/` per crate; hand-written code and wrappers live outside it
- [ ] One declaration, two verbs: `check()` (fails aloud on stale output or a moved spec) and `write()` (regenerates `src/generated/`)
- [ ] `write()` is not called from a build script of a publishable crate — `check()` is the build.rs/CI verb
- [ ] `api_version` pinned into the client; callers never pass it
- [ ] `spec_version` validated against the spec's `info.version` — **generation fails on a mismatch**, naming both versions
- [ ] Upgrading is a deliberate act: bump the pin, regenerate, review the diff
- [x] `foundation_openapi`'s suite is green before building on it (§6) — 2 stale GCP tests fixed 2026-07-17
- [x] Selection (§1) — `Selection` with path globs/tags/operationIds; 11 tests incl. Linode's real 334→3
- [ ] Canonicalisation stage wired (§0): `extract_inline_schemas` + `normalize_nullable_types` + `ensure_servers` composed per provider — they exist, tested, and unused today
- [ ] A bundled spec (Linode/Hetzner) yields **named** request/response types, not `serde_json::Value`
- [ ] `include_paths` (with globs) and `include_tags` honoured **at generation time**
- [ ] Transitive schema closure emitted — nothing missing, nothing extra
- [ ] `allOf`/`oneOf`/`anyOf`, recursive schemas, and `components/{parameters,responses}` refs handled
- [ ] build.rs-callable API on `foundation_codegentools`
- [ ] An unselected operation's types are **provably absent** from the output
- [ ] Linode's 6 endpoints generate from the 9.3 MB spec into a bounded surface
- [ ] Existing providers (cloudflare, docker, stripe, …) regenerate unchanged, or their diffs are reviewed deliberately
