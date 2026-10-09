---
feature: "Composed deployables (VPS + hardening + bootstrap)"
description: "Separate focused Deployables composed with regular Rust — a per-vendor server deployable that outputs { id, public_ip }, plus provider-agnostic SshHardening and DockerBootstrap that take a host via their struct fields"
status: "not-started"
priority: "high"
phase: 2
depends_on: ["03-deployable-vps-model", "05-vps-hardening", "04-cloud-init-and-ssh-bootstrap"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 06: Composed Deployables

## What

The owner's requirements: *"we setup deployables for these to use the crates or
API each provider provides to deploy, setup and harden these VPS instances for
use"*, and — *"deployables can be composed allowing us have separate Deployables
focused on specific parts ... passing in what they need via their struct fields,
allowing us making them very specific to type, or provider, or functionality."*

So this is **not** one `VpsDeployment` doing everything. It is three focused
`Deployable`s composed with regular Rust, each taking what it needs via its
fields:

| Deployable | Focus | Per-vendor? |
|---|---|---|
| `HetznerServer` / `DigitalOceanDroplet` / `LinodeInstance` | create/destroy the box, await ready; outputs `{ id, public_ip }` | **yes** |
| `SshHardening` | apply + verify the policy on *a host* | no |
| `DockerBootstrap` | install Docker on *a host*, verify it answers | no |

```rust
let server = HetznerServer { name: "ewe-testbed".into(), server_type: "cx22".into(), .. };
let out = client.deploy(&server, 0).await?;      // -> { id, public_ip }

// the next one's field is literally the previous one's output:
client.deploy(&SshHardening { host: out.host(), policy: HardeningPolicy::default() }, 0).await?;
client.deploy(&DockerBootstrap { host: out.host() }, 0).await?;

// the box is now a Docker host:
let docker = DockerClient::connect_ssh(&format!("ssh://root@{}", out.public_ip))?;
```

A convenience composite (a fn, or a `VpsStack` holding the three) is welcome — but
it is *just* Rust sequencing these, not a fourth abstraction.

## Prior art

`foundation_deployment_docker::ContainerDeployment` (spec-54, decision 05) is the
model and the closest analogue: `deploy` creates + starts and **persists the id**
through the `Deployable::store`; `destroy` reads it back and removes the thing;
both are plain async returning `BoxFuture`; and it builds its **own** client
because Docker's transport does not fit the `ProviderClient`'s HTTP client — "the
canonical unique underlying mechanics case". A VPS deployment is the same shape
with an HTTPS API instead of a Unix socket and an instance id instead of a
container id.

## Why this shape pays

- **`SshHardening` and `DockerBootstrap` are provider-agnostic**, so they need no
  cloud API to test — **not even a mock**. They take a host, and the
  docker-in-docker + sshd fixture *is* a host. The two parts most likely to hide
  bugs become the cheapest things to verify.
- They work on **machines we did not create** — any Linux box with sshd.
- Each carries its own `NAMESPACE` (`Deployable`'s `"provider/group/resource"`
  convention), so their state is independent: re-hardening does not disturb the
  server's record.

## What the three together guarantee

Per [decision 03](../../decisions/03-deployable-vps-model.md), when all three have
deployed the box is genuinely usable:

1. the instance exists and the vendor reports it running;
2. it has a public IP;
3. sshd answers with our key — the vendor's "running" precedes sshd by many
   seconds, and this is where a naive implementation races;
4. hardening is applied **and verified** (feature 05);
5. Docker is installed and `docker version` answers over SSH (feature 04).

Anything less pushes the waiting onto every caller, which is where the subtle bugs
live. Decision 03 asks whether that is too much for one call.

## Decisions (mostly resolved)

[Decision 03](../../decisions/03-deployable-vps-model.md):

- ✅ **Shape** — separate composed Deployables, fields carry what they need.
- ✅ **State** — per-deployable, under each one's own `NAMESPACE`; the vendor one
  persists id + public IP + provider.
- ✅ **create-or-find** — a live recorded instance is returned, never billed twice;
  a recorded-but-gone instance is recreated **and logged**.
- ✅ **Partial failure** — the box is destroyed by default; `keep_on_failure(true)`
  preserves it, and a cleanup failure never masks the original error.
- ⏳ **Vendor-side SSH keys on destroy** — still open.

## What each must honour

- **Idempotent.** Deployed independently, in any order, any number of times:
  `SshHardening` on a hardened box is a no-op; `DockerBootstrap` on a Dockered box
  is a no-op.
- **States its own precondition.** `DockerBootstrap` against a host with no sshd
  says so — it does not hang, and it does not assume something sequenced it.

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

- [ ] Three focused Deployables; composition is plain Rust, fields carry the inputs
- [ ] `SshHardening` + `DockerBootstrap` are provider-agnostic and tested against the local sshd fixture **with no cloud API at all**
- [ ] Each has its own `NAMESPACE`; state is independent
- [ ] The vendor deployable outputs `{ id, public_ip }`; the others consume it as fields
- [ ] All three idempotent; each fails clearly when its precondition is unmet
- [ ] `destroy` works after a process restart, from persisted state alone
- [ ] Second deploy with live state issues **no** create call; a recorded-but-gone instance is recreated **and logged**
- [ ] Partial failure destroys by default; `keep_on_failure(true)` preserves; cleanup failure never masks the cause
- [ ] Tests: mock API + local sshd fixture, incl. restart and create-or-find
