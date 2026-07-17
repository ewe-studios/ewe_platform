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

### 3. A build.rs-callable form

`foundation_codegentools` is already a lib (`pub mod cli; pub mod schema_gen;`)
with `genapi` as a bin, so a crate can call it from `build.rs`:

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

### 4. Where the output goes — decide first

Both patterns exist in this tree, and they trade off differently:

| | Checked-in `src/generated/` (cloudflare) | build.rs → `OUT_DIR` (deployment_docker's BuildKit protos) |
|---|---|---|
| Review | diffs are visible | invisible; you review the manifest instead |
| Drift | can go stale vs the manifest | impossible by construction |
| Build | free | every clean build pays for it |
| Offline | works | needs the spec vendored (all three are, in `artefacts/`) |
| Debugging | `git grep` finds the code | `OUT_DIR` spelunking |

A middle option: **build.rs generates, output is checked in, CI fails if
regenerating produces a diff.** Reviewable *and* drift-proof, at the cost of a CI
step.

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
- **A generated crate compiles and its calls work against a mock server** — which
  the provider features (01–03) then build on.

## Acceptance criteria

- [ ] Output location decided (checked-in / `OUT_DIR` / generate-and-verify-in-CI)
- [ ] `include_paths` (with globs) and `include_tags` honoured **at generation time**
- [ ] Transitive schema closure emitted — nothing missing, nothing extra
- [ ] `allOf`/`oneOf`/`anyOf`, recursive schemas, and `components/{parameters,responses}` refs handled
- [ ] build.rs-callable API on `foundation_codegentools`
- [ ] An unselected operation's types are **provably absent** from the output
- [ ] Linode's 6 endpoints generate from the 9.3 MB spec into a bounded surface
- [ ] Existing providers (cloudflare, docker, stripe, …) regenerate unchanged, or their diffs are reviewed deliberately
