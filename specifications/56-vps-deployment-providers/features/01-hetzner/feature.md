---
feature: "foundation_deployment_hetzner"
description: "Hetzner Cloud provider crate: create/destroy a server through the Cloud API with HCLOUD_TOKEN from the environment, exposing a VpsDeployment that implements Deployable"
status: "not-started"
priority: "high"
phase: 1
depends_on: ["01-provider-clients", "02-credentials-from-environment", "03-deployable-vps-model", "04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 01: `foundation_deployment_hetzner`

## Why Hetzner first

The smallest API of the three and the cleanest fit: one `POST /servers` creates a
box, one `DELETE /servers/{id}` removes it, and **its OpenAPI spec is reachable**
(`https://docs.hetzner.cloud/cloud.spec.json`, HTTP 200 as of 2026-07-17). If the
shape works here it transfers to the other two.

## What

A split-out provider crate following the `foundation_deployment_cloudflare`
pattern:

```
backends/foundation_deployment_hetzner/
├── src/
│   ├── generated/      # from artefacts/cloud_providers/hetzner/openapi.json (decision 01)
│   ├── client.rs       # HetznerClient::from_env() -> HCLOUD_TOKEN (decision 02)
│   ├── types.rs        # the few types we use: Server, ServerStatus, SshKey, Image, ServerType
│   ├── server_ops.rs   # create / get / list / delete / await_running / public_ip
│   └── deployable.rs   # VpsDeployment: impl Deployable (decision 03)
└── tests/
    ├── types_tests.rs           # serde round-trips (no network)
    ├── server_ops_tests.rs      # vs a mock Hetzner API
    └── deployable_tests.rs      # vs a mock API + the local sshd fixture
```

Registered in `genapi`'s `SPLIT_OUT_PROVIDERS` (`backends/foundation_codegentools/src/cli/gen_api.rs`)
if decision 01 lands on codegen.

## Endpoints we need

Six, out of a much larger API:

| Operation | Endpoint |
|---|---|
| create server | `POST /servers` (name, server_type, image, location, ssh_keys, user_data) |
| get server | `GET /servers/{id}` — status + `public_net.ipv4.ip` |
| list servers | `GET /servers?name=` — for create-or-find (decision 03) |
| delete server | `DELETE /servers/{id}` |
| list ssh keys | `GET /ssh_keys` |
| create ssh key | `POST /ssh_keys` |

`user_data` on create is where cloud-init goes (feature 04), which is what makes
hardening-before-first-boot possible on this provider.

## Notes specific to Hetzner

- **Actions are asynchronous.** `POST /servers` returns an `action` that is still
  `running`; the server is not usable when the call returns. `await_running` polls
  `GET /servers/{id}` until `status == "running"` — and then still waits for sshd,
  which comes later (decision 03).
- **Rate limit** is 3600/hour per project, returned in `RateLimit-*` headers. The
  poll loop must back off rather than spin.
- Errors come back as `{"error": {"code": "...", "message": "..."}}` — map `code`
  to typed variants, especially `unauthorized` (decision 02, item 3).

## Verification

- **`types_tests`** — serde round-trips of the response shapes. No network.
- **`server_ops_tests`** — against a **mock Hetzner API**: assert the create body
  (name/type/image/ssh_keys/user_data), that `await_running` polls until running
  rather than returning early, that a 401 maps to the credential error, that a
  429 backs off, and that delete issues the right call.
- **`deployable_tests`** — `deploy`/`destroy` against the mock API, with the SSH
  bootstrap + hardening pointed at the **docker-in-docker + sshd fixture** (the
  one `foundation_deployment_docker::ssh_transport_tests` already uses). This
  exercises the real hardening and Docker-install path with no cloud account.
- **Live path: unverified until someone runs it with a real `HCLOUD_TOKEN`.**
  Say so in the feature's status — do not mark it complete on mock evidence.
  (Spec-53's F02/F03/F04 were all "code present" with no consumer; a
  mock-verified provider is better than that, but it is not "it works".)

## Acceptance criteria

- [ ] `HetznerClient::from_env()` reads `HCLOUD_TOKEN` and **errors by name** when absent
- [ ] Token never appears in `Debug`, logs, or errors
- [ ] `create_server` sends cloud-init `user_data` and the deploy key
- [ ] `await_running` waits for `running` **and** for sshd to accept our key
- [ ] `VpsDeployment::deploy` leaves a hardened box with Docker answering over SSH
- [ ] `deploy` is create-or-find — it does not bill a second server (decision 03)
- [ ] `destroy` removes the server and clears persisted state
- [ ] Partial failure does not strand a billing instance (decision 03, item 5)
- [ ] mock-API tests cover create/get/list/delete/401/429
- [ ] hardening asserted against the local sshd fixture (feature 05)
