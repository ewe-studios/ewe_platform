# Requirements — Spec 56: VPS Deployment Providers

**Status:** Draft (decision 01 resolved; 02–04 open, awaiting resolution)
**Created:** 2026-07-17
**Owner:** Main Agent
**has_features:** true

---

## Summary

Stand up a **VPS on a real cloud provider, harden it, and make it a deployment
target** — from Rust, with credentials supplied through the environment and no
CLI tooling in the loop.

Three provider crates, one per vendor:

- `foundation_deployment_digitalocean`
- `foundation_deployment_hetzner`
- `foundation_deployment_linode`

Each exposes a **`Deployable`**: `deploy` creates the VPS, waits for it to come
up, hardens SSH access, installs Docker, and persists the instance id; `destroy`
tears it down. After that the box is an ordinary Docker host —
`DockerClient::connect_ssh("ssh://user@host")` drives containers on it, and
`foundation_proxy` fronts them with TLS.

This spec exists because **spec-53 is done**: its own scope (Docker runtime,
platform consolidation, proxy capabilities) is implemented and verified end-to-end
against real daemons. What it left pending was never in its feature set — cloud
deployment (its decision 06, resolved as a design and never built) — plus two
gaps its final review surfaced in the TLS path. Those move here rather than hold
spec-53 open.

## Why this is worth doing now

The expensive half already exists and is proven (spec-53's final review,
2026-07-17):

| Piece | State |
|---|---|
| Bootstrap over SSH — execute, upload, download, auth | ✅ `foundation_sshkit`, 5 e2e vs a real container |
| Drive Docker on a remote box | ✅ `DockerClient::connect_ssh` + `docker system dial-stdio`; e2e vs docker-in-docker + sshd |
| Front services, terminate TLS, provision certs | ✅ `foundation_proxy` — 9/9 reverse-proxy e2e, ACME vs a mock CA (with the caveats in feature 07) |
| Build images on the remote daemon | ✅ `DockerFileConfig` + `BuildBackend::Classic` (spec-53 F04) |
| **Create the VPS** | ❌ **this spec** |

So the missing link is the provider APIs and the hardening sequence. The piece
most likely to hide surprises — "drive Docker on a machine you only reach over
SSH" — already works.

## Goals

1. **Create and destroy a VPS** on DigitalOcean, Hetzner and Linode, from Rust,
   through each vendor's HTTP API.
2. **Credentials from the environment** — a token per provider, never a file
   path, never a CLI's ambient login, never a constructor argument that invites
   hard-coding.
3. **Harden it before it is used** — key-only SSH, no root password, a firewall,
   unattended security updates. A box that is reachable but not hardened is not
   "deployed".
4. **Leave a working Docker host** — Docker installed and answering over SSH, so
   spec-53's runtime takes over unchanged.
5. **One shape across providers** — the same `Deployable`, the same hardening,
   the same bootstrap; only the create/destroy calls differ.
6. **Verifiable without an account** for everything except the vendor call
   itself (see [Verification](#verification)).

## Non-goals

- Kubernetes, managed databases, load balancers, or any vendor service beyond
  "give me a Linux box with an IP".
- Multi-region topology, autoscaling, or fleet management.
- Replacing the spec-53 runtime. Once the box is up, it is just a Docker host.
- Windows or non-Linux images.

## Scope

### Per provider crate

Following the established split-out provider pattern (`foundation_deployment_cloudflare`
is the model: generated API client in `generated/`, hand-written domain logic on
top, registered in `genapi`'s `SPLIT_OUT_PROVIDERS`):

- **Generated client** from the vendor's OpenAPI spec — **all three**, per
  [decision 01](decisions/01-provider-clients.md), using **selective generation**
  ([feature 00](features/00-selective-codegen/feature.md)) so each crate carries
  only the ~6 endpoints it uses and their schema closure. Sources (2026-07-17):
  **Hetzner** (`docs.hetzner.cloud/cloud.spec.json`, 200), **DigitalOcean**
  (`…/DigitalOcean-public.v2.yaml`, 200, ~3 MB), **Linode**
  (`linode/linode-api-openapi`, owner-supplied local clone — OpenAPI 3.0.1,
  v4.229.1, 9.3 MB, 334 paths).
- **Hand-written domain logic** — `client.rs` (env credentials), `types.rs`
  (the handful of types we actually use), `server_ops.rs` (create / get / list /
  delete / await-ready / public IP).
- **`VpsDeployment` implementing `Deployable`** — deploy/destroy with the id
  persisted through the `Deployable` store.

### Shared

- **Cloud-init + SSH bootstrap** (feature 04, moved from spec-53 decision 06).
- **Hardening** (feature 05) — the same policy on every provider.
- **TLS termination proof + Cloudflare cert provider** (feature 07, moved from
  spec-53's final review): the proxy's TLS termination is wired but no test ever
  makes a TLS connection through it, and `SslProvider::Cloudflare` errors out
  while decision 18 asks for ACME DNS-01 via the Cloudflare API. A VPS fronting
  HTTPS depends on both.

## Verification

The constraint that shapes this spec: **we have no cloud accounts here**, and
committing unverified code behind a "done" label is exactly what left spec-53's
F02/F03/F04 broken for weeks. So:

- **Everything except the vendor call is verified locally.** Cloud-init emission
  is a string; the SSH bootstrap and hardening run against the docker-in-docker +
  sshd fixture the spec-53 SSH transport tests already use — real sshd, real
  `authorized_keys`, real `sshd_config`, real Docker install.
- **The vendor call is verified against a mock HTTP server** for shape: request
  path/body/headers, pagination, error mapping, and the await-ready poll loop.
- **No provider is marked complete for the live path** until someone has run it
  with real credentials. Mock-verified means mock-verified, and the feature docs
  say so.

## Decisions

| # | Decision | Why it matters |
|---|---|---|
| 01 | [Provider clients](decisions/01-provider-clients.md) | ✅ **Resolved** — codegen for all three, conditional on selective generation (feature 00). |
| 02 | [Credentials from the environment](decisions/02-credentials-from-environment.md) | Variable names, precedence, and what happens when a token is absent or rejected. |
| 03 | [The Deployable VPS model](decisions/03-deployable-vps-model.md) | What `deploy` guarantees, what is persisted, and whether it is idempotent. |
| 04 | [Hardening policy](decisions/04-hardening-policy.md) | What "hardened" means concretely, and whether it is provider-side, cloud-init, or post-boot SSH. |

## Related specs

- **[Spec 53](../completed/53-docker-container-testbed/)** — the Docker runtime,
  platform and proxy this deploys. Done; its decision 06 (cloud deployment) is
  the seed of this spec.
- **[Spec 54](../completed/54-foundation-deployment-docker/)** — the bollard-free
  Docker client, including the `ssh` transport this relies on.
- **[Spec 11](../11-foundation-deployment/)** — the `Deployable`/provider model
  and the existing split-out provider crates.
