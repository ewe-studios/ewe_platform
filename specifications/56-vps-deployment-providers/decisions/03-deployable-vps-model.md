# 03 — The `Deployable` VPS model

**Date:** 2026-07-17
**Status:** Open

## The question

The owner's requirement: **"we setup deployables for these to use the crates or
API each provider provides to deploy, setup and harden these VPS instances for
use."**

So each provider crate exposes a `Deployable`. What does `deploy` guarantee, what
does it persist, and what happens when it is called twice?

## Prior art in this tree

`foundation_deployment_docker::ContainerDeployment` is the model and it is close
to what we want (spec-54, decision 05):

- `deploy` creates + starts a container and **persists its id** through the
  `Deployable::store` (a namespaced state store on the `ProviderClient`).
- `destroy` reads the id back and stops + removes it.
- Both are plain async returning `BoxFuture`.
- It builds its **own** client rather than using the `ProviderClient`'s HTTP
  client, because Docker speaks over a Unix socket — "the canonical unique
  underlying mechanics case".

A VPS deployment is the same shape with a different mechanism: an HTTPS API
instead of a Unix socket, and an instance id instead of a container id.

## What to decide

### 1. Shape — **resolved: separate, composed Deployables** (owner, 2026-07-17)

Not one `VpsDeployment` with `create`/`harden`/`bootstrap` methods. The
`Deployable` model composes **with regular Rust code**, each one taking what it
needs **via its struct fields** — "allowing us making them very specific to type,
or provider, or functionality" (owner). So:

| Deployable | Focus | Provider-specific? |
|---|---|---|
| `HetznerServer` / `DigitalOceanDroplet` / `LinodeInstance` | create/destroy the box, await ready | **yes** — one per vendor |
| `SshHardening` | apply + verify the policy on *a host* | **no** |
| `DockerBootstrap` | install Docker on *a host*, verify it answers | **no** |

Composition is ordinary Rust: deploy the box, take its output, hand it to the next
one as a field.

```rust
let server = HetznerServer { name: "ewe-testbed".into(), server_type: "cx22".into(), .. };
let out = client.deploy(&server, 0).await?;          // -> { id, public_ip }

let hardening = SshHardening { host: out.host(), policy: HardeningPolicy::default() };
client.deploy(&hardening, 0).await?;

let docker = DockerBootstrap { host: out.host() };
client.deploy(&docker, 0).await?;
```

**Why this is the better shape:**

- **`SshHardening` and `DockerBootstrap` are provider-agnostic**, so they are
  written once and — crucially — **verifiable with no cloud API at all**, not even
  a mock: they take a host, and the docker-in-docker + sshd fixture *is* a host.
  The two parts most likely to hide bugs become the cheapest to test.
- They work on **machines we did not create** — any Linux box with sshd becomes a
  Docker target.
- Each carries its own `NAMESPACE` (the trait's `"provider/group/resource"`
  convention) so their state is independent: re-hardening does not disturb the
  server's record.
- Each is **specific to its functionality**, which is what makes the fields
  meaningful — `SshHardening` holds a host and a policy, not a vendor token.

**What it obliges:**

- **Every one is idempotent.** They are deployed independently, in any order, any
  number of times: `SshHardening` on a hardened box is a no-op, `DockerBootstrap`
  on a Dockered box is a no-op.
- **Each states its own precondition** and fails clearly when unmet, rather than
  assuming something sequenced it — `DockerBootstrap` against a host with no sshd
  says so, it does not hang.
- A convenience composite (a fn, or a `VpsStack` holding the three) is welcome,
  but it is *just* Rust sequencing these — not a fourth abstraction.

The guarantee is unchanged, it just lands per-deployable: when all three have
deployed, the box is running, addressable, hardened **and verified**, with Docker
answering over SSH.

### 2. What each persists — **follows from §1**

Each deployable owns its own state under its own `NAMESPACE`:

| Deployable | Persists | Why |
|---|---|---|
| `HetznerServer` etc. | instance id, public IP, provider | `destroy` after a restart; and the IP is the *output* the other two consume |
| `SshHardening` | the policy fingerprint it applied | so a re-deploy can tell "already hardened to this policy" from "drifted" |
| `DockerBootstrap` | the version it installed | same reason |

The vendor deployable's `DeployOutput` carrying `{ id, public_ip }` is what makes
composition-by-field work — the next deployable's field is literally the previous
one's output.

### 3. Is `deploy` idempotent? — **resolved: create-or-find, recreate if gone**

**Resolved** (owner, 2026-07-17):

| Persisted state | Instance | `deploy` does |
|---|---|---|
| `{ id: 42 }` | alive | **returns it** — no create call, no second bill |
| `{ id: 42 }` | gone (deleted out of band) | **creates a fresh one**, updates state |
| none | — | creates, saves the id |

Mirrors `NetworkHandle::create_or_find` from spec-53. A duplicate VPS costs real
money, so an accidental second create is worse than a no-op.

**The known cost, accepted:** a `deploy` after an out-of-band delete silently
bills a new box. That is usually what the caller wanted (it is the recovery
case), but nobody explicitly asked for the charge — so:

- **log it plainly** at the point of recreation ("recorded instance 42 no longer
  exists; creating a replacement"), rather than letting a new bill appear
  silently;
- "alive" must mean *alive*, not merely "the API answered". A terminated instance
  that still resolves must count as gone, or `deploy` returns a corpse.

### 4. What does `destroy` do about the SSH key?

If `deploy` uploaded a key to the vendor's account, does `destroy` remove it?
Shared keys across deployments make this dangerous — deleting a key another
instance still uses would lock it out.

### 5. Rollback on partial failure — **resolved: destroy by default, opt-out to keep**

If create succeeds and hardening fails, do we destroy the instance or leave it
for inspection? Leaving it costs money silently; destroying it discards the
evidence. Spec-53 hit the mirror of this and chose cleanup: `ContainerHandle::start_async`
now removes a container when a later step fails, because a stranded container
"kept holding its ports and made every later run fail". A stranded VPS bills.

**Resolved** (owner, 2026-07-17): **destroy by default**, with
`keep_on_failure(true)` to preserve the box for debugging — the same shape as
`force_rm` landed in spec-53 F04.

```
deploy():
  create  -> ok (instance 42)
  harden  -> FAILS
  => destroy 42, clear state, return the harden error
```

This is the mirror of `ContainerHandle::start_async`, which now removes a
container when a later step fails — spec-53 learned that the hard way: a stranded
container "kept holding its ports and made every later run fail with an opaque
docker API error (status 500)". Here the stranded thing bills instead.

**The known cost, accepted:** the evidence is gone by default, so the first hit of
a hardening bug is harder to diagnose. Two things keep that bearable:

- the error returned must carry **what failed**, not just "deploy failed" — the
  hardening verifier's own assertion (feature 05) is the diagnostic, and it runs
  against the local sshd fixture where the box is not billing;
- `keep_on_failure(true)` is the escape hatch, and the error message should
  mention it exists.

Rollback is best-effort: if the destroy itself fails, say so **and** surface the
original error — never let cleanup mask the cause (spec-53's `discard()` does
exactly this).

## To resolve

1. ~~Does `deploy` do everything, or compose smaller steps?~~ — **resolved**:
   separate Deployables (`HetznerServer`/`…Droplet`/`…Instance`, `SshHardening`,
   `DockerBootstrap`), composed with regular Rust, each taking what it needs via
   struct fields. All idempotent.
2. ~~What is persisted beyond the id?~~ — **resolved**: per-deployable state under
   each one's own `NAMESPACE`; the vendor one persists id + public IP + provider
   and outputs `{ id, public_ip }` for the others to take as fields.
3. ~~Idempotent create-or-find? And what if the recorded instance is gone?~~ —
   **resolved**: create-or-find; recreate (and log) when the recorded instance is
   gone.
4. Does `destroy` touch vendor-side SSH keys?
5. ~~Destroy-on-partial-failure by default, with an opt-out?~~ — **resolved**:
   destroy by default; `keep_on_failure(true)` to preserve; cleanup failures never
   mask the original error.
