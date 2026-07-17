---
feature: "foundation_deployment_hetzner"
description: "Hetzner Cloud provider crate: create/destroy a server through the Cloud API with HCLOUD_TOKEN from the environment, exposing a VpsDeployment that implements Deployable"
status: "in-progress"
priority: "high"
phase: 1
depends_on: ["00-selective-codegen", "01-provider-clients", "02-credentials-from-environment", "03-deployable-vps-model", "04-hardening-policy"]
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
│   ├── generated/      # selective codegen from artefacts/cloud_providers/hetzner/openapi.json
│   ├── client.rs       # HetznerClient::from_env() -> HCLOUD_TOKEN (decision 02)
│   ├── types.rs        # the few types we use: Server, ServerStatus, SshKey, Image, ServerType
│   ├── server_ops.rs   # create / get / list / delete / await_running / public_ip
│   └── deployable.rs   # VpsDeployment: impl Deployable (decision 03)
└── tests/
    ├── types_tests.rs           # serde round-trips (no network)
    ├── server_ops_tests.rs      # vs a mock Hetzner API
    └── deployable_tests.rs      # vs a mock API + the local sshd fixture
```

Registered in `genapi`'s `SPLIT_OUT_PROVIDERS`
(`backends/foundation_codegentools/src/cli/gen_api.rs`), and generated through
[feature 00](../00-selective-codegen/feature.md) so only these six endpoints and
their schema closure are emitted.

## Spec + version pins

Validated 2026-07-17: `https://docs.hetzner.cloud/cloud.spec.json` — **Hetzner
Cloud API**, OpenAPI **3.1.2**, `info.version` **1.0.0**, **151 paths**.

- **API version:** `v1` — pinned into the client; callers never pass it.
- **Spec revision:** `1.0.0` — validated against `info.version` at generation;
  a mismatch fails (feature 00 §3).

## Endpoints we need

Six, out of 151 paths:

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
- **Live path: VERIFIED 2026-07-17** — `tests/live_tests.rs`, 5 read-only tests
  against the real API, `#[ignore]`d so they are opted into rather than swept up
  by a bare `cargo test`:

  ```
  HCLOUD_TOKEN=… cargo test --profile uat -p foundation_deployment_hetzner \
      --test live_tests -- --ignored --nocapture
  ```

  All **6** pass, including a **create/destroy round-trip that bills real money**
  (`deploy_then_destroy_a_real_server`, ~€0.0104/hr on a `cx23` for ~20s). It
  proves on a real machine what the mocks only assert: a second `deploy` returns
  the same id and **does not bill a second server**.

  **The first one found a bug within seconds**, and the round-trip found three
  more — see below. That is the whole argument for running them.

#### Mocks earned nothing here — the score, honestly

| Source | Bugs found |
|---|---|
| Real vendor specs | 6 |
| Real transport / the live API | **4** |
| **31 mock tests** | **0** |

Not one. And one mock test **asserted the bug outright** — `destroy` "does not call
Hetzner to find that out" was the exact reasoning that stranded a billing server.
A mock answers what you told it; teaching it a misunderstanding turns that
misunderstanding into a green check forever.

**So: prefer the live test wherever the vendor can be put in the state for real**
(owner, 2026-07-17). Hetzner costs a fraction of a cent and answers in 20 seconds.
The mock's remaining job is the narrow set of faults a vendor cannot be *asked* for
— an unparseable 201, a 429 with a specific reset header — and even those are worth
suspecting.

#### Teardown for a billing resource cannot be keyed on our own state

The first run of the round-trip **leaked a live server while reporting success**.
Worth writing down, because the failure is not obvious:

1. Hetzner **created** the server and returned 201.
2. Our client **failed to parse that 201** (the nullable-`$ref` bug below).
3. So `create_server` returned `Err` — and **we never learned the server's id**.
4. `deploy`'s own unwind (decision 03 §5) could not fire: it deletes `created.id`,
   and `created` never existed.
5. `destroy` read the state store, found nothing recorded, and correctly returned
   `Ok(())` — *"nothing to destroy"*.
6. The test printed **"destroyed"**. The machine ran on, billing.

**The vendor is the authority on what exists, not our state store.** And the fix
belongs in the **deployable**, not the test — a first attempt swept the leak up in
the test, which fixed nothing for an actual user hitting the same parse failure:

- **`destroy` falls back to the name.** It never needed the store: `self.name` is
  right there, and the name *is* the identity — the same lookup `deploy` already
  uses to avoid billing twice. An empty store means "we have no record", **not**
  "nothing exists".
- **`deploy` treats a create failure as ambiguous.** A decode error, a dropped
  connection or a timeout after `POST /servers` may well mean Hetzner built the
  machine and failed us on the way back. The ordinary unwind cannot help — it
  deletes `created.id`, and there is no id. So it asks by name before giving up.

`sweep_leftovers` still runs *before* each live test, for the one case nothing
in-process can catch: a run killed (`^C`) between create and destroy.

The general shape: *the vendor created it; we just cannot name it.* Any resource
that bills needs a teardown that does not depend on having successfully observed
the creation.

The test also holds no `assert!`/`unwrap` between create and destroy — every check
collects into a `Result` inspected *after* teardown, because a panic there unwinds
straight past the cleanup.

- **(historical) Live path: unverified until someone runs it with a real `HCLOUD_TOKEN`.**
  Say so in the feature's status — do not mark it complete on mock evidence.
  (Spec-53's F02/F03/F04 were all "code present" with no consumer; a
  mock-verified provider is better than that, but it is not "it works".)

## Status (2026-07-17)

**Code complete and mock-verified; the live path is unverified.** What remains:

- `await_running` waits for Hetzner to finish building — **not** for sshd. Waiting
  for SSH belongs to feature 04, and the criterion below stays unchecked until it
  exists.
- `VpsDeployment::deploy` leaving a hardened box with Docker answering is
  features 05/06 — the provider-agnostic `SshHardening`/`DockerBootstrap`
  deployables (decision 03 §1), which are testable against the local sshd fixture
  with no cloud account.
- **Nobody has run this against Hetzner.** Mock evidence is not "it works": the
  mock answers what we told it to. Two of this feature's bugs were found only by
  moving from fixtures to a real transport, and the same will be true of the live
  path.

### The live test earned its keep on the first call

`list_servers` against the real API died immediately:

```
Decode("invalid type: null, expected i64")
```

Hetzner sends `"next_page": null` on the last page. We generated `pub next_page: i64`.

**`required` and `nullable` are different statements** — the first says the key is
always present, the second says its value may be `null`. A field can be both, and
Hetzner's pagination is: `next_page` is *required and nullable*. The generator
treated `required` as "not an Option" and ignored `nullable` entirely.

The chain had three links and the last one was missing: `normalize_nullable_types`
correctly rewrote 3.1's `"type": ["integer","null"]` into `nullable: true`, and
wrote it to the artefact — but **`Schema` never deserialised a `nullable` field**,
so the generator never saw it. Computed, persisted, and thrown away.

**857 fields across the four vendored specs are required AND nullable** — hetzner
441, cloudflare 292, linode 108, digitalocean 16. Every one was a latent panic on
real data.

**No mock could have found this.** These tests build fixtures from the generated
types via `Default`, which yields `0` — never `null`. The mock answers what we told
it to; only the vendor knows what the vendor sends.

A second instance, caught by the same fix: `root_password` is nullable because
Hetzner returns `null` for it **when you create a server with SSH keys** — which is
exactly what this feature does. `create_server` would have panicked on its first
real call.

#### The round-trip found three more, all in one call

| Bug | Why a mock could never see it |
|---|---|
| **nullable `$ref`** — `deprecation` is a *required, nullable, inline object*. `normalize_nullable_types` recorded `nullable: true`, then hoisting moved the schema into `components` and left the property a bare `$ref` carrying nothing. `is_nullable` now follows the ref | `create_server` died on the real 201 with `invalid type: null, expected struct …Deprecation` — **after Hetzner had built the machine** |
| **`public_net.ipv4` is nullable** — a server can have no public IPv4. Our hand-written `server_ops` did `.ipv4.ip` | the fix surfaced it as a compile error; `Server.public_ipv4: Option<String>` had modelled it correctly all along |
| **`cx22` no longer exists** — it was `HetznerServer::new`'s default | a mock accepts any server type you tell it to |

### What building it taught us

Six bugs, none of which a fixture would have caught, all fixed:

| Where | Bug |
|---|---|
| `foundation_openapi` | `resolve_schema_key` guessed at the inverse of pascal-casing, so **52 of Hetzner's 100 types silently became `HashMap<String, Value>` blobs** — including `public_net`, where the server's IP lives |
| `foundation_openapi` | the generator emitted `ApiError::HttpStatus { body: None }` unconditionally — **every vendor's error detail, on every endpoint, read off the socket and discarded** (Cloudflare: 2442 of them) |
| `foundation_testing` | `TestHttpServer::with_response` never answered `Expect: 100-continue`, which our client sends by default on any request with a body — so **every request body arrived empty, silently** |
| `foundation_openapi` | `nullable` was computed, written to the artefact, and **never read** — `Schema` had no such field. 857 required-and-nullable fields across four vendors generated as non-`Option`, each a panic waiting for real data |

## Acceptance criteria

- [x] Spec vendored into `artefacts/cloud_providers/hetzner/`; `api_version` pinned to `v1`, `spec_version` to `1.0.0` and validated
- [x] Generated via [feature 00](../00-selective-codegen/feature.md) — **6 endpoints, not 151**; 1636 lines, 100 structs, every response a named type (Hetzner's spec is bundled, so without canonicalisation these would have generated nothing usable). Selected by `operationId`, not path: `/servers/{id}` also carries PUT `update_server`, which a path selection would drag in
- [x] `HetznerClient::from_env()` reads `EWE_HCLOUD_TOKEN` then `HCLOUD_TOKEN`, and **errors naming both** when absent (an empty token counts as absent — it would otherwise surface as a 401, blaming the token rather than the setup)
- [x] Token never appears in `Debug`, logs, or errors — hand-written `Debug`, asserted in `{:?}` and `{:#?}`; no error variant holds it. (Audited `CloudflareClient` per decision 02 §4: it derives no `Debug` at all, so it is clean by accident rather than design.)
- [x] `create_server` sends cloud-init `user_data` and the deploy key — asserted on the captured request body
- [ ] `await_running` waits for `running` **and** for sshd to accept our key — the sshd half is feature 04
- [ ] `VpsDeployment::deploy` leaves a hardened box with Docker answering over SSH
- [x] `deploy` is create-or-find — proven by asserting **no POST** on the second deploy, on adoption of an unrecorded box with our name (the crash-between-create-and-persist case), and on a second instance id
- [x] `destroy` removes the server and clears persisted state; destroying what was never deployed is not an error and does not call Hetzner
- [x] Partial failure does not strand a billing instance — a created-but-unusable box is destroyed; `keep_on_failure(true)` opts out and logs that it is still billing. A failed cleanup logs at **error** ("IT IS STILL BILLING") and still reports the original failure
- [x] mock-API tests cover create/get/list/delete/401/429 + unparseable bodies, ssh-key duplicate handling, and `await_running` (17 in `server_ops_tests`, 11 in `deployable_tests`, 9 in `client_tests`)
- [ ] hardening asserted against the local sshd fixture (feature 05)
