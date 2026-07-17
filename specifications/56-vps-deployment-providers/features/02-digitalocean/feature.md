---
feature: "foundation_deployment_digitalocean"
description: "DigitalOcean provider crate: create/destroy a Droplet through the v2 API with DIGITALOCEAN_TOKEN from the environment, exposing a VpsDeployment that implements Deployable"
status: "not-started"
priority: "high"
phase: 1
depends_on: ["00-selective-codegen", "01-provider-clients", "02-credentials-from-environment", "03-deployable-vps-model", "04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 02: `foundation_deployment_digitalocean`

## What

A split-out provider crate, same shape as [feature 01](../01-hetzner/feature.md):
`generated/` + `client.rs` (`from_env` → `DIGITALOCEAN_TOKEN`) + `types.rs` +
`droplet_ops.rs` + `deployable.rs` (`VpsDeployment: Deployable`).

## Endpoints we need

| Operation | Endpoint |
|---|---|
| create droplet | `POST /v2/droplets` (name, region, size, image, ssh_keys, user_data) |
| get droplet | `GET /v2/droplets/{id}` — status + `networks.v4[].ip_address` (public) |
| list droplets | `GET /v2/droplets?tag_name=` — create-or-find (decision 03) |
| delete droplet | `DELETE /v2/droplets/{id}` |
| list ssh keys | `GET /v2/account/keys` |
| create ssh key | `POST /v2/account/keys` |

## Notes specific to DigitalOcean

- **The spec is ~3 MB** (`api-engineering.nyc3.digitaloceanspaces.com/spec-ci/DigitalOcean-public.v2.yaml`,
  HTTP 200 on 2026-07-17) and covers the entire platform — droplets, Kubernetes,
  Spaces, databases, App Platform. We need six endpoints. This is the case that
  makes **decision 01** worth answering before any code: generating it whole
  would add a very large surface of dead types, exactly as the Cloudflare crate
  already carries.
- **Create returns before the droplet is up** — status is `new`, then `active`.
  The public IP is **absent** from the create response and appears later, so
  `await_running` must poll until both `status == "active"` and a public v4
  address exists.
- **`networks.v4` mixes public and private** entries; pick by `type == "public"`.
  Taking `[0]` is the obvious bug here.
- **Rate limit** 5000/hour, `RateLimit-*` headers; 429 must back off.
- Errors: `{"id": "...", "message": "..."}` — map `unauthorized` to the credential
  error (decision 02).
- **Tags** are the natural create-or-find key (`tag_name=ewe-deploy`), since names
  are not unique on DO.

## Verification

Same three layers as feature 01 — `types_tests` (serde, no network),
`droplet_ops_tests` (mock DO API: create body, poll-until-active-with-public-IP,
public-vs-private network selection, 401, 429, delete), and `deployable_tests`
(mock API + the local docker-in-docker + sshd fixture for hardening/bootstrap).

**The live path stays unverified until someone runs it with a real
`DIGITALOCEAN_TOKEN`**, and the feature must say so rather than claim completion
on mock evidence.

## Acceptance criteria

- [ ] Spec vendored into `artefacts/cloud_providers/digitalocean/`; `api_version` pinned to `v2`, `spec_version` pinned to the vendored spec's `info.version` and validated
- [ ] Generated via [feature 00](../00-selective-codegen/feature.md) — 6 endpoints, not the whole platform
- [ ] `DigitalOceanClient::from_env()` reads `DIGITALOCEAN_TOKEN` and **errors by name** when absent
- [ ] Token never appears in `Debug`, logs, or errors
- [ ] `create_droplet` sends cloud-init `user_data` and the deploy key
- [ ] `await_running` waits for `active` **and** a public IPv4 **and** sshd accepting our key
- [ ] The public IP is chosen by `type == "public"`, never by index
- [ ] `VpsDeployment::deploy` leaves a hardened box with Docker answering over SSH
- [ ] `deploy` is create-or-find by tag — it does not bill a second droplet
- [ ] `destroy` removes the droplet and clears persisted state
- [ ] Partial failure does not strand a billing droplet
- [ ] mock-API tests cover create/get/list/delete/401/429 + the public-IP selection
