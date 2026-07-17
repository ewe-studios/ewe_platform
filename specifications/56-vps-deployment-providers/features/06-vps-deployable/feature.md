---
feature: "VpsDeployment (the shared Deployable)"
description: "The shared Deployable shape every provider crate implements: deploy creates + hardens + bootstraps a VPS and persists its id; destroy tears it down; state survives a process restart"
status: "not-started"
priority: "high"
phase: 2
depends_on: ["03-deployable-vps-model", "05-vps-hardening", "04-cloud-init-and-ssh-bootstrap"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 06: `VpsDeployment` — the shared `Deployable`

## What

The owner's requirement: *"we setup deployables for these to use the crates or API
each provider provides to deploy, setup and harden these VPS instances for use."*

This is that shape — the contract the three provider crates implement, so a caller
writes the same code whether the box is a Droplet, a Hetzner server or a Linode:

```rust
let vps = HetznerVps::new("ewe-testbed")          // or DigitalOceanVps / LinodeVps
    .region("nbg1")
    .size("cx22")
    .deploy_key(&pubkey);

client.deploy(&vps).await?;                        // create → harden → bootstrap
// the box is now a Docker host:
let docker = DockerClient::connect_ssh(&format!("ssh://root@{}", vps.public_ip()?))?;
```

## Prior art

`foundation_deployment_docker::ContainerDeployment` (spec-54, decision 05) is the
model and the closest analogue: `deploy` creates + starts and **persists the id**
through the `Deployable::store`; `destroy` reads it back and removes the thing;
both are plain async returning `BoxFuture`; and it builds its **own** client
because Docker's transport does not fit the `ProviderClient`'s HTTP client — "the
canonical unique underlying mechanics case". A VPS deployment is the same shape
with an HTTPS API instead of a Unix socket and an instance id instead of a
container id.

## What `deploy` guarantees

Per [decision 03](../../decisions/03-deployable-vps-model.md) (**open**), the
leaning is that `deploy` returns only when the box is genuinely usable:

1. the instance exists and the vendor reports it running;
2. it has a public IP;
3. sshd answers with our key — the vendor's "running" precedes sshd by many
   seconds, and this is where a naive implementation races;
4. hardening is applied **and verified** (feature 05);
5. Docker is installed and `docker version` answers over SSH (feature 04).

Anything less pushes the waiting onto every caller, which is where the subtle bugs
live. Decision 03 asks whether that is too much for one call.

## Decisions this feature is waiting on

All of [decision 03](../../decisions/03-deployable-vps-model.md):

- one `deploy`, or composed `create`/`harden`/`bootstrap` steps?
- what is persisted beyond the id (public IP? provider?) — a caller that restarts
  should not have to hit the API to `connect_ssh` again;
- **create-or-find**: if state holds a live instance, return it rather than bill a
  second box; and what to do when the recorded instance is gone;
- whether `destroy` touches vendor-side SSH keys (shared keys make this dangerous);
- **destroy-on-partial-failure**, defaulting on: a stranded VPS bills, which is
  the money version of the stranded containers spec-53 F04 had to fix.

## Verification

- **Mock API + local sshd fixture.** `deploy`/`destroy` drive a mock vendor API
  for create/get/delete, while the hardening and bootstrap steps run against the
  docker-in-docker + sshd fixture — so the sequencing, the await-ready loop, the
  hardening and the Docker install are all exercised for real, without an account.
- **State**: assert the id survives a store round-trip, and that `destroy` after a
  simulated restart still finds and removes the instance (the shape
  `deployable_tests` already proves for containers in spec-54).
- **Create-or-find**: assert a second `deploy` with live state does **not** issue a
  create call.
- The live vendor path stays unverified until run with real credentials.

## Acceptance criteria

- [ ] Decision 03 resolved
- [ ] One `VpsDeployment` contract; all three crates implement it identically
- [ ] `deploy` returns only when the box is hardened, Dockered, and reachable over SSH
- [ ] Instance id (and IP, if decided) persisted through the `Deployable` store
- [ ] `destroy` works after a process restart, from persisted state alone
- [ ] Second `deploy` with live state issues no create call
- [ ] Partial failure does not strand a billing instance
- [ ] Tests: mock API + local sshd fixture, incl. the restart and create-or-find cases
