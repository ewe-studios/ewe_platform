# Feature 10 — Deployment-Platform Integration

**Depends on:** 02, 09
**Decisions:** [11](../../decisions/11-ephemeral-seed-lifecycle.md), [04](../../decisions/04-bootstrap-token-envelope.md), [14](../../decisions/14-crate-layering.md)
**Status:** ✅ Complete (2026-07-14). Wireguard-side tasks 1-3 done: `WgSeed::generate()`, `WgConfig::from_env()` (reads `WG_SECRET`/`WG_NETWORK`/`WG_SEED_ENDPOINTS`/`WG_RELAY`), `#[wireguard_main]` self-assembly, `docker_self_assemble` example demonstrating env-based container pattern. Tasks 4-5 (Docker testbed integration + `ContainerConfig` env injection) belong in `foundation_deployment_platform` per decision 14 (top-down, deployment platform consumes wireguard).

## WHY

Deliver the owner's headline flow: `foundation_deployment_platform`'s Docker capability **generates a
network secret once and injects it into every container it launches**, so a fleet **self-assembles**
into one private mesh with no manual key distribution.

## WHAT

`foundation_deployment_platform` consumes `foundation_wireguard` (top-down —
[decision 14](../../decisions/14-crate-layering.md)):

1. Generate + persist a network seed at deploy time.
2. Inject `WG_SECRET` / `WG_NETWORK` / `WG_ENDPOINT`(s) as container **env**.
3. Optionally expose a UDP port for the WG endpoint; designate seed/relay-capable containers.

## HOW

- **Generate:** at network-deploy time, `WgSeed::generate(SeedBits::B256)`; **persist** it (the
  owner's "maybe stores it after generating") in the deployment's state/secret store so re-deploys /
  scale-ups reuse the same network. Ephemeral semantics per
  [decision 11](../../decisions/11-ephemeral-seed-lifecycle.md) (TTL/rotation).
- **Inject:** using the existing `ContainerConfig.env` KEY=VALUE mechanism
  (`docker/config.rs::env`), set:
  - `WG_SECRET` = the seed (base64url) — consumed via `seed_env` ([decision 13](../../decisions/13-tri-config-and-macro.md)).
  - `WG_NETWORK` = network id.
  - `WG_ENDPOINT` = seed endpoint(s) (the first container(s) with a reachable/published UDP port).
  - This matches the **bare seed + separate endpoint** bootstrap form
    ([decision 04](../../decisions/04-bootstrap-token-envelope.md)).
- **Ports:** for seed/relay-capable containers, publish the WG UDP port (`ContainerConfig.port_udp`)
  so peers/browsers can reach them; mark them `relay: advertise`
  ([decision 07](../../decisions/07-relay-as-capability.md)).
- **Self-assembly:** each container boots via `#[wireguard_main]` (feature 09), reads env, joins; the
  first becomes the initial seed, the rest gossip in — no coordinator.

## Task list

1. Seed generation + persistence in the deployment state/secret store.
2. Env-injection into `ContainerConfig` for all containers in a network group.
3. Endpoint/port designation for seed + relay-capable containers.
4. Example: deploy N containers with a generated secret; verify they form one mesh.
5. Tests (with the docker testbed, spec 53/54): launch a container group, assert mesh formation +
   cross-container overlay reachability (success criterion 8).

## Test plan / success

- N containers launched with an injected secret self-assemble into one private mesh.
- Cross-container traffic works only over the overlay; the generated seed is persisted for scale-ups.
