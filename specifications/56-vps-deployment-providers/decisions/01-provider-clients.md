# 01 — Provider clients: codegen vs hand-written

**Date:** 2026-07-17
**Status:** Open

## The question

Each provider crate needs an HTTP client for its vendor's API. The workspace has
an established pipeline — `genapi generate <provider>` reads
`artefacts/cloud_providers/<provider>/openapi.json` and emits `src/generated/`,
with hand-written domain logic on top (`foundation_deployment_cloudflare` is the
model: `client.rs` + `types.rs` + `dns_ops.rs` over a generated client, registered
in `SPLIT_OUT_PROVIDERS`).

Do these three follow it?

## What we know (probed 2026-07-17)

| Provider | Spec | Status |
|---|---|---|
| Hetzner Cloud | `https://docs.hetzner.cloud/cloud.spec.json` | **HTTP 200** — reachable, and the API is small and server-centric |
| DigitalOcean | `https://api-engineering.nyc3.digitaloceanspaces.com/spec-ci/DigitalOcean-public.v2.yaml` | **HTTP 200, ~3 MB** — the whole platform (droplets, k8s, spaces, databases, apps, …) |
| Linode | four candidates 404'd (`linode/linode-openapi`, `linode/linode-api-docs`, `www.linode.com/docs/api/openapi.yaml`, `api.linode.com/v4/openapi.yaml`) | **unconfirmed** — needs a source, or hand-writing |

We need roughly **six endpoints per provider**: create server, get server, list
servers, delete server, list/create SSH key, and (Hetzner/Linode) list images or
sizes. Everything else in those specs is noise for this spec.

## Options

### A. Codegen for all three (matches the workspace pattern)

Fetch each spec into `artefacts/cloud_providers/<name>/`, register in
`SPLIT_OUT_PROVIDERS`, run `genapi generate`.

- **For:** one pattern across every provider crate in the tree; the generated
  types track the vendor's schema; regenerating picks up API changes; no bespoke
  request-building to review.
- **Against:** DO's 3 MB spec generates an enormous surface for six endpoints
  (compile time, review burden, and a great deal of dead code — the Cloudflare
  crate already carries thousands of generated types nothing references). Linode
  has no confirmed spec, so it cannot follow this path today without one.

### B. Hand-written thin clients

~6 typed calls per provider over `foundation_netio`, no generated code.

- **For:** small, readable, exactly the surface we use; no spec dependency, so
  Linode is unblocked; no 3 MB artefact per provider.
- **Against:** breaks the workspace convention; hand-rolled request building is
  the class of code that produced spec-53's "no body on POST /build" and
  "fieldless `NetworkInspect`" defects; schema drift is silent.

### C. Codegen where a spec exists, hand-written where not

Hetzner + DigitalOcean generated; Linode hand-written until a spec is confirmed.

- **For:** unblocks all three now; keeps the convention where it is affordable.
- **Against:** two shapes to maintain; the odd one out invites divergence.

## Recommendation

**C, leaning to A once Linode's spec is found** — but the DO spec's size is a
real cost worth confirming with the owner, since it is 3 MB of schema for six
endpoints, and the Cloudflare precedent shows most of it will be dead code.

## To resolve

1. Which option?
2. If codegen: does `genapi` support trimming a spec to selected paths, or do we
   generate the whole surface?
3. Where does Linode's OpenAPI spec live?
