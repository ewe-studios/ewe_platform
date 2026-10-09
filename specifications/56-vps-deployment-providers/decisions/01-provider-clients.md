# 01 — Provider clients: codegen, with selective generation

**Date:** 2026-07-17
**Status:** **Resolved** (2026-07-17, owner)

## Decision

**Codegen for all three providers** — keep the workspace pattern
(`genapi` → `src/generated/`, hand-written domain logic on top, as
`foundation_deployment_cloudflare` does).

The objection to codegen was cost, not principle: DigitalOcean's spec is ~3 MB
covering the whole platform (droplets, Kubernetes, Spaces, databases, App
Platform) and we need **six endpoints**. Generating it whole would add a large
surface of dead types — the Cloudflare crate already carries thousands of
generated types nothing references.

**The resolution removes that cost: teach the generator to emit only the parts we
ask for.** A crate declares which slice of a spec it wants, and the generator
ignores the rest. Then codegen is affordable for a 3 MB spec, the workspace keeps
one pattern, and we zero in on exactly the surface we support.

That capability is [feature 00](../features/00-selective-codegen/feature.md), and
it **blocks** features 01–03.

## What the generator does today

- Groups a spec's endpoints into modules by tag (`cloudflare_access`,
  `cloudflare_workers`, …) and gates each group behind a **cargo feature**
  (`<provider>_<group>`, plus a `<provider>_all`).
- Has a `--spec` filter for multi-spec providers (gcp's many sub-APIs).
- **Generates every group regardless.** The features gate *compilation*, not
  *generation* — so the dead code is still emitted, still reviewed, still in the
  tree. That is the gap.

## What changes

An **allowlist**, honoured at generation time. Sketch (exact shape is feature 00):

```rust
// backends/foundation_deployment_hetzner/build.rs
foundation_codegentools::generate()
    .provider("hetzner")
    .spec("artefacts/cloud_providers/hetzner/openapi.json")
    .include_tags(["Servers", "SSHKeys"])          // or:
    .include_paths(["/servers", "/servers/{id}", "/ssh_keys"])
    .run()?;
```

Anything not reachable from the allowlist — including schemas — is not emitted.

## If a provider needs RPC, it uses `foundation_connectrpc`

All three of these vendors are plain HTTP/REST, so this does not bite here — but
the rule stands for any provider that is not (owner, 2026-07-17): **reach for
`foundation_connectrpc`**, never a second gRPC stack.

There is precedent in the tree: `foundation_deployment_docker` drives BuildKit's
gRPC Control API through `foundation_connectrpc` (with
`foundation_connectrpc_codegen` in `[build-dependencies]` generating the message
types), over its `H2Transport`. A provider needing gRPC follows that shape.

Note the one constraint that comes with it, learned in spec-53: the
`foundation_connectrpc` path reaches BoringSSL via `jwt-simple` →
`foundation_auth`, which **cannot share a binary with libssh2/OpenSSL** (the
`ssh` transport). Two libcryptos, duplicate `EVP_*` symbols, link failure. Since
this spec's whole point is reaching a box over SSH, an RPC-based provider crate
would collide with that — so it would need the same feature-gating treatment
`foundation_deployment_docker` now has (`ssh` and `buildkit` are mutually
exclusive features).

## Consequences to work through in feature 00

- **Transitive schemas.** Including `POST /servers` pulls its request/response
  types, which pull their `$ref`s, and so on. The generator must walk refs and
  emit exactly the reachable closure. This is the actual work, and it is where the
  saving comes from.
- **Selection granularity.** Tags are the natural key (they already drive the
  group modules), but tags are the vendor's taxonomy, not ours — DO's "Droplets"
  tag is far more than the four droplet calls we want. Paths are precise but
  brittle across spec revisions. Probably both, with paths winning.
- **Generate at build time or check the output in?** Both patterns exist here:
  cloudflare checks `src/generated/` in and regenerates via the CLI;
  `foundation_deployment_docker` has a build.rs emitting BuildKit protos to
  `OUT_DIR`. Checked-in output is reviewable and offline; build.rs output cannot
  drift from the manifest. Feature 00 decides.
- **Version pinning (resolved, owner 2026-07-17).** Each crate pins the vendor's
  **API version** (Linode `v4`, DO `v2`, Hetzner `v1`) into its client, and pins
  the **spec revision** it generated from (`info.version`); generation **fails**
  if the spec on disk no longer matches. Upgrading is then a deliberate act — bump
  the pin, regenerate, review the diff. See
  [feature 00](../features/00-selective-codegen/feature.md) §3.

## Linode: spec found — all three providers codegen

Linode's spec was **not** where the public URLs suggested (four candidates 404'd
on 2026-07-17; the docs moved to Akamai TechDocs and the spec moved with them).
The owner supplied it: the official **`linode/linode-api-openapi`** repo, cloned
locally at
`/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/src.linode/linode-api-openapi`.

Validated 2026-07-17: OpenAPI **3.0.1**, "Akamai: Linode API" **v4.229.1**,
**9.3 MB**, **334 paths**, and every endpoint feature 03 needs is present. So
Linode follows the same codegen path as the other two — no hand-written client.

**Quirk to handle:** Linode's paths are prefixed with the API version as a *path
parameter* — `/{apiVersion}/linode/instances`, not `/v4/linode/instances` — so
every generated call would take `apiVersion` as an argument. Pin it to `v4` (the
version-pinning rule above); callers never pass it. And pin the spec revision at
**4.229.1**, the document validated on 2026-07-17. See
[feature 03](../features/03-linode/feature.md).

## Spec sources (2026-07-17)

| Provider | Spec | Size | Status |
|---|---|---|---|
| Hetzner Cloud | `https://docs.hetzner.cloud/cloud.spec.json` | small | **HTTP 200** |
| DigitalOcean | `https://api-engineering.nyc3.digitaloceanspaces.com/spec-ci/DigitalOcean-public.v2.yaml` | ~3 MB | **HTTP 200** |
| Linode | `linode/linode-api-openapi` (local clone; owner-supplied) | **9.3 MB**, 334 paths | **validated** |

Note what that table says about the decision: **9.3 MB, 3 MB and one small spec —
for six endpoints each.** Selective generation is not a nicety here; without it
the Linode crate alone would carry a 334-path surface to create and delete a
server.
