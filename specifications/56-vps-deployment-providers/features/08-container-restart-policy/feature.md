---
feature: "ContainerConfig restart policy"
description: "Wire HostConfig.RestartPolicy into the platform's ContainerConfig so containers come back after a reboot — the prerequisite decision 04's auto-reboot depends on"
status: "not-started"
priority: "high"
phase: 2
depends_on: ["04-hardening-policy"]
estimated_effort: "small"
blocks: ["05-vps-hardening"]
created: 2026-07-17
---
# Feature 08: `ContainerConfig` Restart Policy

## Why this is here

[Decision 04](../../decisions/04-hardening-policy.md) turned on **unattended
security upgrades with auto-reboot**. A box that reboots at 03:00 comes back with
**every container down** unless they carry a restart policy — and they cannot,
because the platform never exposes one:

| | State (checked 2026-07-17) |
|---|---|
| `HostConfig.RestartPolicy` in `foundation_deployment_docker` | **exists** (generated) |
| `ContainerConfig::restart_policy(..)` in `foundation_deployment_platform` | **missing** — nothing reads or sets it |

So auto-reboot as decided would silently trade "unpatched box" for "box up,
service down at 3am". This feature is the prerequisite that makes that decision
safe, and [feature 05](../05-vps-hardening/feature.md) is blocked on it.

This is **spec-53 code** (the platform's `ContainerConfig`), owned here because a
spec-56 decision is what requires it — spec-53 is closed and its own scope is
verified. It is the same "capability exists in the client, never wired in the
platform" shape that spec-53's audit found in F03 (`network_alias`, named volumes,
`read_only`, `subnet`) and F04 (`memory = "256m"`).

## Scope

1. **`ContainerConfig::restart_policy(..)`** → `HostConfig.RestartPolicy`, covering
   Docker's four: `no`, `always`, `unless-stopped`, `on-failure[:max-retries]`.
2. **Default stays `no`** — matching Docker itself. A test container that silently
   resurrects after `docker rm -f` would be its own bug, and spec-53's whole test
   suite assumes containers stay dead when dropped.
3. **What this spec deploys sets `unless-stopped`** — chosen over `always` because
   it honours a deliberate `docker stop` across a reboot, which is what an operator
   pausing a service expects.

## Verification

Local, against a real daemon — the honest test is not "the field serialises", it
is **the container actually comes back**:

- start a container with `unless-stopped`, restart the Docker daemon, assert it is
  running again;
- start one with the default (`no`), restart the daemon, assert it is **not**
  running — proving the default did not change under everyone;
- `on-failure:3` is honoured (the policy reaches the daemon with its retry count);
- a stopped `unless-stopped` container stays stopped across a daemon restart —
  the distinction from `always`, and the reason for the choice.

`inspect` round-trips the policy, which catches a serialisation mistake, but on
its own it would pass while the container stayed dead. Assert the behaviour.

## Acceptance criteria

- [ ] `ContainerConfig::restart_policy(..)` wired to `HostConfig.RestartPolicy`, all four modes
- [ ] Default is `no` — unchanged for every existing spec-53 test
- [ ] Deployments from this spec set `unless-stopped`
- [ ] e2e: daemon restart → `unless-stopped` container returns; `no` container does not
- [ ] e2e: a deliberately stopped `unless-stopped` container stays stopped across a restart
- [ ] `on-failure:N` reaches the daemon with its retry count
