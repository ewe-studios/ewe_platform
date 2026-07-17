# Progress — Spec 56: VPS Deployment Providers

**Last updated:** 2026-07-17
**Status:** Draft — **all decisions resolved**, no implementation started

## Where this came from

Spec-53 is **done**: its scope (Docker runtime, platform consolidation, proxy
capabilities) is implemented and verified end-to-end against real daemons. Its
final completeness review (2026-07-17) left four things pending that were never in
its feature set. They move here rather than hold that spec open:

| Pending item | Landed as |
|---|---|
| Cloud deployment — spec-53 decision 06, "Resolved" 2026-07-10, never built | [feature 04](features/04-cloud-init-and-ssh-bootstrap/feature.md) + the three provider crates |
| TLS termination wired but never exercised (no test makes a TLS connection through the proxy) | [feature 07](features/07-tls-termination-and-cloudflare/feature.md) |
| `SslProvider::Cloudflare` not implemented (errors out; decision 18 wants ACME DNS-01 via the Cloudflare API) | [feature 07](features/07-tls-termination-and-cloudflare/feature.md) |
| Cloudflare DNS ops untested; `find_zone` ignores its `domain` argument | [feature 07](features/07-tls-termination-and-cloudflare/feature.md) |

Plus the owner's new requirement (2026-07-17): **three VPS provider crates** —
DigitalOcean, Hetzner, Linode — with credentials from the environment and
`Deployable`s that deploy, set up and harden the instances.

## Features

| # | Feature | Phase | Status |
|---|---------|-------|--------|
| 00 | [Selective codegen](features/00-selective-codegen/feature.md) | 0 | not started — **blocks 01–03** |
| 01 | [`foundation_deployment_hetzner`](features/01-hetzner/feature.md) | 1 | not started |
| 02 | [`foundation_deployment_digitalocean`](features/02-digitalocean/feature.md) | 1 | not started |
| 03 | [`foundation_deployment_linode`](features/03-linode/feature.md) | 1 | not started — spec found (owner-supplied, validated) |
| 04 | [Cloud-init + SSH bootstrap](features/04-cloud-init-and-ssh-bootstrap/feature.md) | 2 | not started |
| 05 | [VPS hardening](features/05-vps-hardening/feature.md) | 2 | not started |
| 06 | [Composed deployables (VPS + hardening + bootstrap)](features/06-composed-deployables/feature.md) | 2 | not started |
| 07 | [TLS termination proof + Cloudflare provider](features/07-tls-termination-and-cloudflare/feature.md) | 4 | not started |
| 08 | [`ContainerConfig` restart policy](features/08-container-restart-policy/feature.md) | 2 | not started — **blocks 05** (decision 04 auto-reboot needs it) |

## Decisions

**All four are resolved** (2026-07-17). Implementation can start with feature 00,
which blocks the three provider crates.

Decision 04's auto-reboot surfaced one prerequisite, now
**[feature 08](features/08-container-restart-policy/feature.md)**: container
restart policies. Spec-53's `ContainerConfig` has none (the Docker client's
`HostConfig.RestartPolicy` exists; nothing wires it), so a 03:00 security reboot
would bring the box back with every service down. It is spec-53 code, owned here
because a spec-56 decision requires it — spec-53 is closed and verified.

| # | Decision | The question |
|---|---|---|
| 01 | [Provider clients](decisions/01-provider-clients.md) | ✅ **Resolved** — codegen for all three via selective generation ([feature 00](features/00-selective-codegen/feature.md)); output stays a checked-in `src/generated/` per crate with hand-written code outside it; API version pinned + spec revision validated; RPC (if ever needed) goes through `foundation_connectrpc`. |
| 02 | [Credentials from the environment](decisions/02-credentials-from-environment.md) | ✅ **Resolved** — `EWE_<VENDOR>_TOKEN` wins over the vendor-native name; `from_env()` errors naming both; 401 gets its own variant; token never in `Debug`/logs/errors; `new(token)`/`with_client` stay for tests + secret stores. |
| 03 | [The Deployable model](decisions/03-deployable-vps-model.md) | ✅ **Resolved** — separate Deployables composed with plain Rust (fields carry inputs); per-vendor server + provider-agnostic `SshHardening`/`DockerBootstrap`; create-or-find (recreate + log if gone); destroy-by-default on partial failure (`keep_on_failure` to opt out); vendor SSH keys removed only if we uploaded them (per-deployment key). |
| 04 | [Hardening policy](decisions/04-hardening-policy.md) | ✅ **Resolved** — root hardens then a docker-group user (passwordless sudo; root pathway still offered); cloud-init applies best-effort **and** SSH re-applies + verifies (one `HardeningPolicy` renders both); provider **and** host firewall + `DOCKER-USER`; sshd on 2222; unattended upgrades **with** auto-reboot; fail2ban. |

## What already exists (verified, spec-53's review)

The expensive half is done — this spec is the missing link, not a rebuild:

| Piece | Evidence |
|---|---|
| Bootstrap over SSH (execute/upload/download/auth) | `foundation_sshkit` — `ssh_backend_tests`, 5 e2e vs a real container |
| Drive Docker on a remote box | `DockerClient::connect_ssh` + `docker system dial-stdio` — `ssh_transport_tests` (info + container round-trip) vs docker-in-docker + sshd |
| Front services + terminate TLS + ACME | `foundation_proxy` — 9/9 reverse-proxy e2e, ACME vs a mock CA (with feature 07's caveats) |
| Build images on the remote daemon | `DockerFileConfig` + `BuildBackend::Classic` (spec-53 F04) |
| Provider/`Deployable` model + split-out provider crates | spec-11; `foundation_deployment_cloudflare` is the crate-shape model, `ContainerDeployment` the `Deployable` model |

## Verification stance

**No cloud accounts here.** Spec-53's audit found F02/F03/F04 all marked "code
present" with no consumer and no test — none of them worked. The lesson is
recorded in this spec's rule:

- everything except the vendor call is verified **locally** — cloud-init emission
  is a string; hardening and bootstrap run against the docker-in-docker + sshd
  fixture;
- the vendor call is verified against a **mock HTTP server** for shape;
- **no provider is marked complete for the live path** until someone has run it
  with real credentials. Mock-verified means mock-verified.

## Probes (2026-07-17)

| Spec source | Result |
|---|---|
| `https://docs.hetzner.cloud/cloud.spec.json` | **HTTP 200** |
| `https://api-engineering.nyc3.digitaloceanspaces.com/spec-ci/DigitalOcean-public.v2.yaml` | **HTTP 200**, ~3 MB |
| Linode — 4 public candidate URLs | all **404** (docs moved to Akamai TechDocs) |
| Linode — `linode/linode-api-openapi`, owner-supplied local clone | **validated**: OpenAPI 3.0.1, "Akamai: Linode API" v4.229.1, **9.3 MB, 334 paths**, all needed endpoints present |
