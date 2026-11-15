# Spec 77 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** remote procedure call (RPC); Hypertext Transfer Protocol
(HTTP); virtual machine (VM); Durable
Object (DO); minimum viable product (MVP); pull request (PR); continuous integration (CI);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## Landed

- 2026-11-11: the research. The Firecracker, libkrun (1.19 and `main`), libkrunfw,
  Cloudflare sandbox-sdk, Cloudflare containers and Rust `worker` crate sources were read.
  The facts and their sources are in `learnings.md`.
- 2026-11-11: the first design (one guest agent with an own frame protocol; a TypeScript
  bridge Worker). The owner replaced it the same day; `learnings.md` section 7 records why
  each part went.
- 2026-11-11: the owner's architecture. A control plane (`agent-sandbox`) and a slave in
  each sandbox (`agent-slave`) that talk over Connect RPC. All logic is library code in
  `foundation_platform_sandbox`, split into `shared`, `native` and `cloudflare`. Programs,
  the DO and the `Provider` implementations are boundaries. The MVP is Firecracker, then
  Cloudflare.
- 2026-11-11: the spec in the story-driven form: `requirements.md` and 16 feature files.
  Each feature has its story, bounded context, law worksheet, invariants, must-have lists,
  acceptance scenarios, technical breakdown and a table from laws to tests.
- 2026-11-12: a review by a fresh agent (a smaller model with no context, as the owner
  asked). It found design defects and gaps; the spec was changed for each. The main ones:
  a program start no longer needs `chroot` when the root is `/`; children are spawned from
  one long-lived thread, because the parent-death signal follows the spawning thread; a
  Durable Object stores its sandbox ID and restores before it handles an alarm; a manager
  has a drop policy, so a restart does not kill what `restore` must find; one boot
  deadline covers the launch, the readiness wait and `Prepare`; file paths resolve inside
  the program root with `openat2`; the slave is a child subreaper when it is not PID 1;
  `ControlService`, `SandboxRpcClient` and the bearer check moved to feature 02, so feature
  07 does not need feature 06.
- 2026-11-11: the skill `story-driven-specification` in the `.agents` repository (on its
  `master`), and `specifications-management` version 5.0, which makes that layout
  mandatory. The submodule pointer of this repository moved to that commit.

- 2026-11-12: the owner's answers. "Docker sandbox" and "Cloudflare sandbox" are the
  products. Feature 16 (the driver for Docker Sandboxes, through the `sbx` program) and
  feature 17 (support for the Cloudflare Sandbox product: product images and a plain HTTP
  way in) were added. Requirements section 2.3 was rewritten. Decisions D10 to D13 record
  the answers; questions Q1 and Q3 are closed. `agent-slave` stays in `bin/agent_slave`.

- 2026-11-12: the owner made feature 16 (Docker Sandboxes product) a draft that is not a
  must have, because the product is not ready (decision D14). It is out of the lane order.

- 2026-11-14: the split-review fixes. The product examples use the control service path
  `ewe.sandbox.control.v1`. Features 12, 13 and 15 match the feature graph. Feature 08 names
  the crate `sandbox_worker`. Features 07 and 17 use `worker` 0.8.7, and learnings section 4.2
  records the re-check. The spec has a `start.md` at its root and in every feature directory;
  Q5 is closed.

- 2026-11-14: the Cloudflare merge. Spec 83 feature 06 said the opposite of this spec: it put
  Cloudflare deployment and a Sandboxes/Containers surface in `foundation_deployment_cloudflare`.
  The owner merged both pathways into this spec (decision D15):
  - A provider-neutral container surface: `ContainerControl`, `ContainerHandle`,
    `ContainerOps`, `ContainerSpec`, `ContainerInfo`, `ContainerStatus`,
    `ContainerResources`, `ContainerCapabilities`, `ContainerId`, `PortRequest`,
    `PortResponse` and `PortError` (requirements section 6.6, feature 00 section 6.13).
    `SandboxHandle::container()` and `SubstrateHandle::container()` return the handle of the
    sandbox's own container.
  - Feature 07 adds `CloudflareContainerControl`, `DoContainerOps`, `DoContainerClient` and
    the internal `ContainerService` (section 6.4.1).
  - Feature 08 owns Cloudflare deployment. Both paths are supported and apply one
    `DeploymentPlan`: `FoundationCloudflareDeployer` (Rust, primary) and
    `WranglerCloudflareDeployer` (manual, fallback) (section 6.8).
  - Feature 10 adds `DockerContainerControl` and `DockerContainerOps` (section 6.7.1).
  - Feature 17 deploys the product image through the same `DeploymentPlan`.
  - Spec 83 feature 06 now consumes these surfaces. It is renamed "Cloudflare Host". It
    adds no file to `foundation_deployment_cloudflare` or `foundation_platform_sandbox`.
  - Q4 is closed.

- 2026-11-15: the six-conflict review. The owner sent six defects with one direction each.
  Five of them change this spec:

  | Defect | Direction | What changed here |
  |---|---|---|
  | The wasm fetch pump was planned in three places | One owner, delivered first | [Spec 85](../85-wasm-fetch-transport/requirements.md) owns `FetchTransport`, `FetchServe` and `DoStubTransport`. Feature 07 section 6.5 and feature 08 section 6.4 use it and write no pump. |
  | Mutual TLS was deferred to spec 65, which no longer owns it | One specification for mTLS | [Spec 87](../87-mutual-tls/requirements.md) owns the CA, the trust store, the verifiers and the mTLS providers. Question Q4 points there. Feature 06 section 6.5 names `ServerTlsConfig` and `MtlsAuth`. |
  | Two `Secret` types and two bearer parsers | One small specification in `foundation_auth` | [Spec 86](../86-auth-primitives/requirements.md) owns `Secret`, `constant_time_eq` and the bearer parser. Feature 00 section 6.10 defines none; feature 02 section 6.10 has `authorize` over spec 86's `Secret`. |
  | Two meanings of `IdSource`, and an `Env` design that did not match `Ctx` | One featureless specification | [Spec 84](../84-context-injection/requirements.md) owns the `Env` entry. Feature 02's enum is renamed `SandboxIdPolicy`, and the field is `id_policy`. |
  | No central handle for containers and sandboxes | One control surface at two levels | Section 6.0 states the handle doctrine and the `Platform` wrapper. `foundation_context` gets no change; each part adds a wrapper of its own (`NativePlatform`, `DockerPlatform`, `CloudflarePlatform`). `SandboxControl` is renamed `SandboxProvider`; `Sandbox` is renamed `SandboxHandle`; feature 06's deployment adapter is renamed `SandboxDeploymentProvider` and `SandboxDeploymentHandle` (decision D16). |

  `related_specs` now names spec 84, 85, 86 and 87.

- 2026-11-15 (second note): the owner rejected a `Platform` entry in the `Ctx` and a `Ctx`
  method. "We wont go edit foundation_context, but rather wrap Ctx with whatever the cloudflare
  part needs that provides the other cloudflare specific handles", and "same for others". So
  section 6.0 gives a wrapper value instead: `Platform` holds the `Ctx` and the two controls,
  and each part adds one wrapper (`NativePlatform` in features 05 and 09, `DockerPlatform` in
  feature 10, `CloudflarePlatform` in feature 07). The wrappers share the `Platform` trait and
  the `PlatformKind` enum, with `as_*` downcast helpers that give `PlatformMismatch` on a wrong
  part. A function for one part takes the concrete type; a function that serves several takes
  `&dyn Platform`. Spec 83 feature 06 takes `&CloudflarePlatform`. Feature 09 reuses
  `NativePlatform`: a libkrun host registers a different driver in the same manager, so the
  wrapper names no driver.
- 2026-11-15 (third note): "just wrap the js with a SendWrapper to make it look Send, wasm is
  non-send anyway". `CloudflarePlatform` now carries `Option<RuntimeFetch>`, with
  `foundation_compact::SendWrapper` (no new dependency), and the spec forbids a wasm `atomics`
  build. The owner also rejected a wasm-only wrapper: "this should properly separate whats wasm
  and native, so the same API services both". So the wrapper lives in
  `shared/cloudflare/platform.rs` and compiles everywhere. The deployer is not in the wrapper;
  both deployers are native deployment-crate values.
- 2026-11-15 (fourth note): "you know you can just add a RuntimeFetchInner with a
  RuntimeFetchOuter(RuntimeFetchInner) that wraps it and hides away the presence or absence of a
  SendWrapper". `RuntimeFetch` is now one outer newtype on every target, over a private inner
  type: `SendWrapper<Rc<dyn Fetcher>>` on wasm, an uninhabited enum on native. The accessors
  (`new`, `fetcher`) are wasm-only; the type and its fields are the same everywhere, and the
  type is `Send + Sync` on both.
- 2026-11-15 (audit): the owner asked "can you check where else you have made similar mistake".
  Seven places had the same two shapes of defect, and all are fixed: the section 2.4 module
  table, the `cloudflare` feature row, spec 85's `FetchTransport` field and body stream, spec
  81's `KeySource` variant, spec 80's `MigrationSource` variant, spec 83's `SealFuture` and the
  two crate rows of `agent-rpc-architecture.md`. The `worker` adapters of feature 07 now show
  the wrapper, and `DockerApi` names its `Send + Sync + 'static` bound. The table is in
  `learnings.md` section 10.1. Issue #556 records the set, and it is closed.

- 2026-11-15 (fifth note): "should it not use our Http api that properly abstract out wasm/web
  fetch vs native http as well? the `DynHttpClient` trait from foundation_netio". Section 6.0
  now says what `RuntimeFetch` is not. It holds the JavaScript `fetch` function that spec 85
  needs; a plain HTTP request goes through the `DynNetClient` of the `Env`. The wasm
  `HttpClient` reads whole response bodies (`wasm/client/client.rs:334`), so it cannot carry a
  streamed RPC response. There is no `DynHttpClient` type; the names are `HttpClient` and
  `DynNetClient`. The two code defects are issues #557 and #558. `learnings.md` section 10.4
  records the answer.

No product code exists. The spec is a draft in a pull request for the owner's review.

## In progress

Nothing. The spec waits for the owner review.

## Next

1. The owner reviews the PR and answers the open questions (requirements section 10.2).
2. Update the spec with the answers. Set `status: "in-progress"`.
3. Feature 00 (one agent).
4. Features 01, 02, 03 and 04 in parallel (one agent each).
5. Feature 05, then feature 06: MVP-A (Firecracker).
6. Feature 07 in parallel with 05 and 06 (it needs only 01 and 02); then feature 08:
   MVP-B (Cloudflare).
7. Phase 2: features 09 (libkrun), 10 (Docker), 11 (nested), 17 (Cloudflare Sandbox
   product). Feature 17 needs 08.
8. Phase 3: features 12 to 15 (images).

## Milestones

| Milestone | Features | Features deep (longest path) | State |
|---|---|---|---|
| MVP-A: a command runs in a Firecracker microVM through `agent-sandbox`, the library, RPC and a `Provider` | 00 to 06 | 4 (00, then one of 01 to 04, then 05, then 06) | not started |
| MVP-B: the same calls work against a Cloudflare Worker | 07, 08 | 2 more (07 can start after 01 and 02; 08 also needs 06) | not started |
| Phase 2: libkrun, Docker, nested sandboxes, Cloudflare Sandbox product | 09, 10, 11, 17 | 1 to 2 more each | not started |
| Draft, not scheduled: Docker Sandboxes product | 16 | 1 more after 10 | draft |
| Phase 3: sandbox images | 12, 13, 14, 15 | 4 more | not started |

## What was checked while the spec was written, and what was not

Checked by a command in the specification session:

- `foundation_nativeapis` with the `fd` feature compiles for `x86_64-unknown-linux-musl`.
- `foundation_deployment_platform` does not compile for that target with default features
  (a C dependency through TLS).
- `foundation_deployment` depends on `foundation_db`.

Taken from source code and documents, with file and line in `learnings.md`: every
Firecracker, libkrun, libkrunfw, Cloudflare and `worker` crate fact.

Not run: no microVM, no container and no Worker was started. The session had no
`/dev/kvm`. The first real run of each driver is in its feature, and each feature's "Done
when" needs that run.

## Things a feature decides by a run

These are not open design questions. The design does not depend on the answer; the feature
records the value.

| Feature | Value to record |
|---|---|
| 05 | The boot time; the oldest Firecracker version that the real tests pass on. |
| 06 | The size of `agent-slave` for each target. |
| 08 | Whether `wrangler dev` keeps a container across a restart (the test accepts both). |
| 09 | The boot time on Linux and macOS; the libkrun version of each run. |
| 12 | Which hosts have a libkrun with block support. |
| 16 | The `sbx` version of the first real run; the template name for the README; whether the slave stays when the `sbx exec` client stops (the test accepts both). |
| 17 | The digests of the two pinned images. |

## Open questions for the owner

Known layout difference: features 03, 04 and 09 have the story, context, rules and
scenario sections with letters (A to D) between "1. Goal" and their numbered technical
sections; the other features use the nine numbered sections of the skill. The content is
the same. A renumber is a mechanical edit for a later pass.

Requirements section 10.2: Q2 (`foundation_platform_docker` or the existing
`foundation_deployment_docker`) remains. Q1 and Q3 are closed (decisions D10 and D11). Q5 is
closed: the 2026-11-14 pass added the `start.md` files. Q4 is closed by decisions D15 and D17: a signed
secret or Cloudflare's own mTLS, through the `Secret` of spec 86 and the `CfMtlsAuth` of spec 87.

## Findings to file as GitHub issues

Decision D9: no issue is filed while the spec is in review. Feature 06, section 6.10, has
the list to file when the owner approves.
