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

### 1. An allowlist honoured at generation time

Select by **path** and by **tag**:

- `include_paths` — exact or glob (`/servers`, `/servers/*`). Precise; the primary
  key.
- `include_tags` — the vendor's grouping. Convenient, but coarse: DO's "Droplets"
  tag is far more than the four droplet calls we want.
- Probably also `include_operations` (by `operationId`) since that is what the
  vendor's docs name.

Everything not selected is not emitted.

### 2. The transitive schema closure — the actual work

Including `POST /servers` pulls its request body and responses, which pull their
`$ref`s, which pull theirs. The generator must walk refs from each selected
operation and emit **exactly the reachable set**. Get this wrong in either
direction and the feature fails: too little and it does not compile; too much and
we are back to 9.3 MB of types.

Watch for: `allOf`/`oneOf`/`anyOf` composition, recursive schemas (a type that
refs itself), shared error envelopes, and `$ref`s into `components/parameters` and
`components/responses`, not just `components/schemas`.

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

#### The mechanic still to settle: can build.rs write there?

The owner also asked for "a build.rs format that these crates can have that calls
into our codegen and indicate what part of the API specs we want generated". A
build.rs that *writes into `src/generated/`* has a real constraint behind it:

**These crates are publishable** — none set `publish = false`. If build.rs
regenerated `src/generated/` at build time, a downstream consumer building from
crates.io would need the spec artefact (which is not published) and would be
writing into the registry's read-only source directory. Cargo's own rule is that
build scripts write only to `OUT_DIR`; violating it also risks `cargo package
--verify` seeing a dirty tree.

Two ways to honour both asks:

- **A — build.rs declares, and generates only in-workspace.** The slice lives in
  `build.rs` next to the code that uses it; generation runs when the spec artefact
  is present (a workspace checkout) and **skips silently** when it is not (a
  consumer building from crates.io, who just compiles the committed output).
  Needs `cargo:rerun-if-changed` on the spec + manifest, and a content-compare so
  an unchanged regeneration does not churn mtimes and re-trigger builds.
- **B — build.rs declares and *verifies*; the CLI generates.** `genapi generate
  <provider>` reads the same declaration and writes `src/generated/`; build.rs
  only fails when the committed output is stale against the manifest. Closest to
  today's pattern (cloudflare has no build.rs at all), and it never writes to
  `src/`.

**Recommendation: B**, with the staleness check behind CI rather than every
developer build. It gives the declaration-next-to-the-crate that A does, without
a build script that writes into a published crate's source tree.

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
- **Glob/tag selection** — selecting by tag pulls the tag's operations; selecting
  by path pulls just that path.
- **Version pinning** — a spec whose `info.version` differs from the pinned
  `spec_version` fails generation with both versions named; a matching one
  proceeds.
- **A generated crate compiles and its calls work against a mock server** — which
  the provider features (01–03) then build on.

## Acceptance criteria

- [ ] Output is a checked-in `src/generated/` per crate; hand-written code and wrappers live outside it
- [ ] build.rs mechanic settled (A: generate in-workspace only, or B: declare + verify, CLI generates) — it must not write into `src/` for a published consumer
- [ ] `api_version` pinned into the client; callers never pass it
- [ ] `spec_version` validated against the spec's `info.version` — **generation fails on a mismatch**, naming both versions
- [ ] Upgrading is a deliberate act: bump the pin, regenerate, review the diff
- [ ] `include_paths` (with globs) and `include_tags` honoured **at generation time**
- [ ] Transitive schema closure emitted — nothing missing, nothing extra
- [ ] `allOf`/`oneOf`/`anyOf`, recursive schemas, and `components/{parameters,responses}` refs handled
- [ ] build.rs-callable API on `foundation_codegentools`
- [ ] An unselected operation's types are **provably absent** from the output
- [ ] Linode's 6 endpoints generate from the 9.3 MB spec into a bounded surface
- [ ] Existing providers (cloudflare, docker, stripe, …) regenerate unchanged, or their diffs are reviewed deliberately
