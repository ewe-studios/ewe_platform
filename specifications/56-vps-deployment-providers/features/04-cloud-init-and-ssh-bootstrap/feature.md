---
feature: "Cloud-init + SSH bootstrap"
description: "Emit the cloud-init user_data every provider passes at create, then bootstrap the box over SSH — install Docker, upload the binary, verify it answers — leaving a Docker host spec-53's runtime can drive"
status: "not-started"
priority: "high"
phase: 2
depends_on: ["03-deployable-vps-model", "04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
moved_from: "spec-53 feature 20 (its decision 06, resolved 2026-07-10, never built)"
---
# Feature 04: Cloud-init + SSH Bootstrap

> **Moved here from spec-53** (where it was feature 20, seeded by that spec's
> decision 06 "Cloud Deployment"). Spec-53 is done; this was never in its feature
> set and is pending, so it lives here with the rest of the VPS work.

## What

Spec-53's decision 06 has been "Resolved" since 2026-07-10 with nothing built.
This feature is the part of that build which is provider-agnostic: the cloud-init
`user_data` every provider hands to its create call, and the SSH bootstrap that
follows. The audit of 2026-07-17 found **nothing technical blocks it** — four
of the five moving parts already exist and are verified end-to-end; what is
missing is a cloud-init template, a bootstrap sequence, and one provider API
client for creating the VM.

| Spec-53 decision 06 step | State (2026-07-17) |
|---|---|
| cloud-init provisions curl + ca-certificates | **not built** — a string template; depends on nothing |
| bootstrap Docker + testbed binary over SSH | **exists, verified** — `foundation_sshkit`: `ssh_backend_tests` (execute/upload/download/auth, 5 e2e vs a real container) |
| container ops over SSH afterwards | **exists, verified** — `DockerClient::connect_ssh` + `docker system dial-stdio`; `ssh_transport_tests` (info + container round-trip) pass against docker-in-docker + sshd |
| service exposure + TLS via `foundation_proxy` | **exists, verified** — 9/9 reverse-proxy e2e, ACME provisioning (but see the TLS caveat below) |
| create the VM (Hetzner/AWS/GCP) | **not built** — the only piece with an external dependency |

## Why it is worth doing now

The expensive parts are done. `connect_ssh` was proven against a live daemon on
2026-07-17 (`container_round_trip_over_ssh`), so "drive Docker on a remote box"
— the piece most likely to hide surprises — already works. What remains is
sequencing that machinery, plus one API client.

## Scope

1. **Cloud-init template** — emits a `#cloud-config` installing `curl` and
   `ca-certificates` and authorising the deploy key. Pure string generation;
   unit-testable with no network.
2. **SSH bootstrap** — over `foundation_sshkit`: install Docker via
   `get.docker.com`, upload the testbed binary, verify `docker version` answers.
   Idempotent, so re-running a bootstrap on a live host is a no-op.
3. **Remote runtime** — `DockerClient::connect_ssh("ssh://user@host")` for every
   subsequent container operation. Already works; this step is wiring.
4. **Provider: create/destroy the VM** — now three crates of their own:
   [Hetzner](../01-hetzner/feature.md), [DigitalOcean](../02-digitalocean/feature.md),
   [Linode](../03-linode/feature.md), behind the shared
   [`VpsDeployment`](../06-vps-deployable/feature.md).
5. **Service exposure** — `foundation_proxy` in front, TLS per decision 18.

## Constraints to respect

- **`ssh` and `buildkit` are mutually exclusive features.** `connect_ssh` links
  libssh2/OpenSSL; `buildkit` reaches BoringSSL via
  `jwt-simple → foundation_auth → foundation_connectrpc`. Two libcryptos in one
  binary means duplicate `EVP_*` symbols and a link failure (decision 05, "How
  the build runs"). A deployment driving a remote daemon over SSH must therefore
  build images with the **`Classic`** backend on that daemon, not by driving a
  standalone buildkitd.
- **bollard is gone.** Decision 06's "Remote Docker via bollard SSH" predates the
  migration; the equivalent is `DockerClient::connect_ssh`.
- **TLS termination is wired but not proven end-to-end** (see
  [feature 07](../07-tls-termination-and-cloudflare/feature.md)), and
  `SslProvider::Cloudflare` is not implemented. A VPS fronting HTTPS depends on both.

## Verification plan

The point of this feature is that most of it is verifiable **without a cloud
account**:

- **cloud-init template** — unit tests on the emitted YAML.
- **Bootstrap + remote runtime** — end-to-end against the docker-in-docker + sshd
  fixture the `ssh_transport_tests` already use (`docker run -d --privileged
  docker:dind` + `apk add openssh`). This exercises the real path: SSH in,
  install/verify, then run containers over `dial-stdio`. No cloud, no cost.
- **Provider VM create/destroy** — this is the part that **cannot be verified
  here**: it needs a real account and costs money. Verify against a mock HTTP
  server for shape (request/response, error handling), and mark the feature
  complete for the mock only — **do not claim the live path works until someone
  has run it with credentials.** Committing an unverified provider is exactly the
  pattern that left F02/F03/F04 broken behind a "code present" label.

## Open questions

- **Hardening in cloud-init, over SSH, or both?** See
  [decision 04](../../decisions/04-hardening-policy.md) — the cloud-init path
  cannot be verified locally, which argues for one applied-and-tested path.
- **What does the bootstrap install** beyond Docker — the testbed binary always,
  or only when asked?
