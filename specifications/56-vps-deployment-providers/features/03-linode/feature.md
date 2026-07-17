---
feature: "foundation_deployment_linode"
description: "Linode provider crate: create/destroy a Linode instance through the v4 API with LINODE_TOKEN from the environment, exposing a VpsDeployment that implements Deployable"
status: "not-started"
priority: "high"
phase: 1
depends_on: ["01-provider-clients", "02-credentials-from-environment", "03-deployable-vps-model", "04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 03: `foundation_deployment_linode`

## What

A split-out provider crate, same shape as [feature 01](../01-hetzner/feature.md):
`client.rs` (`from_env` → `LINODE_TOKEN`) + `types.rs` + `instance_ops.rs` +
`deployable.rs` (`VpsDeployment: Deployable`).

## Blocked on a spec source

**Linode's OpenAPI spec source is unconfirmed.** Probed 2026-07-17, all 404:

- `https://raw.githubusercontent.com/linode/linode-openapi/main/openapi.yaml`
- `https://raw.githubusercontent.com/linode/linode-api-docs/development/openapi.yaml`
- `https://www.linode.com/docs/api/openapi.yaml`
- `https://api.linode.com/v4/openapi.yaml`

Linode's docs moved to Akamai TechDocs, which likely relocated the spec. So this
feature carries an extra step the other two do not: **find the spec, or hand-write
the client** — see [decision 01](../../decisions/01-provider-clients.md), which is
partly *about* this case. Do that first; the rest of the feature depends on the
answer.

## Endpoints we need

| Operation | Endpoint |
|---|---|
| create instance | `POST /v4/linode/instances` (label, region, type, image, authorized_keys, root_pass, metadata.user_data) |
| get instance | `GET /v4/linode/instances/{id}` — status + `ipv4[]` |
| list instances | `GET /v4/linode/instances?tags=` — create-or-find (decision 03) |
| delete instance | `DELETE /v4/linode/instances/{id}` |
| list ssh keys | `GET /v4/profile/sshkeys` |
| create ssh key | `POST /v4/profile/sshkeys` |

## Notes specific to Linode

- **`root_pass` is required on create** even when we only ever intend key auth.
  Generate a long random one, never log it, and let hardening disable password
  auth (decision 04) — the password exists only to satisfy the API.
- **cloud-init lives under `metadata.user_data`** (base64), and **only on regions
  and images that support the Metadata service**. Where it is unsupported,
  cloud-init hardening is unavailable and the post-boot SSH path is the only
  option — which is an argument for decision 04's "one tested path" option.
- **`authorized_keys`** on create takes the public keys directly, which gets us in
  without a password.
- Status goes `provisioning` → `booting` → `running`; `ipv4[]` mixes public and
  private (private are in `192.168.128.0/17`), so selection matters as it does on
  DO.
- Errors: `{"errors": [{"field": "...", "reason": "..."}]}` — a different shape
  from the other two; map `reason` and treat 401 distinctly (decision 02).

## Verification

Same three layers as features 01/02 — `types_tests`, `instance_ops_tests` (mock
Linode API: create body incl. `authorized_keys` + `metadata.user_data`,
poll-until-running, public-vs-private IP selection, the `errors[]` shape, 401,
429), and `deployable_tests` (mock API + the local docker-in-docker + sshd
fixture).

**The live path stays unverified until someone runs it with a real
`LINODE_TOKEN`.**

## Acceptance criteria

- [ ] Spec source confirmed, or decision 01 resolved to hand-write this client
- [ ] `LinodeClient::from_env()` reads `LINODE_TOKEN` and **errors by name** when absent
- [ ] Token and the generated `root_pass` never appear in `Debug`, logs, or errors
- [ ] `create_instance` sends `authorized_keys` and (where supported) `metadata.user_data`
- [ ] Regions/images without the Metadata service fall back to post-boot hardening — and say so, rather than silently skipping it
- [ ] `await_running` waits for `running` **and** a public IPv4 **and** sshd accepting our key
- [ ] The public IP is chosen by range, never by index
- [ ] `VpsDeployment::deploy` leaves a hardened box with Docker answering over SSH
- [ ] `deploy` is create-or-find by tag — it does not bill a second instance
- [ ] `destroy` removes the instance and clears persisted state
- [ ] mock-API tests cover create/get/list/delete/401/429 + the `errors[]` shape
