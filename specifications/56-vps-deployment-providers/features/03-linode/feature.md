---
feature: "foundation_deployment_linode"
description: "Linode provider crate: create/destroy a Linode instance through the v4 API with LINODE_TOKEN from the environment, exposing a VpsDeployment that implements Deployable"
status: "not-started"
priority: "high"
phase: 1
depends_on: ["00-selective-codegen", "01-provider-clients", "02-credentials-from-environment", "03-deployable-vps-model", "04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 03: `foundation_deployment_linode`

## What

A split-out provider crate, same shape as [feature 01](../01-hetzner/feature.md):
`client.rs` (`from_env` → `LINODE_TOKEN`) + `types.rs` + `instance_ops.rs` +
`deployable.rs` (`VpsDeployment: Deployable`).

## Spec source — found (2026-07-17)

The public URLs all 404 (Linode's docs moved to Akamai TechDocs and the spec moved
with them). The owner supplied the real one: the official
**`linode/linode-api-openapi`** repo, cloned at

```
/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/src.linode/linode-api-openapi/openapi.json
```

Validated: OpenAPI **3.0.1**, "Akamai: Linode API" **v4.229.1**, **9.3 MB**,
**334 paths**, and every endpoint below is present. Vendor it into
`artefacts/cloud_providers/linode/` like the other providers.

At 9.3 MB for six endpoints, this crate is the clearest case for
[feature 00](../00-selective-codegen/feature.md) — without selective generation it
would carry a 334-path surface to create and delete a server.

## Endpoints we need

Note the spec's paths carry the API version as a **path parameter**
(`/{apiVersion}/…`), not a base-URL constant:

| Operation | Endpoint (as the spec declares it) |
|---|---|
| create instance | `POST /{apiVersion}/linode/instances` (label, region, type, image, authorized_keys, root_pass, metadata.user_data) |
| get instance | `GET /{apiVersion}/linode/instances/{linodeId}` — status + `ipv4[]` |
| list instances | `GET /{apiVersion}/linode/instances?tags=` — create-or-find (decision 03) |
| delete instance | `DELETE /{apiVersion}/linode/instances/{linodeId}` |
| list ssh keys | `GET /{apiVersion}/profile/sshkeys` |
| create ssh key | `POST /{apiVersion}/profile/sshkeys` |

## Notes specific to Linode

- **`apiVersion` is a path parameter.** Every generated call will take it as an
  argument unless it is pinned. The hand-written layer should fix it at `v4` so
  callers never pass it — an API version is not a per-call decision.
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

- [ ] Spec vendored into `artefacts/cloud_providers/linode/` from the owner-supplied repo
- [ ] Generated via [feature 00](../00-selective-codegen/feature.md) — 6 endpoints, not 334
- [ ] `apiVersion` pinned to `v4` by the hand-written layer, not exposed to callers
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
